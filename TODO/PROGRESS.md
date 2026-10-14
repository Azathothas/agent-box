# Progress

The current state, the baseline, and the only work order. A closed entry
keeps its proof in the entry. This page keeps no history; git does.

## State

- The planning session of 2026-10-14 rewrote the plan. agent-box is now one
  Rust CLI that manages persistent Linux microVMs on Windows and Linux hosts.
- The engines are native Hyper-V through HCS on Windows, and Firecracker
  with its jailer on Linux. kage runs as the harness in each instance.
- `docs/decisions.md` records the rulings of that session, D1 to D14, and the
  decisions P1 to P6 of the planning session. Q7 asks the operator to
  confirm P5.
- The same session built the tooling: the `cargo xtask` commands and the
  record checker `cargo todo`. T-007 waits for its first observed CI run.
- No product code exists. There is no crates/ directory: T-033 creates the
  product crates as the first step of M1 (ruling D14).
- The references branch is dropped (ruling D9). The pins table of
  `docs/references.md` and `cargo xtask refs fetch` replace it.
- The spikes T-014 and T-015 have not run.
- The kage requests of `TODO/upstream.md` are not sent (T-169).

## Baseline

Measure again at the start of each session. Do not trust a recorded result.
This list is the one home of the baseline commands. AGENTS.md and
`TODO/RULES.md` point here.

- `docs/status.md` holds what the planning session measured, with the date,
  the host and the conditions. Its Gate table gives the result of each gate
  step on each host. Compare your results with it.

```sh
cargo xtask doctor
cargo xtask gate --strict
cargo todo check
cargo xtask remote
cargo xtask linux -- cargo xtask gate fmt clippy test
```

- The Linux runner needs wsl-toolkit on Windows, or podman or docker on
  Linux. Without them, record the step as not run, with the reason.
- The runner copy has no .git/ and no cargo-deny, so it runs the cargo steps
  of the gate only.
- `cargo xtask remote` and the GitHub probe of `doctor` need `gh` with a
  login. Without it they report "could not run", which is not a failure of
  the tree.
- CI runs the full strict gate on Linux and Windows.

The commands are POSIX sh. On Windows, run them in Git Bash, or run each one in
PowerShell and read `$LASTEXITCODE`.

## Counts

`TODO/INDEX.md` holds 136 entries: 123 open, 1 partial, 1 blocked, 11 done.

Open includes the backlog. `cargo todo check` verifies this line.

## Work order

Finish each entry, and close it in place. Update this page in the same
commit, then take the next entry. The order below keeps the start condition
of each entry.

First, T-007, the only `partial` entry. Its last step needs minutes: read the
first CI run of `main` with the command below. If it passed, record its URL and
result in the entry, and close it. If it failed, the failure is your first
task.

```sh
gh run list --workflow ci.yml --branch main --limit 1
```

Without `gh`, read the Actions page of the repository on GitHub. Then start M1.

The other M0 close-outs can run alongside M1, in other sessions, on the hosts
that they need:

- T-013: re-pin the references at current upstream commits, and re-check the
  kept claims.
- T-014: the HCS spike, on a Windows Hyper-V host as administrator.
- T-015: the Firecracker vsock latency spike, on a Linux KVM host.

M1, in this order:

1. T-033: create the product crates. Every other M1 entry starts after it.
   T-259 (the unsafe check) can come before or after it; T-260 (the musl
   build in the gate) comes right after it.
2. T-030: the P0 rule that the host never runs a tool against guest-written
   bytes.
3. T-031: the P0 rule of the trusted event channel. Its start condition is
   T-022, so take T-021 and T-022 first, as part of this step.
4. T-256: the P0 rule that no secret enters the guest image, a disk, the
   kernel command line, MMDS or the VMM argv.
5. The rest of T-020 to T-029, in id order.
6. T-032: the Engine trait and the test double.
7. T-040, T-041 and T-042: box-guest against the test double.

Then by milestone. In each milestone, take the entries in this order:

- M2: T-060, T-062, T-063, T-066, T-067, T-068, T-043, T-044, then T-069,
  T-065, T-064. The boot checks of T-043, T-044, T-062, T-063 and T-064 run
  in T-088 and T-105. T-061 and T-065 wait for both.
- M3: T-081, T-080, T-083, T-085, T-084, T-082, T-087, T-092, T-086, T-088,
  T-089, T-090, T-091.
- M4: T-100, T-101, T-102, T-103, T-104, T-108, T-105, T-106, T-107, then
  T-109. T-109 can run on a Home host at any time after T-014.
- M5: T-120, T-045, T-121, T-126, T-122, T-123, T-124, T-125.
- M6: T-140, T-141, T-142, T-144, T-143, T-146, T-147, then T-145, T-257.
- M7: T-160, T-162, T-173, T-175, T-176, T-046, T-161, T-170, T-171, T-172,
  T-174, then T-163.
- M8: T-180, T-181, T-182, then T-110, T-186, T-093, T-127, T-183, T-185.
- M9: T-200, T-201, then T-202.
- M10: T-220, T-221, T-222, T-223, then T-224, T-258, T-226, T-225.
- M11: T-240, T-241, then T-242.
- M12: T-250, T-251, T-252, then T-253, T-254, T-255.

Skip T-169 while it is `blocked`: the operator sends the requests and relays
the answers. Take a backlog entry (T-016, T-094, T-111, T-148, T-149, T-150,
T-184) only when the operator schedules it.

## Questions for the operator

A question has a number, the entries that it concerns, and a recommendation.
The session takes the next entry while it waits. Nothing closes as out of
scope until the operator rules, and each ruling goes into
`docs/decisions.md`.

Q1 (2026-10-14, T-140): which credential store on Linux? The choice is a key
file of mode 0600 in the state directory of the daemon, or the Secret Service.

- Recommendation: the key file, with the Secret Service as a later option.
- Reason: the daemon runs unattended as a user service. The Secret Service
  needs a D-Bus session and an unlocked collection, which such a service may
  not have.
- Both protect a value from other users only. Neither protects it from root
  or from the operator's own uid; DPAPI on Windows is the same.

Q2 (2026-10-14, T-162): MCP OAuth in the guest is refused in v1, or brokered.

- Recommendation: refuse it in v1. The image has no MCP OAuth login, and an
  MCP server with an API key goes through a broker binding.
- Reason: `kage mcp login` stores real access and refresh tokens in the
  guest, and kage refreshes them itself. That breaks R-NO-SECRET-IN-GUEST.
- Brokering MCP OAuth is new work. It can be a later entry, after M6.

Q3 (2026-10-14, T-069): who owns the kernel and image rebuilds, and on which
cadence.

- Recommendation: the operator is the named owner. A scheduled image
  workflow (T-068) builds new candidates, and each change of a digest is a
  pins change that the owner approves.
- A security fix in a package of the image, or in the kernel, starts a
  rebuild out of schedule.

Q4 (2026-10-14, T-016): the record checker becomes one crate shared with
podssh, or stays a copy in each repository.

- Recommendation: keep the copy (planning decision P2), and compare the two
  copies when a later session changes either one.
- Reason: a shared crate adds a dependency between two repositories. A drift
  is visible in a diff of the two module trees.

Q5 (2026-10-14, T-169): which kage requests of `TODO/upstream.md` to send,
and whether M7 waits for the answers.

- Recommendation: send K3, K7, K5, K4 and K6 now, in one message. Hold K1
  and K2 until T-149 or T-162 needs them.
- Reason: none of the requests blocks a milestone; each one has a workaround.
  K1 and K2 matter only for designs that v1 does not use.
- Recommendation: M7 does not wait for the answers. T-169 stays `blocked`
  until each answer is recorded.

Q6 (2026-10-14, T-109): Windows 11 Home is a target, or not.

- Recommendation: measure Home with T-109, but name only Pro, Enterprise and
  Education in the release until the Home spike passes.
- Reason: there is no evidence either way for a third-party HCS caller on
  Home. If the spike fails, T-111 decides the Home route.

Q7 (2026-10-14, T-184): confirm planning decision P5 of `docs/decisions.md`.

- P5: pause is an in-memory vCPU freeze, and stop is a guest poweroff.
  `hibernate` is a later opt-in verb (T-184).
- Recommendation: confirm P5. A pause is lost when the VMM or the host goes
  away, and the instance then shows `stopped`.
- Reason: a snapshot holds guest memory and every provisioned nonce. A
  snapshot is insecure when it resumes twice, so it needs the single-use
  rules of T-184.

Q8 (2026-10-14, T-200): "signed" in the M9 exit criterion means attested.

- Recommendation: accept GitHub build-provenance attestations as the
  signature of v1. Leave Authenticode signing of the Windows binaries out of
  v1.
- Reason: `gh attestation verify` checks the repository and the workflow of
  each asset. T-201 records whether SmartScreen or Defender blocks an
  attested binary that has no Authenticode signature.

Q9 (2026-10-14, T-202): the GPL sources of the images.

- Question: is a pointer to the kernel and Debian sources enough, or must
  the images release carry the kernel source tarball?
- Recommendation: carry the kernel tarball and its config as assets of the
  images release. snapshot.debian.org keeps the Debian sources at the pinned
  timestamp.

Q10 (2026-10-14, T-123): two names on one address.

- Question: in `allowlist` mode, an allowed name opens its address to every
  name hosted there, for example on a CDN.
- Recommendation: read the ClientHello of each TCP flow to port 443. Refuse
  the flow when its SNI is absent, or is not an allowed name answered with
  that address.
- Until the operator rules, `docs/limits.md` states the gap.

## Operator actions

- Send the kage requests that Q5 selects, and relay each answer (T-169).
- Give a Linux KVM host, not nested, for T-015 and for M3.
- Run the HCS spike T-014 as administrator, or allow a session to run it.
- Answer Q1 to Q10.

## Drafts

No draft waits to be merged. A draft of an entry gets one row here until the
main session merges it, builds it, runs its tests and closes the entry.

| Entry | Branch | Commit | State |
| --- | --- | --- | --- |
