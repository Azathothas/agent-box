# The no-KVM shortlist, tried on this host

A log kept while working, newest step last. Each step names the command or
experiment behind it. The question: which engine, on a Linux host with no KVM
(see README item 3), gives an untrusted agent a session it can do real work
in (apt, builds, a pty, docker or podman inside) with the strongest
isolation available?

Host: Linux 6.18.44-fc-v140 x86_64, 4 vCPUs, 16 GiB, cgroup v1 with no
delegation. PID 1 is the platform's `process_api`, not systemd. Docker 29.9.1
and rootless podman 4.9.3 run there, with runc and with gVisor
release-20261019.0 (experiment 70).

**Read section 13 first.** It is the current answer. The sections before it
are the log in the order the work was done, and later sections correct
earlier ones: section 10 corrects 5.2, 5.3, 7, 9.2 and 9.5, and section 13
replaces 7.

The shortlist came from a first web pass (runtime-survey.md), and this log
tests it:
1. Sysbox (nestybox/sysbox)
2. Incus unprivileged system containers
3. PVM with a VMM such as Firecracker

## 1. Sysbox

### 1.1 Getting it

- v0.7.1, published 2026-07-31. The release ships only
  `sysbox-ce_0.7.1.linux_amd64.deb`, with no checksum file, so the digest
  below is trust on first use, recorded so a later fetch can be compared.
- sha256 of that .deb: `9d6d5484f980d0a17f86c492c1262015c2afb66280bdb97215b79fde6a0261c5`
  (read 2026-10-23).
- Its postinst needs systemd, which this host does not run (PID 1 is the
  platform's `process_api`). It was unpacked with `dpkg-deb -x` and its steps
  done by hand instead: the three binaries go to /usr/bin; the sysctls are
  raised; a `sysbox-runc` runtime is added to /etc/docker/daemon.json; and
  `sysbox-mgr`, then `sysbox-fs`, are started with nohup.
- The sysctls, before and after:

  | key | before | after |
  | --- | --- | --- |
  | fs.inotify.max_queued_events | 16384 | 1048576 |
  | fs.inotify.max_user_watches | 130016 | 1048576 |
  | fs.inotify.max_user_instances | 128 | 1048576 |
  | kernel.keys.maxkeys | 200 | 20000 |
  | kernel.keys.maxbytes | 20000 | 1400000 |
  | kernel.pid_max | 32768 | 4194304 |

  kernel.unprivileged_userns_clone does not exist on this kernel (it is a
  Debian patch), so it was not set.
- Packages installed: rsync, kmod (for `modprobe`), and, for the source
  build, protobuf-compiler, libprotobuf-dev and libseccomp-dev.

### 1.2 Three things stopped the stock release here, and each was patched

Sources were read at sysbox v0.7.1 (82881aaaf2ce), with submodules at
sysbox-mgr 2d45af01bcfd, sysbox-runc 081856cc5d17 and sysbox-fs c3d2ebc65102.
These are the same commits the shipped binaries report with `--version`.

1. **sysbox-mgr exits, because there is no `/proc/modules`.**
   - First message: "modprobe is not installed on host". Installing kmod
     fixed it.
   - Then: "shiftfs kernel module check failed: open /proc/modules".
     `--disable-shiftfs` moved past it.
   - Then: "binfmt_misc kernel module check failed". No flag covers this
     one.
   - Cause: `KernelModSupported` (sysbox-libs `linuxUtils/linux.go:247-271`)
     reads only `/proc/modules`. This kernel is built without CONFIG_MODULES,
     so the file does not exist, although binfmt_misc is built in
     (`CONFIG_BINFMT_MISC=y`, and it is listed in /proc/filesystems).
   - Routes examined:
     - `--syscont-mode=false` gets past the check, but drops the
       system-container mode that is the point of Sysbox.
     - Making a /proc/modules is not possible: procfs takes no new file,
       and a bind mount needs a target that exists.
     - Patch the check: when /proc/modules is absent, look in
       /proc/filesystems, because binfmt_misc and shiftfs are both
       filesystems. **Taken.**
2. **Every container's init dies: "failed to update
   /proc/self/oom_score_adj: Permission denied".**
   - sysbox-runc writes `-999` unconditionally before it enters the user
     namespace (`libcontainer/nsenter/nsexec.c:744-752`, citing Sysbox issue
     #381).
   - Lowering an oom score needs CAP_SYS_RESOURCE. Root on this host lacks
     it: bit 24 is clear in CapEff and in CapBnd (`000001fffeffffff`), so
     even `echo -1 > /proc/self/oom_score_adj` fails as root.
   - runc and gVisor never lower the score, which is why they run here.
   - Patch: make that one write best effort, logged at debug.
   - What is lost: processes inside cannot set an oom score below the
     configured one.
3. **`/proc/sys` inside a container is empty.**
   - Seen first as two failures: "write sysctl key
     net.ipv4.ip_unprivileged_port_start: no such file", and "mounting
     binfmt_misc at proc/sys/fs/binfmt_misc: no such file".
   - Both keys exist on the host and inside a runc container.
   - With `--syscont-mode=false`, used only for this diagnosis,
     `/proc/sys/fs` and `/proc/sys/net/ipv4` were absent inside, against 148
     entries on the host. sysbox-fs logged "Error receiving first-child pid:
     EOF" once per lookup.
   - Cause: sysbox-fs links sysbox-runc's `nsenter` (`nsenter/event.go:36`),
     so its helper dies on the same `-999` write as in item 2.
   - Fix: rebuild sysbox-fs against the patched sysbox-runc. No change to
     sysbox-fs itself.

A defect of mine, caught on the way: sysbox-mgr's Makefile adds `-tags
idmapped_mnt` on kernels newer than 5.x, and my first `go build` did not. That
build reported "ID-mapped mounts supported by kernel: no". Rebuilt with the
tag, it reports yes, and so does overlayfs on ID-mapped mounts.

Build commands, from the sysbox v0.7.1 checkout with submodules:

```sh
# protobuf stubs (not committed upstream); generator pinned to sysbox-ipc's go.mod
GOBIN=$PWD/gobin go install github.com/golang/protobuf/protoc-gen-go@v1.5.4
(cd sysbox-ipc/sysboxMgrGrpc/sysboxMgrProtobuf && PATH=$GOBIN:$PATH protoc -I . -I /usr/include *.proto --go_out=plugins=grpc:.)
(cd sysbox-ipc/sysboxFsGrpc/sysboxFsProtobuf   && PATH=$GOBIN:$PATH protoc -I . -I /usr/include *.proto --go_out=plugins=grpc:.)
(cd sysbox-mgr  && go build -tags idmapped_mnt -o sysbox-mgr-patched .)
(cd sysbox-runc && CGO_ENABLED=1 go build -buildmode=pie -tags "seccomp apparmor idmapped_mnt" -o sysbox-runc-patched .)
(cd sysbox-fs   && git submodule update --init && CGO_ENABLED=1 go build -tags "netgo osusergo" -o sysbox-fs-patched ./cmd/sysbox-fs)
```

The two source patches are in `patches/sysbox-v0.7.1-no-modules-no-sys-resource.diff`.

### 1.3 First run

`docker run --rm --runtime=sysbox-runc debian:trixie-slim ...`, run once:

- 0.705 s of wall time.
- Inside, the process is root. Its uid_map is `0 231072 65536`, so on the
  host it is an unprivileged id.
- `/dev/pts/ptmx` is present.
- `/proc/sys/net/ipv4` has 134 entries, and ip_unprivileged_port_start reads
  0.
- `/proc/sys/fs/binfmt_misc` is mounted.

### 1.4 Docker inside a Sysbox container (the deciding test)

Setup for egress, needed by any container on this host:
- The platform's proxy listens on host loopback, 127.0.0.1:33337, and only
  allows CONNECT. A plain-http apt fetch gets 405.
- So: `socat TCP-LISTEN:33337,bind=172.17.0.1,fork TCP:127.0.0.1:<proxy port>`
  puts it on the Docker bridge. `experiments/75-host-up.sh` now does this,
  reading the upstream from `$HTTPS_PROXY`, because the proxy's port changed
  across a restart (section 13).
- `experiments/lib/apt-proxy.sh` switches a container's apt to
  https://deb.debian.org, through the relay, trusting the proxy's CA bundle
  mounted read-only at /run/proxy-ca.crt. On this host the bundle is at
  /root/.ccr/ca-bundle.crt (`PROXY_CA` in experiment 80).
- This is the same shape as errand's broker: one exit, outside the sandbox.

Run once, 2026-10-23, outer container `debian:trixie-slim` under sysbox-runc:
- `apt-get install docker.io docker-cli ca-certificates iptables`: 15 s of
  wall time for the first part.
- Debian 13 splits the client into `docker-cli`.
- `dockerd` started by hand, since there is no systemd inside either. It
  reports 26.1.5, storage driver overlay2, cgroup version 1.
- An image went in with `docker save | docker exec -i sbx1 docker load`. This
  avoids Docker Hub, which had rate-limited this host (429).
- Inner `docker run debian:trixie-slim`: uid_map `0 231072 65536`, and
  Seccomp 2 (filtering).
- Inner `docker build`, legacy builder, a Dockerfile that runs
  `apt-get install gcc libc6-dev`, compiles a C file and runs it: built (an
  image of 295 MB), and the binary ran.
- Seen from the host, the inner container's `sleep` runs as uid **231072**:
  an unprivileged id, not root.

So on this host, Sysbox gives a session root in its own Debian, apt, a
compiler, and Docker inside, with no `--privileged` and no host socket.

Not established yet:
- Isolation strength beyond the uid. The boundary is the shared host kernel.
- cgroup limits per session.
- Cost against runc and gVisor. Section 5 measures it.

## 2. Incus 6.0 LTS, unprivileged system containers

The survey (runtime-survey.md) says Incus **7.0** removed cgroup v1. Ubuntu
24.04 ships the **6.0** LTS line (`incus 6.0.0-1ubuntu0.3`), which comes
before that removal. So 6.0 was tried. Run once, 2026-10-23:

- `apt-get install incus` took one step. incusd was started by hand,
  `/usr/libexec/incus/incusd --group root`, since there is no systemd. Then
  `incus admin init --minimal` gave a `dir` storage pool and the bridge
  `incusbr0` (10.233.176.1/24).
- `incus info` reports the LXC driver as operational, with
  idmapped_mounts, unpriv_binfmt and unpriv_fscaps all true. The QEMU driver
  reports "KVM support is missing", as expected.
- Warnings that did not stop it: AppArmor is absent from this kernel; there
  is no blkio.weight, so disk priority is ignored; VLAN filtering on the
  bridge was refused. lxc.log warns that some cgroup2 files are missing
  (memory.oom.group, memory.reclaim).
- `incus launch images:debian/13 c1` took 7.9 s, including fetching the
  image through the proxy, which incusd took from its environment.
  `core.https_proxy` is an unknown key in 6.0.
- Inside: **systemd is PID 1**, a pty is present, and the uid_map is
  `0 1000000 1000000000`.
- Docker inside needed `security.nesting=true` and a restart, plus the same
  proxy relay bound on 10.233.176.1. Then
  `apt-get install docker.io docker-cli ca-certificates iptables` took 12 s,
  and **systemd started docker**: 26.1.5, overlay2, cgroup v1.
- Inner `docker run`: uid_map `0 1000000 1000000000`, Seccomp 2.
- Inner `docker build`, the same gcc Dockerfile as in 1.4: built, and the
  binary ran.
- Seen from the host, the inner container's `sleep` runs as uid **1000000**.

Against Sysbox, on this host:
- Incus needed no patches. It is a stock package, and its whole lifecycle is
  managed by a daemon.
- It boots a full init, so `systemctl` works for the agent.
- It is a separate engine. errand's podman backend cannot drive it, so it
  would need a bailey shim of its own (`incus launch`, `exec` and `delete`).
- **It ends at 6.0 LTS here.** 7.0 and later need cgroup v2. Per the Incus
  support page, as read in incus-gaps-survey.md, 6.0 LTS is supported until
  June 2029, with security-only fixes for its last 3 years. Ubuntu 24.04
  ships 6.0.0; 6.0.7 was tagged 2026-10-17.

## 3. gVisor with Docker inside (the survey's #2)

Run once, 2026-10-23, under the runtime `runsc-dind`: runsc with
`--platform=systrap --net-raw --allow-packet-socket-write`. The container
ran with `--cap-add all` and `--tmpfs /var/lib/docker`. The capabilities
are gVisor's, not the host's.

It took six steps, and each was needed:
1. `apt-get install docker.io docker-cli ca-certificates iptables` took
   **101 s**, against 15 s under Sysbox and 12 s under Incus. That is one
   run each, so it is indicative only; section 5 measures properly.
2. dockerd's default nftables-backed iptables fails ("failed to create NAT
   chain DOCKER"). Switching to iptables-legacy reaches the next failure.
3. With legacy iptables, "Extension addrtype ... Invalid argument": gVisor's
   netfilter lacks the addrtype match.
4. So dockerd runs with `--iptables=false --ip6tables=false --bridge=none`.
5. An inner `exec /usr/bin/sh: permission denied`: Docker's `--tmpfs` is
   noexec by default. `mount -o remount,exec /var/lib/docker` fixes it.
   Product use would pass `--tmpfs /var/lib/docker:exec`.
6. Inner containers and builds need `--network=host`, because there is no
   bridge.

Result:
- Inner `docker run`: Debian 13.7, kernel `4.19.0-gvisor`. The uid_map is
  the identity, because the inner container shares gVisor's own kernel and
  user namespace.
- Inner `docker build --network=host` of the gcc Dockerfile: built in 34 s,
  and the binary ran.

What it costs in isolation terms:
- Inner containers get no network isolation from each other, or from the
  session.
- Raw and packet sockets are enabled inside the sandbox. gVisor's own help
  text warns that they let a workload craft arbitrary packets. Those are
  bounded by the session's network, which is the egress relay.

## 4. PVM

Not testable on this host. README item 3 has the evidence: the kernel has
KVM_PVM built in, but registers no /dev/kvm without `nested_pvm=1` on the
command line, and the platform sets that command line.

The survey adds that out-of-tree PVM (virt-pvm, 6.12.33) refuses a host
with KPTI or without FSGSBASE, RDTSCP or CX16. Checked on this host:
- /proc/cpuinfo lists rdtscp, cx16 and fsgsbase.
- `/sys/devices/system/cpu/vulnerabilities/meltdown` reads "Not affected",
  so this CPU needs no KPTI. INFERRED: KPTI is therefore off.

On those checks, nothing on this host refuses PVM except the boot
parameter.

Reopens if: the platform boots with `nested_pvm=1`. Then the microVM route
(Firecracker, CubeSandbox, AgentENV) can be measured here.

## 5. Cost, measured

### 5.1 A correction first: experiment 80's "Sysbox is 8x slower on files" was wrong

Experiment 80 ran one engine after another. Its runc pass measured
`files@workspace` at 184 ms. Minutes later, the same work took about 1500 ms
in three places:
- unprivileged runc;
- privileged runc, on a plain bind and an ID-mapped one;
- host root, with no container at all.

This host's disk speed drifts, so 80's disk-backed rows compared moments, not
engines.

Candidates that were tested and refuted along the way:
- **ID-mapped mounts are slow.** Refuted: inside one runc container, a plain
  bind and an ID-mapped bind of the same directory took the same time.
- **cgroup v1 throttles writeback.** Refuted: the shell's memory cgroup has a
  14.3 GB limit.
- **fanotify watchers.** Refuted: the only mark belongs to incusd, on a
  pseudo filesystem.

Interleaved runs of runc and Sysbox on the same directory
(`results/81-pre-ab-runc-vs-sysbox.txt`) put `files` at 1531 ms against
1603 ms, the same. Experiment 81 replaces 80.

### 5.2 Experiment 81: interleaved, every engine at once

5 rounds, median ms, 2026-10-23. Raw rounds are in
`results/81-raw-rounds.txt`. The factor is against runc.

| workload@dir | runc | sysbox-runc | runsc (gVisor) | incus 6.0 |
| --- | --- | --- | --- | --- |
| spawn@workspace | 423 | 452 (1.1x) | 3467 (8.2x) | 436 (1.0x) |
| files@workspace | 1518 | 1449 (1.0x) | 2705 (1.8x) | 1398 (0.9x) |
| git@workspace | 1216 | 1279 (1.1x) | 3603 (3.0x) | 1253 (1.0x) |
| cpu@workspace | 453 | 451 (1.0x) | 518 (1.1x) | 450 (1.0x) |
| compile@workspace | 1512 | 1456 (1.0x) | 9490 (6.3x) | 1444 (1.0x) |
| spawn@scratch | 426 | 411 (1.0x) | 3247 (7.6x) | 436 (1.0x) |
| files@scratch | 133 | 138 (1.0x) | 423 (3.2x) | 137 (1.0x) |
| git@scratch | 148 | 161 (1.1x) | 1131 (7.6x) | 156 (1.1x) |
| cpu@scratch | 469 | 435 (0.9x) | 525 (1.1x) | 518 (1.1x) |
| compile@scratch | 1527 | 1528 (1.0x) | 9721 (6.4x) | 1448 (0.9x) |

- `/workspace` is a host directory, bind-mounted, and `/scratch` is a tmpfs.
- Workloads are in `lib/workload.sh`. compile is 200 generated C files, make
  -j4, link and run.
- The engines ran one at a time within each round, so they never competed
  for the 4 vCPUs.

Start-up and memory. These are sequential, not interleaved; start-up and
memory do not depend on disk drift the way file work does, but they are one
sample set each.

| | runc | sysbox-runc | runsc | incus 6.0 |
| --- | --- | --- | --- | --- |
| cold start: create, `true`, remove, median ms | 396 | 694 | 485 | 16903 to 24367 (dir pool; see 10.2 and 10.3: about 0.4 s with a golden copy on btrfs) |
| memory per idle container, KiB (MemAvailable over 5) | 2540 | 5307 | 12562 | 58809 |

Notes on these:
- The Sysbox figures leave out its two daemons. Those are paid once per
  host.
- CORRECTED in 10.2: the Incus cold start on the dir pool is mostly
  unpacking the image (init 11.2 s), not the "Remapping container
  filesystem" rewrite (first start 0.8 s). The original note blamed the
  rewrite. Its memory includes a booted systemd.
- The Incus start times rose across the 5 runs. The cause was not
  investigated. Plausible causes: disk drift, or leftover image layers.

### 5.3 What this says, for this host only

- Sysbox and Incus cost what plain runc costs, on every workload an agent
  runs. The shared kernel is why.
- gVisor costs 6-8x on spawning and compiling, the work an agent does most.
  Its user-space kernel is why.
- Sysbox starts in 0.7 s. Incus launched from an image on the dir pool took
  17-24 s. CORRECTED in sections 9.2 and 10: an existing Incus instance
  starts in about 0.27 s, and a golden copy on btrfs makes a new one in
  about 0.4 s.

## 6. What root inside each engine can reach

`lib/escape-probe.sh` runs in each engine, and its output is in
`results/90-host-surface-probe.txt`. The probe is coarse:
- "open" means the call succeeded inside. It does not mean the host was
  reached.
- Each probe has a 5 s timeout, because an open /dev/kmsg blocks on read.

| probe | runc | sysbox-runc | runsc | incus 6.0 | incus, nesting on |
| --- | --- | --- | --- | --- | --- |
| mknod the host disk and read it | closed | closed | closed | closed | closed |
| dmesg | closed | closed | open (1) | closed | closed |
| see more than 50 processes | closed | closed | closed | closed | closed |
| mount tmpfs / mount procfs | closed | closed (2) | closed | open / open | open / open |
| unshare a user namespace | closed | open | open | open | open |
| write a host sysctl | closed | closed | closed | closed | closed |
| read /proc/kcore | closed | closed | closed | closed | closed |
| change the hostname | closed | open | closed | open | open |
| open /dev/kmsg | closed | open (3) | closed | closed | closed |
| write under /sys | closed | closed | closed | closed | closed |
| uid 0 inside is, on the host | 0 | 231072 | 0 (4) | 1000000 | 1000000 |

Notes to the table:
1. gVisor's dmesg is the sentry's own synthetic log, not the host's.
2. The probe's tmpfs mount was refused under Sysbox, yet dockerd inside
   Sysbox mounted overlayfs and proc for its containers (section 1.4). The
   reason was not looked into. Sysbox screens mounts with seccomp-notify, and
   this probe's mount source name `t` may be what it refused. Open question.
3. Inside Sysbox, /dev/kmsg is device 1:3, that is /dev/null bind-mounted
   over it. A read returns 0 bytes. This is a false positive of the probe.
4. gVisor's uid 0 is root in the sentry's own kernel. On the host, the
   sandbox runs as the user who ran runsc.

What this shows:
- runc is the tightest by these probes, because Docker drops CAP_SYS_ADMIN.
  That is also why Docker cannot run inside it.
- Sysbox and Incus give the session root inside its own user namespace, which
  is what lets Docker and systemd run. It maps to an unprivileged host id, so
  an escape through a mistake in the container setup lands as an
  unprivileged user. A kernel exploit is a different case: it runs in the
  host kernel, whatever uid it started as. Their boundary is the shared host
  kernel, and their syscall surface is the host's, less seccomp.
- gVisor is the only one of the four in which the host kernel does not run
  the session's syscalls. That is its whole price, measured in section 5.

Not tested: actual kernel exploits; seccomp profile contents; side channels;
resource exhaustion against the host. No cgroup limits apply under rootless
use on this cgroup v1 host.

## 7. The first answer for this host (SUPERSEDED by section 13)

| | Sysbox (patched) | Incus 6.0 LTS | gVisor | microVM |
| --- | --- | --- | --- | --- |
| docker inside | yes | yes, with systemd | yes, after 6 concessions | — |
| cost against runc | 1.0-1.1x | 0.9-1.1x | 1.1-8.2x | — |
| cold start | 0.7 s | 17-24 s (rootfs chown) | 0.5 s | — |
| boundary | shared kernel, userns, seccomp | shared kernel, userns | user-space kernel | hardware |
| on this host | 3 patches to Sysbox | stock; ends at 6.0 (7.0 needs cgroup v2) | stock | blocked: nested_pvm=1 |

This section was written before sections 8 to 11, and recommended Sysbox
because the Incus start-up figure here was wrong (section 10). Kept as it
was written, for the record.

- For agents that build software and run containers, on a host without KVM:
  **Sysbox**. It runs at runc's speed, starts in under a second, and plugs
  into the Docker already here (`--runtime=sysbox-runc`). That is how errand's
  podman backend would reach it too, or a small bailey shim.
  - Against it: its maintenance is best effort (runtime-survey.md). On this
    platform it needs three patches, which no release carries.
- **Incus** is equally fast for work, and it gives a real init.
  - Against it: 17-24 s starts here, and no road past 6.0 on cgroup v1.
- **gVisor** is the strongest boundary available without KVM.
  - Against it: 6-8x slower on the work an agent does most. Docker inside it
    loses network isolation and needs raw sockets.
- **The microVM** remains the design for hardware isolation. On this platform
  it waits on one boot parameter. PVM's CPU prerequisites are met here.

Assume more claims here are wrong than have been caught. Two were caught in
this section's own work: experiment 80's 8x, and the ID-mapping "no" that
came from my own build.

## 8. FUSE (including network storage) and podman inside

Question: can a session mount FUSE filesystems, including network storage,
and run podman? The probe is `lib/fuse-podman-probe.sh`, in image
`bench-fuse:trixie` (bench, plus fuse3, rclone 1.60, sshfs, podman 5.4.2,
crun and fuse-overlayfs). Network storage is `rclone mount :http:` of
https://deb.debian.org/debian/ through the relayed proxy, reading
`dists/trixie/Release` through the mount.

Host change: `chmod 666 /dev/fuse`. It was 0600; distributions ship 0666,
and a session's root is an unprivileged host uid. FUSE is built in
(CONFIG_FUSE_FS=y).

First run, 2026-10-23 (results/91-fuse-podman-docker-engines.txt):

| engine | fuse, local dir | fuse, network storage | podman run |
| --- | --- | --- | --- |
| runc, default capabilities | fail | fail | fail ("cannot clone: Operation not permitted") |
| runc, with `--device /dev/fuse` | fail | fail | fail |
| sysbox-runc, no device | fail (no /dev/fuse) | fail | fail (fuse-overlayfs finds no device) |
| sysbox-runc, `--device /dev/fuse` | **pass** (fuse.rclone) | **pass** (138612 bytes read) | fail: crun's self bind mount of the overlay `merged` dir gives ENOENT |
| runsc, with or without the device | fail ("mount not ready") | fail | fail (lstat of overlay `merged`) |

Under runc both fail for want of CAP_SYS_ADMIN, which Docker's default set
drops. That is the control.

Follow-ups, 2026-10-23 (results/92-fuse-podman-followups.txt):

- **runsc with `--device /dev/fuse --cap-add SYS_ADMIN`**: FUSE still fails
  ("mount not ready"), and podman still fails. Only rclone was tried as the
  FUSE program. Whether gVisor's FUSE serves a simpler filesystem was not
  tested.
- **podman inside Sysbox**: the vfs storage driver **works** (Debian 13.7
  ran). Overlay through fuse-overlayfs still fails on crun's self bind
  mount. vfs copies each layer, so it costs disk and start time; that was
  not measured.
- **Incus 6.0, nesting on**, with nothing else configured:
  - It exposes `/dev/fuse` by default.
  - FUSE on a local dir: **pass** (fuse.rclone).
  - Network storage over FUSE: **pass** (138612 bytes read through the
    mount).
  - podman with defaults fails: "creating cgroup directory ...: read-only
    file system". The cgroup v1 controller trees are read-only inside an
    unprivileged container.
  - podman with `--cgroups=disabled`: **works**, on the native overlay
    driver.
  - Its network needs `nft`: with pasta it works, and with the default
    bridge it works once nftables is installed.
  - What `--cgroups=disabled` costs: podman sets no limits on inner
    containers. They share the Incus instance's own limits.

| engine | docker inside | podman inside | fuse | network storage over fuse |
| --- | --- | --- | --- | --- |
| Sysbox (patched), `--device /dev/fuse` | yes | yes, vfs driver only | yes | yes |
| Incus 6.0, nesting on | yes | yes, overlay, `--cgroups=disabled` | yes, by default | yes |
| gVisor | yes, 6 concessions | no | no (tried with SYS_ADMIN) | no |
| runc, default | no | no | no | no |

So for "docker or podman, plus FUSE network storage", gVisor is out, and
Incus does it with the least configuration.

## 9. Incus: the remaining questions, measured 2026-10-23

### 9.1 The host restarted mid-session

uptime showed 5 minutes at 03:28.
- Survived, on disk: packages, binaries, Incus images and instances, and
  /etc/docker/daemon.json.
- Lost, in memory: the /dev/fuse and /dev/net/tun modes (back to 0600),
  every sysctl set in section 1.1, and every daemon.

`experiments/75-host-up.sh` puts it all back in one idempotent command:
- First run, about 1 minute: every step reported "started" or "set", and a
  Sysbox container and an Incus container both ran.
- Second run: every step reported "already up".

### 9.2 Start-up, by what is started (10.2 corrects the cause, 10.3 adds the golden copy)

| what | dir pool | btrfs pool (loop file, 30 GiB) |
| --- | --- | --- |
| launch from image, first | 15642 ms | 12417 ms |
| launch from image, later | 15000 to 24000 ms (section 5.2) | 1808 ms (snapshot of the image volume) |
| start an existing, stopped instance, then exec `true` | 255 to 327 ms (5 runs) | - |
| systemd settled after that start | +699 ms | - |
| launch with `security.idmap.isolated=true` | - | about 2 s, including its own remap |

- Use persistent instances (one per agent or per project), on btrfs.
- Starting an existing instance is faster than a fresh Sysbox container
  (0.69 s).

### 9.3 Limits

- **Disk:**
  - The dir pool accepts `root.size=1GiB` and does not enforce it. 1.5 GB
    was written, dd exited 0, and incusd logged only "The backing filesystem
    doesn't support quotas, skipping set quota".
  - btrfs enforces it: "Disk quota exceeded", dd exit 1.
  - Inside a btrfs guest, `df /` shows the 30 GB pool, not the quota.
- **Memory**, `limits.memory=256MiB` on cgroup v1: the cgroup limit read
  268435456, and a python allocation loop was killed (exit 137).
- **CPU**, `limits.cpu=1`: nproc read 1. Four CPU-bound processes took
  1501 ms, against 436 ms unlimited.
- **The guest's view of its limits:** without lxcfs, /proc/meminfo shows the
  host's 16 GB. With lxcfs 5.0.4 running (`lxcfs /var/lib/lxcfs`), it shows
  256 MiB, `free` shows 256 total, and the CPU count is 1.
- lxcfs is a root FUSE daemon that serves the guests' /proc, so it widens
  the trusted surface, the same objection made to sysbox-fs in section 6.
  It is in 75-host-up.sh.

### 9.4 Separate uid ranges per guest

- With `security.idmap.isolated=true`: guest i1 is uid_map
  `0 1065536 65536`, and guest i2 is `0 1131072 65536`. Their processes run
  on the host as 1065536 and 1131072.
- Instances without the setting share `0 1000000 1000000000`.
- So one guest's root is not another guest's root on the host.
- Cost: about 2 s more per launch, on btrfs, for its own remap.

### 9.5 incusd dying takes the guests with it (WRONG: see 10.1)

- `kill -9` of incusd: the guest's init (host pid 7542) died at once.
- After `75-host-up.sh` restarted incusd, the guest was running again, but as
  a new process (host pid 9230, a different start tick). Everything running
  inside was lost.
- Not established why. Under systemd, Incus is normally restarted without
  stopping guests. Candidates, none tested:
  - (a) the per-guest monitor processes are set to die with their parent;
  - (b) systemd's separate service scope is what keeps them alive normally;
  - (c) incusd was started from a tool shell with nohup.
- What it means: on a host with no init, a supervisor that restarts incusd at
  once is required, and even then a crash costs every running agent its
  in-memory state.

### 9.6 A client hang, and a habit

- One `incus launch -q ... -c security.idmap.isolated=true`, run with other
  jobs in the background, waited 482 s while the server listed no operation.
- The same instance, made with `incus init` and `incus start` with stdin
  closed (`</dev/null`), took 2 s.
- Cause not proven (INFERRED: the client waited on stdin).
- errand-box should call the client with stdin closed, and with a timeout.

## 10. Corrections to section 9, from the second survey and tests that followed it

`incus-gaps-survey.md` holds the delegated survey that prompted these. Each
correction below was then tested here, 2026-10-23.

1. **Section 9.5 was wrong: incusd dying does not take the guests with it.**
   - Each guest's LXC monitor process is also named `incusd` (pids 9048 to
     9917, ppid 1, cmdline `[lxc monitor] ... <name>`).
   - My `kill -9 $(pgrep -x incusd)` killed every monitor, and LXC kills a
     guest's init when its monitor exits.
   - Killing only the daemon's pid (8965, found by its full command line)
     left the guest's init alive: the same host pid 9230 and start tick
     115052 before and after. The restarted incusd took it back.
   - This matches the Incus docs on daemon behaviour (SIGTERM leaves
     instances running).
   - Rule for errand-box: never stop or signal incusd by name. Use its pid,
     or `incus admin shutdown`.
2. **Sections 5.2 and 9.2 blamed the wrong step for slow launches.** The cost
   is unpacking the image, not the ownership rewrite. A launch on the dir
   pool, split in two:
   - `incus init`: 11240 ms, and `volatile.last_state.idmap` is still `[]`
     after it.
   - The first `incus start`: 826 ms, and the rewrite happened here
     ("Remapping container filesystem").

   The Incus docs say the dir driver has no optimized image storage, so it
   unpacks on every init.
3. **The golden copy removes the cold start.** On the btrfs pool: stop a
   prepared instance once, then `incus copy` it.

   | run | copy | first start | rewrite | idmap carried over |
   | --- | --- | --- | --- | --- |
   | g1 | 145 ms | 289 ms | none | yes |
   | g2 | 109 ms | 280 ms | none | yes |
   | g3 | 112 ms | 309 ms | none | yes |

   So a new instance takes about 0.4 s, with no unpack and no rewrite.
   Copies of instances with `security.idmap.isolated` get a new map, so they
   still rewrite; that case was not measured.

Still open, unchanged:
- Why incusd falls back to rewriting at all, when the survey's own probe
  (open_tree, a new userns, mount_setattr IDMAP, the same sequence as
  Incus's CanIdmapMount) succeeds on these ext4 and btrfs paths.
- Why `shift=true` disk devices are refused.

## 11. Disk is thin; memory per idle session

Measured 2026-10-23, btrfs pool `bt` on a loop file
(`/var/lib/incus/disks/bt.img`), with discard=async set by Incus.

| step | file size on disk |
| --- | --- |
| the 30 GiB pool, with the bench image and earlier test instances gone | 1.2 GB |
| plus a new instance with `root.size=20GiB` | 1.2 GB |
| plus 2 GiB of random data written inside it | 3.3 GB |
| the data deleted inside the guest | 3.3 GB, not returned |
| `fstrim -v /var/lib/incus/storage-pools/bt` | 1006 MB |

- The quota is a ceiling, not a reservation.
- Freed space comes back only with fstrim; discard=async alone did not
  return it within the seconds watched.
- Idle memory: about 59 MB per Incus instance (section 5.2), and 18.9 MB RSS
  for `kage rpc` idle inside one. A session's working memory is whatever its
  builds take. That was not measured.

## 12. An rclone FUSE mount as a cache, on a 10-20 MiB/s link (estimate)

Not measured. This is arithmetic, plus how rclone's VFS layer works. Section
8 measured only that an rclone mount works inside Incus and Sysbox.

- Bandwidth: 10-20 MiB/s is about 36-72 GB per hour. 1 GB cold takes 50-100
  s. The link is shared, so six sessions missing the cache at once get about
  2-3 MiB/s each.
- Per-file latency matters more than bandwidth. An object-store remote costs
  one or more requests per file, typically tens of ms each. 3000 small files
  at 50 ms is about 150 s, against 1-2 s for their bytes. Build caches are
  mostly small files.
- Viable for large, read-mostly, shared data (toolchains, release
  artifacts, dependency tarballs, model weights, dataset snapshots), when:
  - rclone keeps a large local cache: `--vfs-cache-mode full`, a large
    `--vfs-cache-max-size` on local SSD, a long `--dir-cache-time`,
    `--vfs-write-back`;
  - there is one writer per area, because rclone has no cross-host locking.
- Not viable for:
  - build output (`target/`, `build/`, `node_modules`);
  - git working trees;
  - databases;
  - anything that needs exact local fsync, rename or lock semantics;
  - write-heavy caches pulled cold each session.
- Better on a slow link:
  1. Sync the cache as one archive: fetch one compressed tarball at session
     start, and upload it at the end if it changed.
  2. Use the tools' own remote caches: sccache (S3 or GCS backend), ccache
     remote storage, Bazel's remote cache.
  3. Run a caching proxy on the host, shared by every session:
     apt-cacher-ng, a pull-through registry mirror (this also covers Docker
     Hub's 429), verdaccio, or a crates mirror.
- For errand-box:
  - session work stays on the instance's local thin disk;
  - the host runs caching proxies;
  - rclone is optional, for large read-mostly data, with its local cache
    counted against host disk rather than session quotas, and fstrim in
    maintenance.
- To measure later: an rclone mount with `--bwlimit 10M` and `20M`, with and
  without the VFS cache, cold and warm, on a small-file tree and a 1 GB
  file. `--bwlimit` adds no latency, so add delay with netem if the kernel
  has it, or keep latency as arithmetic.

## 13. The answer for this host, 2026-10-23 (replaces section 7)

| | Incus 6.0 LTS | Sysbox (patched) | gVisor | microVM |
| --- | --- | --- | --- | --- |
| docker inside | yes, started by systemd | yes | yes, after 6 concessions | not testable here |
| podman inside | yes, overlay, `--cgroups=disabled` | yes, vfs only | no | - |
| FUSE, and network storage over it | yes, yes | yes (`--device /dev/fuse`), yes | no, no | - |
| systemd as PID 1 | yes | not tried | not tried | - |
| cost against runc (spawn, files, git, cpu, compile) | 0.9-1.1x | 1.0-1.1x | 1.1-8.2x | - |
| new session | about 0.4 s (golden copy on btrfs) | about 0.7 s | about 0.5 s | - |
| existing session start | about 0.27 s | - | - | - |
| idle memory | about 59 MB, plus 19 MB for kage | about 5 MB | about 12 MB | - |
| limits on cgroup v1 | memory, CPU, disk (btrfs) enforced | not measured | not measured | - |
| guest uid on the host | 1000000, or a private range each | 231072 | the runsc user | - |
| patches needed here | none | 3 | none | - |
| boundary | shared kernel, user namespace, seccomp | the same, plus sysbox-fs | user-space kernel | hardware |

**Incus 6.0 LTS**, run as persistent or golden-copied instances on a btrfs
pool, with lxcfs, `security.idmap.isolated=true` and nesting on. It is the
only engine tested that does all of it: Docker, podman, FUSE and network
storage, systemd, runc speed, and enforced limits. It needs no patches. The
same tool runs VMs on hosts that have KVM.

Against it:
- A root daemon (incusd) and a root FUSE daemon (lxcfs).
- A shared kernel with no AppArmor on this host.
- On cgroup v1 it ends at the 6.0 line, supported to June 2029.
- It needs a supervisor on a host with no init. Never signal incusd by name
  (10.1).

Host edge cases found here, each with its fix, are in next-session.md
section 1.2. One more came up during the final review: the platform's egress
proxy moved from port 33337 to 42151 across the restart. The relay in
75-host-up.sh now reads its upstream from `$HTTPS_PROXY` on every run, and
replaces a relay pointing at a stale upstream. It was verified with an apt
fetch from a container through the relay.

Assume more claims here are wrong than have been caught. Caught and
corrected so far:
- experiment 80's 8x;
- the ID-mapping "no" from my own Sysbox build;
- the Landlock claim in the gVisor shim's comment (README);
- "incusd dying kills its guests" (10.1);
- "the cold start is the rewrite" (10.2);
- section 7's recommendation.
