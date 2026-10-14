# Limits

This page says what agent-box does not do. Each limit names its source or the
entry that carries it. A limit here is a property of the design, or of a
component that agent-box builds on. Open questions are in the last section,
each with its entry. When this page conflicts with
[architecture.md](architecture.md), follow architecture.md.

NOTE: agent-box has measured almost nothing yet, and its product code is
still planned (ruling D14 in [decisions.md](decisions.md)).
[status.md](status.md) holds the measured state. A figure on this page is an
upstream project's own figure, unless it carries the label MEASURED.

READ and MEASURED have the meanings that [conventions.md](conventions.md)
gives them.

## The limit that matters most: exfiltration through an authorized channel

The broker stops credential theft. It does not stop exfiltration through a
channel that the operator authorized.

- An agent that never sees a token can still push a private project's content
  to a repository that it may push to.
- It can send content to its model provider. Every prompt and every file that
  the agent reads goes there.
- It can send content in DNS labels inside a wildcard zone. In `open` mode it
  can send content to any public address.
- The credential was never stolen. The agent used it for what it was granted.

WARNING: Do not describe agent-box as a tool that prevents exfiltration. It
prevents credential theft and narrows where data can go. That narrow sentence
is the true one.

### What helps, and what is theatre

| approach | assessment | entry |
| --- | --- | --- |
| bindings narrowed per task phase | Real, and the strongest option. Somebody must define the phases, and a wrong answer blocks legitimate work. | T-150 |
| the push namespace | Real for the forge. The agent cannot change main or the branches of another instance. It can still push what it read into its own namespace. | T-146 |
| an approval out of band for each widening operation | Real. The operator is in the loop, which is the point and the cost. The one-use pull request command of the chat gateway is one form. | T-223 |
| short-lived credentials minted per instance or per task | Real. No reference read for this project implements them. | T-148 |
| the flow policy in `allowlist` or `deny` mode | Real but coarse. It stops a paste site. It does not stop a push to an allowed repository. | T-123 |
| NXDOMAIN for names the policy does not allow | Real against DNS tunnels to other zones. It does nothing inside a wildcard zone or in `open` mode. | T-122 |
| content scanning on egress | Theatre for this threat. The agent chooses the encoding, so it can avoid any pattern a scanner matches. Use it only as telemetry on honest mistakes. | none |
| a model that judges whether a request looks like exfiltration | Theatre. That model reads the same untrusted input as the prompt-injected model it defends against. | none |
| masking secrets in output | Not a boundary. Trivial encoding defeats it. Keep it as damage control for honest logs. | none |
| hiding secrets from the model before a provider call | Not a boundary either, because any tool call sees the real value. Here it applies only to secrets inside project files. | none |

NOTE: The guest holds no provider secret, only nonces. Nobody checked whether
kage hides secrets from the model.

The general form of the problem is a lifetime mismatch. An authorization lives
as long as the instance, not as long as the task that needs it.

- A persistent instance runs task after task with the same bindings. The grant
  is the union over every task, the task after a prompt injection included.
- agent-sandbox has this open as
  [issue 1580](https://github.com/kubernetes-sigs/agent-sandbox/issues/1580).
- Its open [PR 1644](https://github.com/kubernetes-sigs/agent-sandbox/pull/1644)
  keeps three properties together (READ). The real key is never in the
  sandbox. The token reaches one process only. The token is revoked at process
  exit.
- T-150 carries the design for agent-box.

## What the broker cannot reach

| limit | source or entry |
| --- | --- |
| A tool that cannot take a base URL. It goes to its upstream under the flow policy, with no credential, and its call fails there. | ruling P4; T-149 |
| A client that pins certificates. It would refuse the later TLS interception too. | design; T-149 |
| A credential inside WebSocket frames, or inside another protocol after an HTTP upgrade. The broker refuses Upgrade in v1. | T-145 |
| Anything that is not HTTP. ssh and other TCP or UDP work under the flow policy, but no credential is injected. | ruling D5; T-146 |
| ssh with a key. The guest holds no SSH key, so until T-257 lands, ssh reaches only hosts that need no credential. | T-257 |
| git over SSH to a private repository. The guest holds no SSH key, and the git configuration sends SSH remotes to the broker over HTTP. | T-146 |
| MCP OAuth. `kage mcp login` stores real access and refresh tokens in the guest, and kage refreshes them itself (READ, the citation below). | T-162 |
| Registry tokens of package managers, for example npm, cargo or PyPI. Each needs its own binding and its own base URL setting. None is designed yet. | T-141 |

Source for the MCP OAuth row (READ):
`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/mcp_auth.rs:187`.

Two old limits changed:

- The old page said that a WebSocket credential is copied raw. agent-vault now
  has a WebSocket surface ([PR 225](https://github.com/Infisical/agent-vault/pull/225),
  merged). The limit is still true for agent-box v1.
- The old page said that a client which refuses the proxy falls back and
  passes ([issue 194](https://github.com/Infisical/agent-vault/issues/194)).
  There is no proxy now: the netstack sees every packet.
- A client that ignores its base URL and dials the provider host directly is
  refused in `allowlist` mode. In `open` mode the provider gets a nonce and
  refuses it. The failure is visible, and no credential leaves.

## Network and protocol limits

| limit | source or entry |
| --- | --- |
| The guest has no IPv6 in v1 (proposal). A destination with no IPv4 address is out of reach. | T-045 |
| ICMP to the outside is not forwarded, so `ping` to the Internet fails. Only the gateway answers an echo request. | T-121 |
| The netstack refuses a flow to an address that its DNS did not answer, `open` mode included. A tool with a fixed address needs an address rule. | T-123 |
| A guest that caches an address past its record expiry gets new flows refused. Established flows go on. | T-122, T-123 |
| A flow to an allowed address can carry any SNI or Host. Two names on one CDN address are both reachable when one is allowed. | T-123; CubeSandbox issue 1697 |
| QUIC and other UDP need a rule that names UDP. Without one, clients fall back to TCP. | T-123 |
| DNS over TLS on port 853 is refused. DNS over HTTPS to an allowed name looks like any HTTPS flow and passes. | T-122 |
| In `open` mode the host resolves every name upstream. DNS then carries data out without limit. | T-122 |
| A wildcard rule lets the guest put data in DNS labels under that zone. | T-122 |
| Every instance leaves from the host's own address. Upstreams see the host, and limits per address apply to all instances together. | T-124 |
| A forwarded port serves guest content on the host. Any local process can connect, and a browser runs a guest page in a localhost origin. | T-125 |
| A large SSE event can be cut when an HTTP/1.1 upstream closes with RST. The broker offers HTTP/2, but an upstream with HTTP/1.1 only keeps the risk. | T-144; [broker.md](broker.md) |

## The isolation is not the same on Windows and Linux

| host | what the guest can reach on the host |
| --- | --- |
| Linux, Firecracker | KVM in the host kernel. virtio-blk and vsock run in the jailed user-mode VMM. |
| Windows, HCS | Kernel code of the root partition: the VMBus root side, storvsp, vhdmp and the hvsocket provider. Also the vmwp.exe worker of that VM. |

- Firecracker's device threads run in its VMM process (READ:
  `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/design.md:72-78`).
- The Windows row is an inference from READ facts in the engine review of
  2026-10-14. Nobody measured it.
- HCS is still the strongest option on Windows. Microsoft runs the VMM in a
  hardened worker per VM (ruling D11). It is still not the same boundary as
  Firecracker's.
- [engines.md](engines.md) and [SECURITY.md](../SECURITY.md) state the same
  difference.

## The host kernel and the hypervisor are the floor

- Every boundary on this page ends at the host kernel and the hypervisor. On
  Linux that is KVM. On Windows it is the Hyper-V hypervisor and the root
  partition.
- A defect there is outside what agent-box can contain. The operator patches
  the host.
- agent-box reports a host kernel outside Firecracker's validated set as a
  note (T-092).
- Side channels between instances are not addressed: Spectre-class attacks,
  timing and shared caches.
- Firecracker recommends SMT off for tenant separation (READ:
  `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/prod-host-setup.md:327-333`).
  agent-box does not check SMT yet.
- Each OS has a privileged helper: root on Linux, LocalSystem on Windows. It
  has a fixed operation set and never parses guest data. It needs its own
  threat model (T-080).
- The daemon holds the credential store, the broker and the netstack in one
  process. Until T-127 confines the packet parser, a parser defect that the
  guest can exploit reaches the credentials.

## A guest kernel defect loses only that instance

- The guest kernel is untrusted. An exploit of it gives root in that guest,
  which by design reaches nothing on the host.
- Root in the guest does break the guarantees inside the guest. It can read
  the memory of box-guest: the event nonce, the egress token and every nonce.
- Then it can forge events on the trusted channel. T-031 claims only that a
  process at the agent's uid cannot forge events.
- The patch cadence of the guest kernel moves to this project. A sandbox on a
  shared kernel used the distribution's cadence (research document, section
  12, in [research/microvm-tier.md](research/microvm-tier.md)).
- T-069 names who rebuilds the kernel and the image.

## kage holds no isolation of its own

- kage is the harness, not a boundary. Its permissions, project trust and
  agent budgets come from files that the agent's uid can write.
- kage merges its user file, the project file and the environment (READ:
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/guide/config.md:164-176`).
  Its trust records live in the agent's home (same file, line 218).
- The approval mode is allow, because the instance and the host policy are the
  boundary (T-160).
- An untrusted project file can set the shell program and the scrub list
  today. Environment pins override it (T-160).
- The request to make those keys depend on trust goes to the kage maintainer
  through the operator (T-169).
- kage's ACP stream, its usage events and its transcripts are agent-reported.
  Any guest process at kage's uid can forge them, for example through
  `/proc/<kage-pid>/fd`.
- Nothing for authorization, billing, backoff or lifecycle reads them. This is
  the rule R-TRUSTED-CHANNEL (T-031, T-176).
- The token of kage serve is readable by the agent's uid. It authenticates the
  host to the guest only.

## Firecracker block I/O and the vsock thread

- One VMM thread emulates the block, vsock and other virtio devices (READ, the
  design.md lines above).
- The data disk uses the Sync engine, which makes blocking system calls (READ:
  `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-io-engine.md:3-5`).
- Discard works only with the Sync engine (READ:
  `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-discard.md:12-17`).
- So guest disk load, for example from cargo or npm, shares one thread with
  every host channel. These are broker egress, the kage bridge and the guest
  agent.
- Expect head-of-line blocking. This is an inference, and nothing is measured.
- T-015 measures vsock latency during a cargo build. T-093 decides whether the
  reopening condition holds.
- vhost-user-blk is one mitigation, and it is a developer preview (READ:
  `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-vhost-user.md:3-6`).
  Cloud Hypervisor with a jail that agent-box builds is the other.

## Lifecycle limits

| limit | source or entry |
| --- | --- |
| Pause keeps the guest RAM on the host. A paused instance holds its memory until resume or stop. | T-089, T-106 |
| A paused instance does not survive the loss of its VMM process or a host restart. Its RAM state is lost and it shows `stopped`. The data disk survives. | [engines.md](engines.md); T-089 |
| A pause of more than about 600 s during a model stream fails that request. kage's loop retries it. | T-089; kage http.rs, cited in [network.md](network.md) |
| There is no hibernate in v1: no state goes to disk. A host reboot cold-boots each instance, and running processes are lost. | T-184 |
| Under HCS, pause, save and survival past the creator have no reference user with a Linux guest. WSL2 and hcsshim never use them. | T-014 |
| From M10, the memory facts of the chat gateway survive the delete of an instance on purpose. They are agent-authored and kept per tenant. They are the one named exception of the teardown check. | T-225, T-180 |
| Sparse data disks overcommit host storage. A full host disk makes guest writes fail with I/O errors, not with a clean ENOSPC. This is an inference; not measured. | T-087, T-103 |

## Host-side limits

| limit | source or entry |
| --- | --- |
| The credential store protects against a copied disk or a backup. A process that runs as the operator's user can still read it, through DPAPI or the key file. | T-140; [broker.md](broker.md) |
| A deleted instance leaves its freed blocks on the host filesystem until something overwrites them. The teardown check reads files only, so it cannot see them. | T-091 |
| An attached terminal gets the bytes of the guest unchanged, terminal escape sequences included. This is the same as an ssh session. | T-172 |
| A host behind a TLS-intercepting proxy fails the upstream certificate check of the broker. v1 has no option for an extra root. | T-144 |

## Daemon restart

- A daemon restart does not stop running instances. The daemon re-adopts them
  (T-090 on Linux, T-107 on Windows).
- The netstack and the broker live in the daemon. So every flow and every
  broker connection ends at the restart.
- box-guest connects the network channel again, and deny first runs again
  before any packet passes (T-045, T-126).
- Model calls in flight fail, and kage's loop retries them. Attached terminals
  and the kage bridge drop and must connect again.
- Paused instances stay paused, because the VMM holds them, not the daemon.
  On Windows this depends on the HCS spike (T-014).
- The research document says that a daemon restart kills every session in
  flight. The brief replaced that with re-adoption.

## Development and platform limits

| limit | source or entry |
| --- | --- |
| Nested virtualization is for development tests only. No production claim rests on it. | brief section 4.1; T-010 |
| `/dev/kvm` works inside a wsl-toolkit job on the operator's machine. MEASURED 2026-10-14: WSL2 kernel 7.2.0, kvm_intel nested=Y. | T-010 |
| Firecracker inside WSL2 is not an engine. All WSL distributions share one kernel. | [research/engines-2026-10.md](research/engines-2026-10.md) |
| Windows 11 Home is unverified. No evidence exists either way for a third-party HCS caller on Home. | T-109; T-111 holds the fallbacks |
| Windows on ARM64 is not a target. HCS boots LinuxKernelDirect on x64 only. | [engines.md](engines.md) |
| aarch64 Linux works only on hosts with 4K pages, until a boot on a host with 16K pages is measured. | T-094 |
| Firecracker tests aarch64 only on AWS Graviton 2 to 5. It lists only host kernels with 4K pages (READ, the citations below). | T-094 |
| Raspberry Pi 5 and Asahi default to 16K pages. QEMU under libvirt is the recorded fallback for such hosts. | T-094 |
| A host kernel outside Firecracker's validated set (5.10, 6.1, 6.18) gets a note, not a refusal. | T-092 |

Sources for the aarch64 rows (READ):
`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:README.md:133-153`
and
`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:26-30`.

## Out of scope

These are decisions, not gaps. Each says why.

| item | why |
| --- | --- |
| a shared filesystem between host and guest | project files move only through a forge (ruling D4) |
| a guest NIC on a host switch | the NIC goes over vsock to the netstack (ruling D5) |
| TLS interception for credentialed traffic in v1 | explicit endpoints (ruling P4); interception is a later option (T-149) |
| a harness of agent-box's own | kage is the harness, and gaps go to its maintainer through the operator (ruling D2) |
| a hosted control plane | the operator keeps the credentials on their own host |
| instances made from a shared memory snapshot | no instance inherits the memory of another; oh-my-pi's CI turns VM templating off for this reason |
| a display server | not a workload that agent-box serves |

- The old limits of the shared-kernel sandbox no longer apply. The bailey and
  podman backends are retired; [decisions.md](decisions.md) lists them under
  Superseded.
- Source for the snapshot row (READ):
  `can1357/oh-my-pi@6f2c14b3e4cc065139789da893e4f86f3d72958c:infra/docs/02-kata-runtime.md:388-397`.
- The old page called an upstream patch a closed topic. Ruling D2 replaces
  that: the operator asks the kage maintainer, and agents write the request
  into [TODO/upstream.md](../TODO/upstream.md).

## What is undecided

These questions are open. Each row names the entry that decides it.

| question | entry |
| --- | --- |
| which userspace TCP/IP stack | T-120 |
| the guest address plan, IPv6 and the MTU | T-045 |
| the host's upstream resolver, the expiry clamps and query types beyond A | T-122 |
| the rule syntax and the address rules | T-123, T-027 |
| whether loopback and metadata addresses stay closed to every rule | T-123 |
| the connect timeout | T-124 |
| the Linux credential store: a key file or the Secret Service | T-140 |
| how git presents the forge nonce, and fetches of public repositories outside the binding | T-146 |
| the push namespace: instance name or instance id | T-146 |
| whether the 30 min write bound is total or per write | T-144 |
| a nonce per task, revoked at the end of the task | T-150 |
| short-lived forge credentials | T-148 |
| ssh with a host-held key, and the limits of the SSH agent | T-257 |
| TLS interception for tools without a base URL | T-149 |
| MCP OAuth: refuse or broker | T-162 |
| HCS pause, save, re-adoption and sparse files | T-014 |
| Windows 11 Home | T-109, T-111 |
| block I/O and the vsock thread | T-015, T-093 |
| aarch64 on hosts with 16K pages | T-094 |
| hibernate | T-184 |
| containers inside the guest | T-185 |
| one kernel with virtio_vsock and hv_sock | T-061 |
| who rebuilds the kernel and the image | T-069 |
| what a running ephemeral instance does when the chat gateway or the daemon stops | T-222 |
