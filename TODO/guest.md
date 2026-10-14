# Guest

This area holds box-guest, the agent that runs as PID 1 in every instance. It
mounts the file systems, builds the overlay root, serves the host on the vsock
ports, carries the network channel and supervises kage.
[docs/guest.md](../docs/guest.md) gives the guest design.
[docs/protocol.md](../docs/protocol.md) gives the ports and the messages.
[image.md](image.md) holds the kernel and image entries, and [kage.md](kage.md)
holds the kage configuration.

The crate crates/box-guest does not exist yet. T-033 creates it. box-guest is
a static musl binary that depends on box-core and box-proto only (T-002).
Everything inside the guest is untrusted. box-guest runs as root, and kage runs
as uid 1000, the user agent.

# T-040: box-guest PID 1: mounts, reaping, signals, poweroff

**Source:** The planning brief of 2026-10-14, section 5; Firecracker docs/api_requests/actions.md at v1.17.0.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

PID 1 has duties that no other process has. It must mount the pseudo file
systems, reap orphans, survive signals and power the guest off cleanly. A PID 1
that panics stops the instance.

## Premise

- Design: box-guest is a static musl Rust binary and PID 1. It mounts, reaps
  zombies, handles signals and powers off (brief section 5).
- READ: Firecracker exits on a CPU reset. SendCtrlAltDel exists on Intel and AMD
  only
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/actions.md:34-40`).
  An aarch64 guest needs an in-guest poweroff.
- READ: Firecracker has no guest power management, so the restart call of a
  guest ends the VMM
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/getting-started.md:336-338`).
  T-088 measures what the power-off call does.
- READ: kage serve cancels its runs on SIGTERM and waits 1 s before it exits
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/serve/mod.rs:66`).
- READ: on Debian slim, /bin/sh is dash
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:155-174`).
  box-guest never builds a shell command string.
- Design: the T-032 double runs box-guest as an ordinary process for M1 tests.

## Approach

1. In crates/box-guest/src/main.rs, choose the mode at start. PID 1 runs the
   init path.
2. With `--test-transport unix:DIR`, run as an ordinary process: no mounts and
   no power calls.
3. In that mode, use AF_UNIX sockets in DIR with the hybrid-vsock handshake, for
   the T-032 double.
4. In crates/box-guest/src/init.rs, mount /proc and /sys with nosuid, nodev and
   noexec.
5. Mount /dev as devtmpfs, then /dev/pts as devpts with gid 5 and mode 0620.
6. Mount /dev/shm, /run and /tmp as tmpfs with nosuid and nodev.
7. Mount the cgroup2 file system at /sys/fs/cgroup.
8. If a mount fails, log the error and power off. Never run half set up.
9. Log to /dev/kmsg until the event channel of T-031 is up.
10. In crates/box-guest/src/reap.rs, call waitpid in a loop on SIGCHLD.
11. Report the exit of a tracked child (kage, a session). Reap other children
    in silence.
12. In crates/box-guest/src/signal.rs, handle SIGTERM, SIGINT and SIGPWR as a
    poweroff request.
13. Disable Ctrl-Alt-Del with reboot(2), so that the kernel sends SIGINT to
    PID 1. SendCtrlAltDel then gives a clean poweroff.
14. Answer when called as /sbin/poweroff, by a signal to PID 1. Verify the
    Hyper-V shutdown path through it in T-105.
15. In crates/box-guest/src/power.rs, stop new sessions first.
16. Send SIGTERM to kage and to each session process group. Wait the grace time
    of the request, 10 s by default.
17. Send SIGKILL to what remains, sync, and unmount the data disk or remount it
    read-only.
18. End with one reboot call. The command is a setting of power.rs, power-off
    by default.
19. T-088 measures the power-off call under Firecracker. If the VMM stays up,
    the default becomes the restart command, which ends Firecracker.
20. With `StopOnReset` true, the restart command also ends the HCS compute
    system (T-105). Then one call serves both engines.
21. Treat a reboot request the same way. The daemon decides about a new start.
22. Build with panic abort. Never call `unwrap` on data from a socket or a file.

## Prove

```sh
cargo test -p box-guest
```

It exits 0 on any Linux host, or in `cargo xtask linux` on Windows. Tests
drive the test mode with `POWEROFF`, and check the order of the poweroff steps with a
fake system layer.

The live check is `pid1_boot_poweroff` in T-088 and T-105: box-guest boots as
PID 1, answers the handshake and powers off.

## Start condition

T-033.

# T-041: box-guest control server and provisioning

**Source:** The planning brief of 2026-10-14, sections 3 (R-NO-SECRET-IN-GUEST), 5 and 7.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The guest needs its nonces, its egress token, the kage token, its name and the
time before anything else runs. Each place a guest process can read is the wrong
place for them. A guest process that reaches the control port would get root.

## Premise

- Design: no credential ever goes into the image, the disk, the kernel command
  line, MMDS or the VMM argv. T-256 owns that rule, and this entry keeps it
  in box-guest.
- Design: nonces and the egress token go only over the control port, after
  boot and before kage starts (brief section 3).
- READ: the kernel command line is readable in /proc/cmdline. MMDS is readable
  by every guest process. The jailer argv is visible on the host.
- Design ([docs/protocol.md](../docs/protocol.md)): port 5000 carries the
  handshake, provisioning, time, rotation, stats, poweroff and fstrim. Every
  exec runs on port 5001 (T-042).
- READ: kage loads a well-formed token file as it is, so the host can provide
  the token
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-remote/src/token.rs:37-58`).
- READ: guest discard reaches the host file only with the Sync engine
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-discard.md:12-17`).
- READ: send a command into a guest as an argument vector, never as a string
  that a shell parses again
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:155-174`).

## Approach

1. In crates/box-guest/src/control.rs, listen on vsock port 5000 before any
   other process starts.
2. Accept a connection only from the host CID 2. Close any other peer, a guest
   process on the local CID included.
3. Apply the same peer rule to ports 5001 and 5004.
4. Request a kernel config without the vsock loopback transport (T-060).
5. Serve one host connection at a time. A new host connection replaces the old
   one, for a daemon restart.
6. Expect `HELLO` with the version first. On a mismatch, send
   `ERROR version_mismatch` and close.
7. Accept `PROVISION` once per boot. Refuse a second one. Rotation uses
   `ROTATE`.
8. `PROVISION` carries the event nonce, the egress token, the kage token, the
   provider nonces, the hostname and the host time.
   - Add the design fields of [docs/guest.md](../docs/guest.md#boot-sequence)
     to it: the first-boot flag (T-043), the engine name, the binding endpoints
     and the forge settings.
   - The daemon sets the first-boot flag from the instance record. Write the
     new fields into docs/protocol.md in the same change.
9. Keep these values as the `ProvisionedValue` type of T-256, which zeroizes
   its memory on drop.
10. Lock that memory with mlock, and make the process not dumpable.
11. Set the hostname and the clock (T-044) from `PROVISION`.
12. Hand the kage token to T-046, which keeps it on a tmpfs.
13. Start the event channel (T-031) and the network channel (T-045) after
    `PROVISION`.
14. Start kage only on `READY` (T-046, T-126).
15. `STATS_REQUEST`: return the fields of docs/protocol.md, read from /proc
    and statvfs.
16. `POWEROFF`: run the sequence of T-040 with the grace time of the daemon.
17. `FSTRIM`: run /sbin/fstrim on the data disk mount. Send the result as an
    event on port 5002.
18. Run fstrim once a week from a timer in box-guest. There is no systemd in
    the guest.
19. Never write a provisioned value to a file, except the tmpfs token of T-046.

## Prove

```sh
cargo test -p box-guest control::
cargo test -p box-daemon --test guest_double
```

Both exit 0 on any Linux host, or in `cargo xtask linux`. The tests use the
T-032 double. They cover the handshake, a second `PROVISION`, stats, a
rotation and poweroff.

A test refuses a peer whose CID is not 2. Another test provisions marker values,
then scans every file that box-guest wrote. No marker appears in raw, hex or
base64 form.

The live check is `no_secret_on_disk` in T-088 and T-105: the same scan over
the raw data disk of a stopped instance.

## Start condition

T-022, T-033 and T-256.

# T-042: box-guest PTY and exec sessions

**Source:** The planning brief of 2026-10-14, section 7; TEMPLATE docs/containers.md on watching a guest command.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The operator attaches to an instance with a terminal, and the daemon runs
commands in it. A terminal must pass every key, resize and signal. A command
without a terminal must keep stdout, stderr and the exit code apart.

## Premise

- Design: port 5001 carries the PTY and exec streams, host to guest (brief
  section 7).
- READ: report a heartbeat on silence, not on a timer. Give a command a time
  limit with its own exit code. Inject nothing into the guest for this
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:201-222`).
- READ: the kage TUI attaches to a local kage serve only, through a same-user
  Unix socket. Attach is a PTY in the guest that runs kage
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/remote.md:168-194`).
- READ: the kage shell tool runs each command in its own process group
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-tools/src/builtin/shell.rs:284-286`).

## Approach

1. In crates/box-guest/src/session.rs, listen on vsock port 5001, for the host
   CID only.
2. Expect `OPEN` first, with the fields of docs/protocol.md: the kind (PTY or
   exec), the argument vector, the environment, the directory and the size.
3. `OPEN` names the user: `agent` by default, or root on an explicit request
   of the operator or for a battery check (T-172, T-180).
4. Take the argument vector as it is. Never join it into a shell string.
5. In PTY mode, an empty argument vector starts the login shell of the user.
6. Open a PTY pair. Start the child in a new session with the PTY as its
   controlling terminal.
7. Drop to uid and gid 1000 (agent) unless `OPEN` names root. Clear the
   supplementary groups.
8. Build the environment from nothing: HOME, USER, SHELL, TERM, PATH and LANG,
   then the variables of `OPEN`.
9. Copy `STDIN` frames to the PTY. Send PTY output as `STDOUT` frames.
10. On `RESIZE`, set the window size on the PTY. The kernel then sends
    SIGWINCH.
11. In PTY mode, Ctrl-C is the byte 0x03 on the PTY. The line discipline sends
    SIGINT.
12. In exec mode, use three pipes. Send stderr as `STDERR` frames.
13. Close the child stdin on the `STDIN_EOF` frame.
14. On `SIGNAL`, send the signal to the process group of the child. Accept
    INT, TERM, HUP, QUIT and KILL only.
15. Send `SILENT` after 30 s with no output, and again after each further
    30 s.
16. At the time limit of `OPEN`, send SIGKILL and report outcome 3 (timed
    out) in `EXIT`.
17. Send `EXIT` with the exit code or the signal, after the output drains.
18. Bound the queue of each session. When the host stops reading, stop reading
    the PTY.
19. When the host connection drops, send SIGHUP to the process group of a PTY
    session.

## Prove

```sh
cargo test -p box-guest session::
```

It exits 0 with the T-032 double on any Linux host, or in `cargo xtask linux`.
Tests echo through a PTY, and check that a resize changes `stty size`.

Other tests check that the byte 0x03 stops `sleep 100` with SIGINT. They check
the exec exit codes, the stderr stream, the time limit and SIGHUP on disconnect.

The live check is the attach test of T-172, on both OSes: `vi`, a resize and
Ctrl-C through `agent-box attach NAME --shell`.

## Start condition

T-033.

# T-043: Data disk: first-boot format and the overlay root

**Source:** The planning brief of 2026-10-14, sections 4.1 and 5; the engine evaluation, Linux engine facts.
**Category:** feature
**Milestone:** M2
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

An instance keeps 30 GiB of work by default across stops, host reboots and engine
upgrades. The base image is shared by digest and never changes. A host that
formats or mounts a guest-written disk parses untrusted data.

## Premise

- Design: box-guest formats the data disk inside the guest on the first boot.
  The host never parses it (brief section 5).
- Design ([docs/guest.md](../docs/guest.md#first-boot)): a format needs both
  the first-boot flag of the provisioning and a first MiB of zeros. So a
  damaged disk is never formatted over.
- Design: box-guest mounts an overlay root, with the base as the lower layer and
  the upper and work directories on the data disk.
- Design: /home, /var and the work of the agent live on the data disk.
- READ: the host must never fsck, mount or resize an agent-written ext4 image.
  That is an inference of the engine evaluation.
- READ: Firecracker reads the drive capacity at boot. A sparse 30 GiB file shows
  30 GiB with no preallocation
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/devices/virtio/block/virtio/device.rs:70-95`).
- READ: an overlay on a fixed-size disk fills like any disk. It can show as a
  broken build. T-185 measures it.
- Design: discard is on, and box-guest runs fstrim weekly (brief section 4.1).

## Approach

1. In crates/box-guest/src/disk.rs, find the data disk as
   [docs/guest.md](../docs/guest.md#every-boot) gives it. It is the one block
   device in /sys/class/block that is not the root device.
2. No kernel parameter names the disk. More than one candidate, or none, stops
   the boot with an event that names the devices.
3. Check that the base disk is read-only and the data disk is writable, from
   the `ro` files in /sys.
4. Read the first-boot flag from `PROVISION` (T-041), and read the first MiB
   of the data disk.
5. First-boot flag set and the first MiB all zero: run `mkfs.ext4` from the
   base image with the label `agent-box-data`. Then mount it.
6. No first-boot flag and an ext4 superblock with the label `agent-box-data`:
   mount the disk.
7. Any other case: do not format. Send the `data-disk-unrecognized` event and
   stop the boot. A damaged disk is never formatted over.
8. Mount the data disk at /run/agent-box/data, mode 0700, owner root, without
   the discard option. fstrim runs in batches (T-041).
9. Build the overlay. The lower layer is a read-only bind of the base root.
10. Put the upper and work directories in /run/agent-box/data/root/upper and
    /run/agent-box/data/root/work. Create them when they are missing.
11. Move /proc, /sys, /dev and /run into the overlay. Pivot into it with
    pivot_root, then detach the old root.
12. Do all of this before any other process starts.
13. Open question: an image update changes the lower layer under an existing
    upper layer. Decide the update path in T-064 and T-067.
14. Resize of the data disk is not in this entry.

## Prove

```sh
cargo test -p box-guest disk::
```

It exits 0 on any Linux host, or in `cargo xtask linux` on Windows. Tests give
the disk module image files with an ext4 superblock, with zeros and with other
bytes, each with and without the first-boot flag. The module formats only
zeros with the flag. It mounts only the labelled ext4 disk without the flag,
and refuses every other case.

Other tests give the module a fake /sys/class/block. They refuse two
candidate disks, no candidate, and a writable base disk. The boot check is
`data_disk_survives_restart` in T-088 and T-105: a marker survives stop and
start, and the base digest stays.

## Start condition

T-062 and T-041.

# T-044: Guest time sync

**Source:** The planning brief of 2026-10-14, section 5; the adversarial review, item 13b.
**Category:** feature
**Milestone:** M2
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

The guest has no NTP, because it has no route to a time server of its own. TLS
needs a correct clock. A pause of minutes leaves the guest clock behind the
host.

## Premise

- Design: use ptp_kvm under KVM and hv_utils time sync under Hyper-V (brief
  section 5).
- Design: also send the host time over the control port at boot and after each
  resume.
- READ: on x86_64, Firecracker calls KVM_KVMCLOCK_CTRL after it pauses the
  vCPUs, so the soft-lockup watchdog does not fire (Firecracker CHANGELOG,
  v1.10).
- READ: the adversarial review infers that hv_utils syncs time over VMBus and
  that ptp_kvm needs no VMM support. Both need a test, aarch64 included.
- READ: agent-sandbox issue
  [1237](https://github.com/kubernetes-sigs/agent-sandbox/issues/1237) proposes
  a clock re-sync on resume, and readiness only after it. It is a proposal.

## Approach

1. In crates/box-guest/src/time.rs, set CLOCK_REALTIME from the host time in
   `PROVISION`.
2. Do it before the network channel and kage start.
3. After each resume, the daemon sends `TIME_SET` on port 5000 (T-089, T-106).
4. On `TIME_SET`, step the clock when the offset is above 500 ms, a design
   value.
5. The daemon shows the instance as running only after `TIME_ACK`.
6. Under KVM, read the PTP clock of /dev/ptp0 every 60 s, a design value.
7. Step the system clock when the offset is above 500 ms.
8. Under Hyper-V, let the hv_utils driver keep the clock.
9. Send the event `GUEST_STATE` kind 5 (clock stepped) on each step.
10. Request PTP_1588_CLOCK_KVM and HYPERV_UTILS in the kernel config (T-060).
11. Test ptp_kvm on aarch64 before the aarch64 support claim.
12. Add `guest_clock_offset_ms` to `agent-box inspect --json`. The daemon
    measures it with a `STATS` round trip.

## Prove

```sh
cargo test -p box-guest time::
```

It exits 0 on any Linux host, or in `cargo xtask linux` on Windows. Tests
drive the time module with a fake clock: it steps above 500 ms, keeps the
clock below it, and answers `TIME_SET` with `TIME_ACK`.

The live checks run on a Linux KVM host and on Windows 11 with Hyper-V. In
T-088 and T-105, `boot_time_sync` finds the guest clock within 1 s of the host
after boot.

In T-089 and T-106, `pause_time_sync` finds it within 1 s after a 600 s pause
and a resume.

# T-045: Guest network: the tun device and the network channel

**Source:** The planning brief of 2026-10-14, ruling D5 and section 6.1; bailey crates/bailey/src/cli.rs.
**Category:** feature
**Milestone:** M5
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

DNS, ssh and any TCP or UDP must work in the guest (D5). The VM configuration
has no NIC device.
Every packet must reach the host netstack on a channel that the host can tell
apart per instance.

## Premise

- Design: box-guest creates tun0. The brief proposes guest 10.200.0.2/24,
  gateway 10.200.0.1 and DNS 10.200.0.1, and leaves the values to this entry.
- Design: the MTU fits the vsock framing. IP packets travel as frames on port
  5003. The first frame carries the egress token (brief section 6.1).
- READ: bailey gives the confined side a made-up private address and MAC, never
  those of the host
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:crates/bailey/src/cli.rs:404-412`).
- READ: pasta maps the guest's 127/8 to the host loopback by default, and
  bailey drops it
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:crates/bailey/src/cli.rs:456-475`).
- READ: errand gives a session a made-up resolver, never the resolv.conf of the
  host
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/policy.ts:74-93`).
- READ: the identity of a flow is the host-side channel. All guests can share
  one internal address. That is an insight of the relevance pass.

## Approach

1. Decision: every instance uses guest 10.200.0.2/24, gateway 10.200.0.1 and
   DNS 10.200.0.1. Record it in docs/network.md.
2. Decision: IPv4 only in v1. Turn IPv6 off on tun0 and in the default sysctl.
   The netstack refuses IPv6 frames.
3. In crates/box-guest/src/net.rs, open /dev/net/tun with IFF_TUN and
   IFF_NO_PI.
4. Set the address, the MTU and the link state of tun0. Add the default route
   through 10.200.0.1.
5. Take the MTU from box-proto, where docs/protocol.md puts it. T-120 sets
   it by measurement.
6. Refuse an MTU outside 1280 to 65535.
7. Write /etc/resolv.conf with the one name server 10.200.0.1.
8. Connect to host CID 2, port 5003. Send `NET_AUTH` with the version and the
   egress token first.
9. Keep tun0 down until `NET_READY` arrives (T-126). Then send each packet
   read from tun0 as one `PACKET` frame, and write each host `PACKET` to tun0.
10. Bound both queues. When a queue is full, drop the packet and count it.
11. Never let the network queues block the control or event channels.
12. On an error or end of input, reconnect with backoff from 1 s to 30 s,
    design values.
13. Drop and count packets while the channel is down.
14. Start the channel only after `PROVISION`. Start kage only on `READY` of
    port 5000 (T-046).

## Prove

```sh
cargo test -p box-guest net::
cargo xtask linux --kvm -- cargo test -p box-daemon --test net_live -- --ignored guest_curl
```

The unit tests run on any Linux host. They check that tun0 stays down before
`NET_READY`, the MTU bounds, and the drop counters of full queues.

The live test needs T-121 and a Linux KVM host. It runs `curl` in the guest
over an exec session on port 5001, to an allowed name. curl gets an answer,
and it gets one again after a daemon restart.

## Start condition

T-022.

# T-046: box-guest supervises kage and bridges port 5004

**Source:** The planning brief of 2026-10-14, sections 3 (R-DENY-FIRST) and 5; kage serve at f8df989.
**Category:** feature
**Milestone:** M7
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

kage is the harness. It must run as the agent user, start only after the egress
policy is live, restart after a crash, and stop cleanly. The host reaches kage
serve only through one vsock port.

## Premise

- Design: kage runs as `kage serve` on guest 127.0.0.1:7433 as uid 1000, with a
  host-provisioned token. box-guest bridges a vsock port to it.
- Design: KAGE_SHELL__PROGRAM and KAGE_SHELL__SCRUB_ENV are pinned in the
  environment. Nobody runs `kage trust` on an untrusted repository.
- Design: kage does not start before the policy and the bindings are live in
  the daemon (R-DENY-FIRST).
- READ: `kage serve` takes `--host` and `--port`, with 127.0.0.1 and 7433 as
  defaults
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/main.rs:246-253`).
- READ: serve keeps its token at `$XDG_DATA_HOME/kage/remote-token`
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/serve/mod.rs:89-92`).
- READ: serve writes the connect URL, token included, to connect.txt beside the
  token file
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/serve/mod.rs:220-235`).
- READ: kage reads a provider key from its environment variable first
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/providers.rs:275-285`).
- READ: serve listens on TCP only. It has no Unix or vsock listener for ACP
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/serve/mod.rs:138`).
- READ: the engine evaluation infers that a tool process at kage's uid can read
  the token and answer its own approval prompts.

## Approach

1. In crates/box-guest/src/kage.rs, start kage only after `PROVISION` and after
   `READY` on port 5000.
2. The daemon sends `READY` only when the policy and the bindings are live
   (T-126). There is no timer fallback.
3. Run `kage serve --host 127.0.0.1 --port 7433` from the path that T-063 pins.
   Never pass `--rotate-token`.
4. Run it as uid and gid 1000 with no supplementary groups, in a new session,
   in /home/agent.
5. Build the environment from nothing: HOME, USER, PATH, LANG and the pins of
   T-160.
6. Put each provider nonce from `PROVISION` in the key variable of its
   provider.
7. Set XDG_DATA_HOME of kage to the tmpfs directory /run/agent-box/kage/data,
   and XDG_CONFIG_HOME to /run/agent-box/kage/config (T-160).
8. Link kage's session directory to /home/agent/.local/share/kage/sessions on
   the data disk, so that sessions persist. docs/guest.md asks this entry to
   test that layout.
9. Write the kage token to remote-token in the tmpfs directory, mode 0600,
   owner agent.
10. The token and connect.txt then stay off the data disk.
11. Give PTY sessions of T-042 the same XDG_DATA_HOME, so that `kage resume`
    finds the sessions.
12. Poll TCP 127.0.0.1:7433 every 100 ms for up to 30 s, design values. Then
    reply `READY_ACK` and send `KAGE_STATE` listening on port 5002.
13. On a kage exit, send `KAGE_STATE` exited with the code or the signal.
14. Restart with backoff from 1 s to 60 s, design values. Reset the backoff
    after 10 minutes of uptime.
15. On poweroff, send SIGTERM to kage, wait 5 s, then send SIGKILL.
16. Never run another kage subcommand. Never run `kage trust`.
17. Listen on vsock port 5004 for the host CID only. Expect `BRIDGE_OPEN`
    first.
18. For each bridge connection, connect TCP 127.0.0.1:7433, send
    `BRIDGE_READY`, and copy the bytes in both directions.
19. Do not read or change the bridged bytes. The daemon authenticates with the
    token (T-161).
20. Accept at most 16 bridge connections, as many as kage serve accepts.

## Prove

```sh
cargo test -p box-guest kage::
cargo xtask linux --kvm -- cargo test -p box-daemon --test kage_live bridge_restart -- --ignored
```

The unit tests build the environment and the restart backoff with a fake
clock, on any Linux host. The live test opens port 5004 with the kage token,
and the WebSocket upgrade of kage serve succeeds. Without the token, kage
refuses it.

The test kills kage over an exec session. Port 5002 then shows `KAGE_STATE`
exited, then listening, and a new bridge connection works.

A scan of the raw data disk finds neither the kage token nor a provider nonce.
It needs a Linux KVM host. Windows 11 with Hyper-V, as admin, runs the same
test.

## Start condition

T-160.
