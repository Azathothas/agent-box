# Security

agent-box has no release and no product code yet. This page describes the
planned security design. An entry of `TODO/` implements each part of it, and
the containment battery (T-180) tests it.

## Report a vulnerability

Use the private vulnerability reporting of GitHub on Azathothas/agent-box.
Open the Security tab of the repository, then select "Report a
vulnerability". The form is at
https://github.com/Azathothas/agent-box/security/advisories/new.

Do not open a public issue for a security problem.

A defect in a project that agent-box uses (Firecracker, kage, a Rust crate)
belongs to that project. Report it to that project.
[docs/references.md](docs/references.md) names each project and its pinned
commit.

## Supported versions

| Version | Supported |
| --- | --- |
| None released | Not applicable |

The first release is milestone M9 (T-200). Until then, a report applies to
the plan and the tooling on the `main` branch.

## The threat

A model chooses code at run time, and that code runs inside an instance. The
code can be wrong, injected through a file or a web page, or hostile.
agent-box treats everything inside the guest as hostile, the guest kernel
included. The attacker is not the operator and has no code on the host.

## Trust boundary

| Component | Trusted | Notes |
| --- | --- | --- |
| The `agent-box` CLI | Yes | Runs as the operator, without privilege |
| The daemon (`agent-box daemon`) | Yes | Runs as the operator; holds the credential store, the broker and the netstacks |
| The helper (`agent-box-helper`) | Yes | Root or LocalSystem; a fixed set of operations; never parses guest data |
| The VMM process | Partly | Parses data that the guest controls; the jailer (Linux) or the per-VM vmwp.exe worker (Windows) confines it |
| Everything inside the guest | No | The agent, its tools, the guest kernel, and every byte on the guest disks |
| The output of kage, its ACP stream, transcripts | No | Any guest process at the uid of kage can forge them |
| The content of a forge, the body of a provider response | No | Data from parties that agent-box does not control |
| A chat message (from milestone M10) | No | Authority comes from the platform identity that the gateway checks, never from the message text |

## The four rules

Each rule follows from the trust boundary. A change that breaks one is a
defect.

1. R-HOST-NO-TOOLS. The host runs no tool (git, a package manager, a
   linter, kage) on bytes from the guest. Host code interprets no path,
   gitfile, alternates file, symlink or repository config from the guest.
   Because files move through a forge only, the host holds no project bytes.
   Entry T-030 in [TODO/core.md](TODO/core.md).
2. R-TRUSTED-CHANNEL. Authorization, billing, backoff and lifecycle read
   only two sources: the broker, which sees each provider response, and the
   event channel of box-guest. box-guest runs as root and proves each event
   with a nonce. kage and its tools run as uid 1000, the user `agent`. The
   image has no sudo and no setuid program. The ACP stream of kage is
   labelled agent-reported data. Entry T-031 in [TODO/core.md](TODO/core.md).
3. R-NO-SECRET-IN-GUEST. No credential is in the guest image, on a guest
   disk, on the kernel command line, in MMDS, or in the argv of the VMM. The
   daemon sends the nonces and the tokens over the control port only, after
   boot and before kage starts. They stay in memory or on a tmpfs, never on
   a disk. Entry T-256
   in [TODO/core.md](TODO/core.md) owns the rule. T-041
   ([TODO/guest.md](TODO/guest.md)), T-062 ([TODO/image.md](TODO/image.md))
   and T-141 ([TODO/broker.md](TODO/broker.md)) build parts of it.
4. R-DENY-FIRST. An instance has no egress until the daemon holds its policy
   and bindings. kage starts only after that point. Entry T-126 in
   [TODO/net.md](TODO/net.md).

## What is a boundary

- The hypervisor: KVM on Linux, and the Hyper-V partition on Windows.
- The confinement of the VMM. On Linux, the jailer and the seccomp filters
  of Firecracker. On Windows, the per-VM vmwp.exe worker.
- The helper. It checks the peer (SO_PEERCRED on Linux, the client SID on
  Windows) and validates each input before any system call.
- On Windows, the helper writes each HCS document itself. It accepts no
  schema fragment from a caller, and grants VM access only to files in the
  agent-box store.
- The netstack. DNS answers only allowed names. A new flow must go to an
  address that the netstack answered for an allowed name. Reserved address
  ranges are refused.
- The broker. A request passes the eight conditions of the injection gate
  ([docs/broker.md](docs/broker.md)), or the broker refuses it. It never
  follows a redirect with a credential attached.
- The scope of each nonce and token. Each one is valid for one instance only
  (T-186 tests this).

## What is not a boundary

- The separation of users inside the guest. The guest kernel is untrusted,
  and a root escalation in the guest defeats it.
- The permissions, approval prompts and project trust of kage. Files that
  the agent can write control them. The approval mode is `allow` (T-160).
- The ACP stream of kage and the transcripts. They are what the agent
  reports about itself.
- A test under nested virtualization. It is valid for development only, and
  it is never a security claim or a product number.

NOTE: The event channel of box-guest depends on the separation of users in
the guest. An agent that gets root in the guest can forge events for its own
instance. For this reason, billing, backoff and slot accounting come from the
broker only.

## Isolation is different on each OS

On Linux, the guest reaches KVM in the host kernel. The block and vsock
devices run in the user-mode Firecracker process, inside the jail.

On Windows, the guest reaches the per-VM vmwp.exe worker. It also reaches
kernel code of the root partition: the VMBus root side, storvsp, vhdmp and
the hvsocket provider. A defect in that code is a defect of the host kernel,
outside the sandbox of the worker.

agent-box does not claim the same isolation on both OSes.
[docs/engines.md](docs/engines.md) gives the details for each engine.

## What the design does not protect against

- Data that leaves through an allowed destination. The broker keeps the
  credential from the agent. It does not stop a push to a repository or a
  request to a host that the policy allows.
- A defect in the hypervisor, in KVM, or in the drivers of the Hyper-V root
  partition.
- Hostile code that already runs on the host as the operator. The daemon
  holds each credential, and the account of the operator can read it.
- A credential that reaches the guest by a route other than the broker, for
  example a token that a person types into an attached shell.

[docs/limits.md](docs/limits.md) gives the full list of limits.

## Threat models

- The privileged helper on both OSes: [docs/engines.md](docs/engines.md)
  (T-080).
- The broker and the credential store: [docs/broker.md](docs/broker.md)
  (T-182).
- The netstack, DNS and the flow policy: [docs/network.md](docs/network.md)
  (T-182).

## Advisories of the dependencies

The gate runs cargo-deny against the RustSec advisory database (T-007). An
advisory is ignored only with its reason and its date in `deny.toml`.
A vulnerability that agent-box reaches keeps an open entry until a fix
lands.

## Secrets and this repository

WARNING: The repository is public. A credential never goes into the tree, a
test, a log, a commit message or an issue. If one gets in, rotate it first.
Then tell the operator, and then remove it from the tree.

`cargo xtask check secrets` scans the tracked files and the untracked files
that git does not ignore (T-006).
