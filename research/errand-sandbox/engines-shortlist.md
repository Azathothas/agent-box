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

The shortlist came from a first web pass and has not been checked here:
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
