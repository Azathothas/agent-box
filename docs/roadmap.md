# Roadmap

This page gives the milestones of agent-box, in order, with their exit
criteria and the hosts that each milestone needs.
[status.md](status.md) gives the measured state.

- Each exit criterion is a statement that a command or a measurement can
  verify. Each one names its entries by id.
- The open work of a milestone is entries in
  [TODO/INDEX.md](../TODO/INDEX.md), which also gives the status of each
  entry. [TODO/PROGRESS.md](../TODO/PROGRESS.md) holds the only work order.
- This page has no checkbox. `cargo todo check` refuses an open checkbox,
  and an entry listed under a milestone that is not its own.
- A milestone is done when each of its exit criteria holds on the hosts that
  it names.
- Close an entry in its area file. Record each measurement in
  [status.md](status.md) in the same commit.
- No product code exists yet. T-033 creates the product crates, as the first
  step of M1.

## Hosts

| Host | What it is | Used for |
| --- | --- | --- |
| Any host | Windows or Linux with the pinned Rust toolchain, and no virtualization | unit tests, golden frames, the record |
| Linux runner | `cargo xtask linux -- CMD`: a Linux container, native on Linux, through wsl-toolkit on Windows | Linux builds and tests, kernel and image builds |
| Nested KVM | `cargo xtask linux --kvm`: /dev/kvm inside a wsl-toolkit job on the Windows host | development tests only, never a production claim |
| Linux KVM host | Linux x86_64 with /dev/kvm, systemd and cgroup v2, not nested; root for the one-time setup | the Linux engine, its live tests and its measurements |
| Windows Hyper-V host | Windows 11 Pro x64 with Hyper-V; an administrator for the one-time setup | the Windows engine, its live tests and its measurements |
| Windows Home host | Windows 11 Home x64 with Virtual Machine Platform | the Home measurement of M4 |
| CI | GitHub Actions runners on Linux and Windows | the gate on each push, the image and release workflows |
| Test guild | a Discord server for tests, and a test repository on the forge | M10 only |

## M0: Repository, tooling and spikes

The repository has its tooling, its record and its CI. Two spikes test engine
features that no reference uses, before M1 code depends on them.

Hosts: the Windows host, the Linux runner and CI for the gate. T-014 needs
a Windows Hyper-V host as administrator. T-015 needs a Linux KVM host.

Exit criteria:

- `cargo xtask gate --strict` exits 0 natively on the Windows host and on a
  Linux host (T-007).
- In the Linux runner, `cargo xtask gate fmt clippy test` exits 0. The copy
  in the container has no .git/ and no cargo-deny, so the strict gate runs
  natively only (T-007).
- The first CI run after the push is green on ubuntu-24.04 and
  windows-2025, with the strict gate. T-007 records the URL of that run.
- `cargo build --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`
  exit 0 on both (T-001).
- `cargo xtask layering` exits 0, and a planted upward edge fails its unit
  test (T-002).
- `cargo todo check` exits 0 on this tree, and `cargo todo set` moves the
  counts (T-003).
- `cargo xtask check text prose docs one-home` exits 0. Each planted defect in
  the unit tests is found (T-004, T-005).
- `cargo xtask check secrets placeholders attribution` exits 0, and the tests
  find each planted value (T-006).
- The commit-msg hook refuses a planted tool-crediting message with exit 1,
  and `cargo xtask setup` exits 0 (T-008).
- `cargo xtask doctor --json` exits 0 and names each probe with its evidence
  (T-009).
- `cargo xtask linux --kvm -- ls -l /dev/kvm` exits 0 on the operator's
  Windows host (T-010).
- `cargo xtask refs fetch QaidVoid/kage` exits 0, and the HEAD of the clone
  equals the pin (T-011).
- `cargo xtask remote --json` exits 0; an empty list is valid (T-012).
- Each citation in docs/ resolves at its pin. The corrections of the claims
  that closed without a merge are written (T-013).
- [status.md](status.md) gives pass or fail, with evidence, for each of the
  five HCS spike items on Windows 11 Pro (T-014).
- [status.md](status.md) gives the vsock round-trip latency under guest disk
  load, with host, kernel, versions and sample count (T-015).
- T-015 writes a decision line for the block I/O measurement of M8.

## M1: Foundations

The shared types, the wire protocol, the host transports, the daemon and CLI
skeletons and the guest agent exist. Each test runs against a test double. No
instance boots yet.

Hosts: any host, and CI on both OSes. The AF_HYPERV module needs the Windows
host. The hybrid-vsock module needs a Linux host or the Linux runner. The musl
build of box-guest runs on every host (T-260).

Exit criteria:

- The product crates exist with the edges of the crate graph.
  `cargo xtask layering` and `cargo xtask gate --strict` pass (T-033).
- Only the files that decision P6 lists allow unsafe code (T-259).
- The gate builds box-guest for the musl targets on Windows and on Linux
  (T-260).
- `cargo test -p box-core` passes, with property tests for name and id
  parsing (T-020).
- `cargo test -p box-proto` passes. Golden frames cover the framing and every
  v1 message (T-021, T-022).
- A planted off-by-one in each frame guard fails a test (T-021).
- An unknown message type, an unknown port and a version mismatch are each
  refused in a test (T-022).
- The helper protocol types cannot express a path that the caller supplies
  (T-023).
- `cargo test -p box-transport` passes against a fake hybrid-vsock peer
  (T-024).
- Unit tests on Windows prove the mapping from port to service GUID (T-025).
- `cargo test -p box-daemon` passes for the state records, the configuration,
  the control API and the Engine trait (T-026, T-027, T-028, T-032).
- A test proves that two daemons cannot run at once (T-028).
- `agent-box --help` lists every verb. An unimplemented verb exits 70 and
  names its entry (T-029).
- `cargo xtask check host-tools` exits 0, and a planted `Command::new("git")`
  in box-daemon fails it (T-030).
- Against the test double, an event without the event nonce is refused, and
  the same event with the nonce is accepted (T-031).
- box-guest runs as a normal process against the test double: PID 1 duties,
  the control server, provisioning, and PTY and exec sessions (T-040, T-041,
  T-042).
- A test provisions marker values and scans every file that box-guest
  wrote. No marker appears (T-041).
- A provisioned value prints as `[redacted]`, and it has no `Display` and no
  `Serialize`. A test helper finds a planted value in raw, hex and base64
  form (T-256).
- A planted dependency of box-vmm on box-proto fails the layering test. So
  no engine builder can take a provisioned value (T-256).

NOTE: T-025, T-031, T-040, T-041 and T-042 also name live checks. A live
check needs a booted instance or the attach verb of M7. Each M1 entry closes
on its own Prove: unit tests and the T-032 test double. Its live check is a
step of the entry that first makes it possible. That entry is the Linux boot
entry of M3, the Windows boot entry of M4 or the attach entry of M7. M1 exits
on the test double. The builder and battery checks of the rule of T-256 are steps of the
entries that build them.

## M2: Guest image

A pinned kernel and a reproducible Debian 13 slim base image, with box-guest
and kage, are built by `cargo xtask image build` and verified by digest.

Hosts: the Linux runner builds the kernel and the images. CI runs the image
workflow on ubuntu runners. The boot checks need the hosts of M3 and M4.

Exit criteria:

- Two kernel builds give the same sha256, and images/pins.toml records the
  digests (T-060).
- Two base image builds give the same digest, and no layer holds a
  credential (T-062).
- The image build refuses a wrong kage digest in a test (T-063).
- `cargo test -p box-image` passes, and a planted wrong digest is refused
  (T-066).
- The image verbs pass their CLI tests. gc never removes an image that an
  instance uses (T-067).
- An image workflow run publishes the assets, and `gh attestation verify`
  passes on them (T-068).
- Each toolchain variant builds reproducibly (T-064).
- `cargo test -p box-guest disk::` and `cargo test -p box-guest time::` pass
  (T-043, T-044).
- [decisions.md](decisions.md) names the owner of the kernel and image
  rebuilds (T-069).

Boot checks of M2 entries, as steps of the boot and pause entries of M3 and
M4:

- A file written in the guest survives stop and start, and the base image
  digest does not change (T-043).
- Guest time is within 1 s of the host after boot (T-044).
- Guest time is within 1 s of the host after a 10-minute pause and resume.
  The pause entries of M3 and M4 run this check (T-044).
- One kernel gives working vsock under both engines (T-061).
- The base image and each variant boot, and `kage --version` prints the
  pinned version (T-062, T-063, T-064).
- [status.md](status.md) has the erofs and ext4 numbers with their
  conditions, and T-065 has a decision.

NOTE: The boot checks need the engines of M3 and M4. M2 exits on its build
criteria. T-043, T-044, T-062, T-063 and T-064 close on their own Prove.
Only T-061 and T-065 wait for the boot entries of M3 and M4, so they can stay
open after M2 exits.

## M3: Linux engine

On a Linux KVM host, an instance goes through its whole life. The steps are
create, start, a shell session, pause, resume, stop, delete, and re-adoption
after a daemon restart.

Hosts: a Linux KVM host with root for the one-time setup. Nested KVM serves
development tests. Whether a wsl-toolkit job can host the helper and the
delegated cgroup root is not measured.

Exit criteria:

- Each refusal condition of the helper threat model names a test that exists
  (T-080).
- The helper refuses a caller outside the agent-box group. It refuses a bad
  id before any syscall (T-081).
- memory.max, cpu.max and pids.max read back from the cgroup files of a
  running instance (T-082).
- A wrong Firecracker or jailer digest is refused, and the file is deleted
  (T-083).
- A snapshot test fixes the VM configuration JSON. Adding network-interfaces
  fails a test (T-084).
- The API client passes its tests against a fake API socket (T-085).
- /proc/VMM-PID/root resolves inside the jail, and the VMM runs as the
  per-instance uid (T-086).
- A host file descriptor trace shows no open of the data disk outside the
  VMM (T-087).
- `cargo test -p box-daemon --test linux_boot -- --ignored` passes on the KVM
  host (T-088).
- A shell session on the PTY port survives pause and resume (T-089).
- The daemon is killed with three instances running. After its restart, it
  re-adopts all three (T-090).
- The restarted daemon removes a VMM that no instance record claims (T-090).
- After delete, the teardown check finds no jail, cgroup, disk, pid file or
  uid, and no byte that the agent wrote (T-091).
- The probe module tests map each planted fact to its gap or note. The
  daemon startup report names each probe on a KVM host, and names the gap on
  a host without KVM (T-092).

NOTE: The attach verb is M7 work. In M3, a test opens the shell session on
port 5001 directly.

## M4: Windows engine

The same lifecycle as M3 works on Windows 11 Pro with Hyper-V, through the
Host Compute Service.

Hosts: a Windows Hyper-V host. An administrator installs the helper service
once; the daemon and the CLI run unprivileged. A Windows Home host for T-109.

Exit criteria:

- The helper service refuses an unauthorized user. Its types cannot express
  an added device (T-100).
- `cargo test -p box-vmm hcs::` passes on golden JSON documents, and an
  unknown field is refused (T-101).
- A live test drives each computecore call on the Windows Hyper-V host
  (T-102).
- A new 30 GiB data disk takes about 4 MiB on the host, and the guest sees
  30 GiB (T-103).
- The daemon, as the user, connects and listens on HvSocket. Another user is
  refused (T-104).
- A live test starts an instance, completes the boot handshake on port 5000,
  and stops it (T-105).
- A shell session survives pause and resume (T-106).
- After a daemon kill, the restarted daemon re-adopts the running instances,
  and delete leaves no file (T-107).
- The probe tests map each planted fact to its gap or note, and the daemon
  startup report names each Windows fact (T-108).
- [status.md](status.md) has the result of the HCS spike on Windows 11 Home
  (T-109).

## M5: Network

The guest has a virtual NIC. Its packets reach a userspace netstack in the
daemon, and policy applies per flow.

Hosts: any host for the unit tests; the hosts of M3 and M4 for the live
checks, on both engines.

Exit criteria:

- T-120 records the throughput and CPU numbers of each candidate stack, and
  a decision.
- From the guest, `curl` to an allowed name works through tun0 and the
  netstack (T-045, T-121).
- A test races the network channel against provisioning. No packet leaves
  before ready (T-126).
- The gateway DNS answers allowed names only and answers NXDOMAIN for
  others. It refuses HTTPS and SVCB records (T-122).
- A flow to an address that the netstack did not answer for an allowed name
  is refused (T-123).
- In the modes deny, allowlist and open, reserved ranges are refused after
  resolution and at connect time (T-123, T-124).
- A guest dev server answers on host loopback through a forward. A public
  bind needs a flag (T-125).
- `cargo test -p box-net` passes (T-121).

## M6: Broker

Credentials stay on the host. The broker serves explicit endpoints on the
gateway and injects the credential of a binding.

Hosts: any host for the unit tests. The live checks need an engine host, a
provider API key in the credential store and a test repository on a forge.

Exit criteria:

- `agent-box secret set NAME` reads stdin only, and a value never appears in
  a log (T-140).
- A nonce is 32 bytes per instance per binding, in the provider key format,
  and compared in constant time (T-141).
- The HTTP head parser passes its tests, the three parser traps included
  (T-142).
- One test per condition of the injection gate proves the refusal. A test
  asserts that injection happened (T-143).
- The upstream client verifies the fixed host against a local TLS server
  with a test CA (T-144).
- The upstream client never follows a redirect with a credential (T-144).
- `cargo test -p box-broker subst::` passes (T-145).
- Against a local git server, a push to an allowed ref works, and a push to
  another ref is refused (T-146).
- Usage accounting counts requests and tokens from recorded provider
  responses, SSE included (T-147).
- Live: curl, started in an exec session in the guest, reaches a provider
  through a broker endpoint. The guest holds only nonces (T-143).
- Live: git in the guest pushes to an allowed ref through the forge binding
  (T-146).
- Live: ssh in the guest reaches an allowed host with a key that the daemon
  holds, and the key never enters the guest (T-257).

NOTE: kage arrives in M7. In M6, a test client in the guest (curl, git and
ssh, started over an exec session) proves each binding.

## M7: kage and the CLI

The operator uses agent-box through its CLI. kage runs unattended under
box-guest, and the operator can attach.

Hosts: the Linux KVM host and the Windows Hyper-V host; a provider API key
for the live runs.

Exit criteria:

- [decisions.md](decisions.md) records the kage configuration and the
  approval mode, with the reason (T-160).
- A guest test proves that a project .kage/config.toml cannot change the
  pinned shell (T-160).
- kage restarts after a kill, and the bridge on port 5004 carries an ACP
  session (T-046, T-161).
- An ACP session completes a prompt against a recorded provider, and live
  (T-161).
- The MCP OAuth decision is recorded, and a test proves the chosen behaviour
  (T-162).
- The default policy file names kage's own destinations, and a test checks
  it (T-163).
- Each kage request has an answer in [upstream.md](../TODO/upstream.md)
  (T-169).
- Every lifecycle verb works with `--json` on both engines, for every state
  and for the ephemeral kind (T-170).
- `agent-box run` completes a prompt live (T-171).
- vi, less and Ctrl-C work through `agent-box attach` on both OSes (T-172).
- Profiles, `agent-box policy` and `agent-box forward` pass their CLI tests
  (T-173).
- `agent-box setup` and then `agent-box uninstall` leave no file or service
  behind, on both OSes (T-174).
- `agent-box doctor --json` gives the full startup report on both OSes
  (T-175).
- `agent-box logs` shows the event log with its agent-reported and
  host-observed labels (T-176).

NOTE: T-169 waits for the operator, who sends the requests. The operator
decides whether M7 waits for the answers.

## M8: Assurance

The claims of agent-box are measured on both engines, and the threat models
are written.

Hosts: the Linux KVM host and the Windows Hyper-V host, not nested; CI for
the fuzz targets.

Exit criteria:

- The containment battery passes on both engines. Its negative control fails
  as expected, every run (T-180). On HCS, the planned mutant is a
  NetworkAdapters entry in the document, and it is not verified.
- [status.md](status.md) has boot time, host RSS per idle instance, kage
  serve RSS and turn latency (T-181).
- Each number of T-181 gives its host, versions, sample count and date.
- [engines.md](engines.md), [broker.md](broker.md),
  [network.md](network.md) and [SECURITY.md](../SECURITY.md) hold the
  threat models (T-182).
- Each refusal of the threat models has a test (T-182).
- The fuzz targets run in CI for a fixed time (T-127, T-183).
- The confined packet parser cannot open a socket (T-127).
- A page under docs/ gives the block I/O numbers with conditions, and a
  decision on the reopening condition (T-093).
- [status.md](status.md) has the memory reclaim and fstrim numbers on
  Windows (T-110).
- A page records the result for containers in the guest, positive or
  negative (T-185).
- A battery check proves that two instances cannot reach each other through
  the netstack or the broker (T-186).

## M9: Release

Signed release artifacts exist, verified end to end on both OSes.

Hosts: CI; a fresh Linux KVM host and a fresh Windows Hyper-V host.

Exit criteria:

- A tagged release verifies with `gh attestation verify` (T-200).
- The release has binaries for Linux x86_64 and aarch64 (musl) and Windows
  x86_64, and the image digests are embedded (T-200).
- The aarch64 Linux binaries serve hosts with 4K pages only (T-200).
- The README quick start runs from a fresh clone on a fresh host of each OS,
  and the results are recorded. One of the hosts is a 4K-page aarch64 Linux
  host (T-201).
- Each release holds the third-party licence file from cargo-about (T-202).

NOTE: T-200 reads "signed" as GitHub build-provenance attestations.
Authenticode signing of the Windows binaries is not in v1, and T-201 records
whether SmartScreen or Defender blocks setup. Question Q8 in
[TODO/PROGRESS.md](../TODO/PROGRESS.md) asks the operator to confirm this
reading.

## M10: Chat gateway

A Discord thread starts an ephemeral instance, the agent works, a pull
request opens through the daemon, and the instance is deleted.

Hosts: an engine host; the test guild; a test repository.

Exit criteria:

- A message in the test channel starts an ephemeral instance (T-220).
- The chat token never enters an instance (T-220).
- Authority comes from an allowlist of platform users and roles, never from
  message text (T-221).
- A session runs, takes follow-up prompts and is decommissioned, live in the
  test guild (T-222).
- A pull request opens in a test repository. A second use of the same
  command is refused (T-223).
- The thread commands status, stop, model and usage pass their tests
  (T-224).
- Memory facts stay with their tenant and are never shared (T-225).
- A GitHub mention starts a session in a test repository (T-226).
- The file and context commands `!ls`, `!cat`, `!file`, `!pwd` and
  `!compact` pass their tests, and the host reads no guest file (T-258).

## M11: Web UI

A logged-in user watches and drives only their own sessions.

Hosts: an engine host; a browser; the test guild for the login.

Exit criteria:

- A user cannot see the session of another user (T-240).
- A user watches a session and starts one from the web, through ACP replay
  in the daemon (T-241).
- The metrics views show the broker accounting, and pass their tests
  (T-242).

## M12: Provider pool and metrics

Provider slots, a queue, backoff from broker-observed responses, and usage
and cost views exist.

Hosts: an engine host; provider API keys for the slots.

Exit criteria:

- A slot is one provider API key, and the slot tests pass (T-250).
- The queue has two caps and is bounded by length and by wait. Its tests
  pass (T-251).
- A forged retry event from the guest changes nothing (T-252).
- Usage windows, reset times, cost with its catalogue date, and the stuck
  session view pass their tests (T-253, T-254, T-255).

## backlog: not scheduled

The operator schedules each backlog entry. Until then, it stays `open`.

| Entry | What makes it worth scheduling |
| --- | --- |
| T-016 | the operator decides whether the record checker becomes a shared crate |
| T-094 | a 16K-page aarch64 host is available for a boot test |
| T-111 | HCS fails a spike gate of M0 or M4 |
| T-148 | M6 is done, and a forge credential needs a shorter life |
| T-149 | kage has a CA option, and a tool cannot take a base URL |
| T-150 | M6 is done, and bindings need a narrower life per task |
| T-184 | M8 is done, and the HCS save result of M0 is known |

## The order, and why

1. M0 comes first. The gate, the record checker and the hook exist before any
   product code, so the gate checks each change from its first commit.
2. The spikes T-014 and T-015 test engine features that no reference uses.
   They run in parallel with M1, on hosts that M1 does not need. The other
   M0 close-outs, T-007 after the push and T-013, also run beside M1.
3. T-100 starts after T-014. The pause and re-adoption semantics of T-032
   assume that T-014 passes; a fail reopens them.
4. In M1, T-033 comes first. No other M1 entry has a crate before it.
5. The P0 rules come next in M1, as early as their start conditions allow.
   T-030 and T-256 need the crates, and T-031 needs the messages of T-022.
6. The P0 rules are shape decisions. Code that runs a tool or reads a guest
   event before them gets rebuilt.
7. M2 comes before the engines, because an engine needs a kernel and an image
   to boot. The boot checks of M2 run during M3 and M4.
8. M3 comes before M4. The Linux engine runs on the operator's Windows host
   through nested KVM for development tests.
9. The Windows engine uses HCS features with no Linux-guest reference user,
   so M4 waits for the T-014 result. M4 can then run beside M3.
10. The engines come before the network. The netstack carries frames over
    vsock from a running instance; without one, only unit tests run.
11. The network comes before the broker. The guest reaches each broker
    endpoint through the netstack DNS and the gateway address.
12. T-126 is P0 and sits in M5, so no egress exists before policy and
    bindings are live in the daemon.
13. The broker comes before kage and the CLI. kage holds only nonces, so it
    reaches a provider through the broker only. The SSH agent of T-257 is
    broker work too, because it holds keys for the guest.
14. M8 comes after the full stack, because the battery tests the whole
    system on both engines. M9 comes after M8: no release before the battery
    passes.
15. The late surfaces come last (M10 to M12, ruling D8). Each one adds a way
    to reach an instance, and M10 adds mutually untrusted users.

## Known gaps

- aarch64 Linux is verified on a 4K-page host only, in T-201. T-094 covers
  16K-page hosts, and it stays in the backlog.
- T-092 and T-108 prove their probes through their module tests and the
  daemon startup report of T-028. The doctor verb comes later, in T-175.
- M1 and M2 entries carry live checks that need later milestones. The NOTE
  of each milestone gives the rule. T-088, T-089, T-105, T-106 and T-172 name
  the live checks of the entries before them.
