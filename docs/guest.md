# The guest

This page gives the design of everything inside an instance. It covers the
guest kernel, the base image, the data disk, the guest agent `box-guest`, and
kage.

NOTE: Nothing on this page is built yet. The product crates do not exist.
Entry T-033 creates them in the first implementation session. Every path under
crates/ or images/ on this page is a planned path.

The entries that build this page are in [TODO/guest.md](../TODO/guest.md),
[TODO/image.md](../TODO/image.md) and [TODO/kage.md](../TODO/kage.md). The
trust boundary is in [architecture.md](architecture.md). The engines are in
[engines.md](engines.md). The vsock ports are in [protocol.md](protocol.md).
The network and the broker are in [network.md](network.md) and
[broker.md](broker.md).

How to read the labels:

- READ: somebody opened the upstream source at the cited commit.
- MEASURED: somebody ran it. The text gives the host, the date and the
  conditions.
- No label: the sentence is design. An entry implements it and proves it.

WARNING: Everything inside the guest is untrusted. This includes the guest
kernel, box-guest after boot, kage, every tool, and every byte on the guest
disks. The host never parses guest data and never runs a tool against it.

## The guest kernel

### Source and build

- The kernel is one pinned upstream LTS release from kernel.org. The tarball
  is verified by sha256 before the build (T-060).
- Proposal: the 6.18 series. It is the newest guest kernel series that
  Firecracker v1.17.0 validates (READ
  `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:38`).
  T-060 pins the exact release.
- Firecracker validates its own Amazon Linux microVM kernels, not upstream
  kernels built with its configs (READ
  `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:53`).
  Our upstream build is therefore outside the validated set. T-061 boots it on
  both engines.
- The configs live in images/kernel/ (one per architecture). The planned
  command `cargo xtask image kernel` builds them in the Linux runner (T-060,
  T-010).
- The build is reproducible: `KBUILD_BUILD_TIMESTAMP`, `KBUILD_BUILD_USER` and
  `KBUILD_BUILD_HOST` are fixed values from images/pins.toml. Two builds give the
  same sha256 (the T-060 prove step).
- `CONFIG_MODULES=n`. Every driver is built in. The guest loads no module, so a
  module is not an attack path and needs no signing.

### One config for both engines

One config per architecture carries the virtio drivers for Firecracker and the
Hyper-V drivers for HCS. The same kernel source boots under both engines. This
also hedges the engine choice: a future virtio engine on Windows needs no new
kernel.

| Group | Kconfig symbols | Engine | Why |
| --- | --- | --- | --- |
| virtio transport | `CONFIG_VIRTIO_MMIO` | Firecracker | Firecracker devices are virtio over MMIO. |
| block | `CONFIG_VIRTIO_BLK` | Firecracker | The base image and the data disk. |
| vsock | `CONFIG_VSOCKETS`, `CONFIG_VIRTIO_VSOCKETS` | Firecracker | Every host channel (ports 5000 to 5005). |
| entropy | `CONFIG_HW_RANDOM_VIRTIO` | Firecracker | The Firecracker entropy device seeds the guest RNG. |
| balloon | `CONFIG_MEMORY_BALLOON`, `CONFIG_VIRTIO_BALLOON`, `CONFIG_PAGE_REPORTING` | Firecracker | Memory reclaim. Free page reporting status in v1.17.0 is an open question. |
| KVM clock | `CONFIG_KVM_GUEST`, `CONFIG_PTP_1588_CLOCK`, `CONFIG_PTP_1588_CLOCK_KVM` | Firecracker | Timekeeping and the ptp_kvm clock for time sync. |
| ACPI boot (x86_64) | `CONFIG_ACPI`, `CONFIG_PCI`, `CONFIG_X86_MPPARSE=n`, `CONFIG_VIRTIO_MMIO_CMDLINE_DEVICES=n` | Firecracker | Firecracker marks the MPTable and command-line device paths as deprecated. |
| clean shutdown (x86_64) | `CONFIG_SERIO_I8042`, `CONFIG_KEYBOARD_ATKBD` | Firecracker | SendCtrlAltDel reaches the guest through the i8042 device. |
| VMBus | `CONFIG_HYPERV` | HCS | hv_vmbus, the bus for every Hyper-V device. |
| storage | `CONFIG_HYPERV_STORAGE`, `CONFIG_SCSI`, `CONFIG_BLK_DEV_SD` | HCS | hv_storvsc: the base VHDX and the data VHDX on SCSI. |
| vsock | `CONFIG_VSOCKETS`, `CONFIG_HYPERV_VSOCKETS` | HCS | hv_sock: every host channel over HvSocket. |
| integration services | `CONFIG_HYPERV_UTILS` | HCS | hv_utils: time sync. |
| balloon | `CONFIG_HYPERV_BALLOON`, `CONFIG_PAGE_REPORTING` | HCS | hv_balloon with free page reporting. |
| cgroup v2 | `CONFIG_CGROUPS`, `CONFIG_MEMCG`, `CONFIG_CGROUP_PIDS`, `CONFIG_CGROUP_SCHED` | both | Resource control inside the guest, and containers if T-185 allows them. |
| filesystems | `CONFIG_EROFS_FS`, `CONFIG_EXT4_FS`, `CONFIG_OVERLAY_FS`, `CONFIG_TMPFS`, `CONFIG_DEVTMPFS` | both | The base image, the data disk, the overlay root, and the runtime mounts. |
| namespaces | `CONFIG_USER_NS` | both | User namespaces for tools that need them. |
| network | `CONFIG_TUN` | both | box-guest creates `tun0`. The guest has no NIC device. |
| firewall | netfilter off | both | No firewall in the guest. Policy runs in the host netstack. |
| console | `CONFIG_SERIAL_8250_CONSOLE`, `CONFIG_PRINTK` | Firecracker | Boot logs, only when diagnosing (T-084 keeps serial off). |

The Firecracker rows come from its kernel policy (READ
`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:94`
and lines 64 to 69 and 140 to 146 of the same file).

NOTE: `CONFIG_NETFILTER` stays off. A container runtime in the guest needs it.
T-185 measures containers first, and a positive result is a pins change.

### Outputs

One build of one config gives these files:

| File | Architecture | Booted by | Note |
| --- | --- | --- | --- |
| `vmlinux` | x86_64 | Firecracker | Firecracker recommends an uncompressed `vmlinux` on x86_64. |
| `bzImage` | x86_64 | HCS | HCS LinuxKernelDirect takes a compressed kernel. |
| `Image` | aarch64 | Firecracker | HCS LinuxKernelDirect is x64 only. |

Firecracker v1.17.0 also boots a `bzImage` on x86_64 (READ
`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/rootfs-and-kernel-setup.md:7`).
It names the cost: the guest decompresses it at boot, which costs boot time
and guest memory (READ the same file, line 13). So Firecracker boots
`vmlinux`. A single `bzImage` for both engines is a fallback only.

### The vsock transport question

Question: can `virtio_vsock` and `hv_sock` live in one kernel?

- hcsshim adds `initcall_blacklist=virtio_vsock_init` to the kernel command
  line of its Linux utility VMs, to force `hv_sock` (READ
  https://raw.githubusercontent.com/microsoft/hcsshim/main/internal/uvm/create_lcow.go,
  read 2026-10-14).
- The likely reason: the Linux vsock core takes one guest-to-host transport,
  and the first one that registers wins. This is not verified here.
- Answer from the adversarial review in
  [engines-2026-10.md](research/engines-2026-10.md): no special kernel build is
  needed. Under HCS, the engine adds `initcall_blacklist=virtio_vsock_init` to
  the command line, as hcsshim does. Under Firecracker there is no VMBus, so
  `hv_sock` has no device.
- Status: not measured. T-061 boots the T-060 kernel under both engines and
  proves vsock on each.
- Fallback if T-061 fails: one kernel build per engine from the same source,
  with only the vsock symbols different.

### The kernel command line

The engine builds the command line from a fixed list. No caller and no config
file adds to it. The Firecracker builder is T-084, and the HCS builder is
T-101.

| Parameter | Firecracker | HCS | Why |
| --- | --- | --- | --- |
| `root=` | `/dev/vda`, appended by Firecracker for the root drive | `/dev/sda` | The base image is the first disk. HCS SCSI naming is an open question. |
| `ro` | appended by Firecracker | yes | The base image is read-only. |
| `rootwait` | no | yes | The SCSI disk can appear after the kernel looks for the root, as in hcsshim. |
| `rootfstype=` | `erofs` or `ext4` | same | T-065 decides the format. |
| `init=` | `/usr/sbin/box-guest` | same | box-guest is PID 1. |
| `reboot=k panic=1` | yes | yes | A kernel panic resets the guest, and Firecracker exits. The HCS result of a guest reset is not verified (T-105). |
| `nomodule` | yes | yes | The kernel has no module support. |
| `8250.nr_uarts=0` | yes | `8250_core.nr_uarts=0`, as hcsshim | The guest serial port stays off. |
| `pci=off` | Firecracker adds it itself when PCI is off | yes, as hcsshim | The HCS devices of an instance are on VMBus. |
| `initcall_blacklist=virtio_vsock_init` | no | yes | Forces `hv_sock` (see the vsock question above). |
| `console=ttyS0` | only when diagnosing | no | The serial console stays off unless somebody diagnoses a boot (T-084). |

[engines.md](engines.md) gives the rest of the Firecracker default list. The
Firecracker values `reboot=k panic=1` come from its getting-started guide
(READ
`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/getting-started.md:260`).
The `root=` append is READ in
`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/vmm_config/boot_source.rs:74-86`.

WARNING: The command line never carries a secret. Any guest process reads it in
`/proc/cmdline`. Nonces, tokens and credentials go only over vsock port 5000,
after boot.

## The base image

### Build

- The base is Debian 13 (trixie), slim, glibc.
- mmdebstrap builds it from one pinned snapshot.debian.org timestamp. The
  timestamp is in images/pins.toml. The security archive uses the same
  timestamp.
- `SOURCE_DATE_EPOCH` is the snapshot time in seconds. Every timestamp in the
  image comes from it.
- apt checks the archive signatures of the snapshot. An old snapshot Release
  file can be past its Valid-Until date, so the build turns off only that date
  check.
- The variant is `minbase` plus the package list below. dpkg `path-exclude`
  rules drop `/usr/share/doc`, `/usr/share/man` and `/usr/share/locale`. They
  keep the copyright files.
- The build runs in the Linux runner (`cargo xtask linux`), in a container image
  pinned by digest. The planned command is `cargo xtask image build` (T-062).
- Reproducible means this: two builds from the same pins give the same image
  digest. The T-062 prove step checks it.

The snapshot URLs have this shape:

```text
https://snapshot.debian.org/archive/debian/<YYYYMMDDTHHMMSSZ>/ trixie
https://snapshot.debian.org/archive/debian-security/<YYYYMMDDTHHMMSSZ>/ trixie-security
```

### Contents

| Package or file | Why | Entry |
| --- | --- | --- |
| `box-guest` at `/usr/sbin/box-guest` | PID 1, static musl, built from this repository at the release commit. | T-040 |
| `kage` at `/usr/local/bin/kage` | The harness, a pinned static musl release. | T-063 |
| `bash` | The kage shell tool and the operator shell. | T-062 |
| `coreutils` | Base tools. | T-062 |
| `git` | Forge-only file flow through the broker. | T-062 |
| `ca-certificates` | TLS roots for git, curl and other system tools. kage does not read them. | T-062 |
| `openssh-client` | ssh to destinations the policy allows. Until T-257, only to hosts that need no credential. | T-062, T-257 |
| `curl` | HTTP from the shell. | T-062 |
| `e2fsprogs` | `mkfs.ext4` for the first-boot format of the data disk. | T-043 |
| `util-linux` | Mount and disk tools for the operator. | T-062 |
| `vim-tiny` | `vi`, which the T-042 and T-172 prove steps use. Added beyond the T-062 list. | T-062 |
| `less` | The pager that the T-172 prove step uses. Added beyond the T-062 list. | T-062 |
| user `agent` | uid 1000, gid 1000, home `/home/agent`, shell `/bin/bash`. | T-063 |

The image does not contain: systemd, sudo, an ssh server, an NTP daemon, a
firewall, a broker CA, or a container runtime. A container runtime is added
only if T-185 licenses it.

### Rules

- No credential in any layer. The image holds no key, token, nonce or CA. Every
  instance boots the same bytes, so a secret in the image goes to every
  instance.
- Before it packs the image, the build scans the root tree with the secret rules
  of `cargo xtask check secrets`. A hit stops the build.
- No setuid or setgid file. The build removes every setuid and setgid bit. The
  root password is locked. So uid 1000 has no supported path to root.
- Read-only, three times over:
  1. The engine attaches the base read-only (Firecracker `is_read_only`, HCS
     SCSI `ReadOnly`).
  2. The kernel mounts it read-only (`ro`; erofs has no write support).
  3. On the host, the file is owned by root or SYSTEM and is not writable by
     the operator.
- No image is ever made writable after build. A changed image is a new build
  with a new digest.
- Shared by digest. The host image cache holds one copy per digest. Every
  instance of that variant attaches the same file. The instance record stores
  the digest it booted (T-066, T-067).
- The resolver, `/etc/hosts` and the host name are not fixed in the image.
  box-guest writes them at each boot from provisioning data.
- The system git config is in the image (T-062, T-146). It holds only the
  `insteadOf` rules that send forge URLs to the broker, and no secret.

### Format: erofs or ext4

The base is the read-only lower layer of the overlay root. Two formats are
candidates:

| | erofs | ext4 |
| --- | --- | --- |
| Write support | none in the kernel | yes, mounted `ro` |
| Size | compressed, not measured | not measured |
| Boot time | not measured | not measured |
| Page-cache cost | not measured | not measured |
| Reproducible build | `mkfs.erofs` with a fixed UUID and timestamp | `mke2fs -d` with a fixed UUID and timestamp |

T-065 measures both and records a decision in [status.md](status.md). If T-065
picks ext4, the base is built without a journal, so a read-only mount never
needs a replay.

HCS attaches disks as VHD or VHDX files, so Windows needs a VHDX copy of the
base. The open question is where the copy is made:

- Conservative design: `image pull` verifies the raw image digest first. Then
  the helper converts the verified bytes to a VHDX in the store and records
  the VHDX digest in the cache index.
- Alternative: CI publishes a VHDX with its own pinned digest. This needs a VHDX
  writer with reproducible header GUIDs. Not measured.

T-065 decides between them.

## Variants and the variant rule

- Variants: `base`, and toolchain variants `rust`, `node` and `python`
  (T-064).
- Each variant is a complete, separate image with its own digest. Variants are
  not stacked at run time. An instance boots exactly one image.
- Each variant uses the same snapshot timestamp as `base`. A toolchain that
  Debian does not carry at the needed version comes as an upstream release
  archive, pinned by sha256 in images/pins.toml. T-064 decides the source per
  variant.
- The profile of an instance names its variant. The variant and its digest are
  fixed at create.
- The variant rule: adding a variant, or a package to any image, is a pins
  change. The change names an approver, and the approver is recorded with the
  pins diff.
- Few variants, on purpose. Each variant adds a build, a verification and a
  security rebuild to the T-069 owner's work.
- A tool that the agent installs at run time lands in the overlay upper on that
  instance's own data disk. It never lands in a shared image.
- No writable cache is shared between instances.

## The data disk and the overlay root

Each instance has one data disk: a sparse 30 GiB raw file on Linux (T-087), or
a dynamic 30 GiB VHDX on Windows (T-103). The host creates it empty and never
parses it.

### First boot

box-guest formats the data disk inside the guest. It formats only when both
conditions hold:

1. The provisioning on port 5000 says "first boot". The daemon sets this flag
   from the instance record.
2. The first MiB of the disk reads as zeros.

If one condition holds and the other does not, box-guest does not format. It
sends a `data-disk-unrecognized` event and stops the boot. A damaged disk is
never formatted over.

The format is `mkfs.ext4` from the base image, with the label
`agent-box-data`.

### Every boot

1. The kernel mounts the base image read-only as `/` and starts box-guest as
   PID 1.
2. box-guest finds the data disk. It is the one block device that is not the
   root device. More than one candidate stops the boot.
3. box-guest mounts the data disk at `/run/agent-box/data`, mode 0700, owner
   root.
4. It creates `/run/agent-box/data/root/upper` and
   `/run/agent-box/data/root/work` if they are missing.
5. It mounts an overlay: the base as `lowerdir`, the upper directory as
   `upperdir`, and the work directory as `workdir`.
6. It moves `/proc`, `/sys`, `/dev` and `/run` into the overlay and calls
   `pivot_root` into it. Then it detaches the old root.

T-043 proves it: a file written in the guest survives a stop and a start, and
the base image digest does not change.

### Layout

| Path | Storage | Owner | Survives stop and start |
| --- | --- | --- | --- |
| `/` (overlay) | lower: base image; upper: data disk | root | yes, the changes in the upper |
| `/home/agent` | data disk, through the overlay | uid 1000 | yes |
| `/home/agent/work` | data disk, through the overlay | uid 1000 | yes; the kage serve working directory |
| `/var` | data disk, through the overlay | root | yes |
| `/tmp` | tmpfs, kernel default size (half of guest RAM) | root, mode 1777 | no |
| `/run` | tmpfs | root | no |
| `/run/agent-box/data` | the data disk mount | root, mode 0700 | yes |
| `/run/agent-box/kage` | tmpfs | see the kage pins | no |
| `/run/user/1000` | tmpfs | uid 1000, mode 0700 | no |

The guest has no swap device. A secret in memory never reaches a disk through
swap.

## box-guest, PID 1

box-guest is a static musl Rust binary, crate box-guest, at
crates/box-guest/src/main.rs (planned). It depends on box-core and box-proto
only (T-002 checks the layering). A flag runs it as an ordinary process for
tests (T-040).

### The uid split

| Process | uid | Holds | Notes |
| --- | --- | --- | --- |
| box-guest | 0 | the event nonce, the egress token, the kage token, provider nonces, all in memory | not dumpable (`PR_SET_DUMPABLE` 0) |
| kage serve | 1000 | provider nonces in its environment; the kage token in a tmpfs file | started by box-guest |
| kage tool processes | 1000 | kage's environment minus the scrubbed names | the shell tool, MCP servers |
| attach sessions | 1000 | no nonce | `agent-box attach` |

What follows from the split:

- uid 1000 can read the kage token and the provider nonces. That is accepted.
  The token authenticates the host to kage only. The nonces work only through
  this instance's own network channel.
- uid 1000 cannot read the event nonce or the egress token. So it cannot forge a
  frame on the event channel (port 5002) or open a second network channel (port
  5003). T-031 proves this with a second guest process at uid 1000.
- There is no sudo, no setuid file, and the root password is locked.

WARNING: The split is only as strong as the guest kernel. A guest kernel
exploit that reaches root defeats it. The event channel is trusted for
lifecycle data only to that degree. This limit belongs in
[limits.md](limits.md).

### Duties

| Duty | Detail | Entry |
| --- | --- | --- |
| Early mounts | `/proc`, `/sys`, `/dev` (devtmpfs) and `/run` (tmpfs), before anything else. | T-040 |
| Control server | vsock port 5000: version handshake, provisioning, host time, rotation, stats, poweroff and fstrim requests. Every exec runs on port 5001. | T-041 |
| Provisioning | Holds the event nonce, the egress token, the kage token, provider nonces, host name and host time in memory only. | T-041 |
| Data disk | First-boot format, mount, overlay root and `pivot_root`. | T-043 |
| Late mounts | `/tmp` (tmpfs), `/dev/pts`, `/dev/shm`, and cgroup v2 at `/sys/fs/cgroup`. | T-040 |
| Identity | Writes `/etc/hostname`, `/etc/hosts` and `/etc/resolv.conf` at each boot. | T-041, T-045 |
| Time sync | Steps the clock from host time at boot and resume, and from ptp_kvm between them under KVM. | T-044 |
| Event channel | vsock port 5002, guest to host. The first frame carries the event nonce. A heartbeat goes on silence longer than 30 s. | T-031 |
| Network | Creates `tun0` and carries its packets on vsock port 5003. The first frame carries the egress token. Reconnects after a daemon restart. | T-045 |
| PTY and exec | vsock port 5001: PTY allocation, resize, signals, exit codes, and exec without a PTY. | T-042 |
| kage supervision | Starts `kage serve` as uid 1000 only after the daemon says ready. Restarts it after a crash, with backoff. | T-046, T-126 |
| kage bridge | vsock port 5004 to `127.0.0.1:7433`, one TCP connection per host connection. | T-046 |
| SSH agent | After T-257: serves `SSH_AUTH_SOCK` to uid 1000 and bridges each client to vsock port 5005. The key stays in the daemon. | T-257 |
| Reaping | Reaps every orphaned zombie, as PID 1 must. | T-040 |
| Signals | SIGTERM and SIGINT start the poweroff sequence. SIGINT is the Ctrl-Alt-Del path on x86_64. | T-040 |
| fstrim | Runs `FITRIM` on the data disk every 7 days of uptime, and when the host asks on port 5000. | T-041 |
| Poweroff | The poweroff sequence below. | T-040 |
| Secret hygiene | Never writes a secret to a disk. | T-041 |

The ports, frames and messages are in [protocol.md](protocol.md). The tun
address plan and the policy are in [network.md](network.md).

### Boot sequence

1. Early mounts.
2. Listen on vsock port 5000. Wait for the host handshake. A version mismatch
   refuses the session.
3. Receive provisioning. Step the clock to the host time.
4. Data disk, overlay root and `pivot_root` (see above).
5. Late mounts. Write the identity files.
6. Accept PTY and exec sessions on port 5001. An operator shell works from here
   on, even if kage fails later.
7. Connect to port 5002 and send the event nonce, then a `booted` event.
8. Create `tun0` and connect to port 5003 with the egress token.
9. Wait for the daemon's `ready` message. The daemon sends it only when the
   instance policy and bindings are live (deny first, T-126).
10. Write the kage config and token to tmpfs. Start `kage serve` as uid 1000.
11. Accept bridge connections on port 5004.

The handshake must finish within a bound that the daemon sets (T-088, T-105).
The provisioning additions beyond the protocol list (the first-boot flag, the
engine name, the binding endpoints and the forge settings) are design. T-041
adds them to box-proto.

### Poweroff sequence

1. Stop accepting sessions on ports 5001 and 5004.
2. Send SIGTERM to `kage serve`. Wait for it to exit, up to a bound that T-046
   sets. kage cancels its runs and closes its session files, with a 1 s grace
   (READ
   `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/serve/mod.rs:66`).
3. Send SIGTERM to every other process, wait, then send SIGKILL.
4. Send a last event on port 5002.
5. `sync`. Unmount the overlay and the data disk, or remount them read-only.
6. End the guest with the reboot call, not the power-off call. Firecracker
   does not implement guest power management, and a guest reboot ends it
   (READ
   `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/getting-started.md:336-338`).
   Under HCS, the compute system document sets `StopOnReset`, so a guest
   reset ends the compute system. That is not verified yet (T-105).

A power-off call can halt the vCPUs and leave the VMM running. T-088 and
T-105 measure both calls on each engine. If a step hangs, the host kills the
VMM after the stop grace. The data disk journal covers that case.

### Secret hygiene

- Every secret arrives on port 5000 after boot and stays in box-guest memory.
- The kage token and kage's data directory are on tmpfs (see the kage pins).
- Provider nonces live in the environment of `kage serve` only. Attach sessions
  do not get them.
- No swap, so memory does not reach the data disk.
- T-041 proves it with a scan of every file that box-guest wrote. T-088 and
  T-105 scan the raw data disk of a stopped instance.

CAUTION: A later `hibernate` that uses guest S4 writes guest RAM, nonces
included, to the data disk. T-184 must rotate every nonce and token after such
a resume, or not use guest S4.

## Time sync

TLS from the guest needs a correct clock. The guest has no NTP and the policy
would refuse it.

| Source | Engine | What it does |
| --- | --- | --- |
| Host time on port 5000 | both | At boot and after every resume, the daemon sends its UTC time. box-guest steps the clock to it. |
| ptp_kvm (`/dev/ptp0`) | Firecracker | Between those, box-guest reads the KVM PTP clock and steps the clock when the offset passes a threshold. |
| hv_utils time sync | HCS | The Hyper-V time sync integration service sets the guest clock over VMBus. box-guest does not step it between resumes. |

- Firecracker sends `KVM_KVMCLOCK_CTRL` on pause, which stops the soft-lockup
  watchdog (READ
  `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/vstate/vcpu.rs:257`).
  It does not correct the wall clock. The resume message does that.
- The check interval and the step threshold are not measured. T-044 sets them.
- ptp_kvm on aarch64 hosts and hv_utils under LinuxKernelDirect are not
  verified. T-044 tests both.
- Acceptance (T-044): after a 10-minute pause and resume, guest time is within
  1 s of the host on both engines.

## kage in the guest

kage is the harness (D2 in [decisions.md](decisions.md)). agent-box does not
write a harness. The facts below were read at
`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964` (2026-10-12, after
v0.3.0). Requests for kage changes go to
[TODO/upstream.md](../TODO/upstream.md), and the operator sends them.

kage adds no isolation. It has no OS sandbox: the shell tool says commands run
unsandboxed (READ
`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-tools/src/builtin/shell.rs:3`).
The instance and the host egress policy are the whole boundary.

### The release in the image

- The image holds one pinned kage release: `kage-x86_64-linux.tar.xz` or
  `kage-aarch64-linux.tar.xz`. Both are static musl builds (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:.github/workflows/release.yml:39`).
- kage publishes no sums file. Its release workflow attests build provenance
  instead (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:.github/workflows/release.yml:127`).
- So the image build records its own sha256 of each archive in images/pins.toml.
  It also verifies the provenance attestation at build time (T-063).
- kage changes fast: five releases in eight days up to v0.3.0 (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:CHANGELOG.md:2`).
  Pin one exact version. T-013 re-pins the references.
- The T-063 prove step: `kage --version` in the guest prints the pinned version.
  The build refuses a wrong digest.

### kage serve and its token

box-guest runs one `kage serve` per instance as uid 1000:

```text
kage serve --host 127.0.0.1 --port 7433
```

- Port 7433 and host 127.0.0.1 are the defaults (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/serve/mod.rs:60`).
  The command passes them anyway, so a kage default change does not move them.
- Serve takes at most 16 connections (READ the same file, line 57). Every
  connection shares one engine. There is no TLS.
- The token: serve reads it from `$XDG_DATA_HOME/kage/remote-token` (READ the
  same file, line 91). An existing file of 64 hex characters is loaded as it is
  (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-remote/src/token.rs:37`).
- So the host provisions a fresh 256-bit token on port 5000 at each boot.
  box-guest writes it as 64 hex characters, mode 0600, owner uid 1000.
- Serve also writes `connect.txt` beside the token file. That file holds the
  token (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/serve/mod.rs:224`).
- Therefore `XDG_DATA_HOME` for serve is a tmpfs directory. Both files stay off
  the data disk.
- kage keeps sessions under the same data directory. box-guest links
  `/run/agent-box/kage/data/kage/sessions` to
  `/home/agent/.local/share/kage/sessions` on the data disk, so sessions
  survive a stop.
- The symlink layout is INFERRED from the read code and not tested. T-046 tests
  it. An upstream option for the token path would remove it. That is a new
  request for T-169.
- The daemon's ACP client sends the token in the `Authorization: Bearer` header.
  It never uses the `?token=` query form.

### The bridge port

- vsock port 5004 carries the kage serve bridge. box-guest accepts each host
  connection and opens one TCP connection to `127.0.0.1:7433`. It copies bytes
  both ways and parses nothing.
- The daemon's ACP client (T-161) speaks ACP over a WebSocket through it.
- On Windows, every host socket sets `HVSOCKET_CONNECTED_SUSPEND`, so a pause
  does not drop the bridge (T-025, T-106).
- Every frame from kage is agent-reported data. Any process at uid 1000 can
  write to kage serve or forge its output. The daemon size-bounds every frame
  and never takes authorization, billing or backoff from it (R-TRUSTED-CHANNEL
  in [architecture.md](architecture.md)).

### Configuration pins

box-guest generates kage's configuration at each boot from provisioning data. It
holds endpoints and names, never a credential.

| Setting | Value | Where | Why |
| --- | --- | --- | --- |
| `XDG_CONFIG_HOME` | `/run/agent-box/kage/config` | serve environment | Holds the generated config file on tmpfs, owned by root, read-only to uid 1000. |
| `XDG_DATA_HOME` | `/run/agent-box/kage/data` | serve environment | tmpfs, owner uid 1000, mode 0700: the token, `connect.txt`, `auth.json` and `mcp-auth.json` stay off the disk. |
| `XDG_STATE_HOME` | `/home/agent/.local/state` | serve environment | Data disk: history and `trust.json`. |
| `XDG_CACHE_HOME` | `/home/agent/.cache` | serve environment | Data disk. |
| `XDG_RUNTIME_DIR` | `/run/user/1000` | serve environment | The serve socket that `kage resume` attaches through. |
| `[providers.<id>] base_url` | `http://<binding>.broker.internal` | generated config, and `KAGE_PROVIDERS__<ID>__BASE_URL` | Credentialed traffic goes to a broker endpoint (see TLS roots below). |
| provider key variable | a nonce in the provider's key format | serve environment | kage reads the variable first, then `auth.json`. |
| `KAGE_SHELL__PROGRAM` | `/bin/bash` | serve environment | A project file cannot change the shell program. |
| `KAGE_SHELL__SCRUB_ENV` | `["*_API_KEY"]` | serve environment | Shell tool children do not inherit the provider nonces (T-160). The pattern leaves the `GIT_CONFIG_*` variables of [broker.md](broker.md) to git. |
| `KAGE_TOOLS__WEB_SEARCH__ENGINE` | `duckduckgo` | serve environment | A project file cannot point web search at another engine. Beyond the minimum pins. |
| `[mcp]` | no servers | generated config | MCP OAuth is undecided (T-162). |
| `[plugins]` | no capability grants | generated config | No plugin runs a process or opens the network. |

The environment wins over both config files (READ
`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-core/src/config.rs:266`,
order: defaults, user file, project file, environment). The key variable is
read before `auth.json` (READ
`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/providers.rs:275`).

Provider mapping for the three built-in kinds. The binding kind is the `kind`
field of a broker binding (T-141).

| Binding kind | kage provider id | Key variable | `base_url` | Header the broker replaces |
| --- | --- | --- | --- | --- |
| anthropic | `anthropic` | `ANTHROPIC_API_KEY` | `http://<binding>.broker.internal` | `x-api-key` |
| openai | `openai` | `OPENAI_API_KEY` | `http://<binding>.broker.internal/v1` | `authorization: Bearer` |
| gemini | `google` | `GEMINI_API_KEY` | `http://<binding>.broker.internal` | `x-goog-api-key` |

- The openai kind needs the `/v1` segment in `base_url` (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/guide/providers.md:364`).
- The kage guide names the Gemini provider `gemini`, but the code registers
  the id `google` and reads `GEMINI_API_KEY` (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/providers.rs:12`).
  The override table is therefore `[providers.google]`.
- kage sends exactly one auth header of its kind (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/anthropic.rs:103`,
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/openai.rs:141`
  and
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/gemini.rs:92`).
- An environment variable name cannot hold a dash. So `KAGE_PROVIDERS__...`
  cannot reach a provider id with a dash (READ the providers guide, line
  396). The generated config file carries those.
- A provider registers only when its key is present. So the provisioned nonces
  are the providers the guest can see.
- kage reads the provider keys once, when serve starts (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/rpc/host.rs:86`).
  A nonce rotation therefore restarts serve between runs. T-141 decides.
- The array parse of `KAGE_SHELL__SCRUB_ENV` is INFERRED from figment source.
  T-160 tests it against the release binary.
- Write refusals are expected: "Always allow" and `_kage/config/set` cannot
  write the root-owned config. T-160 checks that serve still starts with it.

### Approval mode allow, and why

Decision (T-160, record in [decisions.md](decisions.md)): kage runs in approval
mode `allow`.

How it is set:

1. The generated config has no `[permissions]` table. A built-in tool with no
   rule runs (READ
   `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/permissions.rs:104`).
2. The daemon's ACP client sends `_meta.kage.unconfiguredTools: "allow"` in its
   client capabilities (READ
   `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/kage-extensions.md:18`).
3. The daemon answers every remaining ACP permission request with allow.
   It logs each answer as agent-reported. Plan mode still asks for commands in
   allow mode, so this answer is needed.

The reasons:

- kage approvals are not a boundary inside the guest. Tool processes run as
  kage's uid. They can read the serve token and answer their own prompts, and
  the first answer wins (INFERRED from
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/remote.md:151`).
- An unanswered ask stalls or degrades an unattended run. Print mode denies an
  ask. Serve waits for a client and denies when the last client leaves (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/guide/permissions.md:113`).
- The real boundary is the instance, the host egress policy and the broker. A
  destructive tool call can harm only the instance's own data disk.
- The operator can attach at any time and watch.

The cost: a destructive command runs without a prompt. Pushes are limited to
the instance's ref namespace by the forge binding (T-146).

### Never kage trust

- agent-box never runs `kage trust`. The generated config marks no project as
  trusted.
- An untrusted `<project>/.kage/config.toml` loses its `[mcp]`, `[permissions]`
  and `[plugins.capabilities]` tables. `[providers]` and `[acp]` never apply
  from a project (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-core/src/trust.rs:110`).
- Project agents and skills wait for trust. Serve ignores them with a warning.
- A `<project>/.kage/AGENTS.md` loads with no trust step when the project has
  no risky table, agent or skill (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-core/src/trust.rs:61`
  and
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/runtime_env.rs:117`).
  It is untrusted prompt input.
- The agent's own uid can run `kage trust` from the shell and write
  `trust.json`. Inside the guest, trust is advisory. The host never depends on
  it.

### The shell trust gap

- An untrusted project file still sets `[shell] program` and
  `[shell] scrub_env`, and also `[tools.web_search]` and `[tools.rename]`. The
  filter removes only the three trust-gated tables (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-core/src/trust.rs:110`).
- A cloned repository can therefore choose the program that every shell tool
  call runs. Inside an instance this is not an escape.
- The environment pins above close it, because the environment wins over the
  project file.
- T-160 proves it: a guest test shows that a `<project>/.kage/config.toml`
  cannot change the pinned shell.
- T-169 records the request to trust-gate `[shell]`, `[tools.web_search]` and
  `[tools.rename]`. The operator sends it to the maintainer.

### MCP OAuth (open question)

- `kage mcp login` stores real OAuth access and refresh tokens in
  `$XDG_DATA_HOME/kage/mcp-auth.json`. kage refreshes them itself (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/mcp_auth.rs:187`).
- That is a second credential path into the guest. It breaks
  R-NO-SECRET-IN-GUEST if the operator's own MCP account is used.
- v1 design: the generated config has no MCP server. `mcp-auth.json` is on
  tmpfs and is lost at every stop.
- T-162 decides: refuse MCP OAuth in v1, or broker it.

### kage's own destinations

T-163 ships two named policies. `default` is `allowlist` mode with no rule,
so only the broker endpoints of the instance's bindings are reachable. `web`
is `open` mode, with the reserved ranges still refused.

| Destination | Used by | Policy `default` | Policy `web` | Entry |
| --- | --- | --- | --- | --- |
| provider APIs | model calls | only through `<binding>.broker.internal` | the same; never direct | T-142 |
| `html.duckduckgo.com:443` | `web_search`, the default engine | refused | allowed | T-163 |
| `api.search.brave.com:443` | `web_search` with the Brave engine | refused | refused: it needs `BRAVE_API_KEY` in the guest and has a fixed https URL | T-163 |
| any URL | `web_fetch` | refused | allowed | T-163 |
| MCP server URLs | MCP tools | refused | refused; an operator rule adds a server | T-162 |
| `models.dev:443` | `kage models refresh` only | refused | allowed; agent-box never runs the refresh | T-163 |
| `github.com` | git | only through the forge binding | the same | T-146 |

The DuckDuckGo and Brave hosts come from the kage source (READ
`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-tools/src/builtin/web_search.rs:195`
and line 334).

### TLS roots and broker endpoints

- kage trusts only the Mozilla roots compiled into webpki-roots. It sets no TLS
  config of its own, and its lock file has no native-certs or platform
  verifier (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/http.rs:43`
  and `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:Cargo.toml:91`).
- So a CA planted in the guest does not work for kage. TLS interception of
  provider traffic would fail every request.
- Decision P4: credentialed traffic goes to plain-HTTP broker endpoints. kage
  calls `http://<binding>.broker.internal`. The broker opens TLS to the
  upstream itself ([broker.md](broker.md)).
- The guest never names the upstream host, and holds only a nonce.
- Traffic without a credential (web search, web fetch) goes out directly as TLS
  through the netstack. kage verifies it with its own roots, so the guest clock
  must be correct.
- kage resolves names in the guest. The netstack DNS answers
  `*.broker.internal` with the gateway ([network.md](network.md)).
- kage allows 600 s of silence per provider call, and opens a new connection per
  call (READ
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/http.rs:43`).
  The netstack flow idle timeout is longer than 600 s. A pause longer than 600 s
  during a stream fails that call, and kage retries.

## The image supply chain

### Pins

images/pins.toml holds every input and every output digest:

| Pin | Holds | Entry |
| --- | --- | --- |
| kernel | the LTS release, the tarball URL and its sha256, the build timestamp values | T-060 |
| Debian | the snapshot timestamp, the suites, the mirror URLs | T-062 |
| builder | the Linux runner container image digest | T-062 |
| kage | the release version, the archive name and sha256 per architecture, the attestation signer workflow | T-063 |
| toolchains | the version and sha256 of each upstream archive per variant | T-064 |
| outputs | the sha256 of each kernel file and each image per variant and architecture | T-060, T-062 |
| approvers | the approver of each variant | T-064 |
| owner | the named owner of rebuilds and security patches | T-069 |

Rules (READ
`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:76`):

- Pin a release tag or a commit, never a branch.
- Check the digest before use. On a mismatch, delete the cached copy and use
  nothing.
- No network and no verified cache is an error. There is no fallback to an
  unverified file.
- Take each digest from the server of the file, not from a checkout. A
  checkout with CRLF line endings hashes differently.

### Three verification tiers

Each tier answers a different question (READ
`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:94`):

| Tier | What it proves | What it cannot say |
| --- | --- | --- |
| 1. A sums file published with the artifact | Transport: the bytes that arrived are the bytes that were uploaded. | Who uploaded them. |
| 2. A digest obtained separately and reviewed | These are the exact bytes somebody reviewed. | Anything about later releases. |
| 3. A signature or attestation checked against the publishing workflow | The artifact came from that workflow. It needs a network lookup at verify time. | That the contents are correct. |

A verifier reports one of four outcomes: verified, nothing published to verify,
no verifier installed, failed. A step that could not check never reports a
pass.

| Artifact | Tier 1 | Tier 2 | Tier 3 |
| --- | --- | --- | --- |
| kernel tarball | kernel.org sums file | sha256 in images/pins.toml | not used; T-060 decides on the PGP signature |
| Debian packages | apt checks archive signatures and package hashes | the snapshot timestamp, then the image digest | the archive signature |
| kage archive | none published | sha256 in images/pins.toml | GitHub build-provenance attestation |
| agent-box kernel and images | `SHA256SUMS` in the release | digests embedded in the `agent-box` binary | GitHub artifact attestation (T-068) |

### Digests in the release binary

- The `agent-box` release binary embeds the digests of the kernel files and the
  images it was built with (T-066, T-200).
- `agent-box image pull` fetches the release assets for those digests and
  verifies tier 2 before the file enters the cache.
- Tier 2 is mandatory. Tier 1 is checked when `SHA256SUMS` is present. Tier 3
  is reported with its outcome, never skipped in silence.
- `agent-box image verify` hashes the cache again and reports each tier.
- The engine uses a cached file only after a verified pull. A mismatch deletes
  the cached copy and refuses (T-066).
- `agent-box image gc` never removes an image that an instance records
  (T-067).

### Ownership

Open question, T-069: who owns the kernel and image rebuilds and the security
patches, and on what cadence? The answer is a named owner, recorded next to the
pins. Without an owner, nothing rebuilds the image when Debian or the kernel
ships a fix.

A related open question: how an existing instance moves to a newer image. The
overlay upper was built over the old lower. No upgrade verb exists in v1.

## Open questions

| Question | Entry |
| --- | --- |
| Do `virtio_vsock` and `hv_sock` work in one kernel with the HCS command-line switch? | T-061 |
| How does HCS name the SCSI disks, and is `root=/dev/sda` stable? The fallback is an initramfs that finds the base by filesystem UUID. | T-061, T-105 |
| erofs or ext4 for the base, and where the VHDX copy is made. | T-065 |
| Is Firecracker free page reporting generally available in v1.17.0? | T-181 |
| Does the tmpfs data directory with a sessions symlink work for kage serve? | T-046 |
| Does kage serve start with a root-owned, read-only config directory? | T-160 |
| Does `KAGE_SHELL__SCRUB_ENV` parse as an array and override a project value? | T-160 |
| Refuse MCP OAuth in v1, or broker it? | T-162 |
| Nonce rotation: restart serve, or use `_kage/auth/set` on a tmpfs `auth.json`? | T-141 |
| Does ptp_kvm work on aarch64 hosts, and hv_utils time sync under LinuxKernelDirect? | T-044 |
| Who owns image rebuilds, and on what cadence? | T-069 |
| How does an instance move to a newer image? | T-069 |
| Does a guest S4 hibernate leak nonces to the data disk? | T-184 |
