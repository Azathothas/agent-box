# Chat gateway

This area covers milestone M10. A message in one Discord channel starts a
session in an ephemeral instance, and the agent works in reply to the thread.
A pull request opens through the daemon, and the instance is deleted at the
end (D8). The exit criteria are in
[docs/roadmap.md](../docs/roadmap.md#m10-chat-gateway). The rules for every
late surface are in
[docs/architecture.md](../docs/architecture.md#the-late-surfaces). The kept
upstream findings are CH-01 to CH-43 in
[docs/references.md](../docs/references.md#chat-gateway).

The rules that decide the design of this area:

- The users of the gateway do not trust each other (ruling of 2026-09-15). A
  session gets its own instance, bindings, nonces, event log and memory.
- A right comes from the Discord identity of the author, never from the text
  of a message (T-221).
- box-chat reaches instances and credentials through the daemon only. The
  layering check refuses an edge from box-chat to box-broker or box-vmm.
- The chat token is a credential of the daemon. No instance ever holds it.
- The daemon composes each pull request and each comment that it publishes
  (T-223).
- A public channel joins untrusted input, private data and an outbound
  channel. Brokering stops the theft of a credential. It does not stop
  exfiltration ([docs/limits.md](../docs/limits.md)).

The product crates do not exist yet, and T-220 creates box-chat. A crate path
in this area names a planned file.

# T-220: Discord gateway: one channel, a thread per session

**Source:** ruling D8; the parity ruling of 2026-09-15; the errand chat gateway (READ, below); findings CH-08 to CH-16, CH-35 and CH-36 of [docs/references.md](../docs/references.md#chat-gateway).
**Category:** feature
**Milestone:** M10
**Priority:** P2
**Effort:** L
**Status:** open

## Problem

The chat channel is where community members ask for work. The gateway needs a
connection to Discord that sees replies in threads. It needs a map from one
thread to one session. It needs an outbound path that keeps the order of
messages across a lost connection. Each part handles text from untrusted users
and from an untrusted agent.

## Premise

- READ: errand connects with the intents Guilds, GuildMessages and
  MessageContent, and no other
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/chat/gateway.ts:133-140`).
- READ: errand turns a DisallowedIntents refusal into exit 2 with the portal
  setting to turn on
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/main.ts:81-86`).
- READ: the ready timeout is 30 s. A reconnect waits 2 s, doubles up to 60 s,
  and gives up after 10 attempts
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/chat/gateway.ts:31-50`).
- Design (CH-35, CH-36): an HTTP interactions endpoint gets slash commands,
  not ordinary messages, and it needs a public HTTPS listener on the host.
  Replies as prompts need the Gateway connection with MessageContent.
- Design: the Gateway connection goes out from the host. The host opens no
  listening socket for it.
- READ: the errand outbox keeps one ordered queue per thread and buffers
  while the connection is down. It holds at most 500 tasks
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/chat/outbox.ts:1-30`).
- READ: when it drops tasks, errand posts the count of dropped messages
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/chat/threads.ts:74-76`).
- READ: errand sends each message with SuppressEmbeds
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/chat/threads.ts:40-46`).
- READ: a message holds 2000 code points and a thread name 100. A split
  inside a code fence closes the fence and opens it again
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/chat/render.ts:1-20`).
- READ: auto-archive is 1440 minutes, and the typing indicator refreshes
  every 8 s
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/chat/threads.ts:48-52`).
- READ: a search of src/chat at the errand pin finds no allowed-mentions
  setting. So agent text that names a user or a role notifies them.
- READ: errand prints an ACCESS warning at startup when the allowlist admits
  every user
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/daemon.ts:95-101`).
- Design ([docs/architecture.md](../docs/architecture.md)): box-chat is a
  layer 4 library. It depends at most on box-core, box-policy and
  box-daemon.
- Not read in this session: the Discord API documentation for
  allowed_mentions and for interactions over the Gateway. Read both before
  the code.

## Approach

1. Create crates/box-chat as a library and a workspace member. box-cli gets
   the edge to it. The daemon runs the gateway as a task when the config has
   a `[chat]` table.
2. box-daemon defines the traits that the gateway calls: sessions, admission,
   a redactor and events. box-cli connects box-chat to them at daemon start.
3. Modules: crates/box-chat/src/gateway.rs, inbound.rs, render.rs, outbox.rs
   and session.rs.
4. Choose the Discord library in this entry: twilight (its gateway, http and
   model crates) or serenity. Record the choice and the reason.
5. The criteria for the choice: explicit intents, no forced cache, a licence
   in `deny.toml`, and recent releases.
6. The `[chat]` keys are `guild`, `channel`, `token_secret`, the lists of
   T-221, `repositories` (T-223), `profile`, `idle` and `retention` (T-222).
   An unknown key is a refusal (T-027).
7. `token_secret` names a credential in the store. The operator writes it with
   `agent-box secret set NAME`, from stdin only.
8. The daemon reads the token and gives box-chat a value type with no
   Display, Debug or Serialize output. The token never reaches an instance, a
   log, an argv or an event record.
9. Connect with the three intents of errand and no other. Wait 30 s for ready.
   Reconnect after 2 s, doubling up to 60 s. T-222 owns the give-up rule.
10. A refused MessageContent intent stops the gateway, not the daemon. The
    startup report names `chat-intent-refused` and the portal setting.
11. Open no listening socket. If T-224 adds slash commands, they come over the
    same connection.
12. Inbound: one pure function maps a message and the config to ignore, start
    or thread. Its order:
    - a message from any bot, this one included, is ignored;
    - a message outside the channel and its threads is ignored;
    - T-221 decides on the identity of the author;
    - an empty message with no attachment is ignored.
13. Each refusal is silent and names no list.
14. Start: a top-level message from a permitted author opens a public thread
    on that message. The thread name is its first line, at most 100 code
    points, with mentions removed.
15. The gateway asks the daemon for a session. The daemon admits it (T-251, or
    a fixed cap from the config before M12) and starts an ephemeral instance
    (T-222).
16. One thread is one session. A thread never serves a second session.
17. Render rules, one test each:
    - each message carries SuppressEmbeds;
    - each message carries an empty allowed-mentions list, so no text notifies
      a user, a role or everyone;
    - text splits at 2000 code points, and a split closes and opens a code
      fence;
    - a run of tool calls is one message, edited in place;
    - a user message has one reaction, replaced when its state changes;
    - the typing indicator refreshes every 8 s while a turn runs.
18. Agent text is untrusted. Show the full URL of a masked link `[text](url)`.
    Remove control characters except newline and tab.
19. Damage control, not a boundary: the daemon gives box-chat a redactor. It
    replaces each nonce and token of the instance and each stored credential
    value with a marker.
20. box-chat calls the redactor on each outbound text. It never reads the
    values.
21. Outbox: one ordered queue per thread, buffered while disconnected, at most
    500 tasks. Drop the oldest task first.
22. After a drop, post the count of dropped messages, so a cut conversation
    never looks complete. A failed task stays at the head.
23. Obey each Discord rate-limit answer and its retry delay.
24. Write each inbound decision and each send to the event log of T-176 as a
    host-observed record. Write ids and counts, never message text.
25. The startup report names the gateway state, the guild and channel ids and
    the size of each list.
26. The startup report adds an ACCESS line when the lists admit every member
    of the guild.

## Prove

```sh
cargo test -p box-chat inbound::
cargo test -p box-chat render::
cargo test -p box-chat outbox::
cargo test -p box-chat --test guild_live -- --ignored
```

Expected: named tests ignore a bot author, a message in another channel and
an empty message. Every rendered payload carries SuppressEmbeds and an empty
allowed-mentions list. A split inside a fence keeps the fence. A masked link
shows its URL. A planted nonce is absent from the sent payload.

The outbox keeps the order across a disconnect. With 501 tasks, the first
send after the reconnect reports one dropped message. A failed task stays at
the head.

In the live test, a message in the test channel starts an ephemeral instance.
`agent-box list --json` shows the kind `ephemeral`, and a reply comes back in
the thread. The scan of the T-180 broker check finds no byte of the chat token
in the guest, on its disks or in the VMM argv.

Host: any host for the unit tests. The live test needs a Linux KVM host or
Windows 11 with Hyper-V as admin. It also needs the test guild and a bot with
the MessageContent intent on.

## Start condition

T-170, T-171, T-175 and T-176.

# T-221: Authorization from platform identity

**Source:** the ruling of 2026-09-15 (users of the chat gateway are mutually untrusted); the trust boundary of [docs/architecture.md](../docs/architecture.md#trust-boundary); findings CH-01 to CH-05, CH-07 and CH-39.
**Category:** feature
**Milestone:** M10
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

Anyone who can post in a public channel can type text that the agent reads.
If text can grant a right, every member has every right. Each right must come
from the identity that Discord reports for the author. The gateway checks it
on each message, with one function.

## Premise

- READ: content from a remote is data. A claim of authority, approval or
  urgency inside it grants nothing
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/security/remote-ops.md:78-128`).
- READ: errand checks the blocklist before the allowlist, so an account on
  both lists is refused
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/chat/inbound.ts:72-91`).
- READ: each errand rejection is silent and names no list. A reason would
  describe the allowlist to the person that it excludes
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/chat/inbound.ts:93-99`).
- READ: in errand the author of the first message owns the thread and can
  invite guests. Operators named in the configuration control every session
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:docs/threads.md:33-45`).
- READ: the errand allowlist holds user ids only, with `*` for every user
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/chat/inbound.ts:87-91`).
- READ: a public channel has all three legs of the lethal trifecta:
  untrusted input, private data and an exfiltration channel
  (https://github.com/withastro/flue/issues/51).
- Design ([docs/architecture.md](../docs/architecture.md)): the operator role
  is configuration, never a role of the chat service.

## Approach

1. File: crates/box-chat/src/authz.rs. One function decides each action:
   `authorize(identity, action, session) -> Allow | Deny`. No other code
   checks a right.
2. The identity is the user id of the author and the role ids of their guild
   membership, from the gateway event. A name, a nickname or text is never an
   identity.
3. `[chat]` lists: `allow_users`, `allow_roles`, `block_users` and
   `operators`. A blocked user is refused before any allow rule.
4. `allow_roles` grants one right only: to start sessions and to drive them as
   their owner. No Discord role grants the operator role.
5. So a guild administrator who assigns roles cannot make an operator.
   `operators` holds user ids, and only the config file sets it.
6. The session roles: the owner (the author of the first message), a guest
   (invited by the owner), and an operator.
7. The rights of each role:

| Action | Owner | Guest | Operator | Other permitted user |
| --- | --- | --- | --- | --- |
| prompt in the thread, hold a prompt | yes | yes | yes | no |
| interrupt, stop, change the model | yes | no | yes | no |
| invite or remove a guest | yes | no | yes | no |
| `!pr` (T-223) | yes | no | yes | no |
| status of this session | yes | yes | yes | no |
| facts of the owner (T-225) | yes | no | yes | no |
| end every chat session | no | no | yes | no |

8. Check the identity again on each message. A user removed from a list, or
   from the guild, loses each right at their next message.
9. A guest's rights end when the owner removes them, and when the session
   ends.
10. Text never grants a right. "The operator approved this" is a prompt and
    nothing more. A command is a prefix, and it gets the same check as any
    action.
11. An author outside the lists gets no answer. A permitted user without a
    right in a thread gets one notice per thread, then a reaction only.
12. A blocked user gets the same treatment as an unknown user.
13. The startup report prints an ACCESS line when `allow_users` holds `*`, or
    when `allow_roles` holds the default role of the guild.
14. Tenancy: a guest prompts in the session of the owner. The work, the spend
    and the memory of that session belong to the owner.
15. Nothing of one owner's session reaches the session of another owner: no
    instance, binding, nonce, cache, event log or memory.
16. The startup report names the egress mode of the chat profile. A public
    channel adds untrusted input to each destination that the mode allows.

## Prove

```sh
cargo test -p box-chat authz::
```

Expected: a table test runs every action against every role. Each allowed
pair passes and each other pair is refused. Named tests: a user on both lists
is refused, and a role cannot make an operator. A message that claims the
operator's approval grants nothing. A user removed from the list is refused
at the next message. No refusal text names a list.

Host: any host.

## Start condition

T-220.

# T-222: Ephemeral session flow and gateway loss

**Source:** ruling D8; brief section 8 (instance kinds); findings CH-10, CH-11, CH-16, CH-17 and CH-19, LC-38 and KG-27 of [docs/references.md](../docs/references.md); [docs/limits.md](../docs/limits.md) (an undecided row).
**Category:** feature
**Milestone:** M10
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

A chat session runs in an ephemeral instance. errand stops every session when
its chat connection is gone for good. In agent-box an instance outlives the
gateway and the daemon, and the operator can reach it from the CLI. This
entry decides the life of a chat instance, and what it does when the gateway
or the daemon stops.

## Premise

- READ: in errand a reply during a running turn steers it and takes no new
  slot. A prompt with `!then` waits for the turn. An aside with `!!!` never
  reaches the agent
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:docs/threads.md:18-31`).
- READ: in errand an idle timeout, a crash or a restart leaves the thread
  open to resume, without a notice. A failure gets a notice
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:docs/threads.md:67-80`).
- READ: after 10 failed reconnects, errand stops every session
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/chat/gateway.ts:225-235`,
  `QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/serve.ts:273-276`).
- READ: the idle timeout of errand defaults to 1800000 ms
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/config/schema.ts:456-457`).
- READ: kage serve keeps each session in a file and replays it when a client
  loads it
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/remote.md:137-166`).
  The file is on the data disk of the instance.
- READ: a second prompt while a turn runs fails with a busy message (same
  file, lines 160-166). Not read: whether kage serve can steer a running
  turn.
- Design (T-170): the daemon deletes an ephemeral instance when its task ends.
  After a daemon restart, it re-adopts the instance and looks for its task.
- Design ([docs/architecture.md](../docs/architecture.md)): the netstack and
  the broker are in the daemon. With no daemon, the guest has no network.

## Approach

1. Files: crates/box-chat/src/session.rs (the flow) and
   crates/box-daemon/src/chat.rs (the session records).
2. The session record is the task of the ephemeral instance (T-170). It is a
   versioned record in the daemon state directory (T-026).
3. The record holds the thread id, the owner, the guests and the instance id.
   It also holds the kage session id, the repository, the state and the
   consumed command ids (T-223).
4. First message: admission (T-251). Then create an ephemeral instance from
   the `[chat] profile`, start it, send `session/new`, and send the text as
   the first prompt.
5. A reply with no running turn starts a new turn.
6. A reply during a running turn waits in the session record, at most 8
   prompts (a design value). It goes in when the turn ends.
7. kage refuses a second prompt during a turn (T-171), so v1 has no steering.
8. An aside (`!!!`) is never sent to the agent.
9. A turn ends when the answer to `session/prompt` arrives. kage sends that
   answer after every child agent has ended (K7 of `TODO/upstream.md`).
10. Idle: when no turn runs and no message comes for `idle`, the daemon stops
    the instance and keeps its disk. The default is 30 min, the errand value.
11. The idle stop has no notice. The next message starts the instance, sends
    `session/load`, and then the prompt.
12. The session ends at `!stop`, at a thread archived or deleted from outside,
    or after `retention` with no message. The design default is 7 days, not
    measured.
13. At the end: cancel a running turn, release every ticket (T-251), delete
    the instance with verified absence, and archive the thread with a notice.
14. A failure gets a notice in the thread: a failed start, a failed turn, or
    an interrupted turn.
15. Gateway loss, with the daemon running:
    - a running turn goes on to its end, within the run timeout of the
      profile (T-171);
    - its output waits in the outbox (T-220);
    - no new session starts.
16. After 10 failed reconnects, cancel each running chat turn. Stop each chat
    instance and keep its disk.
17. Then keep trying every 60 s. The daemon does not exit, because it also
    serves persistent instances.
18. When the gateway is back, each affected thread gets a notice of what
    happened. The retention clock still counts from the last message.
19. The reason to differ from errand: here the operator can reach every
    instance through the CLI, and the run timeout bounds an unwatched turn.
20. Daemon stop or crash: the instances keep running (T-090, T-107). With no
    daemon the guest has no network, so a running turn usually fails.
21. At start, the daemon re-adopts each chat instance and reads its session
    record. A turn that was running gets an "interrupted" notice. The thread
    stays open to resume.
22. A record with no instance, or a chat instance with no record, is an
    orphan. The daemon removes it after verification (T-170).
23. Each session starts from the read-only base image in a new instance. No
    cache or disk passes from one session to another (KG-27).

## Decision

A chat instance lives from the first message to the end of its session. An
idle stop keeps it, and only the end of the session deletes it. This changes
step 22 of T-170 for chat instances. A stop by the idle rule, by the give-up
rule or by a crash does not end the task. A lost gateway lets each running
turn end within its timeout. After the give-up, it cancels the turns and
stops the instances.

## Prove

```sh
cargo test -p box-chat session::
cargo test -p box-chat --test guild_live session_flow -- --ignored
```

Expected: the unit tests use a fake gateway and the T-032 double. The first
message creates, starts and prompts. A reply during a turn waits, and an
aside never reaches the double. An idle stop keeps the disk, and the next
message starts the instance and loads the session.

`!stop` deletes the instance with a full Removal and archives the thread. A
give-up after 10 attempts cancels and stops, and deletes nothing. A daemon
restart posts an "interrupted" notice.

The live test runs a session with two follow-up prompts and one idle stop.
Then `!stop` deletes the instance, and `agent-box list` no longer shows it.

Host: any host for the unit tests. The live test needs a Linux KVM host or
Windows 11 with Hyper-V as admin, and the test guild.

## Start condition

T-220, T-221, T-170 and T-171.

# T-223: Pull requests composed by the daemon

**Source:** ruling D4; rule R-HOST-NO-TOOLS; the errand pull request flow (READ, below); findings CH-20 to CH-29.
**Category:** feature
**Milestone:** M10
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

The work of a chat session lands as a pull request. In errand, the path to a
pull request ran git on the host against the directory of the agent. Its
authorization mark stayed set after one mention of "PR". agent-box must open
a pull request with no host git and no path from the guest. It must open one
only for an explicit command that works once.

## Premise

- READ: errand writes the "asked" mark for any prompt that matches its PR
  pattern
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/commands.ts:175-184`).
- READ: it writes the mark before the participation check, and never removes
  it
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/session.ts:600-613`).
- READ: errand makes a fork and pushes there, so its bot never needs write
  access to the upstream. It waits up to 30 s for the fork, and reads its
  owner and name from the answer
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/pr.ts:234-282`).
- READ: the base is the `default_branch` of the upstream, and the request sets
  `maintainer_can_modify`
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/pr.ts:288-293`,
  `QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/pr.ts:389-398`).
- READ: the body is the commit body, then `---`, then a footer of the daemon
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/pr.ts:82-85`).
  The footer has no `@`
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/github.ts:162-220`).
- READ: errand reads that commit body with host git in the project of the
  agent
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/pr.ts:388`).
  R-HOST-NO-TOOLS forbids that.
- READ: errand accepts `.git` as a file, so a `gitdir:` line from the agent
  moves host git
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/pr.ts:177`).
- Design (T-146): the forge binding limits the repositories, and refuses a
  pushed ref outside `refs/heads/agent-box/<instance>/`.

## Approach

1. Files: crates/box-daemon/src/forge_api.rs (the forge API client of the
   daemon) and crates/box-chat/src/pr.rs (the command).
2. `[chat]` keys: `repositories` (the allowed upstream repositories and the
   default one) and `forge_secret` (the credential name of the forge token of
   the bot).
3. At session start, the daemon makes sure that the fork of the bot exists.
   It waits up to 30 s, and reads the owner and the name from the answer.
4. The forge binding of the instance (T-146) allows fetch from the upstream
   and the fork. It allows a push to the fork only, under
   `refs/heads/agent-box/<instance>/`.
5. The daemon adds a preamble to the first prompt. The preamble names the
   fork URL and the branch prefix.
6. The agent has no way to ask for a pull request: no request file, no tool
   and no text pattern. A participant types `!pr TITLE`.
7. The rights for `!pr` come from T-221. Each command has an id: the message
   id, or the interaction id.
8. The daemon writes the id into the session record as consumed before any
   API call. A replay, an edit or a second delivery of that id is refused.
9. The daemon lists the refs of the instance namespace on the fork, through
   the API. With one ref it uses that ref.
10. With several refs, the command must name one: `!pr BRANCH TITLE`. With no
    ref, the daemon refuses, and its notice asks for a push first.
11. The body, composed by the daemon only:
    - the commit messages of the branch, from the compare API of the forge,
      as untrusted text, cut at 8 KiB (a design value);
    - each `@` mention and each issue-closing keyword in that text, put
      inside a code span so that it notifies and closes nothing;
    - `---`, then a footer with the name of the requester and the thread
      link, with no `@`.
12. Only the daemon adds a closing reference, from its own record (T-226).
13. The title is the text of the command, at most 200 code points (a design
    value).
14. Open the pull request with the head `FORK_OWNER:BRANCH`, the base
    `default_branch` of the upstream, and `maintainer_can_modify`.
15. Post the URL in the thread. A failure posts the forge status and its
    message, never the token.
16. The host never runs git and never reads the data disk. The inputs of the
    daemon are the command, its own record and the answers of the forge API.
17. The broker injects the token for git (T-146). The daemon uses it for API
    calls. The guest never holds it.
18. The commit author is guest data. The system git config of the image
    names the bot, but the agent can change it. Add this limit to
    [docs/limits.md](../docs/limits.md).

## Prove

```sh
cargo test -p box-daemon forge_api::
cargo test -p box-chat pr::
cargo test -p box-chat --test forge_live pr_once -- --ignored
```

Expected: the unit tests run against a recorded forge API. The fork wait, the
owner and name from the answer, and the base from `default_branch` are each
tested. An `@user` and a "fixes #1" in a commit message come out inside code
spans, and the footer has no `@`.

A second delivery of one command id is refused with no API call. A prompt
that asks the agent to open a PR opens nothing. A guest's `!pr` is refused.

In the live test, the agent pushes to the fork through the broker, and `!pr`
opens one pull request. The same command id, sent again, is refused. A push
to `main` of the fork is refused by the broker.

Host: any host for the unit tests. The live test needs a Linux KVM host or
Windows 11 with Hyper-V as admin. It also needs the test guild, and a test
repository with a bot account.

## Start condition

T-146, T-221 and T-222.

# T-224: Thread commands and status

**Source:** the parity ruling of 2026-09-15; the errand command reference (READ, below); findings CH-06, CH-18 and CH-40 to CH-42; T-147.
**Category:** feature
**Milestone:** M10
**Priority:** P3
**Effort:** S
**Status:** open

## Problem

A user in a thread needs to see what the session does and costs, change the
model, and stop it. Each number must come from a source that the agent cannot
forge. No command may show another user's session.

## Premise

- READ: errand runs text commands and slash commands with the same code
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:docs/reference/commands.md:1-6`).
  The same file lists each command and who may use it.
- READ: errand ignores an unknown `!` command, because other bots share the
  channel
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:docs/threads.md:27-29`).
- READ: herdr falls back to idle when no rule matches. A wrong state changes
  only the display, and never sends input
  (https://raw.githubusercontent.com/herdrdev/herdr/v0.9.0/docs/next/website/src/content/docs/agents.mdx,
  lines 60-62).
- READ: kage emits events with a session id and a sequence number, among them
  usage_updated
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/reference/architecture.md:204-222`).
- Design (T-147): broker accounting is the only trusted count of requests and
  tokens.

## Approach

1. File: crates/box-chat/src/commands.rs. One table maps each command to its
   handler, its right (T-221) and its help line. `!name` and `/name` use the
   same table.
2. This entry adds `!status`, `!stop`, `!interrupt`, `!model [NAME]`, `!usage`
   and `!help`.
3. `!status` in a thread shows that session: the instance state, the turn, the
   queue position, and the broker counts and elapsed time.
4. `!status` in the channel shows the sessions of the caller only.
5. `!stop` ends the session (T-222). `!interrupt` sends `session/cancel`.
6. `!model` lists the models of the profile and switches with
   `session/set_config_option`. It is refused while a turn runs, because a
   switch inside a turn mixes two models in one answer.
7. `!usage` shows the provider window of T-253. Before M12 it shows the broker
   counts of T-147.
8. Every number comes from the broker or the daemon. A number that kage
   reports appears beside it, labelled agent-reported. A difference between
   the two is shown, not hidden.
9. No command shows another user's session, the number of slots, or which
   session holds which provider.
10. An unknown `!` command gets no answer.
11. The working, blocked and idle display of T-255 is for display only. No
    command acts on it.
12. Other commands of errand and their entries: `!then`, `!allow`, `!deny` and
    `!guests` (T-221, T-222); `!pr` (T-223); `!facts` and `!forget` (T-225).
13. `!steer` waits for a kage feature (T-222). `!shutdown` becomes "end every
    chat session", for operators. It never powers off the host.
14. `!ls`, `!cat`, `!file`, `!pwd` and `!compact` are T-258. They use the
    command table of this entry.
15. T-258 runs the file commands in the guest over an exec session (T-042).
    The host never reads the data disk.

## Prove

```sh
cargo test -p box-chat commands::
```

Expected: a named test for the output of each command. `!model` is refused
during a turn. `!status` from a user outside the thread is refused. A planted
kage usage number appears with its label and never replaces the broker count.
An unknown `!` command gets no answer. The text and the slash form run the
same handler.

Host: any host.

## Start condition

T-221, T-222 and T-147.

# T-225: Memory facts, kept per tenant

**Source:** the parity ruling of 2026-09-15; the errand memory design (READ, below); findings CH-30 and CH-31.
**Category:** feature
**Milestone:** M10
**Priority:** P3
**Effort:** M
**Status:** open

## Problem

errand keeps facts between sessions: facts about a person and facts about a
project. The agent writes all of them. Here the users do not trust each
other. A fact that the agent of one user writes, and the agent of another
user reads, is a stored prompt injection across tenants.

## Premise

- READ: in errand, facts about a person follow the speaker into every thread.
  Facts about a project go to the next thread that works there, whoever
  starts it
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:docs/memory.md:7-13`).
- READ: the agent appends lines to two files, and the daemon reads them after
  each turn (same file, lines 19-30).
- READ: the bounds are 300 characters for a fact, 2000 for the block, and 500
  project facts (same file, lines 53-57).
- Design (R-HOST-NO-TOOLS): the host never reads the guest disk and never
  follows a path from the guest.
- Design (T-091, T-180): after a delete, no byte that the agent wrote stays in
  the state of the instance.

## Approach

1. File: crates/box-chat/src/memory.rs.
2. The tenant is the owner of the session (T-221). A fact belongs to the
   tenant of the session that wrote it, whoever the fact is about.
3. A fact appears only in later sessions of the same tenant. A guest's fact
   from the owner's thread stays with the owner.
4. Project facts are kept per tenant and per project.
5. The preamble of the session (T-223) names the two notes files in the home
   of `agent`, and asks the agent to append facts there.
6. Collection: after each turn, the daemon runs one fixed exec in the guest
   (T-042), as uid 1000. It prints the two notes files and empties them.
7. The host opens no guest path. The output is untrusted bytes, capped at 64
   KiB (a design value).
8. Remove control characters. Drop a line longer than 300 characters.
9. Store: one file per tenant, outside each instance directory, in the daemon
   state directory. Write it atomically (T-026).
10. A record holds the text, the kind (person or project), the project, the
    session, the time and the label `agent-authored`.
11. Before a turn, the daemon composes the block, newest first, within 2000
    characters. A header says that agents wrote these lines in earlier
    sessions, and that they are notes, not instructions.
12. The header does not make the lines safe. The scope per tenant is the
    control.
13. `!facts` shows the facts of the caller's tenant only. `!forget` removes
    them. The owner and the operators may use both (T-221).
14. `[chat] memory = false` turns memory off.
15. These facts survive the delete of the instance on purpose. Add them to
    [docs/limits.md](../docs/limits.md), and to the teardown check of T-180
    as its one named exception.

## Prove

```sh
cargo test -p box-chat memory::
```

Expected: a fact from the thread of user A is absent from the block of user
B. This holds also when B was a guest in that thread. A line of 301 characters is
dropped, and the block stops at 2000 characters. `!forget` removes the facts.
The test double records the exec argv, and no host-side open of a guest path
happens.

Host: any host.

## Start condition

T-222 and T-224.

# T-226: GitHub mentions as a task source

**Source:** errand issue 3 (https://github.com/QaidVoid/errand/issues/3); findings CH-32 and CH-33.
**Category:** feature
**Milestone:** M10
**Priority:** P3
**Effort:** S
**Status:** open

## Problem

Requests for work also come from the forge: a mention of the bot, or an
assignment on an issue. errand has an open proposal for this source. The text
of an issue is as untrusted as a chat message.

## Premise

- READ: errand issue 3 proposes mentions and assignments as a second task
  source. The session binds to the issue or the pull request, and it answers
  with comments (https://github.com/QaidVoid/errand/issues/3).
- READ: the proposal uses the same admission queue, with allowlists of
  repositories and of authors (same issue).
- Design (T-220): the gateway opens no listening socket. A webhook needs a
  public HTTPS endpoint on the host.
- Not read in this session: the polling rules of the GitHub notifications
  API. Read them before the code.

## Approach

1. File: crates/box-chat/src/forge_tasks.rs. Poll the notifications of the bot
   account through the forge API. Open no listener.
2. `[forge_tasks]` keys: `repositories` and `allow_authors` (forge logins).
3. Accept a notification only for a listed repository and a listed author.
   The forge identity of the author decides, through the API, never the text.
4. The session binds to the issue or the pull request. It is an ephemeral
   instance with the flow of T-222, behind the queue of T-251.
5. The tenant is the forge author (T-225).
6. The daemon composes each comment by the rules of T-223: mentions in code
   spans, a footer, no token.
7. A pull request follows T-223. The command is a comment `/agent-box pr
   TITLE` from a listed author. The daemon adds the closing reference of the
   bound issue from its own record.
8. Obey the poll interval and the rate-limit answers of the forge.

## Prove

```sh
cargo test -p box-chat forge_tasks::
cargo test -p box-chat --test forge_live mention -- --ignored
```

Expected: the unit tests run against recorded API answers. A mention by an
author outside the list starts nothing, and a repository outside the list
starts nothing. In the live test, a mention in a test repository starts a
session, and its answer arrives as a comment.

Host: any host for the unit tests. The live test needs a Linux KVM host or
Windows 11 with Hyper-V as admin, and a test repository with a bot account.

## Start condition

T-222 and T-223.

# T-258: errand file and context commands parity: !ls, !cat, !file, !pwd and !compact

**Source:** the parity ruling of 2026-09-15; T-224, step 14; the errand command reference and session code (READ, below); the kage ACP extensions (READ, below).
**Category:** feature
**Milestone:** M10
**Priority:** P3
**Effort:** S
**Status:** open

## Problem

In errand, the people of a thread can read the project and free the context
of the agent. Parity needs the commands `!ls`, `!cat`, `!file`, `!pwd` and
`!compact`. errand reads the files on the host. Here the host never reads the
guest disk, and a path in a chat message is untrusted input.

## Premise

- READ: in errand, the owner and the invited users may run `!ls`, `!cat`,
  `!file` and `!pwd`. Only the owner and the operators may run `!compact`
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:docs/reference/commands.md:20`
  and lines 37-40).
- READ: errand maps the path to a host path and reads the file on the host
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/session.ts:1294-1327`).
  `!file` uploads at most 8 MiB (same file, line 122).
- READ: errand refuses `!compact` while a turn runs. A compaction under a
  turn changes the conversation that the agent answers about (same file,
  lines 2037-2048).
- READ: kage has the ACP request `_kage/session/compact`, which summarizes
  older turns at once. A `_kage/compaction` update gives the kept turns and
  the context tokens before and after
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/kage-extensions.md:42`
  and line 52).
- Design (R-HOST-NO-TOOLS, T-030): the host never reads the data disk and
  never follows a path from the guest.
- Design (T-224): one table maps each command to its handler, its right
  (T-221) and its help line.

## Approach

1. File: crates/box-chat/src/project_commands.rs. Add the five commands to
   the table of T-224, with the rights that errand gives them.
2. `!ls`, `!cat` and `!file` run one fixed argument vector in the guest over
   an exec session (T-042), as uid 1000. The path is one element of it, and
   no shell parses it.
3. A path is relative to the work directory /home/agent/work. Refuse an
   absolute path and a `..` component before any exec.
4. A symlink in the work directory can point outside it. The guest resolves
   it, so a thread user reads what uid 1000 can read.
5. Write that limit in [docs/limits.md](../docs/limits.md). uid 1000 can read
   the kage token and the provider nonces, which work only inside this
   instance.
6. Bound the output: 64 KiB of text for `!ls` and `!cat` (a design value),
   and 8 MiB for `!file`, as errand does.
7. The output is untrusted bytes. Render it by the rules of T-220: control
   characters removed, an empty allowed-mentions list, and a code fence.
8. `!pwd` answers from the record of the daemon: the project and the work
   directory. It runs no exec.
9. `!compact` sends `_kage/session/compact` through the ACP client of T-161.
   Add that method to the fixed method list of T-161, step 7.
10. Refuse `!compact` while a turn runs, as errand does. Show the counts of
    the `_kage/compaction` update, labelled agent-reported.
11. Each command writes a host-observed event (T-176) with the user, the
    command and the path.

## Prove

```sh
cargo test -p box-chat project_commands::
```

Expected: exit 0. The tests use the T-032 double and the recorded ACP peer
of T-161, with named tests for each of these:

- an absolute path, or a path with `..`, is refused before any exec;
- the exec argument vector is fixed, and the path is one element of it;
- `!file` refuses a 9 MiB file, and `!cat` output stops at 64 KiB;
- control characters and mentions in a file reach the thread inert;
- `!compact` during a turn is refused, and after the turn it sends one
  `_kage/session/compact`;
- a user without the right gets no file content;
- the double records every exec, and no host-side open of a guest path
  happens.

Host: any host.

## Start condition

T-224, T-042 and T-161.
