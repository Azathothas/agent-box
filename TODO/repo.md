# Repository and tooling

This area holds the repository itself: the Cargo workspace, the `cargo xtask`
commands, the record checker, the gate and CI, and the M0 spikes.
[docs/development.md](../docs/development.md) tells how to run each command.
[docs/conventions.md](../docs/conventions.md) gives the rules that the checks
enforce. [RULES.md](RULES.md) gives the rules of this record.
[docs/references.md](../docs/references.md) holds the pins table.

The operator ruled on 2026-10-14 (D14 in
[docs/decisions.md](../docs/decisions.md)) that the planning session builds
only the tooling, the scripts and the documents. There is no crates/ directory.
Entry T-033 creates the product crates in the first implementation session.

# T-001: Cargo workspace, pinned toolchain and lints

**Source:** The planning brief of 2026-10-14, sections 10 to 12, and ruling D14.
**Category:** chore
**Milestone:** M0
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

Every later entry builds in one workspace. A toolchain that floats lets a new
lint fail CI on code that passed on the developer host. Strict lints cost
little on an empty tree and much on a large one.

## Premise

- Design: ruling D14 limits the planning session to the tooling and the
  documents. The workspace therefore has one member, xtask.
- Design: the brief gives the planned crate graph (section 11). T-002 checks
  it, and T-033 creates the crates.
- Design: `unsafe_code` is `deny` at the workspace level, not `forbid`. A
  module can then allow unsafe code for itself, and `forbid` would not let it.
- NOTE: T-025 (the AF_HYPERV socket calls) and the HCS calls of the Windows
  engine need `unsafe`. Decision P6 lists the files that allow it, and T-259
  checks the list.

## Approach

1. Write `Cargo.toml` with resolver 3 and one member, xtask.
2. Put the planned crate graph in a header comment of `Cargo.toml`.
3. Share the package fields: edition 2024, rust-version 1.99, licence 0BSD,
   publish false.
4. Declare each workspace dependency with the reason for it next to it.
5. Set the workspace lints: `unsafe_code` deny, and clippy pedantic as a
   warning. The gate turns each warning into an error.
6. Pin the toolchain in `rust-toolchain.toml` to 1.99.0, with rustfmt and
   clippy.
7. Set `rustfmt.toml` to a maximum width of 120, as in the sibling repository
   podssh.
8. Give `.cargo/config.toml` the aliases `cargo xtask` and `cargo todo`.

## Prove

```sh
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask linux -- cargo clippy --workspace --all-targets -- -D warnings
```

Each command exits 0. The first two need any host with the pinned toolchain.
The third needs the operator's Windows 11 host with wsl-toolkit, or a Linux
host with podman or docker.

## Done

2026-10-14, the squashed initial commit. The measured output is in [docs/status.md](../docs/status.md).

- `Cargo.toml`: resolver 3, the member xtask, and the planned graph in a
  comment. The package fields are shared.
- The workspace lints are `unsafe_code` deny, `missing_debug_implementations`
  and `unreachable_pub` as warnings, and clippy pedantic as a warning.
- Two clippy lints are allowed, each with a reason in the file:
  `module_name_repetitions` and `missing_panics_doc`.
- The release profile has one codegen unit, fat LTO, abort on panic, and
  stripped symbols.
- `rust-toolchain.toml` pins 1.99.0 with rustfmt and clippy, profile minimal.
- `rustfmt.toml` sets `max_width` 120 and `use_small_heuristics` Max.
- `.cargo/config.toml` gives the two aliases. The resolver falls back to
  versions that the declared minimum Rust allows.

# T-002: `cargo xtask layering`: the crate graph goes down only

**Source:** The planning brief of 2026-10-14, section 11; kage keeps the same rule.
**Category:** feature
**Milestone:** M0
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

A crate that depends upward ties the graph into a knot. box-guest must stay
small, because it is a static musl binary. box-helper must stay small, because
it runs as root or LocalSystem. A rule that nobody checks drifts.

## Premise

- Design: the brief gives the layers. Layer 0 is box-core. Layer 1 is
  box-proto, box-policy, box-image and box-transport.
- Design: layer 2 is box-vmm, box-net and box-broker. Layer 3 is box-daemon.
  The binaries are box-cli, box-helper and box-guest.
- Design: box-chat, box-web and box-pool are late crates. Their entries create
  them.
- READ: kage keeps a strict downward graph at both pins
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/reference/architecture.md:6-26`).
- READ: the research document keeps the guest crate separate, because it
  compiles to static musl and must not take host dependencies
  ([docs/research/microvm-tier.md](../docs/research/microvm-tier.md), section 4).

## Approach

1. Put the layer table of the planned graph in `xtask/src/layering.rs`.
2. Refuse an edge from a lower layer to a higher one.
3. Refuse a box-guest dependency other than box-core and box-proto.
4. Refuse a box-helper dependency other than box-core, box-proto and box-vmm.
5. Refuse any dependency on a binary crate: box-cli, box-helper, box-guest or
   xtask.
6. Refuse a dependency on box-vmm from any crate other than box-helper.
7. Refuse a dependency of box-chat, box-web or box-pool on box-broker or
   box-vmm. Refuse a workspace dependency of xtask.
8. Report a workspace member that the table does not name. A new crate gets
   a row first.
9. Register the check as `layering` in `xtask/src/check/mod.rs`, so the gate
   runs it.

## Prove

```sh
cargo xtask layering
cargo test -p xtask
```

The first exits 0. In the second, a unit test plants an upward edge in a
fixture graph and asserts a finding. Any host.

## Done

2026-10-14, the squashed initial commit. The measured output is in [docs/status.md](../docs/status.md).

- `xtask/src/layering.rs` holds the full layer table, also for the crates that
  do not exist yet.
- It applies the rules of the Approach to the members of the workspace. Today
  the only member is xtask.
- The check reports through `xtask/src/report.rs`: exit 0 pass, 1 fail, 2 could
  not run, and `--json`.

# T-003: `cargo xtask todo`: the record checker and writer, ported from podssh-todo

**Source:** The planning brief of 2026-10-14, section 12 and decision P2; podssh-todo.
**Category:** feature
**Milestone:** M0
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

Closing one entry changes the entry, its row in the index and two counts. When
a person types these, they disagree within days. A citation of a line moves
when the cited file changes.

## Premise

- READ: podssh-todo has the commands check, set, counts, next, remap and
  interop
  (`Azathothas/podssh@3ad320382891d1d5697aadedd689ed07cd962e4f:crates/podssh-todo/src/lib.rs:23-31`).
- READ: its constants are podssh values: milestones M3 to M9, no `decision`
  category, and `issues.md` as a record file
  (`Azathothas/podssh@3ad320382891d1d5697aadedd689ed07cd962e4f:crates/podssh-todo/src/model.rs:7-23`).
- READ: its parser refuses an H1 that is not an entry
  (`Azathothas/podssh@3ad320382891d1d5697aadedd689ed07cd962e4f:crates/podssh-todo/src/parse.rs:76-89`).
- Design: each area file of this repository opens with one H1 title before its
  entries. The port must accept that line.
- Design: decision P2 copies and adapts the checker. T-016 records the drift
  risk.

## Approach

1. Copy the modules model, parse, check, write, refs, remap, diff and numbers
   into xtask/src/todo/.
2. Set the milestones to M0 to M12 and backlog.
3. Add the category `decision` to the podssh categories.
4. Set the record files to INDEX.md, PROGRESS.md, RULES.md and upstream.md.
5. Leave out the podssh-only command `interop`.
6. Keep `--base REV` for check and remap.
7. Run the reader as the check `todo` in the gate.
8. Port the unit tests of the copied modules with them.

## Prove

```sh
cargo todo check
cargo test -p xtask
cargo todo set T-016 partial
cargo todo set T-016 open
```

`cargo todo check` exits 0 on this tree. The ported tests pass. After the first
`set`, the counts in TODO/INDEX.md and TODO/PROGRESS.md show one more partial
entry. The second `set` restores them. Any host.

## Done

2026-10-14, the squashed initial commit. The measured output is in [docs/status.md](../docs/status.md).

- `xtask/src/todo/mod.rs` and the copied modules implement check, set, counts,
  next and remap, with the adapted constants of the Approach.
- `cargo todo` is the alias in `.cargo/config.toml`.
- The gate runs the reader through the check `todo` of
  `xtask/src/check/mod.rs`.

# T-004: Text and prose checks (ASCII, control bytes, line endings, STE heuristics)

**Source:** The planning brief of 2026-10-14, sections 9 and 12; TEMPLATE check-markers.sh and check-control-bytes.sh.
**Category:** feature
**Milestone:** M0
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

A non-ASCII character or a control byte hides from review. A long or vague
sentence in a document is a defect that the next session acts on. The ruling
that every text is ASCII-STE needs a machine half.

## Premise

- READ: a character rule that scanned Markdown only left 2290 characters in
  22 script files of TEMPLATE, measured there on 2026-08-28
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-markers.sh:15-20`).
- READ: a literal control byte makes grep call the file binary, and git diff
  then shows no diff
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-control-bytes.sh:4-11`).
- READ: a scope of tracked files only misses a new file, and a scope relative
  to the working directory misses whole trees
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-control-bytes.sh:25-38`).
- READ: a word list cannot catch a metaphor used as jargon. That stays a review
  reading
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-docs.sh:28-35`).
- Design: brief section 9 sets 25 words for a descriptive sentence and 20 for
  a procedural one. It also gives the banned words.

## Approach

1. Scan tracked files plus untracked files that git does not ignore.
2. Read only files that `xtask/src/repo.rs` names as text.
3. In `xtask/src/check/text.rs`, refuse a byte above 127 and a control byte.
4. Refuse a carriage return, and a file without a final newline.
5. In `xtask/src/check/prose.rs`, read the Markdown documents.
6. Skip fenced blocks and block quotes.
7. Count a code span as one word, and a link as its text.
8. Refuse a sentence of more than 25 words, in paragraphs, list items and
   table cells.
9. Refuse each banned word of the brief, in any case.
10. Give each check a floor, so that an empty scope reports "could not run".

## Prove

```sh
cargo xtask check text prose
cargo test -p xtask
```

The first exits 0. The tests plant a non-ASCII byte, a carriage return, a file
without a final newline, a sentence of 26 words and a banned word. Each plant
gives a finding. Any host.

## Done

2026-10-14, the squashed initial commit. The measured output is in [docs/status.md](../docs/status.md).

- `xtask/src/check/text.rs` and `xtask/src/check/prose.rs` implement the
  Approach.
- Both are registered in `xtask/src/check/mod.rs` as `text` and `prose`.
- NOTE: the prose check counts words. It cannot judge meaning. One term for one
  thing stays a review reading.

# T-005: Document checks: links, anchors, orphan pages, repository citations, one fact one home

**Source:** The planning brief of 2026-10-14, section 12; TEMPLATE check-docs.sh and check-one-home.sh.
**Category:** feature
**Milestone:** M0
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

A document that was true when somebody wrote it becomes false when a file moves.
A fact with two homes drifts, and the reader trusts the copy that they saw
first.

## Premise

- READ: TEMPLATE names the shapes of a stale document, the first of which is a
  link or path that stopped resolving
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-docs.sh:5-16`).
- READ: a project built from TEMPLATE held 8 sentences that appeared verbatim
  in two documents
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-one-home.sh:10-16`).
- READ: the first one-home instrument reported zero duplicates over zero files.
  The collector had matched nothing
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-one-home.sh:18-27`).
- Design: brief section 12 skips the parse of fenced shell blocks.
- Design: a path of this repository in backticks must exist. A planned path is
  written without backticks.

## Approach

1. In `xtask/src/check/docs.rs`, resolve each relative link to a file that
   exists.
2. Check each heading anchor against the GitHub slug of a heading in the target.
3. Report a page under docs/ that no other document links.
4. Check that each repository path in a code span exists, with its line when
   one is given.
5. In `xtask/src/check/one_home.rs`, normalize the sentences of each document.
6. Leave out headings, table rows, fenced blocks and code spans.
7. Report a sentence that appears in two documents. Fail past a ceiling.
8. Give both checks a floor.

## Prove

```sh
cargo xtask check docs one-home
cargo test -p xtask
```

The first exits 0. The tests plant a broken link, a wrong anchor and an orphan
page, and each gives a finding. Any host.

## Done

2026-10-14, the squashed initial commit. The measured output is in [docs/status.md](../docs/status.md).

- `xtask/src/check/docs.rs` and `xtask/src/check/one_home.rs` implement the
  Approach.
- Both are registered in `xtask/src/check/mod.rs`, under the names docs and
  one-home.

# T-006: Secret, placeholder and attribution checks

**Source:** The planning brief of 2026-10-14, section 12; TEMPLATE check-no-secrets.sh, check-placeholders.sh and check-attribution.sh.
**Category:** feature
**Milestone:** M0
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

A credential that reaches a public remote stays readable after any history
rewrite. A leftover template placeholder reads as finished text. A commit that
credits a tool breaks the operator's rule, and it arrives from a default.

## Premise

- READ: the secret scan reads tracked files plus untracked files that git does
  not ignore. The next `git add -A` takes a new file
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-no-secrets.sh:5-9`).
- READ: the long-hex rule is narrowed by shape, not switched off
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-no-secrets.sh:122-143`).
- Design: ruling D9 drops the references branch. Every upstream citation now
  carries a full commit in the shape `owner/repo@<40 hex>:path:line`.
- READ: the attribution check reads git log, so it reports a commit after the
  commit exists. The hook refuses before
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-attribution.sh:17-24`).
- READ: a shallow CI checkout lets the log check read one commit. CI needs
  fetch depth 0
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/README.md:398-434`).
- Design: the operator refuses every co-author trailer. TEMPLATE refuses only a
  trailer that names a tool. The stricter rule applies.

## Approach

1. In `xtask/src/check/secrets.rs`, scan tracked plus untracked files that git
   does not ignore.
2. Refuse known credential shapes and private key headers.
3. Refuse a long hex string, except in the citation shape of the Premise.
4. In `xtask/src/check/placeholders.rs`, refuse a leftover double-brace
   placeholder of a template.
5. Refuse a leftover year or author field of a template.
6. In `xtask/src/check/attribution.rs`, read every commit of git log.
7. Refuse a co-author trailer, a generated-with line, and a tool or vendor name.
8. Exit 2 when the log check read no commit.

## Prove

```sh
cargo xtask check secrets placeholders attribution
cargo test -p xtask
```

The first exits 0. The tests plant a credential shape, an uncited long hex
string, a placeholder and a co-author trailer. Each plant gives a finding. Any
host with git.

## Done

2026-10-14, the squashed initial commit. The measured output is in [docs/status.md](../docs/status.md).

- `xtask/src/check/secrets.rs`, `xtask/src/check/placeholders.rs` and
  `xtask/src/check/attribution.rs` implement the Approach.
- The file lists come from `xtask/src/repo.rs`, which runs git from the
  repository root.
- WARNING: a green secret scan is not a clearance. It finds the shapes that it
  knows. A person still reads every new file before a push.

# T-007: `cargo xtask gate` and CI on Linux and Windows

**Source:** The planning brief of 2026-10-14, section 12; TEMPLATE check-gate.sh.
**Category:** feature
**Milestone:** M0
**Priority:** P1
**Effort:** M
**Status:** partial

## Problem

A gate that is a list is run in the order that somebody recalls. CI must run the
same gate on Linux and on Windows, because the product ships on both. The local
half exists. CI can be observed only after the one push of ruling D10.

## Premise

- READ: a session that ran its gate by hand typed a different subset each of
  five times
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-gate.sh:5-9`).
- READ: a skipped check is a skip, never a pass. `--strict` makes a skip a
  failure, which is what CI uses
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-gate.sh:14-24`).
- READ: zero passes is red, whatever the skips say
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/README.md:285-295`).
- READ: pin each third-party action to a commit. A workflow that a fork's pull
  request triggers gets no secrets
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/public/README.md:13-47`).
- READ: a GitHub windows-latest runner booted a Hyper-V-backed WSL2 machine
  (`talaria0101/malaria@499e897e8372a5db0f921542184e3b66ef610e2b:.github/workflows/wsl-battery.yml:9`).
  Whether an HCS instance boots there is not measured.
- Design: whether a hosted Linux runner exposes /dev/kvm is not measured.
  Firecracker has no mode without KVM.
- Design: T-010 leaves .git/ out of the copy in the Linux runner. Every check
  lists its files through git (`xtask/src/repo.rs`).

## Approach

1. In `xtask/src/gate.rs`, run the steps in a fixed order.
2. Step 1 is `cargo fmt --check`.
3. Step 2 is `cargo clippy --workspace --all-targets -- -D warnings`.
4. Step 3 is `cargo test --workspace`.
5. Step 4 runs every check of `xtask/src/check/mod.rs`.
6. Step 5 is `cargo deny check` against `deny.toml`.
7. Count a tool that is not installed as a skip, and print it on its own line.
8. With `--strict`, fail on a skip. With `--list`, print the steps and run
   nothing.
9. In `.github/workflows/ci.yml`, run `cargo xtask gate --strict` on a Linux
   and a Windows runner.
10. Check out with fetch depth 0, and pin each action to a commit.
11. Push once (D10). Record the URL and result of the first CI run in
    docs/status.md.
12. Run `cargo todo set T-007 done` after both jobs pass.

## Decision

The main session ruled the split on 2026-10-14. The full strict gate runs
natively on Windows and on Linux, and in CI on ubuntu-24.04 and windows-2025.
The copy in the Linux runner of T-010 has no .git/ and no cargo-deny. There,
run the cargo steps only: `cargo xtask linux -- cargo xtask gate fmt clippy test`.

## Prove

```sh
cargo xtask gate --strict
cargo xtask linux -- cargo xtask gate fmt clippy test
gh run list --workflow ci.yml --limit 1 --json conclusion,url
```

The first two exit 0 on the operator's Windows 11 host. The first also exits 0
natively on a Linux host. After the push, the third shows `success` for the
run, with both jobs green.

# T-008: `cargo xtask setup` and the commit-msg hook

**Source:** The planning brief of 2026-10-14, sections 2 (P3) and 12; TEMPLATE dotfiles/githooks/commit-msg.
**Category:** chore
**Milestone:** M0
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

git does not clone hooks, so a fresh checkout has none until somebody arms them.
git skips a hook without the executable bit and prints nothing. A hook that
needs a build cannot run in a fresh clone.

## Premise

- READ: each checkout must set `core.hooksPath`. The bit that ships is the
  index mode 100755, not the mode on disk
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/README.md:398-434`).
- READ: a hook that calls a missing script exits 2 on every commit, good and bad
  alike
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:dotfiles/githooks/commit-msg:38-44`).
- Design: decision P3 keeps the hook a self-contained POSIX sh script. The Rust
  check of git log backs it (T-006).
- Design: box-guest needs the musl targets x86_64-unknown-linux-musl and
  aarch64-unknown-linux-musl.

## Approach

1. Keep `.githooks/commit-msg` as POSIX sh with no call to another script.
2. Make the hook refuse a co-author trailer, a generated-with line, and a tool
   or vendor name.
3. Make it exit 0 for an allowed message, 1 for a refusal, and 2 when it cannot
   read the message.
4. In `xtask/src/setup.rs`, run `git config core.hooksPath .githooks`.
5. Check that the index mode of the hook is 100755.
6. Add the two musl targets through rustup.
7. Install cargo-deny with `cargo install --locked cargo-deny` when it does
   not answer. `--no-install` reports it as missing instead.
8. On Windows, check that wsl-toolkit answers.
9. Name each missing tool with the command that installs it.

## Prove

```sh
cargo xtask setup
f=$(mktemp)
printf 'Fix a typo\n\nCo-Authored-By: a tester\n' > "$f"
sh .githooks/commit-msg "$f"; echo "exit $?"
printf 'Fix a typo\n' > "$f"
sh .githooks/commit-msg "$f"; echo "exit $?"
git ls-files -s .githooks/commit-msg
```

Setup exits 0. The hook prints `exit 1` for the trailer and `exit 0` for the
plain message. The index mode is 100755. Any host with git and sh.

## Done

2026-10-14, the squashed initial commit. The measured output is in [docs/status.md](../docs/status.md).

- `.githooks/commit-msg` is self-contained POSIX sh with the three exit codes of
  the Approach.
- `xtask/src/setup.rs` arms the hook, checks the index mode, adds the musl
  targets, installs cargo-deny and checks the other tools.

# T-009: `cargo xtask doctor`: what this host can do for development

**Source:** The planning brief of 2026-10-14, section 12; TEMPLATE scripts/doctor.
**Category:** feature
**Milestone:** M0
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

A session that assumes its host plans around a tool that is not there. A tool on
PATH is not proof that it works. KVM and Hyper-V decide which entries a host can
prove.

## Premise

- READ: a probe is not a gate. A missing tool is data, so the probe exits 0 when
  it ran and 2 when it could not run
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/doctor/README.md:30-37`).
- READ: `gh` was on PATH with a dead token. Probe it with a real API call
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/mine-repo.sh:31-33`).
- READ: Firecracker has no mode without KVM. A host without KVM records a gap
  with its evidence
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/hosted-sessions.md:96-114`).
- MEASURED on 2026-10-14, one run, on the operator's Windows 11 Pro host
  (i7-12700H, 20 logical CPUs, 64 GB): /dev/kvm works in a wsl-toolkit job.
  The WSL2 kernel is 7.2.0, and kvm_intel has nested=Y.
- NOTE: that is nested virtualization. It is valid for development tests and
  never for a product claim.
- MEASURED on the same host and date, one run: `hcsdiag list` with a
  non-elevated token exits 27 ([docs/status.md](../docs/status.md)). HCS
  refuses a caller outside Administrators or Hyper-V Administrators.

## Approach

1. In `xtask/src/doctor.rs`, probe each tool by running it with a time limit
   from `xtask/src/proc.rs`.
2. Probe rustc, cargo, git, cargo-deny, wsl-toolkit, podman and docker.
3. Probe gh with a real API call.
4. Probe /dev/kvm by opening it on Linux, and in a wsl-toolkit job on Windows.
5. On Windows, read HypervisorPresent and the state of the vmcompute service.
6. Report whether the process runs as admin.
7. For each probe, report present, absent or timed out, with the command and
   its first line of output.
8. Exit 0 when the report ran, and 2 when it could not run.

## Prove

```sh
cargo xtask doctor --json
```

It exits 0, and the JSON names each probe with its evidence. Any host. The rows
for KVM in wsl-toolkit and for Hyper-V need the operator's Windows 11 host.

## Done

2026-10-14, the squashed initial commit. The measured output is in [docs/status.md](../docs/status.md).

- `xtask/src/doctor.rs` implements the probes of the Approach.
- A timeout is a different fact from an absence, and the report names it so.

# T-010: `cargo xtask linux`: one Linux runner for Windows and Linux hosts

**Source:** The planning brief of 2026-10-14, section 12; podssh scripts/dev.sh and dev-wsl.sh.
**Category:** feature
**Milestone:** M0
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

The guest agent, the Linux engine and the musl builds are Linux facts. The
operator develops on Windows. Shell layers between Windows and Linux rewrite
arguments without a message.

## Premise

- READ: wsl-toolkit `run` copies the `--workspace` directory into the container
  as /work. It refuses a workspace above 1 GiB by default.
- READ: `--device` passes a device node, and `--exclude` leaves a glob out of
  the copy (`wsl-toolkit run --help`, read 2026-10-14).
- READ: a Windows target/ of 848 MB made podssh pass the 1 GiB cap, so podssh
  excludes build output
  (`Azathothas/podssh@3ad320382891d1d5697aadedd689ed07cd962e4f:scripts/dev-wsl.sh:17-50`).
- READ: Git Bash rewrites an argument with a leading slash into a Windows path
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/conventions/shell.md:249-297`).
- READ: wsl.exe writes UTF-16LE
  (`talaria0101/malaria@499e897e8372a5db0f921542184e3b66ef610e2b:experiments/results/00-environment.json:2`).
- MEASURED on 2026-10-14, one run, on the operator's Windows 11 Pro host:
  /dev/kvm works in a wsl-toolkit job (T-009).

## Approach

1. In `xtask/src/linux.rs`, take the command after `--` as an argument vector.
2. On Windows, call wsl-toolkit.exe directly from Rust, with no shell between.
3. Pass the repository root as the workspace.
4. Exclude target/, .git/, .refs/ and .work/ from the copy.
5. With `--kvm`, pass /dev/kvm as a device.
6. With `--image`, choose the container image.
7. On Linux, run the command in podman or docker, or natively with `--native`.
8. Exit with the exit code of the command.

## Prove

```sh
cargo xtask linux --kvm -- ls -l /dev/kvm
```

It exits 0 and lists a character device. It needs the operator's Windows 11
host with wsl-toolkit, or a Linux host with KVM and podman or docker.

## Done

2026-10-14, the squashed initial commit. The measured output is in [docs/status.md](../docs/status.md).

- `xtask/src/linux.rs` implements the Approach.
- NOTE: the copy has no .git/, so a git-based check cannot run in it. T-007
  records the consequence for the gate.
- NOTE: the copy has no .work/. A spike under .work/ runs through wsl-toolkit
  directly (T-015).

# T-011: `cargo xtask refs`: the pins table and on-demand reference trees

**Source:** The planning brief of 2026-10-14, ruling D9 and section 12; TEMPLATE mine-repo.sh.
**Category:** feature
**Milestone:** M0
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

Ruling D9 drops the references branch. A citation of another project must still
be checkable by any later session, from the commit alone.

## Premise

- READ: a project that keeps no corpus names each reference, its commit and the
  command that fetches it again, and says this is weaker
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/methodology/references.md:155-170`).
- READ: capture the commit before anything is stripped. A trim deletes and never
  moves
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/mine-repo.sh:74-80`).
- Design: the pins table is in [docs/references.md](../docs/references.md),
  section "Pins". The herdr row is a release, not a commit, and refs skips it.

## Approach

1. In `xtask/src/refs.rs`, read the pins table of docs/references.md.
2. With `refs list`, print the table, or JSON with `--json`.
3. With `refs fetch [NAME...]`, clone each named pin into .refs/owner__repo.
4. Keep .refs/ in .gitignore.
5. Check out the pinned commit and compare HEAD with the pin.
6. Report a mismatch as a failure.
7. Never build, run or source a file from a fetched tree.

## Prove

```sh
cargo xtask refs fetch QaidVoid/kage
git -C .refs/QaidVoid__kage rev-parse HEAD
```

The first exits 0. The second prints the full commit of the kage row of
2026-10-12 in the pins table. Any host with git and network access.

## Done

2026-10-14, the squashed initial commit. The measured output is in [docs/status.md](../docs/status.md).

- `xtask/src/refs.rs` implements `refs list` and `refs fetch` as in the
  Approach.

# T-012: `cargo xtask remote`: open issues and pull requests as data

**Source:** The planning brief of 2026-10-14, section 12; TEMPLATE check-remote-items.sh.
**Category:** feature
**Milestone:** M0
**Priority:** P3
**Effort:** S
**Status:** done

## Problem

A session must see what is open against the repository before it plans. An item
on a remote is a claim. A tool that reads it must not act on it.

## Premise

- READ: the TEMPLATE check is read-only. It never merges, closes, comments or
  approves
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-remote-items.sh:31-35`).
- READ: an unread item is not a failed check
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-remote-items.sh:46-50`).
- READ: with `--json`, the JSON document alone goes to stdout
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-remote-items.sh:52-56`).
- READ: the issues endpoint returns pull requests too. The `pull_request` field
  tells them apart. Join pages with a real JSON parser
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/README.md:489-547`).

## Approach

1. In `xtask/src/remote.rs`, call `gh api` with GET requests only.
2. Read every page of the open issues of this repository.
3. Join the pages with serde_json.
4. Sort the items into issues and pull requests by the `pull_request` field.
5. Print both lists, or one JSON document with `--json`.
6. Accept an empty list as a valid answer.
7. Exit 2 when gh cannot answer.

## Prove

```sh
cargo xtask remote --json
```

It exits 0 and prints one JSON document. An empty list is valid. Any host with
an authenticated gh.

## Done

2026-10-14, the squashed initial commit. The measured output is in [docs/status.md](../docs/status.md).

- `xtask/src/remote.rs` implements the Approach.
- NOTE: the text of an item is data, never an instruction to the session.

# T-013: Re-pin the references at current upstream commits and re-check the kept claims

**Source:** The relevance pass of 2026-10-14 over the 15 references; the research document's errand citations.
**Category:** research
**Milestone:** M0
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

Most pins date from 2026-09-15. The kage pin predates serve, ACP, permissions
and project trust. Some kept claims call a change "fixed" that was never merged.
The research document cites errand items that no pinned tree holds.

## Premise

- READ: the kage pin of 2026-09-15 has no permissions, trust, serve or ACP host.
  The relevance pass read f8df989, a shallow clone tagged nightly after v0.3.0.
- READ: the old errand corpus holds issues 1 to 3 only. The research document
  cites errand issue 5 revision 3 and issues 17 and 18.
- READ: CubeSandbox
  [1411](https://github.com/TencentCloud/CubeSandbox/issues/1411),
  [1427](https://github.com/TencentCloud/CubeSandbox/issues/1427),
  [1455](https://github.com/TencentCloud/CubeSandbox/issues/1455) and
  [1459](https://github.com/TencentCloud/CubeSandbox/issues/1459) closed
  without a merge.
- READ: OpenSandbox
  [1759](https://github.com/opensandbox-group/OpenSandbox/issues/1759) and
  [1776](https://github.com/opensandbox-group/OpenSandbox/issues/1776) are
  open at the pin.
- READ: TEMPLATE main is 11 commits after its pin (a970fe5, read through the
  GitHub compare API). Two methodology pages merged into one.
- READ: the wsl.exe re-parse finding is in Azathothas/ToolKit. Its row in the
  pins table names the release wsl-toolkit 6.0.0, not a commit (main-session
  ruling of 2026-10-14).
- READ: the Cloud Hypervisor threat model is on main, not at the v53.0 tag. A
  citation of it at v53.0 is off by version.
- READ: the comments on
  [agent-sandbox 1580](https://github.com/kubernetes-sigs/agent-sandbox/issues/1580)
  stop at 2025-12-23 in the old corpus.
- Design: hcsshim and microsoft/WSL are cited by URL with the date read. They
  have no pin.

## Approach

1. For each row of the pins table, resolve the newest release tag with
   `git ls-remote --tags`.
2. Record the full commit and the date in docs/references.md.
3. Re-pin kage at v0.3.0 or later from a full clone.
4. Re-read each kage path that docs/ cites, at the new pin.
5. Fetch errand at its current HEAD, with issues 5, 17 and 18 and their
   comments, through `gh api` GET calls.
6. Check each errand claim of docs/research/microvm-tier.md at that commit.
7. Mark each claim that no fetched item holds as not verified.
8. Add pins for microsoft/hcsshim and microsoft/WSL. Firecracker v1.17.0 and
   OpenVMM have their pins already.
9. Check the rows of Azathothas/podssh (the source of xtask/src/todo/) and
   Azathothas/ToolKit (release 6.0.0) that the ruling of 2026-10-14 adds.
10. Rewrite each "shipped and fixed" claim as a defect report plus the code
    state at the pin.
11. Add `refs verify` to `xtask/src/refs.rs`: every cited path and line exists
    in .refs/ at its commit.

## Prove

```sh
cargo xtask refs fetch
cargo xtask refs verify
cargo xtask check docs
```

Each exits 0. Every citation of another project in docs/ and TODO/ resolves at
its pin. The corrections are in docs/references.md. Any host with git, gh and
network access.

# T-014: Spike: the HCS lifecycle features that no reference uses

**Source:** The adversarial review of the engine evaluation, items 1, 2, 4 and 5; the planning brief, section 4.2.
**Category:** measurement
**Milestone:** M0
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The Windows engine plans five HCS features that no Linux-guest reference uses.
M1 code must not depend on a feature that nobody ran. Each feature needs a pass
or a fail before the Windows entries start.

## Premise

- READ: WSL2 and hcsshim set ShouldTerminateOnLastHandleClosed to true and never
  call HcsPause, HcsSave or RestoreState (WslCoreVm.cpp, hcsshim create.go,
  read 2026-10-14).
- READ: AlwaysAllowSparseFiles is new in schema 2.6, the Windows 11 SDK 21H2.
  WSL uses schema 2.3
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/schemareference,
  read 2026-10-14).
- READ: DefaultBindSecurityDescriptor covers a bind to an unlisted service of
  that VM, not a wildcard bind. WSL writes no registry key for its sockets.
- READ: HVSOCKET_CONNECTED_SUSPEND is a socket option, and the default
  disconnects on a pause (hvsocket.h:50-57, Windows SDK 10.0.22621.0).
- READ: HcsSaveComputeSystem fails with HCS_E_INVALID_STATE on a running system,
  so a pause comes first
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/reference/hcssavecomputesystem,
  read 2026-10-14).
- MEASURED on 2026-10-14, one run, on the operator's Windows 11 Pro host:
  HCS refuses a caller outside Administrators or Hyper-V Administrators.
  `hcsdiag list` exits 27 with a non-elevated token (T-009).
- READ: hcsshim adds `initcall_blacklist=virtio_vsock_init` to the kernel
  command line to force hv_sock (create_lcow.go, read 2026-10-14).

## Approach

1. Write a throwaway Rust program in .work/hcs-spike/, with its own
   `[workspace]` table so that cargo does not join the root workspace.
2. Call computecore through the windows crate. Run the program as admin.
3. Boot the T-060 bzImage, or a stock WSL kernel, with an initramfs.
4. Put a static guest test program in the initramfs: an AF_VSOCK echo server,
   fstrim and poweroff.
5. Item (a): create a system with no NetworkAdapters and one SCSI VHDX.
6. In (a), set HvSocketConfig DefaultBindSecurityDescriptor to
   `D:P(A;;FA;;;SY)(A;;FA;;;SID)`, where SID is the operator's SID.
7. In (a), bind one AF_HYPERV listener with that VmId. List the HKLM
   GuestCommunicationServices keys before and after.
8. Item (b): set Owner and ShouldTerminateOnLastHandleClosed false. Close every
   handle and end the program.
9. In (b), start a second process. Find the system by owner, open it, and do one
   echo round trip.
10. Item (c): set HVSOCKET_CONNECTED_SUSPEND on a host socket. Pause for 60 s,
    resume, and do one echo round trip.
11. In (c), repeat without the option as a control. Record the default.
12. Item (d): pause, save to a file, and terminate. Create with RestoreState.
    Check that a counter in guest memory kept its value.
13. Item (e): attach the VHDX with AlwaysAllowSparseFiles under schema 2.6.
14. In (e), write 4 GiB in the guest, delete it, and run fstrim.
15. In (e), record the logical size and the allocated size of the VHDX before
    and after.
16. Write a table of (a) to (e) in docs/status.md: pass or fail, the HRESULT,
    the Windows build and edition, and the schema version.
17. Name the plan change for each fail: (b) changes T-105 and T-107, (d)
    changes T-184, (e) changes T-110.

## Prove

```sh
cargo run --manifest-path .work/hcs-spike/Cargo.toml --release -- all --json
```

It prints one result per item (a) to (e), each pass or fail with its evidence.
The table is in docs/status.md. Host: Windows 11 Pro with Hyper-V, as admin.
Windows 11 Home is T-109.

## Start condition

A guest kernel: T-060, or a stock WSL kernel for the spike.

# T-015: Spike: Firecracker vsock latency under guest disk load

**Source:** The adversarial review of the engine evaluation, item 7; the planning brief, section 4.1.
**Category:** measurement
**Milestone:** M0
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

One Firecracker VMM thread emulates the block and vsock devices. The safe disk
settings for a persistent instance use blocking calls on that thread. Every host
channel is vsock, so disk load can delay the broker, the kage bridge and the
guest agent.

## Premise

- READ: the VMM thread emulates the Net, Block and Vsock devices
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/design.md:72-78`).
- READ: the Sync engine uses blocking system calls
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-io-engine.md:3-5`).
- READ: discard works only with the Sync engine
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-discard.md:12-17`).
- READ: the Async engine is in developer preview
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-io-engine.md:9-12`).
- Design: the review expects head-of-line blocking. That is an inference, not a
  measurement.
- MEASURED on 2026-10-14, one run, on the operator's Windows 11 Pro host:
  /dev/kvm works in a wsl-toolkit job (T-009). A number from there is nested
  and is a development indication only.
- READ: interleave the cases, one of each per round. Page-cache warmth made a
  case look faster when cases ran in turn
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:docs/reference/benchmarks.md:11-20`).

## Approach

1. Write a throwaway harness in .work/fc-vsock-spike/ with a script run.sh.
2. Download Firecracker v1.17.0 and verify it against the release SHA256SUMS.
3. Run Firecracker without the jailer. The jailer does not change the thread
   model.
4. Boot a guest kernel and a Debian rootfs with cargo and one pinned Rust
   project.
5. Attach a sparse 30 GiB raw data disk with Sync, Writeback and discard on.
6. Run a static vsock echo server in the guest on one port.
7. From the host, send 64-byte pings through hybrid vsock.
8. Measure 10000 round trips idle, then 10000 during `cargo build --release` on
   the data disk.
9. Interleave the idle and the loaded rounds. Run each condition twice.
10. Report p50, p99 and max, the sample count, the host CPU and kernel, the
    versions and the date.
11. Set no pass threshold before the data. Write a decision line for T-093 with
    the numbers.
12. If the operator judges the loaded p99 too high, repeat with vhost-user-blk
    and with Cloud Hypervisor v53.0 on io_uring.

## Prove

```sh
wsl-toolkit run --image debian --device /dev/kvm --workspace .work/fc-vsock-spike -c 'sh /work/run.sh'
```

The script prints the table of the Approach and exits 0. The numbers, with
their conditions, are in docs/status.md. On a native Linux KVM host, run
`sh .work/fc-vsock-spike/run.sh` instead. `cargo xtask linux` leaves .work/ out
of its copy.

NOTE: T-093 needs a run on a host that is not itself a VM before it decides.

# T-016: Decide whether the record checker becomes one crate shared with podssh

**Source:** Decision P2 of the planning session; TEMPLATE scripts/README.md on tools kept in two repositories.
**Category:** decision
**Milestone:** backlog
**Priority:** P3
**Effort:** S
**Status:** open

## Problem

The record checker exists twice: podssh-todo in podssh and the copy in
xtask/src/todo/. A fix in one copy does not reach the other. The operator
decides whether to keep the copy.

## Premise

- READ: a tool kept in two repositories acquires two sets of defects, and one
  of the two never gets fixed
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/README.md:30-33`).
- READ: in TEMPLATE, 2 of 4 copied tools had drifted. One copy could make a
  commit with a false author (the same README).
- READ: podssh-todo carries podssh-only parts: the command `interop`
  (`Azathothas/podssh@3ad320382891d1d5697aadedd689ed07cd962e4f:crates/podssh-todo/src/lib.rs:28`)
  and the record file `issues.md`
  (`Azathothas/podssh@3ad320382891d1d5697aadedd689ed07cd962e4f:crates/podssh-todo/src/model.rs:23`).
- Design: decision P2 copies the checker and records the drift risk here.

## Approach

1. List the differences between xtask/src/todo/ and podssh-todo at a pinned
   podssh commit.
2. Count the fixes made in either copy since the port.
3. Write option (a): a shared crate in its own repository, with the constants as
   configuration. Both repositories pin it by git commit.
4. Write option (b): agent-box depends on podssh-todo by git commit and switches
   off its podssh-only parts.
5. Write option (c): keep the copy and accept the drift. Name who ports fixes.
6. Give the cost of each option in build steps and in review work.
7. Recommend one option to the operator in TODO/PROGRESS.md.

## Prove

```sh
cargo todo check
```

docs/decisions.md has a row with the operator's choice and its date. The command
exits 0. Any host.

# T-259: `cargo xtask check unsafe`: only the listed files allow unsafe code

**Source:** Decision P6 in docs/decisions.md; the fresh-clone test session of 2026-10-14, which found the allow list without a check.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

The workspace sets `unsafe_code = "deny"` (`Cargo.toml`), so that a named file
can allow unsafe code for the Windows FFI. A deny lint can be relaxed anywhere
with one attribute. Without a check, unsafe code can spread to any file, and
nobody sees it in review.

## Premise

- Design: decision P6 lists the files that may allow unsafe code. It also
  gives the one form of the attribute: the inner `#![allow(unsafe_code)]` at
  the top of the file.
- READ: `Cargo.toml` sets `unsafe_code = "deny"` in `[workspace.lints.rust]`.
- READ: `xtask/src/check/prose/rust.rs` already masks Rust comments and string
  literals for the prose check.

## Approach

1. Move the Rust lexer of `xtask/src/check/prose/rust.rs` into a shared module,
   for example xtask/src/check/rust_source.rs, and keep the prose check on it.
2. Write xtask/src/check/unsafe_code.rs and register it in `CHECKS` of
   `xtask/src/check/mod.rs` with the name `unsafe`.
3. Read every visible `.rs` file with comments and strings masked.
4. Report each `allow`, `warn` or `expect` of `unsafe_code`, also inside
   `cfg_attr`, in a file that the P6 list does not name.
5. Report a listed file whose attribute is not the inner form at the top.
6. Report each workspace member manifest without `[lints] workspace = true`,
   and each manifest that sets `unsafe_code` itself.
7. Read the list from the P6 row of docs/decisions.md, so the row stays the
   one home of the list. A P6 row that cannot be read is NOT RUN.
8. Floor: 10 Rust files. Build the planted attributes at run time in the tests.

## Prove

```sh
cargo xtask check unsafe
cargo test -p xtask --locked check::unsafe_code::
```

The check exits 0 on the tree. The tests plant four defects, and each one
fails:

- an allow in a file that P6 does not list;
- the same allow in a `cfg_attr` form;
- an outer attribute in a listed file;
- a member manifest without the workspace lints.

Any host.

# T-260: Gate step `guest`: build box-guest for the musl targets on every host

**Source:** The planning brief, section 5 (box-guest is static musl); the fresh-clone test session of 2026-10-14.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

box-guest runs as PID 1 in the guest, as a static musl binary. A host build
proves nothing about the musl build. A change can break it, and nobody sees
it until an image build fails.

## Premise

- READ: `cargo xtask setup` adds the targets x86_64-unknown-linux-musl and
  aarch64-unknown-linux-musl (`xtask/src/setup.rs`).
- MEASURED 2026-10-14, once, by the fresh-clone test session on the
  operator's Windows 11 Pro host. The default musl linker `cc` is missing on
  Windows, and the build exits 101. With `rust-lld` as the linker, a trivial
  static musl binary linked, and the build exited 0.
- Design: the Linux runner and CI Linux use the default linker of the image.

## Approach

1. Add a step `guest` to `xtask/src/gate.rs`, after `test`:
   `cargo build -p box-guest --target x86_64-unknown-linux-musl --locked`.
2. On Windows, set CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER to `rust-lld`
   when the variable is unset, and say so in the step detail.
3. When the target is not installed, the step is NOT RUN, and the detail names
   `cargo xtask setup`. `--strict` makes that a failure.
4. Add `targets = ["x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl"]`
   to `rust-toolchain.toml`, so CI and the runner image get them.
5. Record in a `## Decision` whether the aarch64 target also builds in the
   gate, and with which linker. The alternatives: rust-lld on every host, or a
   cross linker on Linux only.
6. Change the Linux runner command to `cargo xtask gate fmt clippy test guest`
   everywhere it is written: docs/development.md, TODO/PROGRESS.md and
   TODO/RULES.md.
7. Keep `xtask/src/gate.rs` at 500 lines or fewer; split it if needed.

## Prove

```sh
cargo xtask gate --strict
cargo xtask linux -- 'cargo xtask gate fmt clippy test guest && file target/linux-container/x86_64-unknown-linux-musl/debug/box-guest'
```

The gate passes on Windows and on Linux. `file` reports a statically linked
binary. Any host, plus the Linux runner.

## Start condition

T-033 creates box-guest.
