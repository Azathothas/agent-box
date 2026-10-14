# Architecture

This page is the technical reference of agent-box. When another document conflicts with this page, this page wins. The other document is then the defect: fix it in the same change.

NOTE: The product does not exist yet. The operator ruled on 2026-10-14 (D14 in [decisions.md](decisions.md)) that this session writes only the dev tooling, the scripts and the documents. The workspace has one member, `xtask`. Entry T-033 creates the product crates first in the implementation session. A statement below in the present tense is a design commitment, not a fact that a reader can run. A measured fact goes in [status.md](status.md), with its host, its date and its conditions.

The rulings behind this design are in [decisions.md](decisions.md). The milestones are in [roadmap.md](roadmap.md). The entries are in [TODO/INDEX.md](../TODO/INDEX.md).

## What agent-box is

agent-box is one Rust CLI, `agent-box`. It runs on Windows 11 x64 hosts and on Linux x86_64 and aarch64 hosts. It manages persistent Linux microVMs. This documentation calls each microVM an instance.

- The verbs are create, list, start, pause, resume, stop, delete and attach. [cli.md](cli.md) gives every verb.
- Each instance has a sparse data disk of about 30 GiB.
- Each instance runs a preconfigured Debian 13 slim userland. It contains kage (https://github.com/QaidVoid/kage) as the harness, plus git and language toolchains.
- Agents run unattended. The operator can attach a terminal to an instance at any time.
- Credentials stay on the host. The broker in the daemon injects them into upstream requests. The guest holds no credential, only per-instance nonces and tokens.
- Project files move only through a git forge, through the broker. The host and the guest share no filesystem.

The goals of the operator, in this order:

1. Maximum isolation and security.
2. Maximum performance.
3. Minimal footprint and overhead.

agent-box does not write a harness, because kage is the harness (D2). When kage lacks a feature, the operator asks the kage maintainer. An agent session never opens an issue or a pull request in another repository. It writes the request into [TODO/upstream.md](../TODO/upstream.md), and the operator sends it.

## Trust boundary

| Component | Trusted | Notes |
| --- | --- | --- |
| `agent-box` CLI | yes | Runs as the operator, unprivileged. |
| `agent-box daemon` | yes | Runs as the operator. Holds the credential store, the broker and the netstack. |
| `agent-box-helper` | yes | Runs as root on Linux and as LocalSystem on Windows. Has a fixed operation set. Never parses guest data. |
| The operator's configuration | yes | The operator writes it. It holds no credential. |
| The VMM process | partly | Parses data that the guest controls. The jailer confines it on Linux. On Windows, Hyper-V runs it in a per-instance worker, vmwp.exe. |
| Everything inside the guest | no | The agent, its tools, box-guest, the guest kernel, and every byte on the guest disks. |
| kage output, the ACP stream, transcripts | no | Any guest process at the uid of kage can forge them. |
| Forge content, provider response bodies | no | They come from outside, through the guest or the broker. |
| A chat message (late milestone) | no | Authority comes from the platform identity that the gateway checks, never from the message text. |

Treat the agent as hostile. Prompt injection, adversarial tool output or a compromised dependency can make it hostile, with no malice at the start. kage adds no isolation: its shell tool runs with no sandbox, and its tool permissions allow by default. That fact is READ in [research/engines-2026-10.md](research/engines-2026-10.md). The instance and the host egress policy are the whole boundary.

NOTE: The event channel of box-guest resists forgery by the agent at uid 1000. It does not resist a guest that gained root. So billing, backoff and slot accounting use broker data only. The event channel carries lifecycle facts.

The uid split in the guest: box-guest runs as root, and kage and its tools run as uid 1000 (user `agent`). The image has no sudo and no setuid or setgid file.

### The four rules

Each rule is a P0 or P1 requirement. A change that breaks one is a defect, whatever else it fixes.

R-HOST-NO-TOOLS (T-030, P0):

- The host never runs a tool against bytes that the guest wrote. Tools include git, a package manager, a linter and kage.
- The host never interprets a path, a gitfile, an alternates file, a symlink or a repository config that came from the guest.
- Project files move only through the forge (D4), so the host holds no project bytes.
- `cargo xtask check host-tools` refuses `std::process::Command` in box-daemon, box-net and box-broker outside an allowlist of named call sites.
- Evidence (READ): the errand daemon accepts `.git` as a file, so host-side git follows a `gitdir:` line that the agent wrote (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/pr.ts:177`).
- Evidence (READ): the errand daemon read a notes file through a symlink that the agent could plant (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/session.ts:1356`).

R-TRUSTED-CHANNEL (T-031, P0):

- Data for authorization, billing, backoff or lifecycle comes from two sources only: the broker, and the event channel of box-guest.
- box-guest runs as root and kage runs as uid 1000. Port 5002 refuses a frame without the event nonce, before it parses the frame.
- The kage ACP stream is agent-reported data. Each consumer labels it so. It never feeds authorization, billing or backoff.
- Broker accounting (T-147) is the only source for billing and backoff (T-252). Event logs keep the agent-reported and host-observed labels (T-176).

R-NO-SECRET-IN-GUEST (T-256, P0):

- No credential enters the guest image, the guest disks, the kernel command line, MMDS or the VMM argv. The guest reads its command line in /proc/cmdline.
- Nonces, the egress token and the kage serve token go to box-guest over port 5000 only. They go after boot and before kage starts.
- box-guest holds them in memory. The kage serve token file is on a tmpfs under /run, never on the data disk.
- In box-proto, a provisioned value has a type that prints as redacted and has no serialization (T-256).
- box-vmm never depends on box-proto, so no engine builder can take such a value. T-256 adds this rule to the layering check.
- The checks are the disk scan of T-041 and the image scan of T-062. The builder tests of T-084 and T-101 and the live scans of T-088 and T-105 add to them.
- The live scans read /proc/cmdline and the VMM argv or the HCS document. The battery repeats them from outside (T-180).
- MCP OAuth tokens are a second credential path into the guest. T-162 refuses them or brokers them.
- An SSH key is a credential too. T-257 keeps each key in the daemon and gives the guest an SSH agent socket only.

R-DENY-FIRST (T-126, P0):

- An instance has no egress until its policy and bindings are live in the daemon.
- The daemon reads no packet of an instance before that point. Only the step that installs the policy can open the network channel for packets (T-126).
- box-guest starts kage only after the daemon reports ready (T-046).

## Processes

### Host processes

| Process | Crate | Runs as | Owns | Entries |
| --- | --- | --- | --- | --- |
| `agent-box` | box-cli | the operator | The verbs, exit codes and `--json`. It starts the daemon when the daemon is absent. | T-029 |
| `agent-box daemon` | box-cli, with the code in box-daemon | the operator | The Engine implementation, instance records, the netstack, the broker, the credential store, the control API, re-adoption and events. | T-028 |
| `agent-box-helper` | box-helper | root on Linux, LocalSystem on Windows | The fixed operations that need privilege. | T-081, T-100 |
| The VMM | none (upstream) | a per-instance uid (Linux), a per-instance virtual account (Windows) | Device emulation for one instance: Firecracker under the jailer, or vmwp.exe. | T-086, T-105 |

The daemon:

- The control API is JSON-RPC 2.0. On Linux it listens on a Unix socket in $XDG_RUNTIME_DIR. On Windows it listens on a named pipe with a DACL for the operator.
- The daemon holds one lock for the life of its process: a flock on Linux, a named mutex on Windows. It never uses a PID file.
- `agent-box setup` can register the daemon as a user service: systemd --user with lingering on Linux, a logon task on Windows.
- The CLI is synchronous, except for its daemon client. The daemon, the broker, the netstack and box-guest use one async runtime, tokio (P1).
- On Windows, a process that agent-box starts runs in a Job Object with kill-on-close, not in a process group.

The helper:

- The helper protocol types are in the `helper` module of box-proto (T-023). The daemon and the helper share that one definition.
- The operations are create, start, pause, resume, stop, kill, delete, inventory and probe.
- The helper checks each input before any syscall that uses a request value. An instance id matches `[0-9a-f]{32}`. A uid is in the allocator range.
- The helper derives every path itself. The types cannot express a path, a device or an engine document from the caller.
- Linux: systemd starts the helper on demand from the socket /run/agent-box/helper.sock (group agent-box, mode 0660). It checks the peer with SO_PEERCRED and exits when idle.
- Windows: the helper is a LocalSystem service. Its named pipe has a DACL for the installing user, and it checks the client SID.
- Windows: the helper builds every HCS JSON document itself. It grants VM access only to files inside the agent-box store.

WARNING: The helper is the privilege boundary of the host. A Plan9, VirtualSmb or NetworkAdapters entry from the caller, or an access grant on any path, makes it a privilege-escalation tool.

State on the host (design; T-026 and T-174 set the paths per OS):

- The state directory of the daemon belongs to the operator. It holds the instance records, the encrypted per-instance secrets and the event logs.
- The agent-box store belongs to the helper. It holds the images, the disks, the jails and the snapshots.
- A record has a version. A write goes to a temporary file, then a rename in the same directory. An unknown version is an error.

### The guest

- box-guest is PID 1, a static musl Rust binary that runs as root. Its entries are T-040 to T-046.
- box-guest mounts the file systems, builds the overlay root, reaps zombies, handles signals and ends the instance at stop.
- box-guest serves the control port, the PTY and exec sessions, the event channel, the tun0 device and the network channel.
- After T-257, box-guest also serves an SSH agent socket to uid 1000 and bridges it to port 5005.
- box-guest syncs the time, supervises kage and runs fstrim each week. It never writes a secret to disk.
- kage runs as `kage serve` on guest 127.0.0.1:7433, as uid 1000 (user `agent`), with a token that the host provisions. box-guest bridges port 5004 to it.
- The environment of kage pins KAGE_SHELL__PROGRAM and KAGE_SHELL__SCRUB_ENV. Today an untrusted project .kage/config.toml can set the shell table.
- The approval mode of kage is allow. T-160 records the reason: the instance and the host egress policy are the boundary.
- Nobody runs `kage trust` on an untrusted repository.
- One kernel config per architecture carries the virtio and the Hyper-V drivers (T-060). T-061 proves vsock under each engine.
- On x86_64, one build gives `vmlinux` for Firecracker and `bzImage` for HCS. On aarch64 it gives `Image`. Firecracker recommends `vmlinux`, because a bzImage costs boot time and memory.

[guest.md](guest.md) gives the kernel, the image, box-guest and the kage configuration.

### Diagram

```text
                        upstream hosts: providers, forges (TLS)
                                    ^
host                                | only the daemon connects
+-----------------------------------|--------------------------------+
| agent-box (CLI)                   |     operator, unprivileged     |
|     | JSON-RPC 2.0                |                                |
|     v                             |                                |
| agent-box daemon  ----------------+     operator                   |
|     credential store, broker, netstack, Engine, records            |
|     |                         ^                                    |
|     | fixed operations        | vsock ports 5000 to 5005           |
|     v                         |                                    |
| agent-box-helper              |          root or LocalSystem       |
|     | jailer, or HCS          |                                    |
|     v                         |                                    |
| VMM: Firecracker in a jail,   |                                    |
|      or vmwp.exe              |                                    |
+-------------------------------|------------------------------------+
                                | virtio-vsock (Linux) or hv_sock (Windows)
+-------------------------------v------------------------------------+
| guest (UNTRUSTED): Debian 13 slim                                  |
|     box-guest   PID 1, root: control, PTY, events, tun0, time      |
|     kage serve  uid 1000 on 127.0.0.1:7433, bridged on port 5004   |
|     disks: base (read-only, shared by digest), data (sparse)       |
+--------------------------------------------------------------------+
```

Only the daemon opens connections to upstream hosts. The guest reaches the network only through the netstack and the broker in the daemon.

## Crate graph

NOTE: No product crate exists yet. T-033 creates exactly the crates and the edges of this table, and nothing else. An entry that adds an edge later changes this table in the same change.

| Layer | Crate | Kind | Owns | Depends on |
| --- | --- | --- | --- | --- |
| 0 | box-core | library | InstanceId, instance names, errors, the State enum, InstanceKind, specs and cancellation. No IO. | none |
| 1 | box-proto | library, host and guest (musl) | Framing, ports, ceilings, v1 messages, versions, and the `helper` module. No IO. | box-core |
| 1 | box-policy | library | Egress policy and binding declarations as data, their validation, and TOML. No secret. | box-core |
| 1 | box-image | library | Image pins, digest verification and the cache layout. | box-core |
| 1 | box-transport | library | Host-side vsock: hybrid vsock over Unix sockets on Linux, AF_HYPERV on Windows. | box-core |
| 2 | box-vmm | library, for the helper only | The firecracker module (jailer, config builder, API client, inventory) and the hcs module (schema, computecore, virtdisk). Each module is gated by target OS. | box-core |
| 2 | box-net | library | The userspace netstack, DNS, flow policy enforcement and forwards. | box-core, box-proto, box-policy |
| 2 | box-broker | library | Endpoints, bindings, the injection gate, the credential store, upstream TLS and accounting. | box-core, box-policy |
| 3 | box-daemon | library | The supervisor, the Engine implementation over the helper, the config, the control API, re-adoption and events. | box-core, box-proto, box-policy, box-image, box-transport, box-net, box-broker |
| 4 | box-chat, box-web, box-pool | library, late | The late surfaces. Their own entries create them. | at most box-core, box-policy and box-daemon |
| 5 | box-cli | binary `agent-box` | The verbs and the `daemon` subcommand. It is the only crate that knows every part. | box-core, box-proto, box-policy, box-daemon; the late crates once they exist |
| 5 | box-helper | binary `agent-box-helper` | The privileged helper. | box-core, box-proto, box-vmm |
| 5 | box-guest | binary `box-guest`, static musl | The guest agent, PID 1. | box-core, box-proto |
| tools | xtask | binary, not shipped | The dev tooling. | none |

Each crate edge is a path dependency. An InstanceId is 128 random bits, written as 32 lowercase hex digits.

`cargo xtask layering` (T-002, `xtask/src/layering.rs`) reads the graph from `cargo metadata`. The file holds a machine copy of the layer table, and the two tables agree. The check enforces these rules:

1. A normal or build dependency points to a crate of a strictly lower layer. An edge inside one layer fails.
2. No crate depends on a binary crate: box-cli, box-helper, box-guest or xtask. This includes dev-dependencies.
3. A dev-dependency can point to any library crate, because it builds tests only. Rules 1 and 4 to 7 do not apply to it.
4. box-guest depends only on box-core and box-proto.
5. box-helper depends only on box-core, box-proto and box-vmm.
6. Only box-helper depends on box-vmm. The daemon reaches the engines through the helper process.
7. box-chat, box-web and box-pool never depend on box-broker or box-vmm. xtask depends on no workspace crate.
8. A workspace member that has no layer in the table fails the check.

Review, not the check, holds these rules:

- box-core and box-proto do no IO. box-proto builds for the musl guest target.
- box-policy holds no secret value. A binding names a credential. It never carries the value.

Unsafe code (design; T-033 applies it):

- The workspace lint table in `Cargo.toml` sets `unsafe_code = "deny"`, not forbid. So one named module can allow the lint.
- Two modules need foreign function calls: the AF_HYPERV module of box-transport (T-025) and the hcs module of box-vmm (T-102, T-103).
- Each of them carries `#![allow(unsafe_code)]` and names its entry. No other module allows the lint.
- T-100 proposes a third module for the service, pipe and token calls of the Windows helper. It records that exception in [decisions.md](decisions.md).
- Each unsafe block has a comment that states why it is sound. Every crate keeps the workspace lints.

## The Engine trait

One narrow trait has two production implementations. Everything else is shared: the guest image, box-guest, the wire protocol, the netstack, the broker and kage.

```rust
pub trait Engine {
    fn probe(&self) -> Result<CapabilityReport, Unavailable>;        // attempts the real setup
    fn create(&self, spec: &InstanceSpec) -> Result<(), EngineError>; // disks, uid, records
    fn start(&self, id: &InstanceId) -> Result<(), EngineError>;      // cold boot
    fn pause(&self, id: &InstanceId) -> Result<(), EngineError>;      // in-memory vCPU freeze
    fn resume(&self, id: &InstanceId) -> Result<(), EngineError>;
    fn stop(&self, id: &InstanceId, grace: Duration) -> Result<(), EngineError>; // guest poweroff, then kill
    fn delete(&self, id: &InstanceId) -> Result<Removal, EngineError>; // verified absence
    fn inventory(&self) -> Result<Vec<Observed>, EngineError>;        // includes State::Unknown
    fn connect(&self, id: &InstanceId, port: u32) -> Result<Stream, EngineError>;  // host-initiated vsock
    fn listen(&self, id: &InstanceId, port: u32) -> Result<Listener, EngineError>; // guest-initiated vsock
}
```

- The trait lives in crates/box-daemon/src/engine.rs (T-032). The unprivileged daemon calls it.
- An implementation sends a fixed helper operation for each step that needs privilege. It uses box-transport directly for vsock data.
- The implementations are Firecracker (Linux), HCS (Windows), and a test double. The double runs box-guest as a local process for M1 tests.
- `probe` attempts the real setup. It never infers support from a sysctl, a feature list or a service that answers queries.
- A CapabilityReport carries gaps and notes. A gap stops the daemon. A note goes into the startup report.
- There is no fallback between engines, and there is no unconfined mode.
- `delete` returns a Removal: each item that it removed, with the check that proved its absence.
- `inventory` returns what the engine observes. A failed query gives `State::Unknown`, never a guess.

The reason for the gap rule: a weaker boundary that looks like a stronger one is worse than the weaker boundary. It takes away the chance of the operator to decide.

Open question for T-032: the trait above is synchronous, and P1 makes the daemon async. T-032 decides between two options. The daemon calls the trait on the blocking pool of tokio, or the trait becomes async. The Stream and Listener types come from box-transport.

### Linux: Firecracker v1.17.0 and the jailer

[engines.md](engines.md) gives the full design. The summary:

- The helper runs the jailer, because the jailer needs root capabilities. The jailer puts Firecracker in a chroot with mount and PID namespaces.
- Firecracker runs under a stable per-instance uid and gid, in a per-instance cgroup v2 under a delegated root. The jailer argument `--cgroup-version 2` is explicit.
- Firecracker applies its per-thread seccomp filters.
- The jail is rebuilt on each boot. The disks are hard-linked into the jail, so they must be on the same filesystem.
- The VM configuration has no `network-interfaces` key. The config builder refuses a key outside its schema, which comes from the pinned v1.17.0 specification.
- The host channels use hybrid vsock over Unix sockets. File permissions control who can open them.
- Firecracker validates the host kernels 5.10, 6.1 and 6.18. The probe reports another kernel as a note (T-092).
- aarch64 support is limited to hosts with 4K pages until a boot on a 16K-page host is measured (T-094).
- One VMM thread emulates block and vsock. Guest disk load can delay every host channel. T-015 measures this in M0.
- If measured block I/O dominates the agent workload, Cloud Hypervisor with a jail that agent-box builds is the fallback. That change needs its own decision (T-093).

### Windows: native Hyper-V through HCS

[engines.md](engines.md) gives the full design. The summary:

- Each running instance is one HCS compute system with schema 2.3 or later. The schema reference puts AlwaysAllowSparseFiles in 2.6, and T-101 tests it in a 2.3 document.
- The boot mode is LinuxKernelDirect on x64. The engine adds `initcall_blacklist=virtio_vsock_init` to the kernel command line, as hcsshim does.
- The devices are the read-only base VHDX, one dynamic data VHDX on SCSI, and HvSocket. The HvSocket SDDL grants only SYSTEM and the SID of the operator.
- A compute system has no NetworkAdapters, no Plan9 and no VirtualSmb.
- The memory settings are AllowOvercommit, EnableDeferredCommit and EnableColdDiscardHint, as in WSL2.
- The helper sets Owner and sets ShouldTerminateOnLastHandleClosed to false. It re-opens or enumerates compute systems by id and owner.
- The daemon binds one AF_HYPERV listener per VmId, never a wildcard. agent-box never sets AllowWildcardBinds.
- Every host socket sets HVSOCKET_CONNECTED_SUSPEND, because the default disconnects the socket on a pause.
- WSL2 and hcsshim never use a compute system that outlives its creator. They never pause, save or restore one.
- So four features have no Linux-guest reference user. They are survival after the helper exits, pause, save and restore, and a VHDX that shrinks after a guest trim. The M0 spike T-014 tests each one.
- The fallbacks, in order, are Hyper-V VMMS through WMI v2, then OpenVMM on WHP (T-111).

WARNING: Isolation is not the same on both OSes. Under HCS, the guest reaches root-partition kernel code: the VMBus root side, storvsp, vhdmp and the hvsocket provider. It also reaches the per-instance vmwp.exe worker. Under Firecracker, the guest reaches KVM in the host kernel, and virtio-blk and vsock run in the jailed user-mode VMM.

The rejected engines and the reasons are in [research/engines-2026-10.md](research/engines-2026-10.md).

## Instance model

### Identity and kinds

An instance has an InstanceId and a name. The name follows the DNS label rules (T-020). The CLI takes the name, and the engines use the id.

| Kind | Meaning |
| --- | --- |
| `persistent` | The operator creates it and deletes it. Its record and its disks survive host reboots. |
| `ephemeral` | agent-box creates it for one task and deletes it at the end of the task. The chat gateway uses this kind. |

### States

| State | Meaning |
| --- | --- |
| `creating` | The engine makes the disks, the uid and the records. |
| `stopped` | No VMM runs. The disks are kept. |
| `starting` | Cold boot, until box-guest completes the handshake on port 5000. |
| `running` | The VMM runs, and box-guest completed the handshake. |
| `paused` | The vCPUs are frozen in memory. |
| `stopping` | box-guest ends the guest. A kill follows when the grace time ends. |
| `deleting` | The engine removes each item and verifies its absence. |
| `unknown` | A query to the VMM or to HCS failed. agent-box never assumes that the instance runs. |

A state is an enum in every crate, never a boolean. A paused instance whose VMM or host went away shows `stopped`, because the helper verified that no VMM runs. A failed query shows `unknown`.

### Verbs and state changes

This table is design. T-170 tests each row on both engines, and [cli.md](cli.md) gives the full transition table.

| Verb | Accepted in | Path |
| --- | --- | --- |
| create | no instance of that name | `creating` -> `stopped` |
| start | `stopped` | `starting` -> `running` |
| pause | `running` | `paused` |
| resume | `paused` | `running` |
| stop | `starting`, `running`, `paused`, `unknown` | `stopping` -> `stopped` |
| delete | `stopped`; with `--force` also `running`, `paused` and `unknown` | `deleting` -> no instance |
| attach, run | `running` | no change |

- A verb in another state is refused. The refusal names the current state.
- A repeated verb, for example a second stop, changes nothing and succeeds. [cli.md](cli.md) gives the rule for each verb.
- Stop on a paused instance resumes it first, because a guest poweroff needs running vCPUs.
- Delete refuses a running or paused instance. The operator stops it first, or `--force` stops it. The ephemeral flow stops the instance and then deletes it.
- In `unknown`, only stop and delete are accepted, and delete needs `--force`.

### Disks

- The base image is read-only. Instances share it by digest. No image is ever made writable after its build.
- The base format is erofs or ext4, and T-065 decides. Windows uses a VHDX copy of the base image.
- Each instance has one sparse data disk. The default size is 30 GiB, and `agent-box create --disk` changes it.
- The data disk is a raw file on Linux and a dynamic VHDX on Windows.
- box-guest formats the data disk on first boot, inside the guest. It mounts an overlay root with the base as the lower layer.
- The upper and work directories of the overlay are on the data disk. box-guest pivots into the overlay root.
- /home, /var and the work of the agent are on the data disk. A file written in the guest survives stop and start.
- The host never parses the data disk. After the host creates the file, only the VMM opens it (T-087).
- Linux disk settings: `io_engine` is Sync, `cache_type` is Writeback, and discard is on. box-guest runs fstrim each week.
- The data disk never uses the Async engine. It is a developer preview, it has no discard, and its io_uring workers escape the cgroup on 5.10.
- The data disk never uses the Unsafe cache type. It drops flushes, and a host power loss can then damage the guest journal.
- Firecracker takes no lock on a disk file. The jailer closes inherited descriptors, so no agent-box process can hold a lock on the disk for the life of the VMM.
- So two locks stop a double start. The daemon holds a lock per instance (T-026), and the helper takes the instance lock for each operation (T-087).
- The helper also refuses `start` while a verified VMM of that instance runs.
- Jail teardown unlinks the hard links. It never deletes the data disk.

### Pause, stop and hibernate

| Verb | Meaning |
| --- | --- |
| start | Always a cold boot from the disks. An HCS compute system is ephemeral, so start creates a new one from the record. |
| pause | An in-memory vCPU freeze, not a snapshot. Guest RAM stays resident, and the vsock connections stay open. |
| resume | The vCPUs run again. box-guest gets the host time over the control port. |
| stop | A poweroff request through port 5000, then a kill after the grace time. On x86 Firecracker, SendCtrlAltDel is a second way to stop. |
| hibernate | A later opt-in verb, P3 (T-184). It writes the guest state to disk. It is not the meaning of pause. |
| delete | Removes every item of the instance and verifies the absence of each one. |

- A pause lives in the VMM. If the VMM, the helper or the host stops, the pause is lost and the next start is a cold boot.
- box-guest ends the guest with a reset, not a power-off call. Firecracker exits on a guest reset, and `StopOnReset` ends an HCS compute system. T-088 and T-105 measure both.
- Firecracker v1.17.0 keeps vsock connections across a pause. HCS keeps them only with HVSOCKET_CONNECTED_SUSPEND, which T-014 tests.
- A Firecracker hibernate uses a Full snapshot. It is single use, and a cold boot of the disk invalidates it.
- Before a restore, a manifest checks the Firecracker version, the host kernel and the CPU. A mismatch falls back to a cold boot.
- Snapshot files outside the jail are root-owned, mode 0600, with a digest that is verified before load.
- HCS save to file is the Windows hibernate only if T-014 passes. Guest S4 hibernation is an engine-neutral option to measure.
- Delete on Linux removes the jail, the cgroup, the disks, the snapshots, the pid file and the uid.
- Delete on Windows removes the compute system, the VHDX files, the saved state and the access grants.

### Re-adoption and orphans

- A daemon restart does not stop the instances, because they are persistent.
- The restarted daemon re-reads its encrypted per-instance secrets and asks the helper for the inventory. It verifies each VMM, then reconnects with re-keyed channels.
- Linux check: /proc/PID/root resolves to the jail, or /proc/PID/exe is the copy in the jail.
- Windows check: the helper enumerates compute systems by Owner and re-opens each one by id.
- A VMM that no instance record claims is an orphan. The helper removes it only after the verification.
- The entries are T-090 (Linux) and T-107 (Windows).

## Host and guest channels

[protocol.md](protocol.md) gives the messages, the versions and the golden frames. The v1 ports are fixed, and an unknown port is refused. Port 5005 is in the v1 set, but nobody listens on it until T-257 lands.

| Port | Direction | Purpose |
| --- | --- | --- |
| 5000 | host to guest | Control: version handshake, provisioning, ready, time, rotation, stats, poweroff and fstrim. Provisioning carries the nonces, the egress token, the kage token, the time and the hostname. Every exec runs on port 5001. |
| 5001 | host to guest | PTY and exec streams, for attach and run. |
| 5002 | guest to host | Events and heartbeat. The first frame carries the event nonce. |
| 5003 | guest to host | Network frames. The first frame carries the egress token. |
| 5004 | host to guest | The kage serve bridge, to guest 127.0.0.1:7433. |
| 5005 | guest to host | The SSH agent of T-257. The first frame carries a token, as on port 5003. |

- A frame is a u32 big-endian length, a u8 type, and the payload.
- The ceilings are MAX_HEAD 8 KiB for heads and MAX_FRAME 8 MiB. The receiver compares the announced length with the ceiling before it allocates, and closes the connection on a larger length.
- A truncated frame is an error. A version mismatch refuses the session, with no downgrade.
- box-guest sends a heartbeat only after 30 s with no frame, never on a timer. A flood of frames is not proof of liveness.
- The event nonce and the egress token are 32 bytes each from the host CSPRNG. The host rotates them.
- On Firecracker, the transport is hybrid vsock: the host writes `CONNECT <port>\n` and reads `OK <port>\n`. Guest-initiated connections arrive on `uds_path_<port>`.
- On HCS, the transport is AF_HYPERV. The VmId is the runtime id of the compute system.
- The ServiceId on HCS is the port as 8 hex digits, followed by `-facb-11e6-bd58-64006a7986d3`.

Two more channels stay on the host. The CLI talks to the daemon through the control API. The daemon talks to the helper through the helper protocol.

## Network

[network.md](network.md) gives the full design. The guest has a virtual NIC (D5), and the daemon carries its packets. Entries T-045 and T-120 to T-127 build it.

- box-guest creates `tun0`. The address plan is guest 10.200.0.2/24, gateway 10.200.0.1, DNS 10.200.0.1. T-045 and T-121 fix the exact values.
- The MTU of `tun0` fits the vsock framing. IP packets travel as frames on port 5003.
- The daemon runs one userspace TCP/IP stack per instance, in box-net. Instances share no netstack state.
- T-120 chooses the stack by measurement. The candidates are smoltcp, ipstack and netstack-smoltcp.
- The DNS server on the gateway answers only names that the policy allows, and resolves them on the host.
- It records the instance, the name, the address and the expiry of each answer. It answers NXDOMAIN for any other name.
- It refuses HTTPS and SVCB records, because they carry ECH configuration. It logs every query.
- The policy has three modes. `deny` allows the broker endpoints only. `open` allows any public destination.
- `allowlist` is the default. It allows the broker endpoints and the listed name and port rules.
- In every mode, a new flow must go to an address that the netstack answered to that instance for an allowed name.
- Reserved ranges are refused after resolution and before every allow, unless the operator writes an explicit address rule. No rule opens loopback or a metadata address.
- The reserved ranges are loopback, link-local (169.254.169.254 included), RFC 1918, CGNAT 100.64.0.0/10, the addresses of the host, and multicast.
- Each flow carries the policy generation. A revoked TCP flow gets a RST. DNS expiry stops new flows only.
- The flow idle timeout is above 600 s, because kage allows 600 s of silence in a provider call.
- `agent-box forward NAME [ADDR:]HOSTPORT:GUESTPORT` makes the daemon listen on the host and open a TCP flow into the guest.
- A forward binds 127.0.0.1 by default. A public bind needs an explicit flag, and the startup report names it.
- The guest view of 127.0.0.0/8 never maps to the host loopback. The bailey and pasta history taught this lesson.
- Later, the packet parser that faces the guest runs in a confined process, and fuzz targets test it (T-127).
- ssh from the guest is a TCP flow under the policy. The guest holds no SSH key, so until T-257 lands, ssh reaches only hosts that need no credential.

## Broker

[broker.md](broker.md) gives the full design. The broker runs in the daemon. Entries T-140 to T-150 build it.

- The guest reaches the broker at `http://<binding>.broker.internal/`. The netstack DNS resolves every broker name to the gateway, and port 80 there is the broker.
- Each binding fixes the upstream scheme, host and port. The guest never names the upstream host.
- The broker opens TLS to the upstream itself with rustls, and verifies the certificate for the fixed host. It uses HTTP/1.1 to the guest.
- kage points each provider `base_url` at a broker endpoint. The key variable holds a per-instance nonce in the key format of the provider.
- The broker replaces exactly one auth header of the kind of the provider. A second auth header makes Anthropic refuse the request.
- The system git config of the guest rewrites `https://github.com/` to the broker endpoint with `url.insteadOf`.
- The forge binding limits paths to the allowed repositories. It refuses a pushed ref outside the namespace of the instance.
- The injection gate has eight conditions. If one fails, the broker refuses the request. It never forwards a request without the credential.
- The broker never follows a redirect with a credential attached.
- A small buffered request that fails closed gets a local 503. A streaming request that fails closed is terminated.
- The credential store is encrypted at rest: DPAPI on Windows, and a 0600 key file or the Secret Service on Linux (T-140 decides).
- A credential enters the store only through the stdin of `agent-box secret set`. No log contains a value.
- The broker counts requests and tokens per instance from provider responses. That count is the only trusted source for billing, backoff and slots.
- TLS interception with a planted CA is a later P3 option (T-149). It stays off until kage can take a CA.
- T-257 adds an SSH agent in the daemon. It signs only with the keys of the instance, and only for a flow to an allowed host.

## Configuration and profiles

[cli.md](cli.md) gives the keys, the profiles and the exit codes. T-027 builds the loader.

| OS | Search order |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/agent-box/config.toml`, then `~/.config/agent-box/config.toml`, then `/etc/agent-box/config.toml` |
| Windows | `%APPDATA%\agent-box\config.toml`, then `%ProgramData%\agent-box\config.toml` |

- The daemon reads the first file that it finds, and only that file.
- The environment variable `AGENT_BOX_CONFIG` names a file outright and skips the search (T-027).
- There is no config file in the working directory. A config that a cloned project carries would make agent-box a confused deputy.
- An unknown key is a refusal, not a warning. An operator who wrote a misspelled key believes that the setting is in force.
- No credential field exists. The config names a credential, and the credential store holds its value.
- A setting that no code reads does not ship.
- A profile is a named template in the config. It sets the CPUs, the memory, the disk size, the image variant, the egress policy and the bindings.
- `agent-box create NAME --profile P` uses a profile. A flag on the command line wins over the profile.

The startup report comes from `agent-box doctor` and from each daemon start (T-175). It contains:

- the engine name and version, and the jail or partition facts;
- the helper availability;
- the guest kernel and image digests, and the disk settings;
- the egress topology as absences: no NIC device in the VM configuration, no shared filesystem, no host path in the guest;
- the place where each credential lives, and the count of refused forgeries;
- every gap, by name.

## The late surfaces

The chat gateway, the web UI and the provider pool stay in the plan as late milestones (D8).

| Surface | Milestone | Crate | What it does | Entries |
| --- | --- | --- | --- | --- |
| Chat gateway | M10 | box-chat | Discord: one channel, a thread per session. A message starts an ephemeral instance, the agent works, and the instance is deleted. | T-220 to T-226 |
| Web UI | M11 | box-web | Login through the identity of the chat service. A user watches and drives sessions. | T-240 to T-242 |
| Provider pool | M12 | box-pool | Provider slots (one API key each), an admission queue, backoff, and usage and cost views. | T-250 to T-255 |

The rules:

- A surface never reaches the broker or the credential store directly. It asks the daemon. Rule 7 of the layering check holds the crate edge.
- The reason: a surface that can read the credential store can leak it.
- The chat token never enters an instance.
- Authority comes from the platform identity that the gateway checks, never from message text (T-221).
- Chat users are mutually untrusted. Each session has its own instance, bindings and nonces. No writable cache is shared.
- A logged-in web user drives exactly the sessions that they started. The operator role is configuration, never a chat-service role.
- A login makes the web UI a second trusted surface. Every call needs per-user authorization, Origin and Host checks, and CSRF protection.
- The web UI never allows credentialed CORS (T-240).
- The daemon composes what it publishes. The guest pushes to a fork through the broker, and the daemon opens the pull request (T-223).
- Usage, cost and backoff come from broker accounting, never from the event stream of the agent (T-252).
- A session never learns the number of slots, or what another session holds. Each of these facts helps an attacker.

## What is deliberately not here

| Item | Reason |
| --- | --- |
| A hosted control plane | The operator holds their own credentials. agent-box exists for that requirement. |
| Warm pools, claims, cluster orchestration | They serve a scale that this project does not have. |
| An agent loop of agent-box | kage is the harness (D2). agent-box drives it over ACP. |
| A filesystem shared by host and guest | Project files move through the forge only (D4). |
| TLS interception by default | kage trusts only its compiled-in roots (P4). T-149 keeps a later option. |
| A fallback between engines, or an unconfined mode | A gap stops the daemon. |
| Snapshots as the meaning of pause | Pause is an in-memory freeze. Hibernate is a later opt-in verb (T-184). |
| A container runtime in the guest | T-185 measures it first. A positive result is a pins change, not a setting. |
| A plugin surface of agent-box | kage has plugins. |
| A host path inside the guest | A host path shows the name of the operator and the shape of the host. |
| A privileged CLI | Only the helper has privilege (D3). |

## Limits

Read [limits.md](limits.md). The most important limit is also here, so that a reader of this page does not believe something false.

WARNING: Brokering stops credential theft. It does not stop exfiltration through an authorized channel. An allowed destination, a provider call or a push to an allowed ref can carry data out.

The second limit is the isolation difference between the two OSes, in the [Windows engine section](#windows-native-hyper-v-through-hcs).
