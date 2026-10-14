# Index

Every entry of the work record, one row each, sorted by id. The entry lives
in the area file that its row links. It closes there, with its Prove run and
the result written down.

The work order is not on this page: [PROGRESS.md](PROGRESS.md) holds the
only one. [RULES.md](RULES.md) tells how the record is kept.
[upstream.md](upstream.md) holds the requests to other projects.
[docs/roadmap.md](../docs/roadmap.md) gives the milestones and their exit
criteria.

`cargo todo set` and `cargo todo counts` derive the counts from the rows.
`cargo todo check` asserts that the counts, the rows and the entries agree.
The gate runs it on each push.

## Values

### Priority

- P0: breaks the trust boundary of
  [docs/architecture.md](../docs/architecture.md), loses data, or stops the
  process.
- P1: a documented capability does not work, or a milestone exit depends on
  it.
- P2: worth doing; nothing is wrong without it.
- P3: worth recording, so that nobody has to find it again.

### Effort

S is under a day. M is a few days. L is a week. There is no XL: an entry of
that size is two entries.

### Status

- `open`: not started, or started with no Prove run yet. Parked and backlog
  work stays `open`.
- `partial`: part of the Prove ran. The rest waits, for example for the first
  CI run after a push (T-007).
- `blocked`: somebody else must act first. The entry names who, and what
  unblocks it.
- `done`: the Prove ran, and the `## Done` section gives the date and the
  result.

### Milestone

`M0` to `M12`, as in [docs/roadmap.md](../docs/roadmap.md). `backlog`: in no
milestone; the operator schedules each one.

### Category

| Category | The deliverable |
| --- | --- |
| `defect` | a repair of something that is wrong |
| `feature` | a new capability, with its tests |
| `measurement` | numbers or a pass or fail, with host, date and conditions; a spike is a measurement |
| `release` | a release workflow, an artifact or a release check |
| `chore` | repository work that changes no capability |
| `research` | a reading of sources, or a request, with its citations |
| `docs` | a document, for example a threat model |
| `decision` | a decision recorded in [docs/decisions.md](../docs/decisions.md) or in the entry |

### Area files

The first plan gave each area file a block of ids, and those blocks stay as
they are. A new entry takes the id that `cargo todo next` prints, in any area
file. T-256 (core), T-257 (broker), T-258 (chat), T-259 and T-260 (repo) are the
first such entries. Their ids fall in the old block of pool.md, and that block does not
bind them.

| Area file | Covers | Id block of the first plan |
| --- | --- | --- |
| [repo.md](repo.md) | the repository and its tooling | 001 to 019 |
| [core.md](core.md) | shared types, protocol, transport, daemon and CLI skeletons, the P0 rules | 020 to 039 |
| [guest.md](guest.md) | the guest agent box-guest | 040 to 059 |
| [image.md](image.md) | the guest kernel and images | 060 to 079 |
| [linux.md](linux.md) | the Linux engine | 080 to 099 |
| [windows.md](windows.md) | the Windows engine | 100 to 119 |
| [net.md](net.md) | the network | 120 to 139 |
| [broker.md](broker.md) | credentials and the broker | 140 to 159 |
| [kage.md](kage.md) | kage in the guest | 160 to 169 |
| [cli.md](cli.md) | the CLI | 170 to 179 |
| [assurance.md](assurance.md) | the battery, measurements, threat models | 180 to 199 |
| [release.md](release.md) | the release | 200 to 209 |
| [chat.md](chat.md) | the chat gateway | 220 to 239 |
| [web.md](web.md) | the web UI | 240 to 249 |
| [pool.md](pool.md) | the provider pool and metrics | 250 to 259 |

## The order, and the argument for it

[docs/roadmap.md](../docs/roadmap.md#the-order-and-why) gives the order of the
milestones and the argument for it. These rules order the entries.

1. The milestones go in their order: M0, then M1 to M12. The backlog waits
   for the operator.
2. Inside a milestone: P0, then P1, then P2, then P3. At one priority, an S
   entry goes first when it does not depend on a larger one.
3. An entry that another entry names in its Start condition goes first.
   T-033 comes before every other M1 entry.
4. Other examples: T-022 before T-031, T-023 before T-081 and T-100, and
   T-120 before T-121.
5. A P0 entry goes as early as its start condition allows: T-030, T-031 and
   T-256 in M1, and T-126 in M5.
6. The M0 close-outs (T-007, T-013, T-014 and T-015) run in parallel with
   M1, because they need hosts or events that M1 does not need.
7. An entry proves in its own milestone, on the host that it names. A live
   check that needs a later entry is a step of that later entry: T-088,
   T-105 or T-172.
8. A `blocked` entry waits for the one it names. T-169 waits for the
   operator, who sends the requests of [upstream.md](upstream.md).
9. A decision that needs the operator gets a question in
   [PROGRESS.md](PROGRESS.md), with a recommendation. The work goes on with
   the next entry.

## Counts

**136 entries: 123 open, 1 partial, 1 blocked, 11 done.**

| Priority | open | partial | blocked | done | total |
| --- | --- | --- | --- | --- | --- |
| P0 | 4 | 0 | 0 | 0 | 4 |
| P1 | 79 | 1 | 0 | 8 | 88 |
| P2 | 28 | 0 | 1 | 2 | 31 |
| P3 | 12 | 0 | 0 | 1 | 13 |
| **All** | 123 | 1 | 1 | 11 | 136 |

## Entries

| ID | Priority | Effort | Milestone | Category | Status | Item |
| --- | --- | --- | --- | --- | --- | --- |
| [T-001](repo.md) | P1 | S | M0 | chore | done | Cargo workspace, pinned toolchain and lints |
| [T-002](repo.md) | P1 | S | M0 | feature | done | `cargo xtask layering`: the crate graph goes down only |
| [T-003](repo.md) | P1 | M | M0 | feature | done | `cargo xtask todo`: the record checker and writer, ported from podssh-todo |
| [T-004](repo.md) | P1 | S | M0 | feature | done | Text and prose checks (ASCII, control bytes, line endings, STE heuristics) |
| [T-005](repo.md) | P1 | S | M0 | feature | done | Document checks: links, anchors, orphan pages, repository citations, one fact one home |
| [T-006](repo.md) | P1 | S | M0 | feature | done | Secret, placeholder and attribution checks |
| [T-007](repo.md) | P1 | M | M0 | feature | partial | `cargo xtask gate` and CI on Linux and Windows |
| [T-008](repo.md) | P1 | S | M0 | chore | done | `cargo xtask setup` and the commit-msg hook |
| [T-009](repo.md) | P2 | S | M0 | feature | done | `cargo xtask doctor`: what this host can do for development |
| [T-010](repo.md) | P1 | S | M0 | feature | done | `cargo xtask linux`: one Linux runner for Windows and Linux hosts |
| [T-011](repo.md) | P2 | S | M0 | feature | done | `cargo xtask refs`: the pins table and on-demand reference trees |
| [T-012](repo.md) | P3 | S | M0 | feature | done | `cargo xtask remote`: open issues and pull requests as data |
| [T-013](repo.md) | P2 | M | M0 | research | open | Re-pin the references at current upstream commits and re-check the kept claims |
| [T-014](repo.md) | P1 | M | M0 | measurement | open | Spike: the HCS lifecycle features that no reference uses |
| [T-015](repo.md) | P1 | S | M0 | measurement | open | Spike: Firecracker vsock latency under guest disk load |
| [T-016](repo.md) | P3 | S | backlog | decision | open | Decide whether the record checker becomes one crate shared with podssh |
| [T-020](core.md) | P1 | S | M1 | feature | open | box-core: InstanceId, names, State, InstanceSpec, errors |
| [T-021](core.md) | P1 | M | M1 | feature | open | box-proto: framing, ceilings and golden frames |
| [T-022](core.md) | P1 | M | M1 | feature | open | box-proto: v1 messages and the version handshake |
| [T-023](core.md) | P1 | S | M1 | feature | open | box-proto: the helper protocol |
| [T-024](core.md) | P1 | M | M1 | feature | open | box-transport on Linux: Firecracker hybrid vsock |
| [T-025](core.md) | P1 | M | M1 | feature | open | box-transport on Windows: AF_HYPERV |
| [T-026](core.md) | P1 | S | M1 | feature | open | Host state directory, instance records and locks |
| [T-027](core.md) | P1 | S | M1 | feature | open | The configuration file and profiles |
| [T-028](core.md) | P1 | M | M1 | feature | open | Daemon skeleton and the control API |
| [T-029](core.md) | P1 | M | M1 | feature | open | CLI skeleton: verbs, exit codes, --json |
| [T-030](core.md) | P0 | S | M1 | feature | open | The host never runs a tool against guest-written bytes |
| [T-031](core.md) | P0 | M | M1 | feature | open | A trusted event channel the guest cannot forge into |
| [T-032](core.md) | P1 | S | M1 | feature | open | The Engine trait, the capability report and a test double |
| [T-033](core.md) | P1 | S | M1 | chore | open | Create the product crates with the edges of the crate graph |
| [T-040](guest.md) | P1 | M | M1 | feature | open | box-guest PID 1: mounts, reaping, signals, poweroff |
| [T-041](guest.md) | P1 | M | M1 | feature | open | box-guest control server and provisioning |
| [T-042](guest.md) | P1 | M | M1 | feature | open | box-guest PTY and exec sessions |
| [T-043](guest.md) | P1 | M | M2 | feature | open | Data disk: first-boot format and the overlay root |
| [T-044](guest.md) | P1 | S | M2 | feature | open | Guest time sync |
| [T-045](guest.md) | P1 | M | M5 | feature | open | Guest network: the tun device and the network channel |
| [T-046](guest.md) | P1 | M | M7 | feature | open | box-guest supervises kage and bridges port 5004 |
| [T-060](image.md) | P1 | M | M2 | feature | open | Guest kernel: pinned source and one config for both engines |
| [T-061](image.md) | P1 | S | M2 | measurement | open | One kernel with virtio_vsock and hv_sock |
| [T-062](image.md) | P1 | M | M2 | feature | open | Debian 13 slim base image, reproducible |
| [T-063](image.md) | P1 | S | M2 | feature | open | kage in the image |
| [T-064](image.md) | P2 | M | M2 | feature | open | Toolchain variants and the variant rule |
| [T-065](image.md) | P2 | S | M2 | measurement | open | Base image format: erofs or ext4, and the VHDX copy |
| [T-066](image.md) | P1 | M | M2 | feature | open | Pins and verification: box-image |
| [T-067](image.md) | P1 | S | M2 | feature | open | `agent-box image pull\|list\|verify\|gc` |
| [T-068](image.md) | P1 | M | M2 | release | open | Image CI workflow and release assets with attestations |
| [T-069](image.md) | P2 | S | M2 | decision | open | Image ownership and rebuild cadence |
| [T-080](linux.md) | P1 | S | M3 | docs | open | Threat model of the privileged helper (both OSes) |
| [T-081](linux.md) | P1 | M | M3 | feature | open | Linux helper: socket activation, peer check, fixed operations |
| [T-082](linux.md) | P1 | M | M3 | feature | open | uid allocator and the delegated cgroup v2 root |
| [T-083](linux.md) | P1 | S | M3 | feature | open | Firecracker and the jailer: download and verify at setup |
| [T-084](linux.md) | P1 | M | M3 | feature | open | The VM configuration builder |
| [T-085](linux.md) | P1 | S | M3 | feature | open | Firecracker API client over a Unix socket |
| [T-086](linux.md) | P1 | M | M3 | feature | open | Jail construction on each boot |
| [T-087](linux.md) | P1 | S | M3 | feature | open | The data disk: sparse create, lock, settings |
| [T-088](linux.md) | P1 | M | M3 | feature | open | Start, boot handshake, stop and kill |
| [T-089](linux.md) | P1 | S | M3 | feature | open | Pause and resume |
| [T-090](linux.md) | P1 | M | M3 | feature | open | Inventory, re-adoption and orphans |
| [T-091](linux.md) | P1 | S | M3 | feature | open | Delete with verified absence |
| [T-092](linux.md) | P1 | S | M3 | feature | open | Linux probe and startup report |
| [T-093](linux.md) | P2 | M | M8 | measurement | open | Block I/O and the reopening condition |
| [T-094](linux.md) | P3 | M | backlog | measurement | open | aarch64 on 16K-page hosts |
| [T-100](windows.md) | P1 | M | M4 | feature | open | Windows helper service: install, pipe ACL, client check, fixed operations |
| [T-101](windows.md) | P1 | M | M4 | feature | open | HCS schema types |
| [T-102](windows.md) | P1 | M | M4 | feature | open | computecore calls |
| [T-103](windows.md) | P1 | M | M4 | feature | open | VHDX: read-only base and dynamic 30 GiB data disk |
| [T-104](windows.md) | P1 | S | M4 | feature | open | HvSocket configuration |
| [T-105](windows.md) | P1 | M | M4 | feature | open | Start, boot handshake, stop and kill on Windows |
| [T-106](windows.md) | P1 | S | M4 | feature | open | Pause and resume on Windows |
| [T-107](windows.md) | P1 | M | M4 | feature | open | Inventory by owner, re-adoption, orphans, delete with verified absence |
| [T-108](windows.md) | P1 | S | M4 | feature | open | Windows probe and preflight |
| [T-109](windows.md) | P2 | S | M4 | measurement | open | Windows 11 Home with Virtual Machine Platform |
| [T-110](windows.md) | P2 | S | M8 | measurement | open | Memory reclaim and fstrim on Windows |
| [T-111](windows.md) | P2 | M | backlog | research | open | Windows fallbacks: Hyper-V VMMS through WMI, then OpenVMM |
| [T-120](net.md) | P1 | S | M5 | measurement | open | Choose the userspace TCP/IP stack |
| [T-121](net.md) | P1 | L | M5 | feature | open | Host netstack per instance |
| [T-122](net.md) | P1 | M | M5 | feature | open | DNS on the gateway |
| [T-123](net.md) | P1 | M | M5 | feature | open | Flow policy engine |
| [T-124](net.md) | P1 | S | M5 | feature | open | Host-side connect |
| [T-125](net.md) | P1 | M | M5 | feature | open | Inbound forwards |
| [T-126](net.md) | P0 | S | M5 | feature | open | Deny first: no egress before policy, no kage before egress |
| [T-127](net.md) | P2 | M | M8 | feature | open | Confine and fuzz the packet parser |
| [T-140](broker.md) | P1 | M | M6 | feature | open | Credential store |
| [T-141](broker.md) | P1 | M | M6 | feature | open | Bindings and nonces |
| [T-142](broker.md) | P1 | M | M6 | feature | open | Broker endpoint server on the gateway |
| [T-143](broker.md) | P1 | M | M6 | feature | open | The injection gate and the fail-closed shapes |
| [T-144](broker.md) | P1 | M | M6 | feature | open | Upstream client |
| [T-145](broker.md) | P2 | M | M6 | feature | open | Substitution model |
| [T-146](broker.md) | P1 | M | M6 | feature | open | Forge binding and the push namespace |
| [T-147](broker.md) | P1 | M | M6 | feature | open | Usage accounting |
| [T-148](broker.md) | P2 | M | backlog | feature | open | Short-lived forge credentials |
| [T-149](broker.md) | P3 | L | backlog | feature | open | Optional TLS interception for tools that cannot take a base URL |
| [T-150](broker.md) | P2 | M | backlog | research | open | Narrow the bindings per task phase |
| [T-160](kage.md) | P1 | S | M7 | decision | open | kage configuration and approval mode |
| [T-161](kage.md) | P1 | M | M7 | feature | open | ACP client in the daemon |
| [T-162](kage.md) | P1 | S | M7 | decision | open | MCP OAuth tokens in the guest |
| [T-163](kage.md) | P2 | S | M7 | feature | open | Default egress for kage's own destinations |
| [T-169](kage.md) | P2 | S | M7 | research | blocked | Requests to the kage maintainer, sent by the operator |
| [T-170](cli.md) | P1 | M | M7 | feature | open | Lifecycle verbs end to end |
| [T-171](cli.md) | P1 | M | M7 | feature | open | `agent-box run` |
| [T-172](cli.md) | P1 | M | M7 | feature | open | `agent-box attach` |
| [T-173](cli.md) | P1 | S | M7 | feature | open | Profiles, `agent-box policy` and `agent-box forward` |
| [T-174](cli.md) | P1 | M | M7 | feature | open | `agent-box setup` and `agent-box uninstall` |
| [T-175](cli.md) | P1 | S | M7 | feature | open | `agent-box doctor` and the startup report |
| [T-176](cli.md) | P2 | S | M7 | feature | open | Events and `agent-box logs` |
| [T-180](assurance.md) | P1 | L | M8 | measurement | open | The containment battery and its negative control |
| [T-181](assurance.md) | P1 | M | M8 | measurement | open | Footprint and latency |
| [T-182](assurance.md) | P1 | M | M8 | docs | open | Threat models: helper, broker, netstack |
| [T-183](assurance.md) | P2 | M | M8 | feature | open | Fuzzing |
| [T-184](assurance.md) | P3 | L | backlog | feature | open | `hibernate` |
| [T-185](assurance.md) | P2 | M | M8 | measurement | open | Containers inside the guest |
| [T-186](assurance.md) | P2 | S | M8 | measurement | open | Instance-to-instance isolation |
| [T-200](release.md) | P1 | M | M9 | release | open | Release workflow |
| [T-201](release.md) | P1 | M | M9 | release | open | Install guide and end-to-end verification |
| [T-202](release.md) | P2 | S | M9 | release | open | Third-party licences |
| [T-220](chat.md) | P2 | L | M10 | feature | open | Discord gateway: one channel, a thread per session |
| [T-221](chat.md) | P2 | M | M10 | feature | open | Authorization from platform identity |
| [T-222](chat.md) | P2 | M | M10 | feature | open | Ephemeral session flow and gateway loss |
| [T-223](chat.md) | P2 | M | M10 | feature | open | Pull requests composed by the daemon |
| [T-224](chat.md) | P3 | S | M10 | feature | open | Thread commands and status |
| [T-225](chat.md) | P3 | M | M10 | feature | open | Memory facts, kept per tenant |
| [T-226](chat.md) | P3 | S | M10 | feature | open | GitHub mentions as a task source |
| [T-240](web.md) | P2 | L | M11 | feature | open | Web UI server with chat-service login |
| [T-241](web.md) | P2 | M | M11 | feature | open | Watch a session, start one from the web |
| [T-242](web.md) | P3 | M | M11 | feature | open | Metrics views |
| [T-250](pool.md) | P2 | M | M12 | feature | open | Provider slots |
| [T-251](pool.md) | P2 | M | M12 | feature | open | Admission queue |
| [T-252](pool.md) | P2 | S | M12 | feature | open | Backoff from broker-observed responses |
| [T-253](pool.md) | P3 | S | M12 | feature | open | Usage windows and reset times |
| [T-254](pool.md) | P3 | S | M12 | feature | open | Cost from the catalogue, with its date |
| [T-255](pool.md) | P3 | S | M12 | feature | open | What a stuck session looks like |
| [T-256](core.md) | P0 | S | M1 | feature | open | No secret in the guest image, the disk, the kernel command line, MMDS or the VMM argv |
| [T-257](broker.md) | P2 | M | M6 | feature | open | SSH from the guest through a host-held key |
| [T-258](chat.md) | P3 | S | M10 | feature | open | errand file and context commands parity: !ls, !cat, !file, !pwd and !compact |
| [T-259](repo.md) | P1 | S | M1 | feature | open | `cargo xtask check unsafe`: only the listed files allow unsafe code |
| [T-260](repo.md) | P1 | S | M1 | feature | open | Gate step `guest`: build box-guest for the musl targets on every host |
