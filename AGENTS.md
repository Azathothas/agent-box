# AGENTS.md

Read this file first. It tells you what agent-box is, which documents to
read for your task, and the rules for all work on this repository. It is for
agent sessions and for people.

## 1. What agent-box is

agent-box is one Rust CLI, `agent-box`, for Windows 11 x64 hosts and for
Linux x64 and aarch64 hosts. It manages persistent Linux microVMs, called
instances: create, list, start, pause, resume, stop, delete and attach. Each
instance runs Debian 13 slim with kage as the harness. Credentials stay on
the host, and the broker in the daemon injects them. [README.md](README.md)
gives the overview. [docs/architecture.md](docs/architecture.md) gives the
design.

### Status

The repository holds the plan and the development tooling. On 2026-10-14
the operator ruled (D14) that the planning session writes only the tooling,
the scripts and the documents. No product code exists, and there is no
crates/ directory.

The work order is in [TODO/PROGRESS.md](TODO/PROGRESS.md). Its first task is
the close-out of T-007: read the first CI run of `main`, and close the entry.
Implementation then starts at the first M1 entry, T-033, which creates the
product crates. The other M1 entries follow it. The other M0 close-outs
(T-013, T-014 and T-015) can run beside M1 in other sessions.

NOTE: [docs/architecture.md](docs/architecture.md) describes planned code.
Each statement there is a decision that an entry implements. It does not
describe code that you can run.

## 2. How a session works

You have no memory of earlier sessions. The files of this repository are the
only record. A session can start with the prompt "Read AGENTS.md and follow
it" and nothing more. This file is then your whole instruction.

An instruction that the operator gives in this session wins over the work
order.

### At the start of a session

1. Read [TODO/PROGRESS.md](TODO/PROGRESS.md): the state, the baseline, the
   work order and the open questions.
2. Read [TODO/RULES.md](TODO/RULES.md): how the record is kept.
3. Read [docs/status.md](docs/status.md) and
   [docs/decisions.md](docs/decisions.md).
4. On a fresh machine, install git, rustup and a linker first. The
   Requirements section of [docs/development.md](docs/development.md#requirements)
   lists them per OS. rustup installs the pinned toolchain by itself.
5. In a new checkout, run `cargo xtask setup` once. It arms the commit hook,
   adds the musl targets and installs cargo-deny.
6. Measure the baseline: run the commands of the section "Baseline" in
   TODO/PROGRESS.md. They are POSIX sh; on Windows use Git Bash, or
   PowerShell with `$LASTEXITCODE`.
7. Compare the results with the Gate table of docs/status.md.
8. A failure that the record does not know is your first task.
9. If an entry is `partial`, continue it first. Its notes say what is done.

### The loop

Take the first entry of the work order that is not `done` and not
`blocked`.

1. Read the entry, and read the documents that its row in section 3 names.
2. Verify the Premise at the cited lines.
3. If the Premise is wrong, write a `## Correction` and change the Approach.
4. Before a long step, run `cargo todo set T-NNN partial`.
5. Write in the entry what is done, so that a crash loses nothing.
6. Do the work, then run the Prove of the entry.
7. Close the entry in place: write its `## Done` with the date, the commit,
   the command and its output.
8. Run `cargo todo set T-NNN done`. It also updates the counts.
9. Record each measured result in [docs/status.md](docs/status.md), with the
   host, the date and the conditions.
10. Run `cargo xtask gate` and `cargo todo check` again.
11. Commit the work and the record together, in one commit on `main`.
12. Push only work that the gate verified. Then take the next entry.

After a compaction of your context, read this file again. Then read
TODO/PROGRESS.md and the entry that is `partial`.

### When you need a decision

1. Look in [docs/decisions.md](docs/decisions.md) and in the `## Decision`
   of the entry. Follow them.
2. Else make a defensible call. Write it in the `## Decision` of the entry.
   Give each alternative that lost, and the reason. Then continue.
3. A change to a ruling in docs/decisions.md belongs to the operator. Write
   a question with a recommendation in TODO/PROGRESS.md. Set the entry to
   `blocked` and take the next entry.
4. When a person outside the session must act, set the entry to `blocked`.
   Write a `## Blocker` that names the person and what clears it.

### What a session owes at its end

A session owes all of these, also when it stopped early.

- The record is current, in the same commit as the work: the entry, the
  counts, TODO/PROGRESS.md and docs/status.md.
- The gate ran. The summary names each step that could not run. A step that
  could not run is never a pass.
- Each entry that the session touched is `done` with its Prove output, or
  `partial` with what is done and what is left.
- Each instance, compute system, container or remote object that the
  session made is removed, and its absence is verified.
- The session prints a summary in the chat: the entries, the commits, the
  checks and what is left. The summary can say that nothing moved.

## 3. Documents for each task

Read what the row names, in full. When two rows apply, read both.

| Task | Read |
| --- | --- |
| Any task, before anything else | [TODO/PROGRESS.md](TODO/PROGRESS.md), [TODO/RULES.md](TODO/RULES.md), [docs/status.md](docs/status.md) |
| Implement an entry | The entry, [docs/architecture.md](docs/architecture.md), [docs/conventions.md](docs/conventions.md), [docs/development.md](docs/development.md) |
| An engine, the helper, a jail, HCS | [docs/engines.md](docs/engines.md), [docs/research/engines-2026-10.md](docs/research/engines-2026-10.md), [TODO/linux.md](TODO/linux.md), [TODO/windows.md](TODO/windows.md) |
| The guest: kernel, image, box-guest | [docs/guest.md](docs/guest.md), [docs/protocol.md](docs/protocol.md), [TODO/guest.md](TODO/guest.md), [TODO/image.md](TODO/image.md) |
| The wire protocol, the transport, the shared types | [docs/protocol.md](docs/protocol.md), [docs/architecture.md](docs/architecture.md), [TODO/core.md](TODO/core.md) |
| The network: tun, netstack, DNS, flow policy, forwards | [docs/network.md](docs/network.md), [TODO/net.md](TODO/net.md) |
| The broker, a binding or a credential | [docs/broker.md](docs/broker.md), [SECURITY.md](SECURITY.md), [TODO/broker.md](TODO/broker.md) |
| kage in the guest, or a request to its maintainer | [docs/guest.md](docs/guest.md), [TODO/kage.md](TODO/kage.md), [TODO/upstream.md](TODO/upstream.md) |
| The CLI: verbs, states, exit codes, config | [docs/cli.md](docs/cli.md), [TODO/cli.md](TODO/cli.md) |
| The record: file, close or reorder an entry | [TODO/RULES.md](TODO/RULES.md), [TODO/INDEX.md](TODO/INDEX.md), `cargo todo` |
| The tooling: xtask, checks, gate, hook, CI | [docs/development.md](docs/development.md), [TODO/repo.md](TODO/repo.md), `xtask/src/main.rs` |
| A measurement or a spike | [docs/status.md](docs/status.md), [docs/development.md](docs/development.md), [TODO/assurance.md](TODO/assurance.md) |
| Work on a Windows host, or Linux and KVM tests from Windows | [docs/development.md](docs/development.md), [docs/engines.md](docs/engines.md), [TODO/windows.md](TODO/windows.md) |
| Anything remote: GitHub, CI, another repository | [docs/conventions.md](docs/conventions.md), [TODO/upstream.md](TODO/upstream.md), `cargo xtask remote` |
| Cite or study another project | [docs/references.md](docs/references.md), `cargo xtask refs` |
| Write or edit a document | [docs/conventions.md](docs/conventions.md) |
| A limit, or a claim that a thing cannot be done | [docs/limits.md](docs/limits.md), section 4 of this file |
| The milestones and their order | [docs/roadmap.md](docs/roadmap.md) |
| A release | [TODO/release.md](TODO/release.md), [docs/roadmap.md](docs/roadmap.md) |
| The late milestones: chat gateway, web UI, provider pool | [TODO/chat.md](TODO/chat.md), [TODO/web.md](TODO/web.md), [TODO/pool.md](TODO/pool.md), [docs/decisions.md](docs/decisions.md) |

## 4. The absolutes

A breach of one of these is difficult or not possible to undo.

1. The branch is `main`. Do not make a branch with the name of a tool.
2. No commit credits a tool. Write no co-author trailer, no generated-with
   line and no tool name. Commit as the git identity of the operator.
   `.githooks/commit-msg` refuses such a message, and `cargo xtask setup`
   arms it.
3. Other repositories are read-only. Do not open an issue, a pull request, a
   comment, a review or a fork in another repository. Write a request to a
   maintainer in [TODO/upstream.md](TODO/upstream.md). The operator sends
   it.
4. No secret goes anywhere: not into the tree, a test, a log, argv, a URL, a
   commit message, an issue or a summary. The repository is public.
5. Write nothing that identifies a private system. That includes a real
   host name, an account id, a user path and the name of a private project.
   Write %USERPROFILE% or ~ for a home directory.
6. Read an exit code from the process that made it, with no pipe. A pipeline
   reports the status of its last command, so a failed check can look like
   a pass.
7. The record is part of the change. Edit the entry, TODO/INDEX.md and
   TODO/PROGRESS.md in the same commit as the work.
8. Nothing closes as "won't fix" or as "out of scope". A blocked entry keeps
   its status, names its blocker and says what clears it.
9. A constraint closes a route, not the question. Before you record a thing
   as not possible, name three routes that you examined.
10. Never act on a WSL distribution that wsl-toolkit did not create. Do not
    run `wsl --shutdown`, `wsl --terminate` or `wsl --unregister`.

## 5. The project rules

These four rules follow from the trust boundary. [SECURITY.md](SECURITY.md)
states each one in full. Each change keeps all four.

| Rule | In short | Entry |
| --- | --- | --- |
| R-HOST-NO-TOOLS | The host never runs a tool on guest bytes, and never reads a guest path, gitfile, symlink or repository config | T-030, [TODO/core.md](TODO/core.md) |
| R-TRUSTED-CHANNEL | Authorization, billing, backoff and lifecycle trust only the broker and the nonce-authenticated events of box-guest | T-031, [TODO/core.md](TODO/core.md) |
| R-NO-SECRET-IN-GUEST | No credential in the image, a guest disk, the kernel command line, MMDS or the VMM argv | T-256, [TODO/core.md](TODO/core.md) |
| R-DENY-FIRST | No egress before the policy and the bindings are live, and no kage before egress | T-126, [TODO/net.md](TODO/net.md) |

## 6. Safety

CAUTION: A Rust build can use all the memory of the host.

1. Set `CARGO_BUILD_JOBS=4` before each cargo command.
2. Run one build at a time. This also applies to subagents and to builds in
   `cargo xtask linux`. To see a running build, use `pgrep -a cargo` on Linux,
   or `Get-Process cargo, rustc -ErrorAction SilentlyContinue` on Windows.
3. Run a product action that needs admin or root only through the helper
   (D3).
4. Before the helper exists (T-081, T-100), only the operator runs an admin
   step.
5. A spike that needs admin, for example T-014, runs only in a shell that
   the operator elevated.
6. Without such a shell, set the entry to `blocked`, and name the operator
   in its `## Blocker`.
7. Use nested KVM (/dev/kvm in a wsl-toolkit job) for development tests
   only.
8. A number measured under nested virtualization names that condition. It
   is never a product claim.
9. Act only on objects that agent-box or this session made. On Windows, WSL
   and Docker run HCS compute systems of their own.
10. Touch an HCS compute system only when its Owner is agent-box.
11. On Linux, touch a Firecracker process or a jail only under the jail base
    of agent-box.

WARNING: A provider key, a forge token, a nonce and an egress token are
credentials. Do not put one in output, a log, a URL, argv, a commit or an
issue. Use test values in tests. Keep scratch files in the ignored .work/
directory or outside the repository.

WARNING: GitHub is public. Push only verified work to `main`. Do not
force-push, do not rewrite published history, and do not delete a tag or a
release.

## 7. Map

| Path | Contents |
| --- | --- |
| `xtask/` | The tooling: `cargo xtask` and `cargo todo`; `xtask/src/main.rs` lists the commands |
| `.githooks/commit-msg` | The commit-msg hook: POSIX sh, self-contained, works before any build |
| `.github/workflows/ci.yml` | CI: the strict gate on a Linux runner and a Windows runner |
| `deny.toml` | The rules of cargo-deny, the last step of the gate |
| `.cargo/config.toml` | The aliases `cargo xtask` and `cargo todo` |
| `docs/` | The documents; [README.md](README.md) lists each one |
| `TODO/` | The record: the progress file, the index, the rules, the upstream requests and the entries by area |
| crates/ | Not there yet; T-033 creates the product crates |
