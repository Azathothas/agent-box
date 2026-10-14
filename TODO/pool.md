# Provider pool and metrics

This area covers milestone M12: provider slots, an admission queue, backoff,
usage windows, cost, and the state of a stuck session. The exit criteria are
in [docs/roadmap.md](../docs/roadmap.md#m12-provider-pool-and-metrics). The
rules for every late surface are in
[docs/architecture.md](../docs/architecture.md#the-late-surfaces). The kept
upstream findings are PL-01 to PL-26 in
[docs/references.md](../docs/references.md#provider-pool).

The rules that decide the design of this area:

- Provider subscriptions are a small pool with a queue in front (ruling of
  2026-09-15). A slot is one provider API key, because kage takes API keys
  only.
- The host decides which slot serves an instance. No file or setting in the
  guest names a slot.
- The guest of a session cannot learn how many slots exist, or which slot
  another session uses.
- Billing, backoff, usage windows and slot accounting use broker data only
  (R-TRUSTED-CHANNEL, T-147). The ACP stream and the event channel never feed
  them.
- An absent number is a dash, never a zero. A guessed number is never shown
  as a measured one.
- A state that the host infers is for display. Nothing ends a session because
  of it.

The product crates do not exist yet, and T-250 creates box-pool. A crate path
in this area names a planned file.

# T-250: Provider slots

**Source:** the ruling of 2026-09-15 (a small pool with a queue); findings PL-01, PL-02, PL-11, PL-12 and PL-15 to PL-18; [TODO/upstream.md](upstream.md) (K6, K7).
**Category:** feature
**Milestone:** M12
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

A deployment has a few provider keys and more users than keys. Something on
the host must decide which key serves which turn, keep each key inside its
limits, and hide that choice from the guest. errand held a slot for a whole
turn, because it could not see the requests inside a turn. The broker sees
every request, so the unit of a slot is a new decision.

## Premise

- READ: kage model providers use API keys only. The writer of an OAuth record
  is compiled for tests only
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/auth.rs:34-55`,
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/auth.rs:305-310`).
- READ: errand holds a slot for a whole turn, because a request inside a tool
  loop is not visible from outside the agent
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/admission/scheduler.ts:1-12`).
- READ: the account pools of oh-my-pi are a routing policy for trusted
  clients, not an authorization boundary
  (`can1357/oh-my-pi@6f2c14b3e4cc065139789da893e4f86f3d72958c:docs/auth-broker-gateway.md:189-211`).
- READ: with OAuth accounts, the oh-my-pi broker is the only writer of
  refresh tokens. A client gets a sentinel in place of each refresh field
  (`can1357/oh-my-pi@6f2c14b3e4cc065139789da893e4f86f3d72958c:docs/auth-broker-gateway.md:42`).
- READ: kage registers a provider only when a key for it is reachable
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/guide/providers.md:157-188`).
  So the provisioned nonces decide which providers the guest sees (T-141).
- Design (T-141): each instance has one nonce per binding. Design (T-143): a
  binding with no credential fails closed.
- Design ([docs/architecture.md](../docs/architecture.md)): box-pool is a
  layer 4 library, and it never depends on box-broker.
- Not read: whether a provider prompt cache is shared between two keys of one
  account. The design below assumes that it is not.

## Approach

1. Create crates/box-pool as a library and a workspace member, with
   slots.rs, lease.rs and config.rs.
2. box-daemon defines the trait that admits and leases. box-pool implements
   it, and box-cli connects the two at daemon start. The broker never calls
   box-pool.
3. Config `[pool.NAME]`: the provider kind, the binding name, and the slots.
   A slot names one credential of the store.
4. Each slot also has `plan` (`metered` or `subscription`, for T-254) and
   `turns`, the number of turns that it serves at once. The default of
   `turns` is 1, a design value.
5. A binding of box-policy can name a pool in place of one credential.
6. The unit of a slot is one turn. A turn starts at `session/prompt` and ends
   at its answer, which kage sends after every child agent ends (K7).
7. The run timeout of T-171 bounds a turn whose answer never comes.
8. The reasons for a turn: a provider cache is per key, so a change of key
   inside a turn loses the cache. A lease per session lets one long session
   hold a key.
9. A request outside a turn, for example a background request of kage, takes
   a short lease when a slot is free. Otherwise the broker fails closed.
10. A lease sets the credential of the instance's binding in the broker. The
    daemon makes that change through the API of box-broker.
11. At release, the binding loses its credential and fails closed again.
12. The guest keeps one nonce per binding. A new lease never changes the
    nonce, and the guest never sees a slot, a count or another lease.
13. A pool serves one provider kind. With the choice `any`, the daemon picks
    a pool before each turn, and sets the model with
    `session/set_config_option`.
14. The profile lists the pools that a session may use. The instance gets a
    nonce for each of their bindings.
15. A session that names a provider waits for that provider. It never moves to
    another provider in silence.
16. A lease is released exactly once on each path. The paths are the end of
    the turn, a failure, a cancel, a timeout, a stop, a delete and a restart.
17. A second release returns false and writes a defect event.
18. Leases are not kept across a daemon restart. A turn that ran then is
    interrupted (T-222).
19. OAuth subscription accounts are out of v1, because kage has no provider
    OAuth (K6). If that changes, the broker writes each refresh token on the
    host, and the guest gets a sentinel only.

## Prove

```sh
cargo test -p box-pool slots::
cargo test -p box-pool lease::
```

Expected: two turns on a pool with two slots get two different credentials,
and a third turn waits. A lease is released once on each exit path, and a
second release returns false. A binding with no lease fails closed in the
broker test double. A scan of the provisioning messages and of the kage
environment finds no slot id and no slot count.

Host: any host.

## Start condition

T-141, T-143, T-147 and T-161.

# T-251: Admission queue

**Source:** the errand scheduler (READ, below); findings PL-07 to PL-10 and PL-26.
**Category:** feature
**Milestone:** M12
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

More people ask than there are slots, or memory for instances. Without a
queue the answer is an error, and a user who gets an error tries again. A
burst then becomes a rate limit for every user of a shared key.

## Premise

- READ: errand has two caps that do not depend on each other: concurrent
  turns, which cost provider load, and live sessions, which cost memory
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/admission/scheduler.ts:1-12`).
- READ: a ticket is released once, and a second release returns false
  (same file, lines 29-46 and 195-200).
- READ: a full queue refuses with a reason
  (same file, lines 170-175).
- READ: an expiry sweep runs at a quarter of the longest wait
  (same file, lines 295-305).
- READ: a position is reported only when it changes (same file, lines
  281-291).
- READ: session starts are at least 750 ms apart (same file, lines 94-96).
  `tryAdmit` admits now or never (line 159).
- READ: the defaults are 2 turns, 4 live sessions, a queue of 32, and a wait
  of 900 s
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/config/schema.ts:441-446`).
- READ: in flue, an abort for the whole instance was the only external
  cancel, and it killed unrelated queued work
  (https://github.com/withastro/flue/issues/439).

## Approach

1. File: crates/box-pool/src/queue.rs, with a clock that tests inject.
2. Two caps: turns at once per pool, and live chat instances on the host. The
   operator sets each one.
3. A live instance holds an instance ticket. A turn holds a turn ticket (T-250
   leases it a slot).
4. An idle stop (T-222) releases the instance ticket, because a stopped
   instance holds no memory.
5. One queue per pool, and one queue for `any`. The first free slot of a
   listed pool admits a request from `any`.
6. Bounds: the length (default 32) and the wait (default 900 s), the errand
   values. A full queue refuses, and the user gets the reason.
7. An expired request is removed, and its user gets the reason.
8. The expiry sweep runs at a quarter of the longest wait.
9. The position goes to the thread (T-220) or the web view (T-241) only when
   it changes.
10. An estimate of the wait comes only from an observed reset time (T-253).
    No estimate is guessed.
11. Starts of instances are at least 750 ms apart.
12. `try_admit` admits now or refuses, for work that is useful only now.
13. A cancel removes one request. The stop of one session never cancels the
    work of another session.
14. Backoff (T-252) stops admission only for the slots that back off.
15. Each ticket is released once on each exit path, as the leases of T-250.

## Prove

```sh
cargo test -p box-pool queue::
```

Expected: the tests drive an injected clock. A request is admitted when a
slot frees. A request that names a provider waits while a slot of another
provider is free. A full queue refuses with its reason. A withdrawal removes
exactly one request.

The reported positions match the real admission order over a shuffled set.
Starts are at least 750 ms apart. A second release returns false.

Host: any host.

## Start condition

T-250.

# T-252: Backoff from broker-observed responses

**Source:** rule R-TRUSTED-CHANNEL; T-147 (step 8); findings PL-05, PL-06, PL-09, PL-19 and PL-22.
**Category:** feature
**Milestone:** M12
**Priority:** P2
**Effort:** S
**Status:** open

## Problem

errand starts a backoff for every session from a retry event that the agent
reports. A guest process can forge that event, so one tenant can stop the
work of every tenant. Backoff must start only from provider answers that the
broker sees, and it must hold only the key that got the answer.

## Premise

- READ: the retry event of errand's agent client calls `noteRateLimit`
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/session.ts:889-890`).
- READ: `noteRateLimit` stops admission for every session, because the limit
  is per account
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/admission/scheduler.ts:214-235`).
- READ: errand starts at 5 s and doubles up to 300 s. A success halves the
  backoff (same file, lines 94-95 and 236-240).
- READ: oh-my-pi keeps a rate-limit block per credential and per scope, with
  a cause
  (`can1357/oh-my-pi@6f2c14b3e4cc065139789da893e4f86f3d72958c:docs/auth-broker-gateway.md:79-80`).
- READ: when it cannot tell clients apart, it keeps the credential blocked
  rather than allow repeated 429 answers (same file, lines 110-116).
- READ: a definitive failure disables the credential with a cause (same
  file, lines 120-125). Such a failure is an invalid grant or token, a revoked
  token, or a 401 or 403 that no network fault caused.
- Design (T-147): the broker records each 429 and 529 answer with
  `retry-after` and the rate-limit headers of the provider.

## Approach

1. File: crates/box-pool/src/backoff.rs. Its only input is the records of
   T-147.
2. No code path reads the ACP stream or the event channel for backoff.
3. Keep the state per slot and per scope. One slot can have more than one
   scope, when a provider meters more than one quota.
4. A 429 or 529 blocks that slot and scope for the longer of `retry-after`
   and the backoff time.
5. The backoff time starts at 5 s and doubles up to 300 s. A success halves
   it. These are the errand values, and the operator can change them.
6. When the broker cannot tell which session caused a limit, the slot stays
   blocked until its time ends.
7. A definitive failure disables the slot with its cause. It never comes back
   on a timer. The startup report and the metrics of T-242 name it.
8. The other slots go on. At its next turn, a session on a blocked slot gets
   a free slot of the same pool (T-250).
9. The broker sends each provider answer to the guest unchanged. kage then
   applies its own retry.

## Prove

```sh
cargo test -p box-pool backoff::
```

Expected: a forged retry event changes no slot. The test forges it in the
ACP stream and on the event channel. A recorded 429 with `retry-after: 20`
blocks that slot for 20 s, and the other slot of the pool still serves. A
recorded 401 disables the slot, and the slot stays disabled after an hour of
injected time.

Host: any host.

## Start condition

T-147 and T-250.

# T-253: Usage windows and reset times

**Source:** the parity ruling of 2026-09-15 (errand `!usage`); findings PL-13, PL-14, PL-20 and PL-21.
**Category:** feature
**Milestone:** M12
**Priority:** P3
**Effort:** S
**Status:** open

## Problem

A key on a plan has a usage window. When the window is spent, the slot is not
free and not busy. It waits for a time that only the provider knows. A queue
that cannot tell these apart tells users to wait for a slot that is not
coming.

## Premise

- READ: errand reads the window of z.ai, a rolling 5-hour token window, from
  a quota endpoint. It uses the `TOKENS_LIMIT` entry with `percentage` and
  `nextResetTime`
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/provider/zai.ts:1-58`).
- READ: a shape that errand does not know never counts as a spent window. A
  spent window gets an answer with the reset time before a prompt is queued
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/session.ts:615-623`).
- READ: oh-my-pi polls usage endpoints with a cache of 5 min per credential
  and 25 percent jitter. A provider limits these endpoints per source
  address
  (`can1357/oh-my-pi@6f2c14b3e4cc065139789da893e4f86f3d72958c:docs/auth-broker-gateway.md:161-169`).
- Not read: where each other provider of the pool publishes its window. It
  can be a header, a body field, an error answer or an endpoint.

## Approach

1. File: crates/box-pool/src/usage.rs. One parser per provider shape, each
   with a committed fixture of a real answer, its credential removed.
2. For each slot keep the remaining allowance, the reset time, and the time
   of the observation.
3. An unknown reset time is a dash, never a guess. A shape that the parser
   does not know never counts as spent.
4. A spent slot is `exhausted` until its observed reset time. The queue never
   admits it.
5. The slot leaves `exhausted` at the observed reset time, or after a
   successful request. It never leaves on a guessed timer.
6. Poll a usage endpoint at most every 5 min per slot, with 25 percent
   jitter. Keep the last good report for 24 h, with its age.
7. The daemon calls a usage endpoint itself, with the key of the slot. The
   guest never calls it.
8. Check the window before a request is queued. A spent window gets an
   answer with the reset time.

## Prove

```sh
cargo test -p box-pool usage::
```

Expected: each provider shape parses its fixture. An unknown shape gives a
dash and never a spent window. An exhausted slot is never admitted, and it
comes back at the observed reset time of the injected clock. Polls of one
slot are at least 5 min apart.

Host: any host.

## Start condition

T-250 and T-251.

# T-254: Cost from the catalogue, with its date

**Source:** the kage model catalogue (READ, below); the old metrics plan of 2026-09-15 (a price carries the date of its catalogue).
**Category:** feature
**Milestone:** M12
**Priority:** P3
**Effort:** S
**Status:** open

## Problem

A token count becomes a cost only with a price, and a price is a number of
somebody else that changes. A cost from an old price, with no date, is a
wrong number on a report. A zero for an unknown price is a false number.

## Premise

- READ: kage generates its model catalogue from models.dev and a manual
  fixture
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/catalog/generated.rs:1-4`).
  The generated file carries no date.
- READ: the price of a model is optional, in dollars per million tokens
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/catalog/mod.rs:67-68`).
- READ: a price has input, output and optional cache read and cache write
  parts, as `f64` values
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-core/src/event.rs:93-105`).

## Approach

1. Add the planned command `cargo xtask catalogue fetch`. It writes the
   models.dev catalogue into crates/box-pool/catalogue.json, with the fetch
   date, the URL and the sha256.
2. The data file is the one home of prices. Nobody writes a price by hand.
3. A change of the file is a reviewed commit, as a pins change is.
4. File: crates/box-pool/src/cost.rs. Cost is the token counts of T-147 times
   the prices of the catalogue, for the model that the broker saw.
5. Compute in integer micro-dollars from the decimal text of the catalogue.
   Never add up floating-point values.
6. Each cost carries the date of the catalogue.
7. A model with no price gives a dash. The type is an `Option`, so no code
   path can turn an absent price into a zero.
8. A slot with `plan = "subscription"` shows "plan" and no cost per token.
9. kage reports its own cost from its own catalogue. That number is
   agent-reported, and it appears only beside the broker number.

## Prove

```sh
cargo test -p box-pool cost::
```

Expected: a priced model gives a figure with the catalogue date. A model with
no price gives a dash. A subscription slot gives no cost per token. A sum of
one million small requests gives the exact integer result.

Host: any host.

## Start condition

T-147 and T-250.

# T-255: What a stuck session looks like

**Source:** the old metrics plan of 2026-09-15 (working, blocked and idle); finding CH-40 (herdr).
**Category:** feature
**Milestone:** M12
**Priority:** P3
**Effort:** S
**Status:** open

## Problem

In a shared deployment, the costly failure is not a crash. It is a session
that holds a slot and does nothing, while nobody can tell it from a session
that works. The state must come from facts that the host sees, and a wrong
state must never end somebody's work.

## Premise

- READ: herdr marks each pane as working, blocked or idle. With no matching
  rule it falls back to idle, and a wrong state changes only the display
  (https://raw.githubusercontent.com/herdrdev/herdr/v0.9.0/docs/next/website/src/content/docs/agents.mdx,
  lines 60-62).
- Design (T-147): the broker knows each request in flight for each instance.
- Design ([docs/protocol.md](../docs/protocol.md)): the control port carries
  stats from box-guest.
- Not measured: the time without activity after which a working session
  looks idle.

## Approach

1. File: crates/box-pool/src/liveness.rs.
2. The states, each from facts of the host:

| State | From |
| --- | --- |
| working | a broker request of the instance in flight, or guest CPU above a threshold in the stats of the control port |
| blocked | a queued prompt, an admission wait, or a permission request with no answer |
| idle | neither, for longer than a threshold |

3. No state comes from a field that the agent sets.
4. The thresholds are design defaults. The entry states that they are
   guesses, and records the measured values when traffic exists.
5. The thread, the web view and the metrics show an idle session that holds
   a slot, to its owner and to the operators.
6. Nothing ends, stops or reaps a session because of this state. Only the
   rules of T-222 stop an instance, and they read the time since the last
   message.

## Prove

```sh
cargo test -p box-pool liveness::
```

Expected: each state comes from planted host facts, never from a reported
field. A session that waits for an answer shows blocked, not idle. A test
asserts that no code path ends a session on the idle state.

Host: any host.

## Start condition

T-147, T-250 and T-222.
