# Linux engine

This area builds the Linux engine: Firecracker v1.17.0 under its jailer,
driven by a root helper with a fixed operation set.
[docs/engines.md](../docs/engines.md) gives the design, and the entries below
turn it into work. [docs/protocol.md](../docs/protocol.md) gives the helper
protocol and the vsock transport. The research behind the engine is in
[docs/research/microvm-tier.md](../docs/research/microvm-tier.md), sections
1.3, 3, 5 and 8.

No product crate exists yet (ruling D14). Every path under crates/ in this file
is planned and written without backticks, and T-033 creates the crates. The
helper is crate box-helper. The jailer, config builder and API client are the
firecracker module of box-vmm. The daemon side is the planned file
crates/box-daemon/src/engine/firecracker.rs.

Hosts: the unit tests run on any host, or in `cargo xtask linux` on Windows.
The live tests need a Linux KVM host with root for the one-time install.
`cargo xtask linux --kvm` gives nested KVM on the operator's Windows host. That
serves development tests only, never a production claim. Nobody has measured
yet whether the helper and its cgroup root work inside a wsl-toolkit job.

# T-080: Threat model of the privileged helper (both OSes)

**Source:** The planning brief of 2026-10-14, sections 3 and 4; the research
document, sections 1.3 and 5.6; the adversarial review, item 12.
**Category:** docs
**Milestone:** M3
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

The helper is the only privileged process of agent-box (ruling D3). A defect in
it gives root on Linux and LocalSystem on Windows. A threat model whose
refusals have no tests is a claim that nobody checks. The model must cover both
OSes, because the Windows helper has other entry points.

## Premise

- Design: docs/engines.md, section "Threat model of the helper", starts the
  model. It lists the assets, the attackers, 23 refusal conditions with
  planned test names, and the residual risks.
- READ (research document, section 1.3): the helper is shape (b) of errand
  F12. It is the only shape that adds a privileged component, so it needs its
  own threat model.
- READ: the jailer treats every input as trusted, the paths included. The
  operator who calls it is part of the trusted computing base
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:266-274`).
- READ: upstream runs the jailer as root. The smaller capability set that it
  needs is "to be determined"
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:291-292`).
- MEASURED by the research document on its own host, not on the operator's
  host. The v1.17.0 jailer failed at `unshare(CLONE_NEWNS)` with `CapEff` 0
  (research document, section 1.3).
- MEASURED on 2026-10-14 on the operator's Windows 11 Pro host (i7-12700H, 20
  logical CPUs, 64 GB), one run. `hcsdiag list` with a non-elevated token
  exited 27 ([docs/status.md](../docs/status.md)).
- Inference of the adversarial review, item 12: a Plan9, VirtualSmb or
  NetworkAdapters entry from the caller makes the LocalSystem helper an
  escalation tool. An access grant on any path does the same.
- Design: the helper parses the HTTP replies of the Firecracker API (T-085).
  The VMM parses guest data, so a compromised VMM writes those replies. The
  model of docs/engines.md does not list this entry point yet.
- Design: T-182 reviews this model in M8. Its step 11 asks for one check that
  reads each refusal table and finds each named test in the source.

## Approach

1. Complete the section "Threat model of the helper" in docs/engines.md. Keep
   the shape that T-182 uses: assets, entry points, input provenance, refusals
   and residual risks.
2. Add a column `OS` to the refusal table, with the values linux, windows or
   both.
3. Give each refusal exactly one test name in the form `module::function`.
4. Put the decode refusals in crates/box-proto/src/helper.rs (T-023). Put all
   other refusals in crates/box-helper/src.
5. Add an entry point: the Firecracker API replies that the helper reads as
   root. Its controls are the caps and the timeouts of T-085, and a fuzz target
   of T-183.
6. Add a refusal: the helper does not start when root does not own
   /etc/agent-box/helper.toml, or when group or other can write it. The test is
   `helper::refuses_unsafe_config_owner`.
7. Add a refusal for Windows. The helper does not start when the DACL of its
   configuration file lets a user outside SYSTEM and Administrators write it.
   The test is `helper::refuses_unsafe_config_acl`.
8. Add a refusal for Windows: a pipe client token below medium integrity is
   refused (T-100). The test is `helper::refuses_low_integrity_client`.
9. Write the Linux capability lifecycle from the measurement of T-081. Write
   the Windows service privileges from the measurement of T-100.
10. Extend the input provenance table with the Windows values. They are the
    client SID, the store paths of a grant, the compute system id and the
    runtime id.
11. Keep the seccomp facts of docs/engines.md as they are now: 82, 37 and 50
    filter rules, which name 51, 32 and 27 distinct syscalls.
12. The research document records that correction as correction 1 of its
    conversion ([docs/research/microvm-tier.md](../docs/research/microvm-tier.md)).
13. Add the check `cargo xtask check refusals`, planned file
    xtask/src/check/refusals.rs. T-182 uses the same check for the broker and
    the netstack.
14. The check reads each Markdown table under docs/ with a column named
    `planned test` or `test`. For each named test, it finds a function with that
    name after a `#[test]` attribute, in a file or module of the named module.
15. The check reports each row whose test it cannot find, with the file and
    the line of the row.
16. Floor: at least one table with at least one row. Without crates/, the check
    exits 2, because it could not run.
17. Register the check in `xtask/src/check/mod.rs`, so that the gate runs it.
18. Add a plant self-test. A fixture table names a test that does not exist, and
    the check must report it.

## Prove

```sh
cargo xtask check refusals --json
cargo test -p xtask check::refusals
cargo test -p box-proto helper::
cargo test -p box-helper
```

Expected: the refusal check exits 0 and gives the file of each named test. The
self-test plants a missing test and sees exit 1. The test runs pass. The check
reads source text only, so it checks the rows of both OSes. Host: any host.

The Windows rows name tests that T-100 writes. This entry stays `partial` until
T-100 is done.

## Start condition

T-023 and T-081. The Windows rows need T-100.

# T-081: Linux helper: socket activation, peer check, fixed operations

**Source:** The planning brief of 2026-10-14, section 4.1; the research
document, sections 1.3 and 5.6; docs/engines.md, section "The privileged
helper".
**Category:** feature
**Milestone:** M3
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The jailer needs root, and the daemon runs as the operator. A root process must
run the jailer for the daemon, and must not become a root shell for other local
users. Each input that the helper takes, and each second that it holds a
capability, adds root attack surface.

## Premise

- MEASURED by the research document on its own host: the jailer failed at
  `unshare(CLONE_NEWNS)` with `CapEff` 0. uid 0 without capabilities was not
  enough (research document, section 1.3).
- READ: the jailer does every privileged step before it drops to the target
  uid and gid
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:113-160`).
- READ: the bailey helper holds all privilege, runs a fixed set of operations,
  and greets with a protocol version
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:docs/security/privilege.md:26-48`).
- Design (docs/engines.md): systemd starts one helper process per connection
  (`Accept=yes`). The socket `/run/agent-box/helper.sock` has the owner
  root:agent-box and the mode 0660.
- Design (docs/engines.md): the peer check compares the `SO_PEERCRED` uid with
  the operator uid that setup recorded. `SO_PEERCRED` gives the primary gid
  only, so a group check misses a supplementary group.
- Design (docs/protocol.md): one operation per connection, with the frames
  `HELPER_HELLO`, `REQUEST`, `RESULT` and `REFUSAL`. The receive deadline is
  open, and this entry sets it.
- Design: protocol.md says that refusals 1 to 15 happen before any syscall
  that uses a request value.
- Not measured: the capability list of docs/engines.md. This entry measures
  it.

## Approach

1. Write the Linux helper in crates/box-helper/src/linux/mod.rs, gated by
   `cfg(target_os = "linux")`. crates/box-helper/src/main.rs selects the OS
   module.
2. Install the binary as `/usr/libexec/agent-box/agent-box-helper`, owner
   root, mode 0755. docs/engines.md puts the engine binaries in the same
   directory.
3. Ship the unit files in crates/box-helper/units/. T-174 installs them. The
   socket unit:

   ```ini
   # agent-box-helper.socket
   [Socket]
   ListenStream=/run/agent-box/helper.sock
   SocketUser=root
   SocketGroup=agent-box
   SocketMode=0660
   DirectoryMode=0755
   Accept=yes
   MaxConnections=8

   [Install]
   WantedBy=sockets.target
   ```

4. The service template that the socket starts per connection:

   ```ini
   # agent-box-helper@.service
   [Service]
   ExecStart=/usr/libexec/agent-box/agent-box-helper
   User=root
   StandardInput=null
   StandardOutput=journal
   StandardError=journal
   CapabilityBoundingSet=CAP_SYS_ADMIN CAP_SYS_CHROOT CAP_MKNOD CAP_CHOWN CAP_FOWNER CAP_SETUID CAP_SETGID CAP_KILL CAP_SYS_PTRACE
   RuntimeMaxSec=120
   ```

5. `MaxConnections=8` and `RuntimeMaxSec=120` are design values. Record what
   systemd does with a ninth connection.
6. At start, check that `LISTEN_PID` is the own pid and that `LISTEN_FDS` is 1.
   Take file descriptor 3. Refuse to run in any other mode.
7. Read /etc/agent-box/helper.toml. Refuse it unless root owns it and group
   and other cannot write it. Parse it with serde `deny_unknown_fields`.
8. The keys are `operator_uid`, `uid_base`, `uid_count`, `store_root`,
   `jail_base`, `cgroup_root` and `max_instances`. The bounds are `cpus_max`,
   `memory_mib_min`, `memory_mib_max` and `disk_gib_max`.
9. Clear the effective capability set before the first read from the socket.
   The candidate is the `rustix` crate, so that box-helper needs no unsafe code.
10. Read the peer credentials with `SO_PEERCRED`. When the uid differs from
    `operator_uid`, send `REFUSAL` with `peer_refused` and close.
11. Set a receive deadline of 5 s per frame with `SO_RCVTIMEO`. The value is a
    design value. Write it into docs/protocol.md, where the question is open.
12. Read `HELPER_HELLO`, then one `REQUEST`. Decode both with the box-proto
    codec of T-023.
13. List the instance directories of the store once, before the decode. This
    list uses no request value.
14. Check the request in one pure function:
    `check::request(&Request, &HelperConfig, &InstanceList) -> Result<Checked, Refusal>`.
15. The check covers the version, the operation, the id shape, the record list
    and the bounds. A unit test drives each refusal with no privilege and no
    syscall.
16. Take the instance lock with `flock(LOCK_EX | LOCK_NB)` on
    `store/instances/<id>/lock`. A held lock is `REFUSAL` `busy`.
17. Raise the capabilities of the operation only. Lower them before the
    reply.
18. The operations come from their entries: create (T-087), start (T-086,
    T-088), pause and resume (T-089), stop and kill (T-088), inventory (T-090),
    delete (T-091), probe (T-092).
19. Until the entry of an operation lands, the operation answers `REFUSAL`
    `unsupported`, and the detail names the entry.
20. Write `RESULT` or `REFUSAL`, then exit. Between two operations no helper
    process runs at all.
21. Log one line per operation to the journal: the peer uid, the operation,
    the id and the result. Never log request bytes.
22. Measure the minimal capability set. Run each operation with one capability
    removed from the bounding set, and record which step fails.
23. Write the measured table into docs/engines.md, section "Capability
    lifecycle". Trim `CapabilityBoundingSet` to it.
24. Measure each hardening directive before use: `NoNewPrivileges`,
    `ProtectSystem`, `ProtectHome`, `PrivateTmp` and `PrivateNetwork`.
25. Keep a directive only when a jailed VMM still writes its data disk and a
    later helper process still verifies it (T-090).
26. Name the refusal tests as docs/engines.md names them, for example
    `helper::refuses_foreign_uid` with a planted peer credential.

## Prove

```sh
cargo test -p box-helper
cargo build --release -p box-helper
sudo install -D -m 0755 target/release/agent-box-helper /usr/libexec/agent-box/agent-box-helper
sudo install -D -m 0644 crates/box-helper/units/agent-box-helper.socket /etc/systemd/system/agent-box-helper.socket
sudo install -D -m 0644 crates/box-helper/units/agent-box-helper@.service /etc/systemd/system/agent-box-helper@.service
sudo systemctl daemon-reload
sudo systemctl enable --now agent-box-helper.socket
sudo env "PATH=$PATH" cargo test -p box-helper --test socket_live -- --ignored
```

Expected: the unit tests pass with no privilege. The live test writes
/etc/agent-box/helper.toml for its own identities and restores the old file.
It forks three children with `setresuid`: the operator uid, a second member of
the group, and a uid outside the group.

- The uid outside the group gets `EACCES` from `connect`.
- The second member gets `REFUSAL` `peer_refused`.
- The operator with the all-zero id gets `REFUSAL` `bad_id`.
- One second after each reply, `pgrep -x agent-box-helper` finds no process.

Host: any host for the unit tests; a Linux KVM host with root and systemd for
the live test.

## Start condition

T-023 and T-033.

# T-082: uid allocator and the delegated cgroup v2 root

**Source:** The planning brief of 2026-10-14, section 4.1; the research
document, section 5.5; docs/engines.md, sections "uid allocator" and "cgroup
v2 delegation".
**Category:** feature
**Milestone:** M3
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

Each VMM must run as its own uid, so that a VMM escape cannot open the disk of
another instance. Its memory, CPU and task use must be bounded, or one guest
can starve the host. Persistent instances need the same uid and cgroup on every
boot. The cgroup must outlive the helper process, and systemd must not kill
the VMMs when the helper exits.

## Premise

- READ: systemd offers delegation "on scope and service units only", and
  expressly not on slices (https://systemd.io/CGROUP_DELEGATION/, read
  2026-10-14).
- READ (same page): a cgroup with child cgroups cannot hold processes. A
  delegated unit moves its main process into a subgroup first.
  `DelegateSubgroup=` does that move from systemd 254 on.
- READ (same page): a program must not create its own cgroups below the root
  cgroup. Its cgroups belong in a delegated subtree.
- READ: with `--cgroup-version 2` and no `--cgroup`, the jailer creates no
  cgroup. It moves itself into `--parent-cgroup` when that cgroup exists
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/jailer/src/env.rs:233-243`).
- READ: that move fails when the target has domain controllers in
  `cgroup.subtree_control`. A missing target is not an error
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:65-74`).
- READ: with a `--cgroup` flag, the jailer writes `+<controller>` into
  `cgroup.subtree_control` of the ancestors
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/jailer/src/cgroup.rs:379-395`).
- READ: the cgroup v2 formats are `cpu.max` as "$MAX $PERIOD" and `io.max`
  as nested keys such as `8:16 rbps=2097152`. Writing 1 to `cgroup.kill` kills
  the whole subtree (https://docs.kernel.org/admin-guide/cgroup-v2.html, read
  2026-10-14).
- READ: the range 1879048192 to 2147352575 (0x70000000 to 0x7FFDFFFF) is
  unused in the systemd UID map. Other packages can still use it
  (https://systemd.io/UIDS-GIDS/, read 2026-10-14).
- READ (same page): avoid UIDs above 2147483647, because some software
  mishandles them.
- READ: Firecracker advises against host swap, because swap puts guest memory
  on storage
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/prod-host-setup.md:274-278`).
- Design: the entry list says "a systemd slice with Delegate=yes". The READ
  above rules that out, and step 1 corrects it.

## Approach

1. Decision proposal for the operator: a service unit carries the delegation,
   not a slice. Record it in docs/decisions.md. docs/engines.md and
   docs/cli.md already name the service.
2. Ship crates/box-helper/units/agent-box-vmm.service. T-174 installs it.

   ```ini
   # agent-box-vmm.service
   [Unit]
   Description=agent-box cgroup root for jailed VMMs
   RefuseManualStop=yes

   [Service]
   Type=exec
   ExecStart=/usr/libexec/agent-box/agent-box-helper --cgroup-anchor
   Delegate=yes
   KillMode=control-group
   TimeoutStopSec=30

   [Install]
   WantedBy=multi-user.target
   ```

3. `--cgroup-anchor` moves its own pid into the subgroup `anchor` of the unit
   cgroup. It drops every capability and waits in `pause()` forever.
4. On systemd 254 and later, `DelegateSubgroup=anchor` makes the move. Keep the
   own move for older versions. Record the systemd versions that the test
   covered.
5. The anchor process then writes `+memory +cpu +pids +io` into
   `cgroup.subtree_control` of the unit cgroup. A missing `io` controller is a
   note.
6. The cgroup root is `/sys/fs/cgroup/system.slice/agent-box-vmm.service`. It
   is the key `cgroup_root` of helper.toml, written by setup, never a request
   field.
7. CAUTION: `systemctl stop agent-box-vmm.service` kills every VMM.
   `RefuseManualStop=yes` blocks a manual stop. Host shutdown still stops it,
   so the daemon powers the guests off first (T-088).
8. Before each start, the helper checks the root: it exists, its
   `cgroup.procs` is empty, and its `cgroup.subtree_control` lists `memory` and
   `pids`.
9. A missing `memory` or `pids` controller is `REFUSAL` `gap`, with the gap
   name `memory-max-missing`. A missing `cpu` or `io` controller is a note.
10. Create the leaf `<cgroup_root>/<id>` with `mkdirat` on a directory file
    descriptor of the root. Never build the path from a request string.
11. Write these files in the leaf before the jailer runs:

    | file | value |
    | --- | --- |
    | `memory.max` | (memory_mib + memory_overhead_mib) * 1048576 |
    | `memory.swap.max` | `0` |
    | `cpu.max` | cpus * 100000, then `100000` |
    | `pids.max` | the measured VMM task count plus a margin (step 13) |
    | `io.max` | `MAJ:MIN rbps=N wbps=N`, only when the profile sets a limit |

12. `memory_overhead_mib` is a key of helper.toml with the default 256. That
    default is a design value, not measured. Measure the VMM RSS above guest
    memory on a booted instance and set the default from it.
13. Count the tasks of a booted VMM in `/proc/<pid>/task` with 1 vCPU and with
    32 vCPUs. Set `pids.max` to the vCPU count plus the measured fixed part
    plus 8.
14. Pass the leaf to the jailer as `--parent-cgroup
    system.slice/agent-box-vmm.service/<id>`, relative to the cgroup2 mount.
    Never pass `--cgroup`.
15. After the jailer exits, read `/proc/<pid>/cgroup` of the VMM. Anything
    other than the leaf kills the VMM and refuses (`helper::kills_vmm_outside_leaf`).
16. Write the uid allocator in crates/box-helper/src/linux/uid.rs. The keys
    `uid_base` and `uid_count` come from helper.toml.
17. Proposal for the default range: base 1879048192 (0x70000000), count 65536.
    It is inside the unused range of the systemd map.
18. At setup, refuse a range that overlaps a uid of /etc/passwd, a gid of
    /etc/group, or a range of /etc/subuid or /etc/subgid.
19. At setup, also call `getpwuid_r` and `getgrgid_r` for each id of the range.
    An NSS source, for example LDAP, can hold an account there.
20. At `create`, take the lowest id of the range that no record holds. Check
    that no process runs as it (the `Uid:` lines of `/proc/*/status`).
21. Also check that no file under the store or the jail base belongs to it.
    Then write uid and gid, with the same number, into the record.
22. The record is `store/instances/<id>/record`, root:root 0600, written to a
    temporary file and renamed. It carries `version`, the id, uid, gid, disk
    size, VMM pid and start time.
23. T-091 frees the uid. Later operations read the uid from the record and
    refuse a recorded uid outside the range (`helper::refuses_uid_outside_range`).

## Prove

```sh
cargo test -p box-helper uid::
cargo test -p box-helper cgroup::
sudo install -D -m 0644 crates/box-helper/units/agent-box-vmm.service /etc/systemd/system/agent-box-vmm.service
sudo systemctl daemon-reload
sudo systemctl enable --now agent-box-vmm.service
cargo test -p box-daemon --test linux_boot -- --ignored cgroup_limits
sudo systemctl stop agent-box-vmm.service
```

Expected: the unit tests pass. They plant passwd, group and subuid files that
overlap the range, and setup refuses each one. Allocation takes the lowest free
id.

The boot test starts an instance with 2 vCPUs and 2048 MiB. It reads back
`memory.max` as 2415919104, `memory.swap.max` as 0 and `cpu.max` as
`200000 100000`. It also reads back `pids.max`.

The boot test finds the leaf in `/proc/<vmm pid>/cgroup`, and the VMM still
runs after the helper process exits. The last command fails, because the unit
refuses a manual stop.

Host: any host for the unit tests; a Linux KVM host with root for the rest.

## Start condition

T-081.

# T-083: Firecracker and the jailer: download and verify at setup

**Source:** The planning brief of 2026-10-14, section 4.1; the research
document, sections 2.3 and 6; TEMPLATE docs/containers.md on verification
tiers.
**Category:** feature
**Milestone:** M3
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

The jailer runs as root, and Firecracker parses every guest byte. A wrong
binary here is wrong code with root at its start. Setup must install only the
bytes that a reviewer pinned, and must never run or keep a file that fails its
check.

## Premise

- MEASURED by the research document on its own host: the v1.17.0 release
  verified against its own `SHA256SUMS`, 19 non-debug files, all `OK`.
  `firecracker --version` printed `v1.17.0`, and `jailer --version` printed
  `Jailer v1.17.0`.
- READ: the release asset is `firecracker-<version>-<arch>.tgz`. It unpacks to
  `release-<version>-<arch>/`
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/getting-started.md:163-169`).
- READ: a jailer binary needs a statically linked Firecracker of the same
  version, from the default musl toolchain
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:5-10`).
- READ: three tiers answer three questions. A sums file proves the transport,
  a reviewed digest proves the bytes, and a signature proves the provenance
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:94-115`).
- READ: a file that fails its digest is deleted, and nothing runs from it
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:76-92`).
- READ: the release ships the default seccomp filter JSON files
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/seccomp.md:35-37`).
- Not read: whether the release publishes a signature or an attestation.
  This entry checks it.

## Approach

1. Add the table `[firecracker]` to images/pins.toml (T-066). It holds the
   version `v1.17.0`, the commit, and per architecture the tarball URL and
   its sha256.
2. Per architecture, the table also holds the sha256 of each member that setup
   keeps. These are the firecracker and jailer binaries, the seccomp filter
   JSON and the spec YAML.
3. Add the field `reviewed_by` to the table. A reviewer downloads the release
   on a second host, checks it against `SHA256SUMS`, and writes the digests in
   a reviewed commit.
4. The release binary embeds the table through `box_image::EMBEDDED` (T-066).
   Setup never reads a pin from the network.
5. Write the install code in crates/box-cli/src/setup/firecracker.rs. T-174
   calls it from `agent-box setup --system`.
6. Download with rustls into a new directory `/var/lib/agent-box/tmp/<random>`,
   mode 0700.
7. Hash the tarball before any parse. On a mismatch, delete the directory and
   stop with the artifact name. The tar reader never reads unverified bytes.
8. Extract only the named members. Refuse a member with `..`, an absolute
   name, a link or a device type.
9. Hash each member against its pin. Also check that the `SHA256SUMS` file in
   the tarball lists the same digest.
10. Report the tiers that ran: transport (`SHA256SUMS`), reviewed digest, and
    provenance. When the release publishes nothing to verify, report that
    outcome by name.
11. Install `/usr/libexec/agent-box/firecracker` and
    `/usr/libexec/agent-box/jailer`, owner root:root, mode 0755. Write a
    temporary file in the same directory, fsync it, then rename it.
12. Install the seccomp filter JSON as
    `/usr/libexec/agent-box/seccomp-filter.json`, mode 0644. The battery reads
    it (T-180).
13. Run `firecracker --version` and `jailer --version` as user `nobody`.
    Compare the output with the pinned version. A difference removes the files
    and stops setup.
14. Delete the download directory in every outcome.
15. The probe hashes both installed binaries again at each run (T-092). A
    mismatch is the gap `engine-digest-mismatch`.

## Prove

```sh
cargo test -p box-cli setup::firecracker::
sudo env "PATH=$PATH" cargo test -p box-cli --test firecracker_install -- --ignored
sha256sum /usr/libexec/agent-box/firecracker /usr/libexec/agent-box/jailer
```

Expected: the unit tests serve tarballs from a local HTTP fixture. A planted
wrong tarball digest is refused before extraction, and the download directory
is gone. A member name with a `..` part, a link member and a wrong member
digest are each refused.

The live test installs the pinned release. The two digests equal the pins.
Host: any host for the unit tests; a Linux KVM host with root and network for
the live test.

## Start condition

T-066 for the pins table. T-174 later calls this code from setup.

# T-084: The VM configuration builder

**Source:** The planning brief of 2026-10-14, section 4.1; the research
document, section 5.4 and its corrections; docs/engines.md, section "VM
configuration fields".
**Category:** feature
**Milestone:** M3
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The configuration decides what devices the guest gets. A `network-interfaces`
key gives the guest a NIC that bypasses the netstack and its policy. A wrong
disk setting loses the journal on host power loss. A secret in `boot_args` is
readable by every guest process in `/proc/cmdline`.

## Premise

- READ: each object type of the Firecracker API refuses unknown fields, for
  example the drive config
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/vmm_config/drive.rs:35`).
- READ: the top-level type of `--config-file` has no such attribute
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/resources.rs:80-102`).
  docs/engines.md generalizes this to every configuration type. That is too
  wide.
- So an unknown key is not the main risk on the API path. A known key that
  adds a device, such as `network-interfaces`, is. Only the builder can refuse
  it.
- READ: the configuration properties are `balloon`, `drives`, `boot-source`,
  `cpu-config`, `logger`, `machine-config`, `metrics`, `memory-hotplug`,
  `mmds-config`, `network-interfaces`, `pmem`, `vsock` and `entropy`
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:1337-1374`).
- READ: `vcpu_count` is 1 or an even number, from 1 to 32. `huge_pages` is
  `None`, `Transparent` or `2M`
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:1446-1490`).
- READ: `discard` works only for writable virtio-block drives on the `Sync`
  engine
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:1241-1247`).
- READ: `Writeback` performs an `fsync` on each guest flush
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-caching.md:21-27`).
- READ: a root drive makes Firecracker append `root=/dev/vda` or
  `root=PARTUUID=<uuid>`, then `ro` or `rw`, to the command line
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/vmm_config/boot_source.rs:74-86`).
- READ: the default command line is `reboot=k panic=1 nomodule 8250.nr_uarts=0
  i8042.noaux i8042.nomux i8042.dumbkbd swiotlb=noforce`, the same on every
  architecture
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/vmm_config/boot_source.rs:19-20`).
- READ: Firecracker recommends `vmlinux`, because the guest decompresses a
  `bzImage` at boot, which costs time and memory
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/rootfs-and-kernel-setup.md:13`).
- READ: the x86_64 `vmm` thread filter allows `io_uring_setup`,
  `io_uring_enter` and `io_uring_register` for every `io_engine`
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:resources/seccomp/x86_64-unknown-linux-musl.json:63-75`).
- READ: `--seccomp-filter` takes a custom filter compiled with
  seccompiler-bin. Upstream calls that use dangerous and recommends the
  default filters
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/seccomp.md:39-60`).
- Design: the operator's main session ruled that one config per architecture
  builds `vmlinux` and `bzImage` on x86_64, and that Firecracker boots
  `vmlinux` (T-060, [docs/guest.md](../docs/guest.md)).

## Approach

1. Write crates/box-vmm/src/firecracker/config.rs, gated by
   `cfg(target_os = "linux")`. Only box-helper depends on box-vmm.
2. Define one struct per API object that the builder sends: `MachineConfig`,
   `BootSource`, `Drive`, `Vsock`, `Balloon` and `Entropy`. Each has
   `#[serde(deny_unknown_fields)]`.
3. Define `VmConfig` with exactly these fields: `machine`, `boot`,
   `drives: [Drive; 2]`, `vsock`, `balloon` and `entropy`. It has no field for a
   network interface, MMDS, pmem, memory hotplug, logger, metrics, CPU config
   or serial.
4. Take the input `BuildInput`: id, cpus (`u8`), memory in MiB (`u32`), arch,
   the kernel kind and the root filesystem type of T-065. It has no string
   field.
5. Refuse cpus that are not 1 or an even number from 2 to 32, or above the
   host CPU count. Refuse memory that is odd or outside the helper bounds.
6. Build the kernel path `/kernel` from the hard link of T-086. The kernel is
   `vmlinux` on x86_64 and `Image` on aarch64.
7. Emit the calls in this fixed order. The values are for 2 vCPUs, 2048 MiB
   and erofs:

   ```text
   PUT /machine-config {"vcpu_count":2,"mem_size_mib":2048,"smt":false,"track_dirty_pages":false,"huge_pages":"None"}
   PUT /boot-source    {"kernel_image_path":"/kernel","boot_args":"reboot=k panic=1 nomodule 8250.nr_uarts=0 i8042.noaux i8042.nomux i8042.dumbkbd swiotlb=noforce rootfstype=erofs init=/usr/sbin/box-guest"}
   PUT /drives/base    {"drive_id":"base","path_on_host":"/base.img","is_root_device":true,"is_read_only":true,"io_engine":"Sync"}
   PUT /drives/data    {"drive_id":"data","path_on_host":"/data.img","is_root_device":false,"is_read_only":false,"cache_type":"Writeback","io_engine":"Sync","discard":true}
   PUT /vsock          {"guest_cid":3,"uds_path":"/vsock/v.sock"}
   PUT /balloon        {"amount_mib":0,"deflate_on_oom":true,"stats_polling_interval_s":0,"free_page_hinting":false,"free_page_reporting":false}
   PUT /entropy        {}
   ```

8. Do not put `root=` in `boot_args`. The root drive makes Firecracker append
   `root=/dev/vda ro`. A GPT with a fixed partition UUID (T-061, T-065) changes
   this to the `partuuid` field.
9. Build `boot_args` only from constants and the enum of the root filesystem
   type. This builder has no string input, so one test can cover every input.
10. For T-256, write the test `boot_args_carry_no_value`. It builds every
    input and scans each emitted body with the helper of T-256.
11. Never send `PUT /serial`, `/logger`, `/metrics`, `/mmds`,
    `/network-interfaces`, `/pmem` or `/hotplug/memory`. `8250.nr_uarts=0`
    keeps the guest serial port off.
12. Vendor the in-tree spec at the pin as crates/box-vmm/spec/firecracker-v1.17.0.yaml.
    A test compares its sha256 with the pin.
13. A test parses the spec and checks that each emitted path exists. It checks
    that each emitted field exists in the definition with the right type.
14. Keep golden files of the emitted calls in crates/box-vmm/tests/golden/firecracker/.
    One file per architecture. A change to a body changes a golden file in the
    same commit.
15. A test reads config.rs with `include_str!`. It fails when the source names
    `network-interfaces`, `mmds`, `serial`, `pmem` or `hotplug`.
16. T-180 owns the negative control that adds a NIC. This entry adds no NIC
    path, not even behind a feature flag.
17. Open question: does agent-box ship a custom seccomp filter without the
    io_uring syscalls? The `Sync` engine does not need them.
18. The cost of that filter: a compiled filter per release and per
    architecture, a review of each upstream filter change, and the upstream
    warning. Write the answer as a decision in docs/decisions.md.

## Prove

```sh
cargo test -p box-vmm firecracker::config::
cargo xtask linux -- cargo test -p box-vmm firecracker::config::
```

Expected: the tests pass. The golden files match byte for byte. Each refused
input of step 5 has a test. The source scan of step 15 passes, and a planted
`network-interfaces` string in a copy of the source makes it fail. Host: any
Linux host; on Windows the second command runs the tests in the Linux runner.

## Start condition

T-033. The kernel kind and the root filesystem type come from T-060 and T-065.

# T-085: Firecracker API client over a Unix socket

**Source:** The planning brief of 2026-10-14, section 4.1; the research
document, section 5.4; the Firecracker swagger file at v1.17.0.
**Category:** feature
**Milestone:** M3
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

The helper configures, starts, pauses and resumes each VMM through its HTTP
API on a Unix socket. The helper runs as root, and the VMM parses guest data.
A client that trusts the replies, or waits without a limit, lets a compromised
VMM attack root or block every operation.

## Premise

- READ: `/actions` defines only `PUT`, with the body `InstanceActionInfo`
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:41-61`).
  Its `action_type` is `FlushMetrics`, `InstanceStart` or `SendCtrlAltDel`
  (same file, `:1376-1389`).
- READ: `PATCH /vm` takes `{"state": "Paused"}` or `{"state": "Resumed"}`
  and answers 204
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:832-855`).
- READ: `GET /` gives `InstanceInfo`, whose `state` is `Not started`,
  `Running` or `Paused`
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:1391-1418`).
- READ: `GET /vm/config` gives the whole configuration as
  `FullVmConfiguration`
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:857-872`).
- READ: an error body is an object with one field, `fault_message`
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:1329-1335`).
- READ: `SendCtrlAltDel` exists on Intel and AMD only. Firecracker exits on a
  guest CPU reset
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/actions.md:34-40`).
- READ: every wait has a hard limit, and a timeout is a different fact from a
  failure
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/conventions/shell.md:422-437`).

## Approach

1. Write crates/box-vmm/src/firecracker/api.rs on std `UnixStream`. Use no
   HTTP library: the client sends seven request shapes and reads one reply
   shape.
2. Send HTTP/1.1 with `Host: localhost`, `Content-Type: application/json`,
   `Accept: application/json` and `Content-Length`. Close the connection after
   each reply.
3. Read the status line and the headers incrementally, with a cap of 8 KiB.
   Require `Content-Length`. Refuse a chunked or an unknown transfer encoding.
4. Cap the body at 64 KiB. `GET /vm/config` of the builder output fits in it.
5. Give each call a limit: connect 2 s, request 5 s, `InstanceStart` 10 s.
   These are design values. A timeout returns `ApiError::Timeout`, never a
   result.
6. Map 204 to success, 200 to a parsed body, and any other status to
   `ApiError::Status` with the code and `fault_message`.
7. Treat `fault_message` as data: cap it at 512 bytes, escape non-ASCII, and
   never interpret it.
8. Write these calls: the `PUT` calls of T-084, `instance_start()`,
   `send_ctrl_alt_del()`, `pause()`, `resume()`, `describe()` for `GET /` and
   `vm_config()` for `GET /vm/config`.
9. Refuse `send_ctrl_alt_del()` on aarch64 before any IO.
10. Take the socket path from the helper only: `<jail>/root/run/fc.sock`.
11. Write a fake API server in crates/box-vmm/tests/fake_api.rs, on a Unix
    socket in a temporary directory.
12. Test a 204, a 200 with a body, a 400 with `fault_message` and a slow
    reply. Also test a header of 8 KiB plus 1, a body above the cap, a missing
    `Content-Length` and a chunked reply.

## Prove

```sh
cargo test -p box-vmm firecracker::api::
cargo xtask linux -- cargo test -p box-vmm firecracker::api::
```

Expected: the tests pass against the fake server. No VM is needed. Each refused
reply has a test, and the slow reply ends in `ApiError::Timeout` within the
limit. Host: any Linux host; on Windows the second command runs the tests in the
Linux runner.

## Start condition

T-033.

# T-086: Jail construction on each boot

**Source:** The planning brief of 2026-10-14, section 4.1. The research
document, sections 1.3 and 5.5, with their corrections. docs/engines.md,
sections "Jailer invocation", "Jail layout per boot" and "The boot sequence".
**Category:** feature
**Milestone:** M3
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The jail is the confinement of the process that parses every guest byte. A
persistent instance keeps its disk across boots, so the jail must be built
fresh each time without deleting the disk. A stale jail, a missing cgroup or a
writable jail directory turns a VMM bug into host access.

## Premise

- READ: the jailer creates `<chroot_base>/<exec_file_name>/<id>/root`. It does
  nothing when that path already exists
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:117-122`).
- READ: every resource given to the VM through the API must be hard-linked or
  copied into the jail. The VMM uid needs read and write access to a writable
  drive
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:275-279`).
- READ: the jailer closes every inherited file descriptor except 0, 1 and 2
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:114-115`).
- READ: the jailer already passes `--id` to Firecracker
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:101-107`).
  The research document passed it twice, and its conversion removed the copy.
- READ: the jailer stores the child pid in `<exec_file_name>.pid` in the jail
  root, with or without `--new-pid-ns`
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:307-314`).
- READ: `--resource-limit` accepts `fsize` and `no-file`. Without a value,
  `no-file` is 2048
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/jailer/src/resource_limits.rs:12`).
  The jailer always installs `no-file`, and `fsize` only when given (same
  file, `:92-100`).
- READ: the jailer makes `/dev/net/tun` and `/dev/kvm` in the jail and gives
  them to the target uid
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:140-144`).
- READ: the x86_64 filters hold 82, 37 and 50 rules for the threads `vmm`,
  `api` and `vcpu`, each with the default action `trap`. The rules name 51,
  32 and 27 distinct syscalls
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:resources/seccomp/x86_64-unknown-linux-musl.json:1`).
- Design (docs/engines.md): the seal makes each jail directory root-owned
  before `InstanceStart`, and `root/` becomes root:GID 0750.
- Design gap: the daemon runs as the operator, outside the instance group. With
  `root/` at 0750 it cannot reach `vsock/v.sock`. Step 15 fixes the mode.

## Approach

1. Write crates/box-vmm/src/firecracker/jail.rs for the paths and the argv,
   and crates/box-helper/src/linux/start.rs for the steps. The steps are those
   of docs/engines.md, section "The boot sequence".
2. Before the jailer, remove a stale jail of the id. Use `openat2` with
   `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS` from a file descriptor of the jail
   base. Never follow a link.
3. Create the cgroup leaf and its limits (T-082).
4. Create `root/`, `root/run/` and `root/vsock/`, owned by the instance uid.
5. Hard-link with `linkat` from directory file descriptors: the kernel to
   `root/kernel`, the base image to `root/base.img`, the data disk to
   `root/data.img`.
6. `EXDEV` from `linkat` is `REFUSAL` `cross_device`: the store and the jail
   base are on two filesystems (`helper::refuses_cross_device_link`).
7. Run the jailer with umask 0007, from the helper, with this argv. Every value
   comes from the helper record and helper.toml.

   ```text
   /usr/libexec/agent-box/jailer
     --id <32 lowercase hex>
     --exec-file /usr/libexec/agent-box/firecracker
     --uid <uid> --gid <gid>
     --chroot-base-dir <jail_base>
     --cgroup-version 2
     --parent-cgroup system.slice/agent-box-vmm.service/<id>
     --new-pid-ns
     --daemonize
     -- --api-sock /run/fc.sock
   ```

8. Never pass `--netns`, `--cgroup`, `--no-seccomp` or `--seccomp-filter`.
   Never pass `--id` after `--`.
9. Open question: `--resource-limit fsize`. docs/engines.md lists it with the
   data disk size plus one 4096-byte block. The entry list leaves it open.
10. With that limit, a VMM bug that writes past the end of a file gets
    `SIGXFSZ`. The cost: a wrong value stops the VMM at the first write past
    it. Decide it here and record it in docs/decisions.md.
11. Wait for the jailer to exit. Read `root/firecracker.pid`. Read the start
    time of that pid from `/proc/<pid>/stat`, field 22.
12. Verify the pid: `stat` of `/proc/<pid>/exe` and of `/proc/<pid>/root`
    give the device and inode of `root/firecracker` and of `root/`. Then check
    the cgroup (T-082).
13. Write the pid and its start time into the record. Guest code has not run,
    so this pid is trustworthy.
14. Configure the VMM (T-084, T-085). Compare `GET /vm/config` with the builder
    output. Any difference kills the VMM and refuses
    (`helper::kills_vmm_on_config_drift`).
15. Seal the jail. Each directory becomes root-owned and not writable by the
    VMM uid. Proposal: `root/` becomes root:GID 0751, so that the daemon can
    reach `vsock/`. The parent directories already keep other users out.
16. Set `run/` to root:GID 0750 and `vsock/` to root:agent-box 02771. Set
    `vsock/v.sock` to UID:agent-box 0660. Use `fchownat` and `fchmodat` with
    `AT_SYMLINK_NOFOLLOW`.
17. Remove `dev/net/tun` in the seal. No configuration uses it, and the VMM
    then has one device node less.
18. Send `InstanceStart` (T-088).
19. Teardown after each stop (T-088 `kill`): unlink every entry of the jail
    with the `openat2` rules of step 2. Remove only the hard link `data.img`,
    never the store copy.
20. After teardown, check that the link count of the store data disk is 1.
21. Write a test fixture with a symbolic link planted in a jail directory.
    Teardown must remove the link, never its target
    (`helper::teardown_never_follows_links`).

## Prove

```sh
cargo test -p box-vmm firecracker::jail::
cargo test -p box-helper start::
sudo env "PATH=$PATH" cargo test -p box-daemon --test linux_boot -- --ignored jail_layout
```

Expected: the unit tests check the argv, the refusals and the no-follow
teardown. On the KVM host, the live test starts an instance and checks:

- `stat` of `/proc/<vmm pid>/root` gives the device and inode of the jail
  `root/`;
- the `Uid:` and `Gid:` lines of `/proc/<vmm pid>/status` hold the instance
  uid and gid, `CapEff` is 0 and `Seccomp:` is 2;
- as the VMM uid, a create, a rename and an unlink in each jail directory fail
  with `EACCES`;
- as the operator, a connect to `vsock/v.sock` works;
- `dev/net/tun` is absent, and the store data disk has link count 2, then 1
  after stop.

Host: any host for the unit tests; a Linux KVM host with root for the live test.

## Start condition

T-081, T-082, T-084 and T-085.

# T-087: The data disk: sparse create, lock, settings

**Source:** The planning brief of 2026-10-14, section 4.1; the research
document, section 5.4 and its correction; docs/engines.md, section "Data disk
settings".
**Category:** feature
**Milestone:** M3
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

The data disk holds the work of the agent across boots. Every byte on it is
guest-written, so a host process that opens it parses untrusted data. Two VMMs
on one disk corrupt it, and Firecracker takes no lock. A disk that never gives
space back fills the host.

## Premise

- READ: Firecracker opens a drive with read and, for a writable drive, write
  access. It sets no `O_DIRECT` and takes no lock. It reads the capacity with a
  seek to the end
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/devices/virtio/block/virtio/device.rs:69-95`).
- READ: for a regular file, discard punches holes with
  `FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE`
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/devices/virtio/block/virtio/io/sync_io.rs:37-68`).
- READ: discard is not supported for read-only drives, the `Async` engine or
  vhost-user
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-discard.md:10-20`).
- READ: the discard test of Firecracker accepts `EOPNOTSUPP` and `ENOSYS`,
  because some host filesystems do not support hole punching
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/devices/virtio/block/virtio/io/sync_io.rs:154-161`).
- READ: the jailer closes every inherited file descriptor except 0, 1 and 2
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:114-115`).
  No agent-box process can pass a held lock on the disk to the VMM.
- Design: the default disk is 30 GiB (D1 says about 30 GB; specs use GiB).
- Design: docs/limits.md states that sparse disks overcommit host storage.

## Approach

1. Write crates/box-helper/src/linux/create.rs for the helper operation
   `create`, with the request fields id and `disk_gib`.
2. Refuse `disk_gib` above `disk_gib_max` with `out_of_bounds`. Refuse it when
   the free space of the store filesystem (`statvfs`) is below the disk size.
3. That check keeps one new instance able to fill its own disk. Overcommit
   across instances stays possible, as docs/limits.md says.
4. Create `store/instances/<id>/`, root:root 0700, with `mkdirat` from a file
   descriptor of the store.
5. Allocate the uid and write the record (T-082).
6. Create `data.img` with `openat(O_CREAT | O_EXCL | O_WRONLY | O_NOFOLLOW |
   O_CLOEXEC, 0600)`. Call `ftruncate` to `disk_gib * 1073741824` bytes. The
   default makes 32212254720 bytes.
7. Call `fchown` to the instance uid and gid, `fsync` the file and the
   directory, and close. The host never opens the file again.
8. The host never formats, mounts, checks, resizes or reads the disk. box-guest
   formats it on first boot (T-043).
9. Hard links (T-086), `stat` of the link count and `unlinkat` (T-091) do not
   open the file.
10. The lock decision: the daemon holds one lock per instance while it runs
    (T-026). The helper holds `store/instances/<id>/lock` for each operation.
11. The helper refuses `start` while a verified VMM for the id runs
    (`helper::refuses_double_start`).
12. The brief asked for an agent-box `flock` on the data disk itself. The
    jailer closes the descriptor, so the locks above take its place, as
    docs/engines.md says.
13. The disk settings are those of T-084: `Sync`, `Writeback` and discard. The
    base image is read-only, and discard does not apply to it.
14. Probe hole punching on the store filesystem with a temporary file (T-092).
    Without it, discard gives no space back. That is the note
    `discard-unsupported`, which docs/cli.md does not list yet.
15. Add that note name to the tables of docs/cli.md.

## Prove

```sh
cargo test -p box-helper create::
sudo env "PATH=$PATH" cargo test -p box-daemon --test linux_boot -- --ignored disk_opens
```

Expected: the unit tests refuse a size above the bound and a size above the free
space of a small planted filesystem. The live test sets a fanotify mark
(`FAN_OPEN`) on the data disk after `create`.

It then runs start, a write of 1 GiB in the guest, stop, start and delete. Each
recorded open comes from the VMM pid of that boot, and from no other process.
`stat` gives a size of 32212254720 bytes and, after create, fewer than 1 MiB of
allocated blocks.

Host: any host for the unit tests; a Linux KVM host with root for the live
test.

## Start condition

T-081 and T-082.

# T-088: Start, boot handshake, stop and kill

**Source:** The planning brief of 2026-10-14, sections 4.1 and 7. The research
document, section 8 (step S5). docs/protocol.md, sections "The handshake" and
"Boot order and deny first".
**Category:** feature
**Milestone:** M3
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

Start is the first point where the guest runs. A boot that hangs must not hold
resources for ever, and a stop must end with no VMM, no jail and the disk
intact. Many earlier entries prove their live checks only once an instance
boots, so this entry also carries those checks.

## Premise

- Design (docs/protocol.md): the daemon creates its listeners on ports 5002
  and 5003, then connects to port 5000 and retries until a boot deadline. This
  entry sets the deadline.
- Design (docs/protocol.md): `HELLO` carries `resume` false after a cold boot.
  `POWEROFF` with `reason` 1 is the first step of `stop`.
- READ: Firecracker exits on a guest CPU reset. `SendCtrlAltDel` exists on
  Intel and AMD only
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/actions.md:34-40`).
- READ: a `reboot` in the guest ends Firecracker, because Firecracker does not
  implement guest power management
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/getting-started.md:336-338`).
- So a guest power-off call may halt the vCPUs and leave the VMM process
  running. T-040 ends with a power-off call. Step 12 measures it.
- READ: a guest-initiated vsock connection makes the VMM thread call a
  blocking `connect` on the host listener
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/devices/virtio/vsock/unix/muxer.rs:640`).
- READ: writing 1 to `cgroup.kill` kills every process of the cgroup and its
  descendants (https://docs.kernel.org/admin-guide/cgroup-v2.html, read
  2026-10-14).
- READ: `InstanceStart` to guest init takes 125 ms or less with 1 vCPU, 128
  MiB and a minimal image (research document, section 2.2). A Debian userland
  is not minimal. Not measured.
- Design (main-session ruling on live proves): M1 and M2 entries list their
  live checks here, and this entry names them.

## Approach

1. Write `start` and `stop` of the Engine trait in
   crates/box-daemon/src/engine/firecracker.rs. The daemon state goes to
   `starting`.
2. Send the helper operation `start` with the id, the kernel and base digests,
   cpus and memory. The helper runs T-086 and replies with the pid, its start
   time and the path of `v.sock`.
3. Bind the listeners `v.sock_5002` and `v.sock_5003` at once. Accept on them
   on their own tokio tasks, so that a blocking `connect` of the VMM thread
   returns at once.
4. Connect to port 5000 through box-transport (T-024). Retry with backoff
   until the boot deadline.
5. Set the boot deadline to 30 s. It is a design value. T-181 measures the boot
   time, and the deadline then gets a margin over the measured p99.
6. Send `HELLO` with `resume` false, and expect `WELCOME` within the deadline.
   Then provision (T-041). The state goes to `running`.
7. On a missed deadline, call the helper `kill` and report "no handshake within
   30 s". The instance goes to `stopped`, and the disk stays.
8. `stop(id, grace)`: on a paused instance, resume first. Send `POWEROFF` with
   `reason` 1 and expect `POWEROFF_ACK`.
9. Wait up to `grace` for the end of the control connection. On x86_64, then
   call the helper `stop`, which sends `SendCtrlAltDel`, and wait 5 s more.
10. Always end with the helper `kill`. It is idempotent and removes the jail.
    The state goes to `stopped`. Stop on a stopped instance does nothing and
    succeeds.
11. The helper `kill`: verify the pid with the checks of T-090. Write 1 to
    `cgroup.kill` of the leaf when the file exists. Else send `SIGKILL` to the
    verified pid only.
12. Wait until the pid is gone, with a limit of 10 s. Remove the jail (T-086)
    and the cgroup leaf. Reply with the absences of jail and leaf.
13. Measure what a guest power-off call does under Firecracker v1.17.0 on
    x86_64 and aarch64. If the VMM stays up, box-guest ends with the reboot
    call, and T-040 changes.
14. With `reboot=k` and `StopOnReset` under HCS (T-105), the reboot call ends
    the instance on both engines.
15. Write the live boot tests in crates/box-daemon/tests/linux_boot.rs, marked
    `#[ignore]`. They open sessions on port 5001 directly. The CLI attach is
    T-172.
16. The file holds the boot test of this entry, `boot_handshake_stop`, and the
    live checks of these entries:

    | entry | test | check |
    | --- | --- | --- |
    | T-024 | `hybrid_vsock_live` | connect and listen through a real Firecracker |
    | T-031 | `forged_event_refused` | a uid 1000 process without the nonce is refused, and the counter grows |
    | T-040 | `pid1_boot_poweroff` | box-guest boots as PID 1, answers the handshake and powers off |
    | T-041 | `no_secret_on_disk` | after provisioning, the raw data disk file of a stopped instance holds no marker |
    | T-043 | `data_disk_survives_restart` | a marker survives stop and start, and the base digest stays |
    | T-044 | `boot_time_sync` | the guest clock is within 1 s of the host after boot |
    | T-061 | `vsock_transport` | vsock works with the T-060 kernel |
    | T-062 | `boot_base` | the base image reaches the handshake |
    | T-063 | `kage_version` | `kage --version` as uid 1000 prints the pin |
    | T-064 | `boot_variants` | each variant prints its toolchain version |
    | T-065 | `image_format_bench` | the boot numbers of each image format |
    | T-256 | `no_secret_in_cmdline` | the guest /proc/cmdline and the VMM argv hold no provisioned value |

17. The pause half of T-044 is in T-089.

## Prove

```sh
cargo xtask linux --kvm -- cargo test -p box-daemon --test linux_boot -- --ignored boot_handshake_stop
sudo env "PATH=$PATH" cargo test -p box-daemon --test linux_boot -- --ignored
```

Expected: `boot_handshake_stop` reaches `WELCOME` within the deadline, stops
the instance and finds no VMM, no jail and no leaf. A second stop succeeds and
does nothing. Every test of the table passes.

A planted kernel that never answers ends in `stopped` after the deadline, with
no VMM left. Host: a Linux KVM host with root. The first command is the nested
development run on Windows; it needs the helper and the cgroup root in the
runner, which is not measured.

## Start condition

T-062, T-081, T-084, T-085 and T-086. The live checks of the table also need
their own entries.

# T-089: Pause and resume

**Source:** The planning brief of 2026-10-14, section 4.1; the engine
evaluation, the Linux lifecycle reader; docs/engines.md, section "Pause, stop,
start and hibernate".
**Category:** feature
**Milestone:** M3
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

The operator pauses an instance to free CPU without a reboot. A pause that
drops the host channels ends every session of the instance. The guest clock
stands still during a pause, and TLS needs a correct clock after it.

## Premise

- READ: before v1.17.0, a bare pause and resume blocked host-to-guest vsock
  delivery. Each new host-initiated connection hung. v1.17.0 fixes it (#6100,
  `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:CHANGELOG.md:83-90`).
- READ: `PATCH /vm` takes `Paused` or `Resumed`
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:1829-1840`).
  It is valid only after boot (same file, `:832-836`).
- READ: on x86_64, Firecracker calls `kvmclock_ctrl` for each vCPU at a
  pause, so that the guest soft-lockup watchdog does not fire
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/vstate/vcpu.rs:257`).
- Design (docs/protocol.md): the liveness deadline does not run while an
  instance is paused. After resume, the daemon sends `TIME_SET` on port 5000.
- Design (architecture): a pause lives in the VMM. When the VMM or the host
  goes away, the instance shows `stopped`.

## Approach

1. The helper operations `pause` and `resume` in
   crates/box-helper/src/linux/pause.rs verify the VMM pid (T-090). Then they
   send `PATCH /vm` through the API client (T-085).
2. Expect 204. Read `GET /` and require `Paused` after a pause and `Running`
   after a resume. Anything else is `REFUSAL` `verification_failed`.
3. `pause` of a stopped instance is `REFUSAL` `wrong_state`. A second pause of a
   paused instance succeeds and does nothing.
4. In crates/box-daemon/src/engine/firecracker.rs, set the state `paused`
   after the helper answers. Stop the liveness deadline of port 5002.
5. On resume, set `running`, start the liveness deadline, and send `TIME_SET`
   with the host time (T-044). Expect `TIME_ACK`.
6. Keep every host connection open across the pause: ports 5000, 5001 and 5004
   and the listeners 5002 and 5003.
7. Write the live test `pause_keeps_sessions` in
   crates/box-daemon/tests/linux_boot.rs. It opens a PTY session on port 5001
   directly, runs a read loop, and pauses for 60 s.
8. After resume, the test sends a line on the same session and reads its echo.
   The event channel on port 5002 shows no new authentication.
9. Add the pause half of T-044 as `pause_time_sync`: pause 600 s, resume, and
   read `TIME_ACK`. The offset after the step is within 1 s.
10. Kill the VMM during a pause in the test `pause_lost_on_vmm_exit`. The
    inventory then shows `stopped` (T-090), and a start cold-boots from the
    disk.

## Prove

```sh
sudo env "PATH=$PATH" cargo test -p box-daemon --test linux_boot -- --ignored pause_keeps_sessions pause_time_sync pause_lost_on_vmm_exit
```

Expected: the session sends and receives after a 60 s pause with no reconnect.
The guest clock is within 1 s of the host after a 600 s pause. A VMM killed
during a pause shows `stopped`, and the next start reaches the handshake.

Host: a Linux KVM host with root.

## Start condition

T-085 and T-088.

# T-090: Inventory, re-adoption and orphans

**Source:** The planning brief of 2026-10-14, section 4.1; the research
document, section 5.5 and its correction; docs/engines.md, section
"Re-adoption and orphans".
**Category:** feature
**Milestone:** M3
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

Instances are persistent, so a daemon restart must not stop them. The daemon
must find each running VMM again and trust it again. A pid that the kernel gave
to another process, or a VMM that no record claims, must never get a signal on
a guess.

## Premise

- READ (research document, section 5.5): orphan identity is a pid file plus a
  verification, never the existence of a directory. Without the check, the
  daemon could signal a reused pid.
- READ: the jailer stores the child pid in `firecracker.pid` in the jail root
  in every mode
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:307-314`).
- READ: the jailer does not clean up after itself
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:283-286`).
- Design (docs/engines.md): six checks verify one VMM, and they compare device
  and inode, not paths. Whether `readlink` works across the mount namespace is
  not measured.
- Design (docs/protocol.md, "Re-adoption and rotation"): the daemon binds its
  listeners again and sends `HELLO` with `resume` true. Then it rotates the
  event nonce and the egress token.
- Design: the research document killed every VMM at a daemon restart.
  Persistent instances invert that rule.

## Approach

1. Write the helper operation `inventory` in
   crates/box-helper/src/linux/inventory.rs. It reads every record of the store.
2. For each record with a pid, run the checks of docs/engines.md in order:
   - 1: the pid and start time come from the record, never from the jail pid
     file;
   - 2: `/proc/<pid>/stat` field 22 equals the recorded start time;
   - 3: `stat` of `/proc/<pid>/exe` equals `root/firecracker`;
   - 4: `stat` of `/proc/<pid>/root` equals the jail `root/`;
   - 5: the pid is in `cgroup.procs` of the leaf;
   - 6: `GET /` answers, and its `state` gives `running` or `paused`.
3. Map the results with the table of docs/engines.md: 1 to 6 pass gives
   `running` or `paused`, and a missing pid gives `stopped`.
4. A failed check 6 gives `unknown`, and so does any of 2 to 5 failing while
   the pid exists. An `unknown` pid never gets a signal.
5. Scan the leaves under the cgroup root and the directories under the jail
   base. A leaf or a jail with no helper record is an entry with `claimed`
   false.
6. Measure checks 3 and 4 with `stat` and with `readlink` on a KVM host. Keep
   the form that works, and write the result into docs/engines.md.
7. In crates/box-daemon/src/readopt.rs, at each daemon start, read the daemon
   records (T-026) and the encrypted per-instance secrets (T-140).
8. Ask the helper for `inventory`. Join the two lists by id.
9. For each `running` or `paused` instance, remove the old listener sockets
   `v.sock_5002` and `v.sock_5003` when they are sockets. Never follow a link.
   Then bind them again.
10. Connect to port 5000 and send `HELLO` with `resume` true. `WELCOME` must
    show `provisioned` true. Otherwise the guest rebooted, and the daemon
    provisions it from the start.
11. Rotate the event nonce and the egress token with `ROTATE` (kinds 1 and 2).
    Accept only the new values after `ROTATED`.
12. An instance with a daemon record and no VMM is `stopped`. An instance whose
    query failed is `unknown`. In `unknown`, only stop and delete are accepted.
13. Orphans: a VMM whose id has no daemon record, or a jail or leaf with no
    helper record, is unclaimed. The daemon removes it with the helper `kill`.
14. `kill` accepts an id with a helper record, or an id whose jail or leaf the
    inventory found. It runs checks 3 to 5 before any signal
    (`helper::never_signals_unverified_pid`).
15. The daemon never deletes the data disk of an unclaimed instance. It
    reports the id and the disk path, and the operator decides.

## Prove

```sh
sudo env "PATH=$PATH" cargo test -p box-daemon --test linux_boot -- --ignored readopt_three orphan_removed
```

Expected: `readopt_three` starts three instances and kills the daemon with
`SIGKILL`. After the restart, all three are `running`. A PTY session on port
5001 in each reads a larger `/proc/uptime` than before, so no guest rebooted.

The test also sees a refused frame with the old event nonce. `orphan_removed`
removes the daemon record of a fourth running instance before the restart.
After the restart, its VMM is gone, its data disk stays, and the report names
it.

Host: a Linux KVM host with root.

## Start condition

T-088 and T-089.

# T-091: Delete with verified absence

**Source:** The planning brief of 2026-10-14, section 4.1; the research
document, section 9 (check 9); docs/engines.md, section "Delete with verified
absence".
**Category:** feature
**Milestone:** M3
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

A delete that removes a path but leaves the bytes somewhere else is not a
delete. A cgroup can go while the jail stays. A freed uid can still own a
file. Both give the next instance the leftovers of the last one.

## Premise

- READ (research document, section 9, check 9): after teardown, a content probe
  over the state directory finds no byte that the agent wrote. An absent path
  is not enough.
- READ: cleanup after the jailer is the job of the user
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:283-286`).
- Design (main-session ruling on verbs): delete refuses a running or paused
  instance. Stop first, or `--force` stops it.
- Design (docs/protocol.md): the absence items are 1 jail, 2 cgroup leaf, 3
  data disk, 4 instance directory, 5 uid and 9 snapshot files.

## Approach

1. Write the helper operation `delete` in crates/box-helper/src/linux/delete.rs.
2. Refuse a verified running VMM with `wrong_state`
   (`helper::refuses_delete_while_running`).
3. Remove the jail with the no-follow teardown of T-086, and the cgroup leaf
   with `rmdir`.
4. Remove `data.img`, the record, the lock and the instance directory with
   `unlinkat` from a file descriptor of the store. Remove the snapshot
   directory of the id when T-184 has made one.
5. Free the uid only after two checks: no process runs as it, and no file under
   the store or the jail base belongs to it. Walk with `fstatat` and
   `AT_SYMLINK_NOFOLLOW`.
6. Verify each absence and reply with the list of items and `absent` flags. An
   item that is still present is `REFUSAL` `absence_not_verified`, with the item.
7. In the daemon, delete the instance record, the lock, the encrypted
   per-instance secrets and the event log of the instance.
8. The event log holds agent-reported bytes. The operator who wants it exports
   it before delete (T-176).
9. Write the live test `delete_verified_absence`. It writes a random 32-byte
   marker into a guest file and sends it once through an exec session and once
   as an event.
10. After delete, the test scans every file under the daemon state directory,
    the store and the jail base. It looks for the marker in raw, hex and base64
    form.
11. NOTE: freed blocks of the host filesystem keep their bytes until something
    overwrites them. A scan of files cannot see them. Write that limit into
    docs/limits.md.

## Prove

```sh
cargo test -p box-helper delete::
sudo env "PATH=$PATH" cargo test -p box-daemon --test linux_boot -- --ignored delete_verified_absence
```

Expected: the reply lists items 1 to 5, each `absent` true. The scan finds no
marker. The allocator lists the uid as free. A delete of a running instance is
refused. Host: any host for the unit tests; a Linux KVM host with root for the
live test. T-180 runs the same check in its battery.

## Start condition

T-088 and T-090.

# T-092: Linux probe and startup report

**Source:** The planning brief of 2026-10-14, sections 4.1 and 8; the research
document, sections 1.1 (C4) and 10; docs/cli.md, the gap and note tables.
**Category:** feature
**Milestone:** M3
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

The daemon must know at start whether this host can run the engine, and say
why when it cannot. A probe that reads a configuration or the presence of a
binary reports a jail that does not work. A host outside what Firecracker
validates must be visible to the operator.

## Premise

- READ (research document, section 1.1, claim C4): the probe attempts the real
  setup and does not read configuration. A gap stops the daemon.
- READ (research document, section 1.3): the jail directory exists before
  `unshare` fails. A probe that reports success on a directory reports a jail
  that does not work.
- READ: Firecracker validates the host kernels 5.10, 6.1 and 6.18, all with 4K
  pages
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:24-30`).
- MEASURED in one run on 2026-10-14 (i7-12700H, 20 logical CPUs, 64 GB). On
  that Windows 11 Pro host of the operator, `/dev/kvm` works in a wsl-toolkit
  job, with `kvm_intel` `nested=Y` ([docs/status.md](../docs/status.md)).
- Design (docs/cli.md): the gap names `no-kvm`, `engine-missing`,
  `engine-digest-mismatch`, `helper-missing`, `helper-refused`,
  `jail-unavailable`, `cgroup-delegation-missing`, `memory-max-missing` and
  `page-size-unsupported`.
- Design (docs/cli.md): the notes `host-kernel-unvalidated`,
  `nested-virtualization` and `io-max-missing`.
- Design (main-session ruling on live proves): this entry proves through the
  probe module tests and the daemon startup report of T-028, not through
  `agent-box doctor` (T-175).

## Approach

1. Write the helper operation `probe` in crates/box-helper/src/linux/probe.rs.
   Each probe runs the real call and returns a fact with its evidence.
2. KVM: open `/dev/kvm`, call `KVM_GET_API_VERSION` and expect 12. Call
   `KVM_CREATE_VM` and close it. A failure is the gap `no-kvm`.
3. Jail: build a throwaway jail with a random id. Run `firecracker --version`
   through the jailer in it, then tear it down. A failure is `jail-unavailable`.
4. Cgroup: the root of T-082 exists and lists `memory` and `pids`. Otherwise
   `cgroup-delegation-missing` or `memory-max-missing`. A missing `io` is
   `io-max-missing`.
5. Binaries: hash firecracker and the jailer against the pins (T-083). A
   missing file is `engine-missing`, and a wrong digest is
   `engine-digest-mismatch`.
6. Host kernel: compare the release of `uname` with 5.10, 6.1 and 6.18. Another
   line is the note `host-kernel-unvalidated`.
7. Page size: on aarch64, a `sysconf(_SC_PAGESIZE)` other than 4096 is the gap
   `page-size-unsupported` (T-094).
8. Nested virtualization: the `hypervisor` flag in `/proc/cpuinfo` on x86_64
   is the note `nested-virtualization`.
9. Store: the store and the jail base have one `st_dev`. A hole punch on a
   temporary file in the store works, or the note `discard-unsupported` applies
   (T-087).
10. Swap: a non-empty `/proc/swaps` is a note, because Firecracker advises
    against host swap.
11. THP: read `/sys/kernel/mm/transparent_hugepage/enabled` as a fact. The
    builder uses `huge_pages` `None`, so it changes nothing.
12. `cgroup.kill`: record whether the file exists in the leaf of the throwaway
    jail (T-088).
13. In the daemon, crates/box-daemon/src/engine/firecracker.rs maps the facts
    to a `CapabilityReport` (T-032). The daemon checks the helper socket
    itself, so a missing helper is the gap `helper-missing`.
14. The daemon prints the report at start through the startup report of T-028.
    It exits 10 on an engine gap and 11 on a helper gap (docs/cli.md).
15. Write pure tests that map planted facts to gaps and notes, with no
    privilege.

## Prove

```sh
cargo test -p box-helper probe::
cargo test -p box-daemon engine::firecracker::report
sudo env "PATH=$PATH" cargo test -p box-daemon --test linux_boot -- --ignored probe_report
cargo xtask linux -- cargo test -p box-daemon --test linux_probe -- --ignored no_kvm_gap
```

Expected: the unit tests map each planted fact to its gap or note. On the KVM
host, the startup report names every probe of the Approach with its evidence,
and the daemon runs.

In the Linux runner without `--kvm`, the report names `no-kvm` and
`helper-missing`, and the daemon exits 10. Host: any host for the unit tests and
the last command; a Linux KVM host with root for the third.

## Start condition

T-028, T-032, T-081 and T-083.

# T-093: Block I/O and the reopening condition

**Source:** The planning brief of 2026-10-14, section 4.1. The research
document, section 3.5. The engine evaluation: the Linux lifecycle reader and the
adversarial review, item 7.
**Category:** measurement
**Milestone:** M8
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

Persistent instances run cargo and npm on their data disk all day. The safe
Firecracker settings give up a faster engine, and they share one thread with
every host channel. If block I/O dominates the cost of agent work, another VMM
may serve better. Only a measurement can decide.

## Premise

- READ (research document, section 3.5): two conditions together replace
  Firecracker. A measurement shows that block I/O or memory pressure
  dominates. No Firecracker release meets the need.
- READ: Firecracker's own `Async` measurements on NVMe show reads with 1.5 to 3
  times the efficiency and up to 30 times the IOPS. `Async` is a developer
  preview and has no discard (engine evaluation, Linux reader).
- READ: Firecracker virtio-block has one queue per device
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/devices/virtio/block/virtio/mod.rs:31`).
- READ: Cloud Hypervisor v53.0 has multiqueue virtio-block, io_uring with
  discard and an optional `O_DIRECT` (engine evaluation, Linux reader).
- READ: interleave the cases, one of each per round, because page-cache warmth
  made a case look faster
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:docs/reference/benchmarks.md:11-20`).
- Not measured: every number of this entry. T-015 gives the first vsock latency
  under load in M0.

## Approach

1. Add the harness command `cargo xtask bench block`, planned file
   xtask/src/bench/block.rs. It drives the cases on a Linux KVM host.
2. Case A: Firecracker v1.17.0 through agent-box, with `Sync`, `Writeback` and
   discard.
3. Case B: Firecracker v1.17.0 without the jailer, with `Async` and
   `Writeback`. It is a measurement only; the builder refuses `Async`.
4. Case C: Cloud Hypervisor v53.0 with io_uring, `num_queues` equal to the
   vCPU count, `sparse=on` and discard. It runs without a jail; it is a
   measurement only.
5. Give each case the same kernel, base image, vCPU count, memory and a fresh
   30 GiB sparse data disk.
6. Workload 1: `cargo build --release` of one pinned Rust project, then
   `cargo clean`, 5 cycles.
7. Workload 2: `npm ci` of one pinned project with a lock file, then
   `rm -rf node_modules`, 5 cycles.
8. Record per cycle: the wall time, the host CPU time of the VMM cgroup
   (`cpu.stat` `usage_usec`), and the allocated size of the data disk file.
9. Run `fstrim` in the guest after each cycle for cases A and C. Record the
   allocated size again.
10. During workload 1, record the vsock round-trip latency p50 and p99 with the
    harness of T-015.
11. Interleave the cases per round. Run 3 rounds. Write the host model, the
    kernel, the filesystem of the store and whether the host is a VM.
12. A nested host gives development numbers only. The decision needs a host
    that is not itself a VM.
13. Write the page docs/research/block-io.md with the tables and the
    conditions. Link it from docs/status.md and docs/engines.md.
14. Put the numbers before the operator. The operator writes the decision into
    docs/decisions.md: keep Firecracker, or open an entry for Cloud Hypervisor
    with a jail that agent-box builds.

## Prove

```sh
cargo xtask bench block --case fc-sync --case fc-async --case clh --rounds 3 --json
cargo xtask check docs
```

Expected: the first command prints one table per workload and case with wall
time, CPU time, allocated size before and after fstrim, and vsock latency. The
page and the decision row exist, and the docs check passes. Host: a Linux KVM
host that is not itself a VM.

## Start condition

T-015, T-088 and T-089.

# T-094: aarch64 on 16K-page hosts

**Source:** The planning brief of 2026-10-14, section 4.1; the adversarial
review, item 8; docs/engines.md, section "aarch64 hosts".
**Category:** measurement
**Milestone:** backlog
**Priority:** P3
**Effort:** M
**Status:** open

## Problem

Common aarch64 desktops and boards run kernels with 16K pages. Firecracker
neither tests nor lists such hosts. agent-box limits aarch64 to 4K-page hosts
until a boot on a 16K-page host is measured, and nobody has measured one.

## Premise

- READ: Firecracker tests aarch64 only on AWS Graviton 2 to 5
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:README.md:148-152`).
- READ: its kernel policy lists host kernels with 4K pages only
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:24-30`).
- READ: the guest memory code asks the host for its page size
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/vstate/memory.rs:166`).
  So a 16K host may work, but nobody supports or tests it.
- READ by the adversarial review in web sources: Raspberry Pi 5 and Asahi Linux
  default to 16K pages.
- Design (main-session ruling on aarch64): release binaries for aarch64 Linux
  serve 4K-page hosts only. T-201 verifies on a 4K-page aarch64 host.
- Design: QEMU under libvirt is the recorded fallback for 16K-page hosts.

## Approach

1. Get a 16K-page aarch64 host: a Raspberry Pi 5 with its default kernel, or an
   Apple silicon machine with Asahi Linux. Record the model and the kernel.
2. Run the probe of T-092. Expect the gap `page-size-unsupported`.
3. Boot the T-060 `Image` and the base image with Firecracker v1.17.0 directly,
   in a throwaway harness under .work/fc-16k/. agent-box keeps its gap, and no
   setting removes it.
4. Record the boot to the port 5000 handshake and a cargo build in the guest.
   Record a pause and resume, and a stop with the guest power-off path.
5. Run the boot 10 times and record each result.
6. Write the result to docs/status.md, with the host, the versions and the date.
7. On a pass: the operator decides whether 16K-page hosts join the support
   claim. That is a pins change and a change of the probe.
8. On a fail: write an entry for QEMU under libvirt on those hosts, or keep the
   documented limit. The operator decides.

## Prove

```sh
getconf PAGESIZE
sh .work/fc-16k/run.sh
```

Expected: the first command prints 16384. The script prints a table of the 10
boots and the other checks, pass or fail each, and exits 0. docs/status.md holds
the table. Host: a Linux KVM host on aarch64 with 16K pages.

## Start condition

T-060 and T-092.
