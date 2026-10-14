# Assurance

This area proves the claims of the other areas from outside. It holds the
containment battery and its negative control, the footprint and latency
numbers, the threat models, and fuzzing. It also holds the measurements that
decide `hibernate` and containers in the guest.

Results go to [docs/status.md](../docs/status.md) with the host, versions,
date and conditions. [SECURITY.md](../SECURITY.md) and
[docs/limits.md](../docs/limits.md) state what the results prove and what
they do not prove. The checks are adapted from the research document,
sections 9 to 12 ([docs/research/microvm-tier.md](../docs/research/microvm-tier.md)).

# T-180: The containment battery and its negative control

**Source:** brief sections 3, 6 and 8; the research document, sections 9, 9.1 and 10; relevance-pass findings cited below.
**Category:** measurement
**Milestone:** M8
**Priority:** P1
**Effort:** L
**Status:** open

## Problem

Every isolation claim in the docs is design until something tries to break
it from outside. A broken probe reads as isolation: a battery that never
failed was never tested. The battery must run against both engines, and it
must fail on a planted defect every time it runs.

## Premise

- READ (research section 9): twelve checks against a live daemon. Each is a
  command with an expected exit code and a recorded output. Each names the
  defect that would make it pass wrongly.
- READ (research section 9.1): the named mutant adds `network-interfaces`
  with a loopback TAP to the VM configuration. Checks 1 and 7 are expected to
  fail. The mutant runs on every invocation, not once.
- Changes since the research (design, this brief):
  - The guest now has a NIC: `tun0`, carried over vsock port 5003 into the
    host netstack (D5). Host reachability now tests the netstack, the gateway
    and every host address.
  - The ports moved. Events and the event nonce use port 5002, not 5003
    (brief section 7).
  - A daemon restart re-adopts running instances (brief section 4.1). The
    orphan check becomes a re-adoption check with one planted orphan.
  - There are two engines. Every check needs an HCS form.
- Analysis of the named mutant in this topology (design, not measured):
  - The guest kernel of T-060 has no virtio_net and no hv_netvsc driver
    (brief section 5). The guest cannot use a NIC that the mutant adds. So the
    mutant run needs a battery kernel with that driver.
  - The guest reaches the broker only through the netstack and its own vsock
    channel. A TAP does not reach the broker. So `egress-identity` can pass
    under the mutant. The first live run measures it.
- READ: a reachability check with no positive control, which never probed the
  host's primary address, does not prove that the host is unreachable
  (`talaria0101/malaria@499e897e8372a5db0f921542184e3b66ef610e2b:experiments/windows/battery.ps1:178-240`).
- READ: assert that the credential was injected, not that the agent
  succeeded (agent-vault issue 194,
  https://github.com/Infisical/agent-vault/issues/194).
- READ: an experiment pins every input, prints its conditions, exits 0, 1 or
  2, keeps negative results, and takes `--expect`
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/methodology/experiments.md:13-133`).
- READ: the default seccomp filters of Firecracker set `default_action: trap`
  for the vmm, api and vcpu threads
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:resources/seccomp/x86_64-unknown-linux-musl.json:2-4`).
- READ: the same holds at `:808-810` (api) and `:1107-1109` (vcpu) of that
  file. No thread's allowlist names `clone`, `clone3`, `unshare` or
  `pivot_root`, on x86_64 or aarch64.
- READ: the release archive carries these filter files
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/seccomp.md:35-37`).
  Debug binaries install no default filters (same file, lines 14-17).
- READ (by the engine sweep, 2026-10-14): each VM's vmwp.exe runs under its
  own virtual account, with ACG and CIG on and SeImpersonatePrivilege
  removed.
- The sources are
  https://learn.microsoft.com/previous-versions/windows/it-pro/windows-server-2012-R2-and-2012/dn741279(v=ws.11)
  and https://www.microsoft.com/en-us/msrc/blog/2019/09/attacking-the-vm-worker-process.
  That this holds for HCS compute systems on Windows 11 is not tested.
- READ: pasta forwards a namespace's 127.0.0.0/8 to the host loopback
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:crates/bailey/src/cli.rs:456-475`).
  A userspace netstack can make the same mistake.

## Approach

Tooling:

1. Write `cargo xtask battery` in xtask/src/battery.rs and xtask/src/battery/
   (planned files). It keeps the xtask contract: exit 0 pass, 1 fail, 2 could
   not run, 64 usage error, and `--json`.
2. Write the guest probe as a new tools package, xtask/battery-probe: static
   musl, not shipped, with no workspace dependency. Add it to the layer table
   of T-002.
3. The probe builds a well-formed event frame from the golden frames of
   T-021, read at build time.
4. The battery sends the probe binary into the guest as stdin of an exec on
   port 5001, then runs it. Nothing flows from the guest to the host except
   probe output.
5. Probe output is guest data. The battery reads it as bounded JSON. Each
   decision also needs a host-side observer: a listener, the receiver, a
   broker counter or the process table.
6. The battery reads a data disk file only as raw bytes for a substring
   search. It never mounts or parses the disk. It is a test tool, not product
   code (R-HOST-NO-TOOLS).
7. The battery-probe package also builds `battery-receiver`. It runs on a
   host that the operator controls, outside the battery host, with a DNS
   name and a publicly trusted certificate.
8. The receiver records TCP, UDP and HTTPS arrivals per run token. It answers
   the battery's queries, and it returns a credential only as its sha256.
9. The configuration file .work/battery/battery.toml names the receiver, the
   engine and the run count. It holds no credential. The battery makes a
   random credential per run.
10. One run builds the normal tree and the mutant trees. It runs the normal
    pass and each mutant pass, compares them with the expectations, and
    writes the results.
11. A pass installs the tree with its own `setup --system` and `setup`
    (T-174), runs the checks, and deletes every instance. Then it runs
    `uninstall --purge` and `uninstall --system`, and the uninstall result is
    part of the run result.

Checks (stable names; the number is the research's check number):

12. `host-reach` (check 1):
    - Does: open TCP and UDP listeners on one port on every host address,
      found with getifaddrs or GetAdaptersAddresses. vEthernet adapters,
      0.0.0.0, `::` and loopback are included.
    - Does: the probe, as `agent` and as root, connects to each address, to
      the gateway 10.200.0.1 on the same port, and to 169.254.169.254.
    - Does: the probe resolves the host name, `host.containers.internal` and
      `host.docker.internal`. It runs in mode `deny`, `allowlist` and `open`.
    - Pass: no listener records an arrival, each name gets NXDOMAIN, and the
      daemon log has a `denied` record for each attempt.
    - Positive control: a loopback client reaches every listener before the
      probe runs. A mutant pass must reach a listener (step 21).
    - Passes wrongly if: a listener bound a wrong address, or a host firewall
      drops the probe while a real path exists. The mutant pass catches the
      second case.
13. `broker` (check 2):
    - Does: store a random credential with `agent-box secret set battery`
      from stdin. Bind `battery` to the receiver's HTTPS port, path scope
      `/echo/`, methods GET and POST, bearer header.
    - Does: the probe sends a request to
      `http://battery.broker.internal/echo/TOKEN` with the binding nonce in
      the header.
    - Pass: the receiver got one Authorization header, and its sha256 equals
      the sha256 of the planted credential.
    - Pass: the credential bytes are absent from /proc/*/environ,
      /proc/*/cmdline, /proc/cmdline and every guest file (probe as root).
    - Pass: the credential bytes are absent from the VMM argv, the VM
      configuration and the data disk file (host side).
    - Pass (T-256): the probe, as root, reads the binding nonce and the kage
      token in the guest. Neither value is in /proc/cmdline, the VMM argv or
      the VM configuration.
    - Passes wrongly if: the guest reaches the receiver directly with no
      injection. The hash comparison catches it, and `host-reach` covers
      the direct path.
14. `egress` (check 3):
    - Does: the probe sends UDP and TCP to the receiver on a port with no
      rule. It does this in mode `deny`, and then in `allowlist` with one rule
      for the receiver's name and one TCP port.
    - Does: the probe queries DNS for a random name in a domain that the
      policy does not list, and for an HTTPS record (type 65).
    - Pass: the receiver records no arrival of the run token on a port with
      no rule. The random name gets NXDOMAIN, and the daemon makes no upstream
      lookup for it. The type 65 query is refused.
    - Positive control: in `allowlist`, a TCP connect to the ruled port
      reaches the receiver with the run token.
    - Passes wrongly if: there is no route at all. The positive control
      catches it.
15. `vmm-confinement` (check 4):
    - Linux does: read the seccomp JSON of the installed release for the host
      architecture. Its architecture must match `uname -m`.
    - Linux pass: `clone`, `clone3`, `unshare` and `pivot_root` are absent
      from every thread's allowlist, and the default action is `trap`.
    - Linux pass: every thread in /proc/PID/task/*/status of the VMM shows
      `Seccomp: 2`. The VMM argv has no `--no-seccomp` and no
      `--seccomp-filter`.
    - Windows does: find the vmwp.exe of the instance's VmId and read its
      token and its mitigation policies (GetProcessMitigationPolicy).
    - Windows pass: the token user is the NT VIRTUAL MACHINE SID of that
      VmId. Dynamic code is prohibited (ACG), and only Microsoft-signed
      binaries load (CIG). SeImpersonatePrivilege is absent.
    - If Windows refuses a query to an admin, the check exits 2 and names
      the query.
    - Passes wrongly if: the filter file of the wrong architecture is
      parsed. The architecture match catches it.
16. `jail` (check 5):
    - Linux does: read the VMM pid from the jail's pid file. Check first that
      the pid is alive and that the instance is `running`.
    - Linux pass: /proc/PID/root resolves inside the jail. The uid and gid
      equal the instance's allocated pair. The cgroup path is the configured
      root plus the id, and memory.max is set.
    - Windows does: read the compute system document with
      HcsGetComputeSystemProperties, as admin.
    - Windows pass: no NetworkAdapters, no Plan9 and no VirtualSmb. HvSocket
      carries the expected SDDL. The VHDX ACLs grant nothing to Users or
      Everyone.
    - Passes wrongly if: the VMM is not running, so a /proc lookup fails
      instead of failing correctly. The liveness check runs first.
17. `trusted-channel` (check 6):
    - Does: the probe runs as `agent` (uid 1000), as its own process, not
      inside box-guest. It connects to host CID 2 on port 5002.
    - Does: it writes a well-formed event frame with no nonce. Then it writes
      one with a wrong nonce of the right length.
    - Pass: the daemon closes each connection before it parses an event. The
      refused-forgery counter rises by 2.
    - Positive control: box-guest heartbeats in the same time window are
      accepted.
    - Passes wrongly if: the forgery comes from inside box-guest, which holds
      the nonce. The probe runs at the agent's uid instead.
18. `egress-identity` (check 7):
    - Does: run instances A and B, both with the binding `battery`. The probe
      in B, as `agent`, reads B's binding nonce through the delivery path of
      T-141.
    - Does: the probe in A sends a request with B's nonce, then one with A's
      own nonce.
    - Pass: the broker refuses B's nonce from A, the receiver records nothing
      for it, and B's usage counters do not change.
    - Pass: A's own nonce reaches the receiver, and the broker counts the
      request for A only.
    - Passes wrongly if: only one instance runs, which cannot tell per-instance
      nonces from one global nonce.
19. `orphans` (check 8, re-adoption form):
    - Does: run four instances. Kill the daemon (SIGKILL, or TerminateProcess
      on Windows). Move the fourth record out of the state directory. Start
      the daemon.
    - Pass: the daemon re-adopts exactly three instances, each `running`, and
      `attach --shell` works on each.
    - Pass: the daemon removes the fourth VMM only after it verifies the pid
      (T-090) or the owner tag (T-107).
    - Pass on Linux: the fourth jail, pid file, cgroup, disks and uid are
      each absent. On Windows: the compute system, the files and the grants
      are each absent.
    - Passes wrongly if: the check lists the orphan and does not remove it, or
      the cgroup goes while the directory stays. Each absence is checked on
      its own.
20. `teardown` (check 9):
    - Does: make a random 32-byte marker. Send it over exec stdin. The probe
      writes it into files in /home/agent and /var/tmp, then runs `sync`.
    - Positive control: before the delete, a raw search of the data disk file
      finds the marker.
    - Pass: after `agent-box delete`, a raw search of every agent-box
      directory on the host finds no marker.
    - The directories: the daemon state directory, the store, the jail base
      and the helper state.
    - The one named exception: the memory facts of the chat gateway (T-225).
      They live outside the instance directories and survive a delete on
      purpose. The search skips that one store file and names it.
    - Passes wrongly if: a path check passes while the bytes stay in a file
      elsewhere. The search reads content.
21. `negative-control` (check 12):
    - Mutant A (named by the research, adapted): the VM configuration gains
      one NIC. Linux: a `network-interfaces` entry with a host TAP. Windows:
      one NetworkAdapters entry on an HCN endpoint.
    - The Windows form of mutant A is the planned HCS negative control. Its
      effect on `host-reach` and `egress-identity` is not verified yet.
    - Mutant A boots the battery kernel with virtio_net or hv_netvsc. The
      probe gives the NIC an address in 198.18.255.0/30 (the RFC 2544
      range).
    - Mutant B (new in this topology): the netstack maps flows for the
      gateway address onto the host loopback. This is the defect class that
      pasta shows.
    - Pass of the control: each mutant pass exits non-zero, and `host-reach`
      is among its failed checks. Each mutant pass is also the non-local
      positive control for the listeners.
    - The results list every failed check of each mutant pass. Whether
      `egress-identity` fails under mutant A is measured, not assumed.
    - Passes wrongly if: a mutant pass exits 2. Could not run is a different
      fact from failed, and the battery then exits 2.
22. `guest-containers` (check 10): run `hello-world` with podman or docker in
    the guest when a runtime is present. Record the result (absent, works,
    or fails with output) either way. T-185 owns the decision.
23. `provenance` (check 11): record the host OS and kernel, the CPU model,
    whether the host is itself a VM, and the engine and its version.
24. `provenance` also records the HCS schema version (Windows), and the
    guest kernel, base image and kage digests. It records the disk settings,
    the agent-box commit, the date and the run count.
25. A missing provenance field makes the run exit 2.
26. T-186 adds check 13, `instance-isolation`.

Mutant builds:

27. Add a cargo feature `negative-control` to box-vmm and box-net, passed
    through by box-helper and box-cli.
28. With the feature on and debug_assertions off, a `compile_error!` stops
    the build. So no release profile can carry a mutant.
29. The feature adds the text `negative-control` to `--version`. `doctor`
    reports a mutant build as a gap.
30. The release workflow (T-200) refuses a build with the feature.
31. The battery kernel is the T-060 configuration plus one fragment
    (CONFIG_VIRTIO_NET or CONFIG_HYPERV_NET). The image tooling builds it once
    and caches it by digest.
32. The normal pass never boots the battery kernel.
33. Host side of mutant A on Linux: the battery creates a TAP with
    198.18.255.1/30, and removes it after the pass.
34. Host side of mutant A on Windows: the battery creates an HCN NAT network
    and one endpoint, and removes both after the pass.

Privilege, results and hosts:

35. The battery needs root (Linux, through `sudo -n`) or an admin token
    (Windows) for setup, uninstall and the mutant network. Without it, the
    battery exits 2.
36. The daemon always runs as the operator, unprivileged. On Windows, how the
    battery starts the daemon with a standard token is open. This entry
    decides it.
37. Write .work/battery/RUN/results.json and summary.md. Never delete them.
38. A published result needs three complete runs per engine (design count).
    docs/status.md gets the summary with host, date, versions and the sha256
    of each results file.
39. The results files of a published run go into the release assets (T-200).
40. `--expect FILE` compares a run with a committed expectation and exits 1
    on a difference. The battery is then also a regression check.
41. `--check NAME` runs a subset for development. Such a run is labelled
    partial and is never published. The mutant passes still run.
42. A published result needs a bare-metal Linux KVM host, or Windows 11 with
    Hyper-V on hardware. A run under `cargo xtask linux --kvm` is nested and
    is for development only.

## Decision

Run mutant B in each invocation, beside the named mutant A. Recommendation:
yes. Mutant A adds a second path past the netstack. So it cannot show that
`host-reach` detects a path through the netstack, which is the path that this
design adds. Mutant B is also cheaper: one build flag, no kernel, no TAP. The
operator may override this.

## Prove

```sh
cargo test -p xtask battery::
cargo xtask battery --json
cargo xtask battery --json --expect xtask/battery/expect-firecracker.json
```

The unit tests prove each check's decision on planted inputs. Planted inputs
include an arrival at a listener, a filter file of the wrong architecture,
and a mutant pass that exits 2.

On a Linux KVM host, the normal pass passes. Each mutant pass exits 1 with
`host-reach` among its failures. The battery exits 0. The same holds on
Windows 11 with Hyper-V as admin, with the HCS expectation file.

Host: a bare-metal Linux KVM host with sudo, and Windows 11 Pro with Hyper-V
on hardware, as admin. Both need an operator-controlled receiver host.

## Start condition

T-031, T-090, T-091, T-107, T-123, T-126, T-143, T-174 and T-256.

# T-181: Footprint and latency

**Source:** brief section 1 (the operator's goals); the research document, section 2; Firecracker SPECIFICATION.md; bailey and TEMPLATE method rules.
**Category:** measurement
**Milestone:** M8
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The operator's goals after isolation are performance and a small footprint
(brief section 1). This project has measured none of its own figures.
Upstream figures use other guests and other conditions. Without numbers, a
claim of low overhead is a guess.

## Premise

- READ: Firecracker states at most 125 ms from InstanceStart to the guest's
  /sbin/init and at most 5 MiB of VMM memory overhead
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:SPECIFICATION.md:24-42`).
- Those figures hold for 1 vCPU, 128 MiB, a minimal kernel and rootfs, and the
  serial console off. The guest of this project is larger, so they are not
  comparable.
- READ (engine evaluation, 2026-10-14): the per-VM host cost of vmwp.exe and
  vmmem under HCS is not published.
- READ: interleave the cases, one of each per round, with the first case
  rotated. A sequential timing made the more confined case look faster
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:docs/reference/benchmarks.md:11-20`).
- READ: a VMM's RSS includes the guest kernel and the workload heap, so only
  like figures compare
  (`kubernetes-sigs/agent-sandbox@4b63868f0d67e4ee0fa690bb14cb5722b78d679d:examples/gke-swap/runtimes/README.md:55-64`).
- READ: run a control twice before you publish a cause, and keep negative
  results
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/methodology/experiments.md:13-133`).
- Not measured by this project: every figure that this entry produces.

## Approach

1. Write `cargo xtask measure boot|footprint|turn` in xtask/src/measure.rs
   (planned). It keeps the battery's contract: exit 0, 1 or 2, `--json`, and
   conditions printed first.
2. Keep every result in .work/measure/RUN/.
3. `boot`: time from the daemon's `start` call to the port 5000 handshake,
   on the host monotonic clock, from host-observed events.
4. Split `boot` into helper start, VMM start and guest handshake. box-guest's
   own boot time is guest-reported and is labelled so.
5. `footprint`: measure 60 s after the handshake, with `kage serve` running
   and no session.
6. Linux footprint: Rss and Pss of the VMM from /proc/PID/smaps_rollup.
   Report the guest memory size and the balloon state beside them, because
   guest pages count in the VMM's RSS.
7. Windows footprint: the working set and private bytes of the instance's
   vmwp.exe and of its vmmem process.
8. Both OSes: the host size and the allocated size of the data disk file,
   after create and after one cargo build in the guest.
9. kage serve RSS: VmRSS of kage from box-guest stats on port 5000. The
   table labels it guest-reported.
10. Footprint control: measure Firecracker memory hotplug (virtio-mem) beside
    the balloon. Record the host RSS of an idle instance after it gives memory
    back, with each method.
11. `turn`: wall time from `agent-box run` to the first agent text and to
    the `end` line (T-171). The provider is a fake on the battery receiver with a fixed
    SSE response.
12. Baseline for `turn`: kage on the host against the same fake endpoint. The
    difference is the cost of agent-box.
13. Interleave the cases per round and rotate the first case. Take at least
    20 samples per figure (design count). Report the count, min, median, p90
    and max.
14. Print the host conditions with each figure: CPU model, cores, RAM, OS and
    kernel, power plan, and nested or not.
15. Print the run conditions with each figure: engine version, guest digests,
    instance cpus and memory, and the date.
16. Write the tables into docs/status.md. Label a nested run as development
    only. Do not publish a number without its conditions.
17. Before a cause for a difference is written, run both cases again on the
    same day.

## Prove

```sh
cargo xtask measure boot --samples 20 --json
cargo xtask measure footprint --json
cargo xtask measure turn --samples 20 --json
```

Each command exits 0 and prints its conditions and figures. docs/status.md
holds the tables with host, versions, sample count and date.

Host: a bare-metal Linux KVM host, and Windows 11 with Hyper-V on hardware as
admin (for the vmmem query). `turn` also needs the receiver host of T-180.

## Start condition

T-171 and T-180.

# T-182: Threat models: helper, broker, netstack

**Source:** brief sections 3, 4.2, 6.2 and 6.3; the research document, section 5.6; bailey's helper rules.
**Category:** docs
**Milestone:** M8
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The helper runs as root or LocalSystem. The broker holds every credential.
The netstack parses packets from the guest. Each needs a written model whose
refusals each have a test. The docs must also state that the isolation is
not the same on the two OSes.

## Premise

- Design (brief section 3): the trust table and the rules R-HOST-NO-TOOLS,
  R-TRUSTED-CHANNEL, R-NO-SECRET-IN-GUEST and R-DENY-FIRST.
- Design (brief section 4.2): under HCS the guest reaches root-partition
  kernel code (the VMBus root side, storvsp, vhdmp, the hvsocket provider)
  and the per-VM vmwp.exe.
- Under Firecracker the guest reaches KVM in the host kernel. virtio-blk and
  vsock run in the jailed user-mode VMM.
- READ: the Firecracker VMM thread emulates the net, block and vsock devices
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/design.md:71-79`).
- READ (research section 5.6): the helper's input provenance table and its
  capability lifecycle.
- READ: bailey keeps all privilege in a small helper with a fixed operation
  set and a versioned protocol greeting
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:docs/security/privilege.md:26-48`).
- READ: a public channel with untrusted input, private data and an
  exfiltration channel is the "lethal trifecta"
  (https://github.com/withastro/flue/issues/51). Brokering removes credential
  theft, not exfiltration.
- T-080 writes the helper's model in M3. This entry checks it against the M8
  evidence and adds the broker and netstack models.

## Approach

1. Give each model one shape: assets, entry points, an input provenance
   table (field, origin, validation), a refusal table (condition, test name)
   and residual risks.
2. Write the residual risks in the words of docs/limits.md.
3. Helper model, in docs/engines.md: start from T-080. Add the evidence of
   T-180 (`jail`, `vmm-confinement`, `orphans`) and of T-174 (the install
   footprint).
4. Broker model, in docs/broker.md: entry points are guest HTTP on the
   gateway, the credential store, upstream TLS and the control API.
5. Broker refusals: the eight gate conditions of T-143, one row each, and the
   two fail-closed shapes.
6. Broker residual risk: exfiltration through an allowed binding. The broker
   does not stop it.
7. Netstack model, in docs/network.md: entry points are frames on port 5003,
   DNS queries and forwards.
8. Netstack refusals: those of T-122 to T-126, the confinement of T-127, and
   the rule of no implicit host mapping (T-180, mutant B).
9. SECURITY.md: a summary of the three models, the reporting route, and what
   is a boundary and what is not.
10. SECURITY.md states the isolation difference between the OSes in two or
    three sentences. docs/limits.md and the startup report (T-175) use the
    same words.
11. Use one mechanism for this entry and T-080: a check in xtask/src/check/
    reads each refusal table and finds the named test function in the
    source.
12. If T-080 has not added that check, add it here as `cargo xtask check
    refusals`, and add it to `cargo xtask gate`.

## Prove

```sh
cargo xtask check refusals --json
cargo xtask check docs prose
```

The refusal check finds every named test of the three models. The docs and
prose checks pass. docs/decisions.md records the operator's review with its
date.

Host: any host.

## Start condition

T-080, T-123, T-143 and T-180.

# T-183: Fuzzing

**Source:** brief section 3 (the trust table); brief section 6.2 (fuzz the parser); overlap with T-127 noted below.
**Category:** feature
**Milestone:** M8
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

Every byte from the guest is untrusted. The first code that an attacker in
the guest reaches is a parser: frames, the HTTP head, DNS, packets, ACP
messages and git ref updates. Unit tests cover the cases that the authors
thought of. A fuzzer finds the others.

## Premise

- Design (brief section 3): everything inside the guest is untrusted,
  including kage's output and the ACP stream.
- Design (T-127): T-127 already plans cargo-fuzz targets for frames, IP, TCP
  and DNS, with the confinement of the packet parser.
- Division of work (design): this entry owns the fuzz layout, the CI job and
  the targets outside box-net. T-127 owns the box-net targets. One CI job runs
  all of them.
- cargo-fuzz needs a nightly toolchain and libFuzzer. The workspace pins a
  stable toolchain in `rust-toolchain.toml`.

## Approach

1. Give each fuzzed crate its own fuzz directory, as cargo-fuzz lays it out:
   crates/box-proto/fuzz/, crates/box-broker/fuzz/, crates/box-daemon/fuzz/.
   Exclude each from the workspace.
2. box-proto targets: `frame_decode` (ceilings, truncation),
   `message_decode` (one per port, 5000 to 5005) and `helper_decode` (helper
   protocol requests).
3. box-broker targets: `http_head` (the parser of T-142),
   `receive_pack_commands` (the ref updates of T-146) and `usage_parse` (the
   provider responses of T-147).
4. box-daemon target: `acp_message` (ACP messages from kage, T-161).
5. box-net targets: those of T-127, in the same job.
6. Each target asserts no panic. Run each with `-malloc_limit_mb=64`, so that
   an allocation far above MAX_FRAME (8 MiB) fails the run.
7. Seed corpora: the golden frames of T-021, the HTTP heads of the T-142
   tests, and the recorded provider responses of T-147.
8. CI: .github/workflows/fuzz.yml on a Linux runner with a nightly pinned by
   date.
9. Each target runs 60 s on every push to main, and 1800 s in a weekly
   scheduled run (design values). A crash uploads the input and fails the
   job.
10. Before a fix merges, the crash input becomes a regression test in the
    crate's normal tests.
11. On a Windows host, run the fuzzers through `cargo xtask linux`.

## Prove

```sh
cargo +nightly fuzz run frame_decode -- -max_total_time=60 -malloc_limit_mb=64
cargo +nightly fuzz run http_head -- -max_total_time=60 -malloc_limit_mb=64
```

Each target runs for the fixed time with no crash. The CI job is green and
lists each target with its run time.

Host: any Linux host, or `cargo xtask linux` on Windows.

## Start condition

T-021, T-142, T-146, T-147 and T-161.

# T-184: `hibernate`

**Source:** brief sections 4.1 and 4.2 (hibernate rules); Firecracker snapshot documentation; the research document, section 5.7.
**Category:** feature
**Milestone:** backlog
**Priority:** P3
**Effort:** L
**Status:** open

## Problem

Pause keeps the guest in RAM. The pause is lost when the VMM or the host goes
away (brief section 4.1). Long agent work may need a state that survives a
host reboot. A snapshot holds guest memory, and a state that resumes twice
is insecure.

## Premise

- READ: Firecracker considers it insecure to resume the same state more than
  once. Identifiers, random seeds, the entropy pool and crypto tokens stop
  being unique
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/snapshotting/snapshot-support.md:540-549`).
- READ: vsock connections that are open at snapshot time are closed. Guest
  listen sockets stay active (same file, lines 56-61).
- READ: Firecracker always enables a VMGenID device. On restore it changes
  the generation id and notifies the guest. Linux 5.18 and later reseed the
  PRNG from it (same file, lines 598-614).
- READ: diff snapshots are in developer preview (same file, line 117).
- Design (planning decision P5): pause is an in-memory vCPU freeze, stop is a
  guest poweroff, and `hibernate` is this later verb. Question Q7 asks the
  operator to confirm it.
- Design (brief section 4.1): `hibernate` is a later, opt-in verb. It uses a
  Full snapshot only, single use (consumed before resume).
- Design (brief section 4.1): a cold boot of the disk invalidates the
  snapshot. A version, kernel and CPU manifest check falls back to cold boot.
- Design (brief section 4.1): files outside the jail are root-owned, mode
  0600, with a verified digest. Guest S4 hibernation is an engine-neutral
  alternative to measure.
- Design (brief section 4.2): HCS save to file is not verified for
  LinuxKernelDirect. T-014 item (d) tests HcsSave plus RestoreState.
- Design: a snapshot holds every nonce and token that box-guest keeps in
  memory. They are not credentials, but they authorize the instance.

## Approach

1. Verb: `agent-box hibernate NAME`, from `running` or `paused`. `start NAME`
   restores when a valid snapshot exists.
2. Add the state `hibernated` to the State enum (T-020) and to docs/cli.md.
   The brief's state list does not have it yet.
3. Firecracker: pause, then `PUT /snapshot/create` with `snapshot_type:
   Full`, then kill the VMM.
4. Store the vmstate and memory files in the store, outside the jail,
   root-owned, mode 0600. Record the sha256 of both in the instance record.
5. Write a manifest into the record: Firecracker version, host kernel, CPU
   model and CPU template. Add the guest kernel and image digests, vcpus,
   memory, and the data disk generation.
6. The data disk generation rises at every cold boot. A snapshot with another
   generation is invalid.
7. Restore: verify both digests and the manifest. Move the files into the
   jail and delete the stored copies, so the snapshot is used once.
8. Then load with `PUT /snapshot/load` in a new jailed VMM.
9. On any mismatch: delete the snapshot, cold boot, and report the reason.
   A second restore finds no file.
10. After a restore, the daemon rotates every nonce and token over port 5000
    before kage takes a prompt. The broker revokes the old ones.
11. After a restore, box-guest resyncs the time (T-044) and the vsock
    channels reconnect.
12. The guest kernel needs the VMGenID driver for the reseed. Add it to the
    T-060 configuration when this entry starts.
13. HCS: only if T-014 item (d) passes. Use HcsSaveComputeSystem with
    SaveType ToFile, and restore through RestoreState at create. The same
    single-use and manifest rules apply.
14. Measure guest S4 as the alternative. It needs a swap file on the data disk
    of at least the guest memory size, and kernel hibernation support.
15. Under S4 the memory image stays in the guest disk. The host never parses
    it.

## Prove

```sh
cargo test -p box-daemon hibernate::
cargo test -p box-daemon --test hibernate_live -- --ignored
```

`hibernate` and then `start` restore once, and an attached shell sees its
earlier processes. A second restore of the same snapshot is refused, because
the files are gone. A changed manifest field falls back to cold boot and
reports the reason. The broker refuses the old nonces after a restore.

Host: a Linux KVM host. Windows 11 with Hyper-V as admin only if T-014 item
(d) passed.

## Start condition

T-014, T-089 and T-090.

# T-185: Containers inside the guest

**Source:** the ruling of 2026-09-15 (measure before deciding); the research document, section 7; brief section 5.
**Category:** measurement
**Milestone:** M8
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

Agents often build images and run services in containers. Whether a guest
may run containers is measured before it is decided. A runtime in the base
image is a supply-chain change, and a full disk inside a container looks
like a broken build.

## Premise

- Ruling (2026-09-15, carried over): measure first, then decide.
- READ (research section 7): in a microVM, the primitives that a nested
  container needs are guest-kernel primitives. The host boundary does not
  move.
- Those primitives are unshare of user, mount, pid and net namespaces, mount,
  CAP_SYS_ADMIN in the target namespace, and cgroup v2 delegation.
- READ (research section 7): the storage trap moves into the guest disk. A
  full overlay presents as a broken build.
- READ: virtiofsd crashed with SIGBUS when sparse writes filled the host disk
  (`kubernetes-sigs/agent-sandbox@4b63868f0d67e4ee0fa690bb14cb5722b78d679d:examples/gke-swap/runtimes/kata/README.md:29-35`).
  Here the same pressure lands on one sparse disk.
- Design (brief section 5): the guest kernel has cgroup v2, overlayfs and
  user namespaces. The root is an overlay: the base image is the lower layer,
  and the upper layer is on the data disk.
- Assumption to measure: overlayfs may not accept another overlayfs as its
  upper layer. Container storage on the overlay root may then fail.
- Design (brief section 5): a container runtime enters the image only if this
  measurement licenses it. That is a pins change with a named approver.

## Approach

1. Build a test-only image variant with podman and docker, by the rules of
   T-064. Never publish it.
2. Measure as `agent` with rootless podman, and as root with docker: `run
   --rm hello-world`, a build of a small Dockerfile, and a container port
   reached through `agent-box forward`.
3. Storage, case 1: the default storage on the overlay root.
4. Storage, case 2: storage in a data disk directory that is mounted
   directly, not through the overlay. Record which case works, and the error
   of the case that fails.
5. Egress: record the registry names that a pull needs, as allowlist rules.
6. Disk fill: write inside a container until the data disk is full. Record
   what the agent sees (expect ENOSPC as a failed build) and the host file
   size.
7. After the fill, stop and start the instance. Record whether it boots and
   whether `fstrim` gives the space back.
8. Cost: the image size added, and boot time and idle memory with the
   runtime against the base image (the method of T-181).
9. Write a section in docs/status.md: host, run count, workloads, date and
   results.
10. Decision rule: a positive result leads to a variant with a runtime, by a
    pins change with a named approver. A negative result is recorded, and
    the entry closes.

## Prove

```sh
cargo xtask battery --check guest-containers --json
```

The section exists in docs/status.md, positive or negative, with its
conditions. docs/decisions.md records the decision.

Host: a Linux KVM host, and Windows 11 with Hyper-V as admin.

## Start condition

T-064 and T-180.

# T-186: Instance-to-instance isolation

**Source:** brief sections 6.1 to 6.3 and 4; the OpenSandbox insight on host-side identity; the battery of T-180.
**Category:** measurement
**Milestone:** M8
**Priority:** P2
**Effort:** S
**Status:** open

## Problem

Instances on one host share the daemon, the netstack code, the broker and
the address plan. Later, the chat gateway runs instances for mutually
untrusted users. One instance must not reach another, act as another, or
read another's files.

## Premise

- Design (brief section 6.1): every instance gets the same guest address plan.
- Design (brief section 6.2): the netstack is per instance with no shared
  state (T-121). DNS answers are recorded per instance as (instance, name,
  address, expiry).
- Design (brief section 6.3): a nonce is valid only for its instance and its
  binding. The caller is the instance whose vsock channel carried the flow.
- Design (brief sections 4.1 and 4.2): Linux gives each instance its own uid
  and gid. Windows gives each VM its own hvsocket SDDL and its own file
  grants.
- READ: the Firecracker profile of OpenSandbox keys each subject on the guest
  source IP. It rests that on the sandbox lacking NET_ADMIN and NET_RAW
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:components/egress/pkg/fastsandboxnft/fastsandboxnft.go:36-41`).
- Design: a VM guest is root in its own kernel, so that premise fails for a
  VM. A source address from the guest is a claim. Identity is the host-side
  channel.

## Approach

Add check 13, `instance-isolation`, to the battery of T-180. Instances A and
B run at the same time.

1. Network: B runs a TCP listener on a guest port. A probes every address of
   the guest plan except its own and the gateway. B's listener records no
   arrival.
2. Forwards: give B a forward on host loopback. A connects to the gateway
   and to every host address on that host port. B's listener records no
   arrival.
3. DNS binding: B resolves an allowed name, and the battery reads the answer
   from B's daemon log. A connects to that address without a lookup. The
   netstack refuses, because it never answered that address to A.
4. Broker: the `egress-identity` check of T-180 covers it. This check refers
   to that result.
5. vsock: the probe in A connects to guest CIDs other than the host. Record
   each result. No connection reaches B.
6. Host files on Linux: as A's uid (through `sudo -u`), open B's data disk
   and list B's jail. Both get EACCES.
7. Host files on Windows: read the ACL of B's VHDX. It grants no SID of A's
   VM.
8. Pass: no arrival at B, every refusal recorded, and every file access
   refused.

## Prove

```sh
cargo xtask battery --check instance-isolation --json
```

The check passes on both engines inside a complete battery run. The results
list each probe and its refusal.

Host: as T-180.

## Start condition

T-121, T-123 and T-180.
