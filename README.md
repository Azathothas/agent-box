# agent-box

One Rust CLI that runs coding agents in persistent Linux microVMs on Windows
and Linux hosts, with every credential held on the host.

WARNING: agent-box has no product code yet. The repository holds the plan,
the record of work in `TODO/`, and the development tooling in `xtask/`.
Nothing that this page describes runs today. The milestones and their exit
criteria are in [docs/roadmap.md](docs/roadmap.md). The measured state is in
[docs/status.md](docs/status.md).

## How it works

```
host: Windows 11 x64, or Linux x64 or aarch64 (trusted)
+----------------------------------------------------------------------+
| agent-box (the CLI, unprivileged)                                    |
|     |  JSON-RPC 2.0 over a Unix socket or a named pipe               |
|     v                                                                |
| agent-box daemon (unprivileged)                                      |
|     credential store    broker      netstack (DNS, flow policy)      |
|     |                                     ^                          |
|     |  fixed operations                   |  vsock (hvsocket on      |
|     v                                     |  Windows): control,      |
| agent-box-helper (root or LocalSystem)    |  PTY, events, network    |
|     |                                     |  frames, kage bridge,    |
|     v                                     |  SSH agent (planned)     |
| engine: Firecracker + jailer (Linux)      |                          |
|         Hyper-V through HCS (Windows)     |                          |
+-----|-------------------------------------|--------------------------+
      |  boots and confines                 |
+-----v-------------------------------------|--------------------------+
| instance: one microVM (untrusted)         |                          |
|     box-guest (PID 1) --------------------+                          |
|     kage serve (uid 1000), git, toolchains                           |
|     nonces and tokens only: no credential, no shared filesystem      |
+----------------------------------------------------------------------+
```

- The CLI, `agent-box`, runs as the operator without privilege. It starts
  the daemon when no daemon runs.
- The daemon holds the credential store, the broker and one netstack for
  each instance. A restart of the daemon does not stop a running instance:
  the daemon finds it again and reconnects.
- The helper, `agent-box-helper`, is the only privileged process. It
  accepts a fixed set of operations, validates each input, and never parses
  guest data.
- The engine runs each instance as a microVM. The guest is Debian 13 slim,
  with `box-guest` as PID 1 and [kage](https://github.com/QaidVoid/kage) as
  the harness, plus git and language toolchains.
- The guest has a virtual NIC, `tun0`. Its packets go over vsock to the
  netstack. The netstack answers DNS and applies the egress policy to each
  flow.
- kage calls each provider through a broker endpoint on the guest gateway,
  with a nonce in place of the key. The broker replaces the nonce with the
  credential and opens TLS to the provider itself.
- Project files move only through a git forge, through the broker. The host
  and the guest share no filesystem.

## The planned CLI

```
agent-box setup --system              # once per host, as admin or root: the helper and its service
agent-box setup                       # once per user: the daemon service and the default image
agent-box doctor                      # what this host can run, and each gap by name
agent-box secret set NAME             # reads the credential from stdin, never from argv
agent-box create work --profile rust  # an instance with a sparse 30 GiB data disk by default
agent-box start work
agent-box run work --prompt "fix the failing test"   # kage works unattended
agent-box attach work --shell         # a terminal in the instance; without --shell, the kage TUI
agent-box forward work 8080:3000      # host 127.0.0.1:8080 to a dev server on guest port 3000
agent-box pause work                  # freeze the vCPUs in memory; resume continues
agent-box stop work                   # guest poweroff, then a kill after a timeout
agent-box delete work                 # remove everything, then verify each absence
```

Each verb takes `--json`. [docs/cli.md](docs/cli.md) gives every verb, the
instance states, the exit codes and the config file.

## Engines

The daemon calls one narrow engine trait with two implementations. The
guest image, `box-guest`, the wire protocol, the netstack, the broker and
kage are the same on both OSes.

### Linux: Firecracker and its jailer

agent-box runs Firecracker v1.17.0 under its jailer. The jailer puts each
VMM in a chroot with mount and PID namespaces, a per-instance uid and gid,
and a per-instance cgroup v2. Firecracker adds a seccomp filter for each of
its threads. Of the Linux VMMs with a jail, Firecracker has the smallest
device model. File permissions control access to its vsock Unix sockets. The
jailer needs root capabilities, so the root helper starts it. On aarch64,
support is limited to hosts with 4K pages until a boot on a 16K-page host is
measured (T-094).

### Windows: Hyper-V through the Host Compute Service

agent-box calls the Host Compute Service (computecore.dll) through a
LocalSystem helper service that the operator installs once. Each running
instance is one HCS compute system with no network adapter and no shared
filesystem. On Windows, only the Hyper-V stack runs the VMM in a hardened
worker for each VM. Microsoft fixes a defect in that VM boundary as a
security defect. The worker, vmwp.exe, has its own virtual account, ACG and
CIG, and no SeImpersonate privilege. Of the two Hyper-V routes, HCS boots the kernel directly and
writes no registry entry. The other route, VMMS through WMI, is the first
fallback. With an engine on the Windows Hypervisor Platform, agent-box would
start and confine the VMM process itself.

### Why two engines

The operator first asked for one engine that is native on both OSes. The
sweep of 2026-10-14 found four candidates: crosvm, OpenVMM, libkrun and
QEMU. Each one weakens isolation on at least one OS, so the split by OS
stays. The ruling is D11 in [docs/decisions.md](docs/decisions.md), and
[docs/research/engines-2026-10.md](docs/research/engines-2026-10.md) gives
the evaluation.

NOTE: The two engines do not give the same isolation. Under HCS, the guest
reaches the vmwp.exe worker and also kernel code of the root partition:
VMBus, storvsp, vhdmp and the hvsocket provider. Under Firecracker, the
guest reaches KVM in the host kernel, and the block and vsock devices run
in the jailed VMM. [SECURITY.md](SECURITY.md) gives the consequences.

## What agent-box is not

- It is not a harness. kage is the harness. When kage lacks a feature, the
  operator asks the kage maintainer.
- It is not a container tool. Each instance has its own kernel. Containers
  inside a guest wait for a measurement (T-185).
- It does not share a filesystem with the guest. Project files go through a
  git forge only.
- It does not intercept TLS for credentialed traffic. The guest calls named
  broker endpoints, and the guest never names the upstream host.
- It does not put an SSH key in the guest. Until the SSH agent in the daemon
  exists (T-257), ssh from the guest reaches only hosts that need no
  credential.
- It does not stop an agent from sending data to a destination that its
  policy allows. The broker stops credential theft, not the misuse of an
  allowed channel.
- It is not a service for many users yet. The chat gateway, the web UI and
  the provider pool are the late milestones M10 to M12.

[docs/limits.md](docs/limits.md) gives each limit and its reason.

## For developers

The tooling is a Rust program in `xtask/`. The toolchain is pinned in
`rust-toolchain.toml`, and `.cargo/config.toml` holds the aliases.

On a fresh machine, install git, rustup and a linker first: the Visual Studio
Build Tools on Windows, gcc or clang on Linux. rustup installs the pinned
toolchain on the first cargo command. The Requirements section of
[docs/development.md](docs/development.md#requirements) gives the full list.

```
cargo xtask setup    # arm the commit hook, add the musl targets, install cargo-deny, check wsl-toolkit
cargo xtask doctor   # what this host can do for development, probed by running each tool
cargo xtask gate     # format, lints, tests, each repository check and cargo-deny; --strict fails a skipped step
```

CAUTION: A Rust build can use all the memory of a small host. Set
`CARGO_BUILD_JOBS=4`, and run one build at a time.

On a Windows host, `cargo xtask linux` runs Linux commands and KVM tests
through wsl-toolkit. [docs/development.md](docs/development.md) gives the
full workflow, the checks and CI. An agent session starts at
[AGENTS.md](AGENTS.md).

## Documentation

| Document | Contents |
| --- | --- |
| [AGENTS.md](AGENTS.md) | The router for agent sessions: the start procedure, the documents for each task, the rules |
| [SECURITY.md](SECURITY.md) | How to report a vulnerability, the trust boundary, and what is not a boundary |
| [docs/architecture.md](docs/architecture.md) | The technical reference: processes, crates and data flow; it wins any conflict |
| [docs/decisions.md](docs/decisions.md) | The rulings of the operator with dates, and the superseded rulings |
| [docs/roadmap.md](docs/roadmap.md) | The milestones M0 to M12 and the exit criterion of each |
| [docs/status.md](docs/status.md) | What was measured, on which host, and when |
| [docs/engines.md](docs/engines.md) | The Linux and Windows engines, and the protocols of the helper |
| [docs/guest.md](docs/guest.md) | The guest kernel, the base image, box-guest, and kage in the guest |
| [docs/network.md](docs/network.md) | The tun device over vsock, the netstack, DNS, flow policy and forwards |
| [docs/broker.md](docs/broker.md) | Endpoints, bindings, the injection gate and the credential store |
| [docs/protocol.md](docs/protocol.md) | The vsock ports, the framing, the messages and the versions |
| [docs/cli.md](docs/cli.md) | The verbs, the states, the exit codes, the config file and profiles |
| [docs/limits.md](docs/limits.md) | What agent-box does not do, and where each promise ends |
| [docs/development.md](docs/development.md) | The toolchain, the xtask commands, Linux and KVM tests from Windows, CI |
| [docs/conventions.md](docs/conventions.md) | Rules for code, prose (ASD-STE100), git, secrets and a public repository |
| [docs/references.md](docs/references.md) | The pins table, and the upstream findings that the design keeps |
| [docs/research/microvm-tier.md](docs/research/microvm-tier.md) | The microVM research of the operator, pinned again, with the old ids mapped |
| [docs/research/engines-2026-10.md](docs/research/engines-2026-10.md) | The engine evaluation of 2026-10, and the reason for each rejected engine |
| [TODO/PROGRESS.md](TODO/PROGRESS.md) | The state, the baseline, the work order and the open questions |
| [TODO/INDEX.md](TODO/INDEX.md) | Each entry with its priority, effort, milestone and status |
| [TODO/RULES.md](TODO/RULES.md) | How the record of work is kept |
| [TODO/upstream.md](TODO/upstream.md) | Requests to the maintainers of other projects, which the operator sends |
| [TODO/repo.md](TODO/repo.md) | The first area file; the other area files hold the entries for each subject |

## Licence

agent-box is under the [0BSD](LICENSE) licence.

The repository started as Llaberry/agent-box and moved to Azathothas/agent-box
with the authorization of the operator.
