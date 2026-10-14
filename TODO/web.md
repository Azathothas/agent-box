# Web UI

This area covers milestone M11, the web UI. A user logs in with the
identity of the chat service. Then the user watches and drives the sessions
that they started. The exit criteria are in
[docs/roadmap.md](../docs/roadmap.md#m11-web-ui). The rules for every late
surface are in
[docs/architecture.md](../docs/architecture.md#the-late-surfaces). The kept
upstream findings are WB-01 to WB-19 in
[docs/references.md](../docs/references.md#web-ui).

The rules that decide the design of this area:

- A logged-in user drives exactly the sessions that they started (ruling of
  2026-09-15). In v1 a user does not see the session of another user.
- The operator role comes from the config file only. No role of the chat
  service grants it.
- A login makes the web UI a second trusted surface. The bind address is not
  the access control.
- Each call is checked for its user, its Host, its Origin and its CSRF token.
  No response allows credentialed cross-origin reads.
- The browser never talks to kage serve. The daemon is the only ACP client
  (T-161).
- Agent text is untrusted data. The UI shows it as text, never as markup.

The product crates do not exist yet, and T-240 creates box-web. A crate path
in this area names a planned file.

# T-240: Web UI server with chat-service login

**Source:** the ruling of 2026-09-15 (a web user drives only their own sessions); [docs/architecture.md](../docs/architecture.md#the-late-surfaces); the READ items below; findings WB-01 to WB-15.
**Category:** feature
**Milestone:** M11
**Priority:** P2
**Effort:** L
**Status:** open

## Problem

The web UI reaches instances, transcripts and prompts. errand took its bind
address as the access control, and a page in the operator's own browser can
still post to it from another site. agent-box needs authorization per user on
every call. It must close the browser attacks before the UI binds to any
address.

## Premise

- READ: errand allows a bind on loopback, RFC 1918, link-local, ULA and
  100.64.0.0/10, and refuses a wildcard, a public address and a host name
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/web/address.ts:1-104`).
- READ: errand has no Host or Origin check, and parses a JSON body whatever
  its content type
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/web/server.ts:174-226`).
- READ: so a page in the operator's browser can post a prompt across sites,
  and DNS rebinding can read transcripts. This is read from code, not
  exercised
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/web/server.ts:237-280`).
- READ: a credential exchange can narrow a grant and never widen it. Client
  labels play no part in authorization
  (`pingdotgg/t3code@9ea892e3b365faff31eed294ac38f70d122656b2:docs/internals/environment-auth.md:9-14`).
- READ: a list of access records holds metadata and never a recoverable
  secret (same file, lines 16-19).
- READ: a failed strong proof fails, and never falls back to a weaker one
  (same file, lines 21-23). A refused credential never falls back to the
  development credential (lines 54-55).
- READ: a successful handshake grants no extra authority. Every RPC declares
  the scope that it needs (same file, lines 27-31).
- READ: revocation and insertion of a credential share one transaction (same
  file, lines 33-38).
- READ: flue's development listener reflected each Origin with credentials
  allowed, so any site could read its authenticated answers
  (https://github.com/withastro/flue/issues/483).
- READ: an agent-sandbox router worked as an unauthenticated proxy into
  sandboxes until a fix (https://github.com/kubernetes-sigs/agent-sandbox/pull/755).
- READ: its threat model asks for strict id validation and a target built
  from a fixed format
  (`kubernetes-sigs/agent-sandbox@4b63868f0d67e4ee0fa690bb14cb5722b78d679d:docs/security/threat_model.md:75-76`).
- Not read in this session: the OAuth 2 documentation of Discord. Read it
  before the code.
- READ: `kage serve` accepts its token in a `?token=` query parameter, and
  the connect URL that it prints uses that form
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-remote/src/head.rs:203-232`).

## Approach

1. Process: `agent-box web` runs as the operator, as a client of the daemon
   control API (T-028). It never opens the credential store or the broker.
2. The reason for a separate process: a defect in the HTTP code then runs
   outside the process that holds each credential in memory.
3. The limit of that reason: both processes run as one user. A code
   execution in the web process can read the store files of that user.
4. The daemon offers the web process a method set in which each call names
   the platform user. The daemon checks ownership again on each call.
5. Create crates/box-web with server.rs, login.rs, session.rs, csrf.rs and
   api.rs. The static assets are embedded in the binary. No asset comes from
   another host.
6. Bind: 127.0.0.1 by default. A bind outside loopback needs `--public` and a
   configured origin, for example `https://agent-box.example.org`.
7. A public bind is named in the startup report. TLS comes from a reverse
   proxy that the operator runs, or from a certificate in the config.
8. Login: the OAuth 2 authorization code flow of Discord, with a `state`
   value that a pre-login cookie binds. Add PKCE if Discord accepts it. The
   scope is `identify`.
9. Guild membership and roles come from the bot's guild data, through the
   daemon. The web UI does not ask the user for a second scope.
10. Session: 32 random bytes in a cookie with HttpOnly and SameSite=Strict.
    On an https origin, the cookie also has Secure and the `__Host-` prefix.
11. The server keeps each session record: the platform user, the creation
    time, the last use and the expiry. Defaults: 12 h total and 1 h idle
    (design values, not measured).
12. Checks on each call, in this order:
    - the Host header equals the configured host, which stops DNS rebinding;
    - a call that changes state has an Origin equal to the configured
      origin, and a missing Origin is a refusal;
    - a call that changes state has the content type `application/json` and
      a CSRF header equal to the token of its session;
    - the session is valid;
    - the platform user is still in the lists of T-221, checked through the
      daemon, cached for at most 60 s (a design value).
13. One function authorizes each call. A session belongs to the platform user
    who started it, in the chat or on the web.
14. The operator role comes from `[web] operators`, user ids in the config.
15. Read and write are separate checks. A call that reads one session and
    writes another needs both.
16. The server sends no Access-Control header of any kind, and never reflects
    an Origin.
17. Response headers: a Content-Security-Policy with `default-src 'self'` and
    `frame-ancestors 'none'`, no inline script, `nosniff`, `no-referrer`, and
    `no-store` on the API.
18. The UI shows agent text as text. Its Markdown view allows no raw HTML and
    no image from another host. A link shows its full URL.
19. The rules of t3code, applied:
    - a list answer holds metadata only, never a nonce, a token, a credential
      or a slot identity;
    - no field from the client names a role;
    - a failed check fails, with no fallback to a weaker check;
    - no development credential exists;
    - a session rotation is one transaction.
20. The web UI never shows the token or the connect URL of `kage serve`. The
    browser has no route to kage serve.
21. Each upgraded connection has a count per user, an idle timeout and a cap
    on message size.
22. An instance in a path is a name of T-020 or an id of 32 hex digits. The
    server looks it up in the daemon. No address or path from the client
    becomes a target.
23. Logout deletes the session record. A user removed from the lists is
    refused at the next call.

## Prove

```sh
cargo test -p box-web authz::
cargo test -p box-web http::
```

Expected: each of these is a refusal that a named test sees:

| Test | Result |
| --- | --- |
| a user reads the session of another user | refused |
| a user sends a prompt to the session of another user | refused |
| a user removed from the lists, with a valid cookie | refused at the next call |
| a request with another Host | refused |
| a call that changes state, with no Origin or another Origin | refused |
| a JSON body sent as `text/plain` | refused |
| a call with no CSRF header | refused |
| a client field that claims the operator role | ignored |
| a failed strong check | refused, never downgraded |
| a public bind without `--public` | refused at start |

A scan of every list answer finds no planted nonce, credential or slot id. No
answer carries an Access-Control header.

Host: any host.

## Start condition

T-028, T-176 and T-221.

# T-241: Watch a session, start one from the web

**Source:** the parity ruling of 2026-09-15 (errand has a live session view); findings WB-14 and WB-16; the multi-client rules of kage serve (READ, below).
**Category:** feature
**Milestone:** M11
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

A long diff or a long transcript is hard to read in a thread. The web view
shows a session as it runs and can start a new one. It must show only what
the caller may see. It must show no control for an action that the caller
may not take, not only refuse that action on the server.

## Premise

- READ: several clients can attach to one kage session and get a replay
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/remote.md:137-166`).
- READ: clients get short-lived WebSocket tickets over authenticated HTTP, so
  long-lived tokens stay out of socket URLs
  (`pingdotgg/t3code@9ea892e3b365faff31eed294ac38f70d122656b2:docs/internals/environment-auth.md:27-31`).
- READ: a signed asset URL is a bearer credential for one file. The server
  checks the identity of the file again when it serves it (same file, lines
  67-74).
- Design (T-176): the event log of each instance holds agent-reported and
  host-observed records with a sequence number.
- Design (T-161): the daemon is the only ACP client of kage serve.

## Approach

1. File: crates/box-web/src/view.rs, and a socket route in
   crates/box-web/src/api.rs.
2. The live view streams the event records of T-176 for one session. The
   chat thread renders the same records, so one source feeds both.
3. The browser gets a socket ticket over authenticated HTTP. The ticket works
   once, for one user and one session, for 30 s (a design value).
4. The socket URL carries the ticket only, never the cookie value or a
   long-lived token.
5. Each message on the socket is authorized again, for one scope on one
   session: watch or drive. The handshake grants nothing more.
6. Replay comes from the event log of the daemon, from a `seq` that the
   browser names. It never comes from kage serve directly.
7. Drive: prompt, cancel and stop, for the owner and the operators. Each
   action goes through the daemon, by the same path as the commands of T-224.
8. Start: a session from the web goes through the lists of T-221 and the
   queue of T-251. It gets an ephemeral instance with the flow of T-222, and
   no chat thread.
9. File view: a fixed read command in the guest, over an exec session (T-042)
   as uid 1000, capped at 1 MiB (a design value). The host never reads the
   data disk.
10. The output of the file view is untrusted text. The UI shows it with the
    rules of T-240.
11. A download link works for one user, one session, one path and the digest
    of that content, for 5 min (a design value).
12. The UI renders no control for an action that the caller may not take. The
    server refuses such an action too.
13. v1 has no observer mode. A user sees only their own sessions, and an
    operator sees each session.

## Prove

```sh
cargo test -p box-web view::
cargo test -p box-web --test browser_live -- --ignored
```

Expected: a second use of a ticket is refused, and an expired ticket is
refused. A message for another session on an authorized socket is refused. A
replay starts at the requested `seq`. The file view runs the fixed argv in
the test double, and no host-side file open happens. A download link is
refused after its expiry and for another user.

The browser test drives the real UI as a user and as an operator. The user
sees only their own sessions. The document of a page has no control for a
refused action.

Host: any host for the unit tests. The browser test needs a Linux KVM host or
Windows 11 with Hyper-V as admin, a browser, and the test guild for the
login.

## Start condition

T-240 and T-222.

# T-242: Metrics views

**Source:** the request of the operator of 2026-09-15 for three places to see usage: a dashboard, the web UI and chat commands; T-147; finding WB-19.
**Category:** feature
**Milestone:** M11
**Priority:** P3
**Effort:** M
**Status:** open

## Problem

Token use, cost, run time and queue depth help only where somebody looks. A
view that computes its own numbers is a second home for one fact. A view that
mixes users shows each user the work of others.

## Premise

- Design (T-147): broker accounting is the only trusted source of requests and
  tokens per instance and binding.
- READ: "denied" (the policy refused) and "failed" (an allowed lookup failed)
  are separate counters, because a failed update looks like a denial from
  inside
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/components/egress.md:289-314`).
- Design (T-254): a cost carries the date of its catalogue. An absent price
  is a dash.

## Approach

1. File: crates/box-web/src/metrics.rs. One read model in the daemon feeds
   the chat command of T-224, the session view, the dashboard and an export.
2. The sources are broker accounting (T-147), the queue and slots of T-250 to
   T-253, the cost of T-254 and the session state of T-255.
3. The session view shows tokens, requests, cost, elapsed time, provider and
   queue position for one session.
4. The dashboard shows totals per provider, per model and per day, the queue
   depth and the slot use.
5. A user sees only the rows of their own sessions. An operator sees each
   row. The same function of T-240 decides.
6. A user never sees the number of slots, or which session holds which slot.
7. An absent value is a dash, never a zero. A cost shows the date of its
   catalogue.
8. A number that kage reports appears only beside the broker number, with the
   label agent-reported.
9. Egress counters keep "denied" and "failed" apart.
10. `GET /api/metrics/export` gives the operator the same records as JSON
    lines.

## Prove

```sh
cargo test -p box-web metrics::
```

Expected: one accounting record gives the same figures in the chat command,
the session view and the dashboard. A user's dashboard holds only that user's
rows. A cost carries its catalogue date. A model with no price shows a dash.
"denied" and "failed" are separate columns.

Host: any host.

## Start condition

T-240 and T-147. The cost column needs T-254, and shows a dash before it.
