# Broker

This area covers the credentials. The broker runs in the daemon and holds every
credential. The guest holds only nonces, one for each instance and binding.
Credentialed traffic goes to plain-HTTP endpoints on the gateway,
`http://<binding>.broker.internal/`. The broker opens TLS to the fixed upstream
of the binding itself. The design is in [broker](../docs/broker.md). The path
from the guest to the gateway is in [net](net.md). kage's side is in
[kage](kage.md).

The earlier plan had a CONNECT broker with TLS interception. Decision P4 and
ruling D5 supersede it. Its parser traps,
timeouts, gate conditions, fail-closed shapes and CA rules carry over where
they still apply. Each entry says what changed with the endpoint model. TLS
interception is now an optional backlog entry (T-149), because kage trusts only
its compiled-in roots.

# T-140: Credential store

**Source:** [broker](../docs/broker.md). The TEMPLATE secret rules and the
OpenSandbox availability finding below. The credential store entry of the
earlier plan maps here.
**Category:** feature
**Milestone:** M6
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The real credentials live on the host. A store that leaks a value into a log,
an argv or the guest defeats the design. A store that fails open forwards a
request without its credential, or with a stale one.

## Premise

- READ: an agent never asks for a secret value. One redaction helper handles
  every secret-shaped value, and a log records the fact, not the value
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/security/secrets.md:23-103`).
- READ: compare a secret in constant time, and take identifiers from a
  cryptographic random source
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/conventions/forbidden-patterns.md:39`).
- READ: OpenSandbox treats its store as a hard dependency of all intercepted
  traffic. A timeout, a refusal, an EOF, a 5xx or a malformed snapshot fails
  the request closed before any upstream byte
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/guides/credential-vault.md:85-97`).
- READ: errand's podman backend puts the real provider credential on argv as
  `--env KEY=VALUE`, so it shows in the host process table
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/podman.ts:124-131`).
- Design: DPAPI on Windows. On Linux, a key file or the Secret Service (the
  Decision below).

## Approach

1. Files: crates/box-broker/src/store.rs and crates/box-broker/src/redact.rs.
   The control API (T-028) carries `secret.set`, `secret.list` and
   `secret.rm`.
2. `agent-box secret set NAME` has no value argument. It reads the value from
   stdin, without echo on a terminal.
   - Remove one final LF or CRLF. Refuse an empty value.
   - Refuse a value above 64 KiB (a design value).
   - A name matches `[a-z0-9][a-z0-9-]{0,62}`.
3. The file is `secrets.v1` in the daemon state directory (T-026).
   - Write a temporary file and rename it in the same directory.
   - Mode 0600 on Linux. On Windows, a DACL for the owner only.
4. A record holds a format version, the name, a kind, two times, an AEAD
   nonce and the ciphertext. Use XChaCha20-Poly1305.
   - The associated data is the format version and the name. A record moved
     to another name fails to decrypt.
5. The key:
   - Linux: `store.key`, 32 bytes from `getrandom`, mode 0600, made on the
     first write. A key file with a wider mode stops the daemon, and the
     startup report names it.
   - Windows: `CryptProtectData` in user scope with
     `CRYPTPROTECT_UI_FORBIDDEN`. The record name is the optional entropy.
     There is no key file.
6. Memory:
   - Hold values in `Zeroizing<Vec<u8>>`.
   - A type that holds a value prints `[redacted NAME]` for `Debug`. It has
     no `Display` and no `Serialize`.
   - The only read accessor is private to the crate. Only the injector of
     T-143 calls it.
7. Redaction. One helper, `redact::scrub`, replaces each value with
   `[redacted NAME]`. It also replaces the base64, base64url, hex and
   percent-encoded forms. Every log line of box-broker passes through it.
8. Freshness. The store has a generation counter.
   - `secret set` answers only after the in-memory store holds the new
     generation, so the next request sees the new value.
   - The gate reads the store for each request (T-143, condition 8). A read
     or decrypt error fails closed.
9. The per-instance secrets use the same key and format, in
   `instances/<id>/secrets.v1`. These are the event nonce, the egress token,
   the kage token and the binding nonces. T-090 and T-107 read them for
   re-adoption. T-091 deletes them.
10. `secret list` shows names, kinds, times and the bindings that name each
    one, never a value. `secret rm` refuses a name that a running instance
    uses, unless `--force`.
11. The guest never reaches the store. The control API is a host socket or
    named pipe (T-028), and the netstack has no route to the host (T-123).

## Decision

Linux default: the key file. The Secret Service is an option,
`store.backend = "secret-service"`, used only when the operator sets it.

Reason: the daemon runs unattended under `systemd --user` with lingering. A
login keyring can be locked at that time, and a locked store fails every
request.

NOTE: a key file next to the ciphertext protects against a copy of the
ciphertext alone. It does not protect against a process of the same user.
DPAPI binds the key to the user's logon secret. Write both limits in
[limits](../docs/limits.md).

## Prove

```sh
cargo test -p box-broker store::
cargo test -p box-cli secret::
```

Expected: exit 0, with named tests for each of these:

- a value written and read back is equal;
- a record copied to another name fails to decrypt;
- one changed ciphertext byte gives an error that names the record;
- on Linux, a key file with mode 0644 stops the daemon;
- `agent-box secret set NAME VALUE` is a usage error;
- a planted value goes in through stdin and through one brokered request. No
  log line and no `Debug` output holds the value or an encoded form of it;
- an unreadable store makes condition 8 of T-143 refuse.

Host: any. The DPAPI tests run on Windows.

# T-141: Bindings and nonces

**Source:** [broker](../docs/broker.md). The kage, pi, OpenSandbox,
CubeSandbox, agent-vault and errand findings below.
**Category:** feature
**Milestone:** M6
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

A binding says which upstream gets which credential, for which requests. A
nonce tells the broker which binding the guest means. A loose binding attaches
a credential to a request nobody meant. A shared or retargeted nonce lets one
instance use the bindings of another.

## Premise

- READ: kage sends exactly one auth header for each wire kind. Anthropic sends
  `x-api-key`, OpenAI and OpenAI Responses send `authorization: Bearer`, and
  Gemini sends `x-goog-api-key`
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/anthropic.rs:103`,
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/openai.rs:141`,
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/openai_responses.rs:86`,
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/gemini.rs:92`).
  Keys never go in the URL
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/guide/providers.md:376-377`).
- READ: a search of kage-provider found no check of the key shape.
- READ: Docker Sandboxes gives the guest a sentinel shaped like the real
  token, so the client's own format checks accept it
  (`earendil-works/pi@f9bcd351dc3cedf989bc5fc0f8aa012db5737df2:packages/coding-agent/docs/containerization.md:127-136`).
- READ: with two credentials on one host, its proxy adds `x-api-key` next to
  the Bearer token, and Anthropic refuses the request
  (`earendil-works/pi@f9bcd351dc3cedf989bc5fc0f8aa012db5737df2:packages/coding-agent/docs/containerization.md:124`).
- READ: OpenSandbox injects only when exactly one binding matches, and refuses
  an ambiguous match
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/guides/credential-vault.md:55-57`).
- READ: OpenSandbox widens a binding without methods or paths to everything.
  The guard is opt-in
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/guides/credential-vault.md:305-310`).
  The request to make it the default is open
  (https://github.com/opensandbox-group/OpenSandbox/issues/1776).
- READ: CubeSandbox does not check the type of each method element at
  admission. `method=[{}]` fails at request time
  (`TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:CubeEgress/lua/policy.lua:115-119`).
- READ: agent-vault never retargets a scope. A mismatched hint is an error,
  not a redirect
  (`Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/brokercore/session.go:90-107`).
- READ: errand mints one nonce for each provider and daemon run, shared by
  every session, so no request traces to one session
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/serve.ts:94-106`).

## Approach

1. Declarations in box-policy: crates/box-policy/src/binding.rs (data,
   validation, TOML, no secret). Runtime in crates/box-broker/src/binding.rs.
2. The TOML shape:

```toml
[bindings.anthropic]
upstream = "https://api.anthropic.com"
kind = "anthropic"
credential = "anthropic-key"
routes = [
  { path = "/v1/messages", match = "exact", methods = ["POST"] },
]

[bindings.github]
upstream = "https://github.com"
kind = "git-basic"
credential = "github-token"
basic_user = "x-access-token"
repos = [{ name = "owner/repo", push = true }]
```

3. Validate at load. Each error names the field.
   - The binding name is a DNS label: 1 to 63 of `a-z`, `0-9` and `-`, with
     no `-` at either end. Names are unique.
   - `upstream` has the scheme `https`, a DNS name and an optional port
     (default 443). Refuse an IP literal, userinfo, a path and a query.
   - `kind` is a row of the table below. `credential` names a store entry.
   - `routes` has at least one route. A path starts with `/` and holds no
     `..`, `%2f`, `%5c`, `\`, `?` or `#`.
   - `match` is `exact` or `prefix`. A `prefix` route of `/` alone is
     host-wide and is refused.
   - `methods` is a list from GET, HEAD, POST, PUT, PATCH and DELETE, not
     empty. Refuse `*`. Check the type of each element.
   - The `git-basic` kind needs `repos` and refuses `routes` (T-146 derives
     them).
4. The kinds:

| Kind | The guest sends | The broker removes | The broker sets | Nonce prefix |
| --- | --- | --- | --- | --- |
| `anthropic` | `x-api-key` | `x-api-key`, `authorization`, `proxy-authorization` | `x-api-key` | `sk-ant-api03-` |
| `openai` | `authorization: Bearer` | `authorization`, `x-api-key`, `proxy-authorization` | `authorization: Bearer` | `sk-proj-` |
| `gemini` | `x-goog-api-key` | `x-goog-api-key`, `authorization`, `proxy-authorization` | `x-goog-api-key` | `AIza` |
| `git-basic` | `authorization: Bearer` | `authorization`, `proxy-authorization` | `authorization: Basic` with `basic_user` | `ghs_` |
| `substitution` | placeholders (T-145) | per surface | per surface | per placeholder |

5. The `openai` kind serves every OpenAI-compatible catalog provider of kage.
   T-160 maps kage provider ids to bindings.
6. A nonce is 32 bytes from `getrandom`. Its text is the kind's prefix and
   then base64url without padding (43 characters).
7. One nonce for each instance and binding. Mint it at each cold start
   (T-088, T-105). Keep it encrypted for re-adoption (T-140). Remove it at
   stop and delete.
8. Send nonces only in the provisioning message on port 5000 (T-041). Never
   put one in the image, a disk, the kernel command line, MMDS or an argv.
9. Lookup. The instance comes from the channel (T-121). The binding comes from
   the endpoint name in `Host` (T-142), among that instance's bindings only.
   The broker never looks a binding up by nonce value.
10. Compare the lengths first (a length is public), then the bytes with
    `subtle::ConstantTimeEq`.
11. Refuse with 403 `nonce`: a missing nonce, a nonce of another instance,
    and a nonce of another binding of the same instance.

## Prove

```sh
cargo test -p box-policy binding::
cargo test -p box-broker binding::
```

Expected: exit 0, with named tests for each of these:

- a binding without `methods` is refused, and the error names the field;
- a `prefix` route of `/` is refused as host-wide;
- `methods = [{}]` is refused at load, not at request time;
- an `http` upstream and an IP literal upstream are refused;
- two bindings with one name are refused;
- the nonce of instance A, presented on the channel of instance B, is
  refused and not retargeted;
- the nonce of binding X, presented at endpoint Y, is refused;
- each kind's nonce has its prefix, and two mints differ;
- the compare runs only on equal lengths (a test double counts the calls).

Host: any.

# T-142: Broker endpoint server on the gateway

**Source:** [broker](../docs/broker.md). The three parser traps of the earlier
CONNECT broker, and the errand, agent-vault and OpenSandbox findings below.
**Category:** feature
**Milestone:** M6
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The guest reaches the broker with HTTP/1.1 on gateway port 80. The parser
reads bytes the guest wrote. A parser mistake becomes request smuggling, or a
credential attached to a request other than the one the gate checked.

## Premise

- READ: errand reads its CONNECT head one byte at a time, and accepts CRLF
  CRLF or a bare LF LF. At the 8192-byte cap it returns the partial head
  instead of a refusal
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/broker.ts:292-326`).
- READ: errand parses the target strictly, and its hop-by-hop list includes
  `proxy-authorization`
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/broker.ts:59-73`,
  `QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/broker.ts:98-109`).
- READ: agent-vault writes every error before it hijacks the connection
  (`Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/mitm/connect.go:74-88`).
- READ: agent-vault sets the upstream `Host` to the authenticated target, with
  the default port removed, so signatures over `Host` still match
  (`Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/mitm/forward.go:147-180`).
- READ: OpenSandbox refuses a path rewrite with dot-segments, encoded
  separators or backslashes
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:components/egress/mitmscripts/system.py:585-640`).
- READ: a writer that coalesced chunks broke `git clone` and SSE in
  agent-vault. The fix flushes after every write
  (`Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/mitm/connect.go:137-145`).
- Design: in the endpoint model, `Host` selects a binding only among the
  bindings of the instance that the channel identifies.

## Approach

1. Files: crates/box-broker/src/http/head.rs (the parser),
   crates/box-broker/src/http/server.rs (the connection loop) and
   crates/box-broker/src/http/hop.rs.
2. The server takes a byte stream and an `InstanceId` from the
   `GatewayService` of T-121. It never reads identity from the request.
3. HTTP/1.1 only. Refuse HTTP/1.0, the HTTP/2 preface and any `Upgrade`
   header in v1.
4. Trap 1, the head boundary:
   - Read up to and including the first CRLF CRLF.
   - Give the bytes after it to the body reader, one time, in order.
   - At 8 KiB (`MAX_HEAD`) without the end of the head, answer 431 and
     close. Never parse a partial head.
5. Trap 2, a target the broker had to guess at, is refused with 400:
   - any form other than origin-form (`/path?query`);
   - a dot-segment, also as `%2e`, and `%2f`, `%5c` or `\` in the path;
   - a NUL, a control byte, a non-ASCII byte or `#`;
   - a missing, repeated or mismatched `Host`. `Host` must equal
     `<binding>.broker.internal`, with or without `:80`, in lower case.
6. Trap 3, hop-by-hop headers, stripped in both directions: `connection`,
   `keep-alive`, `proxy-authenticate`, `proxy-authorization`, `te`,
   `trailer`, `transfer-encoding`, `upgrade` and each header that
   `connection` names. Replace `host` with the upstream host.
7. Line rules, each a 400:
   - a bare LF or a bare CR in the head (CRLF only);
   - a folded header line (starts with a space or a tab);
   - whitespace before the colon, or a header name that is not a token;
   - more than 100 header lines (a design value).
8. Framing rules against request smuggling, each a 400:
   - `content-length` and `transfer-encoding` together;
   - two `content-length` headers;
   - a `transfer-encoding` other than exactly `chunked`;
   - a chunk extension, or a chunk size of more than 16 hex digits.
9. `expect: 100-continue`: send `100 Continue` after the gate passes and the
   upstream connection is open (T-143). Do not send `expect` upstream.
10. Keep guest connections alive between requests, with the timeouts of
    T-144. Send a body of unknown length as chunked. Flush after each write.
11. Write every refusal before any upstream response byte. Add
    `connection: close`, a short text body and `x-agent-box-refusal: CODE`.
    The codes: `head-too-large`, `bad-target`, `bad-host`,
    `unknown-binding`, `smuggling`, `upgrade`, `method`, `path`, `nonce`,
    `store` and `upstream`.

## Prove

```sh
cargo test -p box-broker http::
```

Expected: exit 0, with named tests for each of these:

- a head split over three reads is parsed whole;
- with the head and the body in one read, each body byte reaches the body
  reader one time. The test asserts on the bytes, not on the parsed head;
- an 8193-byte head gets 431 and is never parsed. Plant errand's defect
  (return the partial head), and this test fails;
- an absolute-form target, `/v1/%2e%2e/x` and a repeated `Host` get 400;
- `content-length` with `transfer-encoding` gets 400;
- a folded header line gets 400;
- `proxy-authorization` and a header that `connection` names never reach
  the recording upstream;
- an SSE response reaches the guest one event at a time.

Host: any.

## Start condition

T-121 and T-141.

# T-143: The injection gate and the fail-closed shapes

**Source:** [broker](../docs/broker.md). The CubeSandbox, OpenSandbox and
errand findings below.
**Category:** feature
**Milestone:** M6
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The gate is where the broker attaches a credential. Each condition it does not
check is a way to send the operator's credential where the operator did not
agree. A gate that forwards on failure sends the request without the
credential, and the agent reads that as an upstream fault.

## Premise

- READ: at the pin, a CubeSandbox gate failure drops the injection and still
  forwards the request. Plaintext injection is allowed
  (`TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:CubeEgress/lua/access_phase.lua:348-391`).
  The report is https://github.com/TencentCloud/CubeSandbox/issues/1410.
  The fix, https://github.com/TencentCloud/CubeSandbox/pull/1411, was
  closed without a merge.
- READ: CubeEgress strips every header that a matched rule will inject before
  it evaluates the gate
  (`TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:CubeEgress/lua/access_phase.lua:358-375`).
- READ: OpenSandbox selects a binding from the `Host` header, which is input
  from the sandbox
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:components/egress/mitmscripts/system.py:566-568`).
  The SNI fix is an open pull request at the pin
  (https://github.com/opensandbox-group/OpenSandbox/pull/1759).
- Design: in the endpoint model the binding fixes the scheme, host and port.
  The guest never names the upstream, so the `Host` against SNI class has no
  input to act on.
- READ: OpenSandbox answers a buffered request with a local 503 and ends a
  streaming one when its store fails
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/guides/credential-vault.md:85-97`).
- READ: OpenSandbox refuses a credential vault when upstream TLS verification
  is off
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:oseps/0012-credential-vault.md:186`).
- READ: errand never follows a redirect with the credential, and it forwards
  every other client header, including one that carries the nonce
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/broker.ts:240-248`).
- READ: one gate for each action. A control on one of several paths is the
  most frequent hole
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/conventions/code.md:11-22`).

## Approach

1. File: crates/box-broker/src/gate.rs. One function,
   `admit(head, instance, store) -> Result<Admitted, Refusal>`.
   - `Admitted` is the only type that can build an upstream request.
   - Its constructor always sets the injected header.
2. The eight conditions, checked in this order:

| # | Condition | Refusal |
| --- | --- | --- |
| 1 | the instance is the one whose channel carried the flow (T-121), never a value in the request | none is possible: no other source of identity exists |
| 2 | exactly one binding of that instance has the endpoint name of `Host` | 404 `unknown-binding` |
| 3 | the binding has explicit methods and paths, not host-wide, and a route matches; the longest prefix wins | 403 `method` or `path` |
| 4 | the presented nonce belongs to that instance and binding, compared in constant time (T-141) | 403 `nonce` |
| 5 | each header that carried the nonce, and each header the binding sets, is gone before injection; substitution is one pass | 400 `bad-target` for a nonce in the path or query |
| 6 | the upstream connection is TLS, and its certificate verifies for the fixed host (T-144) | 503 `upstream` |
| 7 | the upstream address passes the reserved-range check (T-124) | 503 `upstream` |
| 8 | the store answered for this request, at the current generation (T-140) | 503 `store` |

3. The order of work:
   1. parse the head (T-142);
   2. check conditions 1 to 5 and 8;
   3. connect upstream, which checks 6 and 7;
   4. inject, then forward.
   No request byte goes upstream before all eight hold.
4. Condition 5 in detail:
   - Search every header value for the nonce. Remove each header that holds
     it.
   - Remove the header set of the kind (T-141), then set the one auth
     header.
5. Redirects. The broker never follows one. It passes a 3xx response to the
   guest unchanged and logs it. A next request passes the gate again.
6. The fail-closed shapes:

| When | Answer |
| --- | --- |
| a refusal before any request byte went upstream | a local status with `connection: close` |
| a failure after the first request byte went upstream | end the guest flow with RST and close the upstream; no local response |
| a failure while the response streams to the guest | end both sides |

7. Never forward a request without the credential. No code path exists that
   does it.
8. Count for each instance: admitted requests, and refusals by condition.
   T-175 shows the counts.
9. The gate never reads the body. Body substitution is T-145.

## Prove

```sh
cargo test -p box-broker gate::
```

Expected: exit 0. A recording upstream (a local TLS server) stands behind a
test-only root store. Named tests:

- one test for each condition proves the refusal. The recording upstream
  then holds zero requests;
- a test asserts that the upstream saw the real `x-api-key` and no nonce,
  not only that the answer was 200;
- plant a defect (skip condition 4), and its test fails;
- an upstream reset after the first request byte ends the guest flow with
  RST, not with a 503;
- the production constructor of the upstream client uses webpki-roots.

Host: any.

## Start condition

T-140, T-141 and T-142.

# T-144: Upstream client

**Source:** [broker](../docs/broker.md). The agent-vault, CubeSandbox,
OpenSandbox and kage findings below.
**Category:** feature
**Milestone:** M6
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The broker opens TLS to the provider itself. This side decides whether the
credential reaches the right peer. It also decides whether a long model
stream or a large clone survives.

## Premise

- READ: agent-vault gives a request head 10 s, a read 60 s, a write 30 min
  and an idle connection 2 min. The upstream response header has 5 min, and
  the TLS handshake 10 s
  (`Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/mitm/connect.go:137-145`,
  `Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/mitm/proxy.go:83-91`).
- READ: a 60 s proxy read timeout cut real model requests in CubeSandbox. It
  went to 2 h
  (`TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:CubeEgress/nginx.conf:126-128`).
- READ: a large SSE event is cut when an HTTP/1.1 upstream closes with RST.
  HTTP/2 upstreams do not have this defect
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/components/egress-mitmproxy-sse-truncation.md:9-31`).
- READ: agent-vault offers only HTTP/1.1 and turns HTTP/2 off upstream
  (`Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/mitm/proxy.go:86`).
- READ: kage allows 600 s to send and 600 s of silence in a response body,
  with no total cap
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/http.rs:47-51`).
- READ: kage builds its TLS from ureq's default roots, the compiled-in
  webpki-roots
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:Cargo.toml:91`).

## Approach

1. File: crates/box-broker/src/upstream.rs. hyper 1 over tokio-rustls, with
   the root store from webpki-roots. ALPN offers `h2` and `http/1.1`. Use
   HTTP/2 when the upstream offers it.
2. The SNI and the verification name are the binding's fixed host, never a
   value from the guest.
3. Dial through `UpstreamDialer` (T-124).
4. One connection pool for each instance and binding. Close an idle upstream
   connection after 90 s (a design value). No pool is shared between
   instances.
5. Timeouts:

| Timeout | Value | What it bounds |
| --- | --- | --- |
| read header | 10 s | the guest's request head, from its first byte |
| request body idle | 60 s | silence while the guest sends a body (agent-vault's read value) |
| write | 30 min | one response to the guest |
| idle | 2 min | the time between two requests on one guest connection |
| upstream response header | 5 min | the end of the request to the upstream status line |
| TLS handshake | 10 s | the upstream handshake |
| connect | 10 s | T-124 |

6. Flush after every chunk, in both directions.
7. Never follow a redirect. Never send a request again after one of its bytes
   went upstream.
8. The upstream `host` or `:authority` is the fixed host, with the default
   port removed.
9. Remove hop-by-hop headers and `alt-svc` from responses.
10. rustls defaults: TLS 1.2 and 1.3, no client certificate. The code has no
    switch that turns verification off.
11. Write in [limits](../docs/limits.md): a host behind a TLS-intercepting
    proxy fails verification. v1 has no option for an extra root.

## Prove

```sh
cargo test -p box-broker upstream::
```

Expected: exit 0. A local TLS server uses a test CA through a test-only
constructor. Named tests:

- a certificate for another name is refused, with no request sent;
- an expired certificate is refused;
- HTTP/2 is used when the server offers it;
- with a test clock, an SSE stream with 9 minutes between events survives;
- no response header after 5 min gives a local 503;
- a 302 reaches the guest, and the server records one request only;
- the guest gets the first SSE event before the server sends the second.

Host: any.

# T-145: Substitution model

**Source:** [broker](../docs/broker.md). The agent-vault and OpenSandbox
substitution findings below.
**Category:** feature
**Milestone:** M6
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

Some upstreams take a credential in a path segment, a query parameter or a
body field, not in a header. Header replacement cannot serve them. A raw
string replacement everywhere makes an injection surface.

## Premise

- READ: a substitution declares a placeholder, its credential and the
  surfaces it may go into. The source calls that list the security boundary
  (`Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/broker/broker.go:62-69`).
- READ: encoding is per surface. A header value with CR or LF fails the whole
  request. On any error the caller must not forward
  (`Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/brokercore/substitution.go:36-87`).
- READ: agent-vault replaces each placeholder in turn, so a later one can
  match text that an earlier one inserted
  (`Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/brokercore/substitution.go:57`).
  OpenSandbox replaces in one pass
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/guides/credential-vault.md:226-228`).
- READ: OpenSandbox excludes `Host`, `Content-Length` and forwarding headers
  from the header surface. A body rewrite sets `Content-Length`. The
  placeholder and every encoded form of the value join the redaction set
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/guides/credential-vault.md:216-236`).
- READ: OpenSandbox streams a body above about 1 MiB before its body hook, so
  body substitution skips it
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:components/egress/mitmscripts/system.py:31-36`).
- READ: agent-vault substitutes WebSocket text frames since PR 225
  (`Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/mitm/websocket.go:296-326`).
  v1 here refuses an upgrade (T-142).

## Approach

1. File: crates/box-broker/src/subst.rs. The binding kind `substitution`
   (T-141) declares its placeholders:

```toml
[bindings.example-api]
upstream = "https://api.example.com"
kind = "substitution"
credential = "example-key"
routes = [{ path = "/v2/search", match = "exact", methods = ["GET"] }]
placeholders = [{ name = "key", surfaces = ["query"] }]
```

2. A placeholder is a nonce of T-141, one for each instance, binding and
   placeholder. box-guest receives it in provisioning, like a provider
   nonce.
3. The encodings:

| Surface | Rule |
| --- | --- |
| path | percent-encode for a path segment, on the wire-encoded path, so the value lands one time |
| query | percent-encode for a query value |
| header | raw. A value with CR or LF refuses the whole request |
| body, form | percent-encode for a query value |
| body, JSON | escape as a JSON string |
| body, multipart or compressed | refuse the request |
| body, other types | raw |

4. One pass. Find every placeholder in the original bytes first, then build
   the output. Never scan an inserted value again.
5. The header surface never touches `host`, `content-length`,
   `transfer-encoding` or a forwarding header. It runs before the auth
   injection of T-143.
6. Refuse a path rewrite that makes a dot-segment, an encoded separator or a
   backslash.
7. A body surface applies only to a body with a known length of 1 MiB or
   less. A larger or chunked body for such a binding is refused. A body
   rewrite sets `content-length`.
8. On any error the request is not forwarded. Partial changes may exist in
   memory, so drop the request.
9. Add each placeholder and each encoded form of each value to the redaction
   set of T-140.
10. Refuse at load a binding that declares a surface not in the table, or
    `websocket`.

## Prove

```sh
cargo test -p box-broker subst::
```

Expected: exit 0, with named tests for each of these:

- each surface's encoding, asserted on the bytes on the wire;
- a header value with CR is refused, and the recording upstream gets
  nothing;
- a path value is escaped one time, not two;
- a value that contains another placeholder's text is not replaced again;
- a multipart body for a body surface is refused;
- a body over 1 MiB for a body surface is refused;
- a binding with a `websocket` surface is refused at load.

Host: any.

## Start condition

T-143.

# T-146: Forge binding and the push namespace

**Source:** Operator ruling D4 (2026-10-14): project files move only through
a git forge. [broker](../docs/broker.md) gives the `insteadOf` rule and the
ref namespace.
**Category:** feature
**Milestone:** M6
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The agent clones and pushes through the broker. A forge token with write
access can push to any branch, including protected work of other people. The
broker must hold the token, limit the repositories, and keep pushes inside the
instance's own ref namespace.

## Premise

- Design: the guest's system git config rewrites `https://github.com/` to
  `http://github.broker.internal/`. The broker reads the ref-update commands
  of `git-receive-pack` and refuses refs outside the namespace.
- READ: errand puts the GitHub token into the session environment
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/github.ts:1-14`).
- READ: a coalescing writer broke `git clone` through agent-vault with a
  sideband early EOF. The fix flushes each write
  (`Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/mitm/connect.go:137-145`).
- Design: git smart HTTP uses `GET <repo>/info/refs?service=...`,
  `POST <repo>/git-upload-pack` and `POST <repo>/git-receive-pack`. The
  receive-pack request body starts with pkt-lines of
  `<old-oid> <new-oid> <refname>`, then a flush packet, then the pack.

## Approach

1. Files: crates/box-broker/src/forge.rs (routes and repositories) and
   crates/box-broker/src/forge/pktline.rs (the command parser).
2. Guest git configuration, written into the image by T-062 in
   `/etc/gitconfig`, with no secret in it:

```ini
[url "http://github.broker.internal/"]
    insteadOf = https://github.com/
    insteadOf = git@github.com:
    insteadOf = ssh://git@github.com/
```

3. The nonce reaches git through the environment of kage (T-160), never
   through a file:

```sh
GIT_CONFIG_COUNT=1
GIT_CONFIG_KEY_0=http.http://github.broker.internal/.extraHeader
GIT_CONFIG_VALUE_0="Authorization: Bearer <forge nonce>"
```

4. Routes derived from `repos`. For each `owner/repo`, with and without the
   `.git` suffix:
   - `GET /owner/repo/info/refs` with `service=git-upload-pack`, always;
   - `POST /owner/repo/git-upload-pack`, always;
   - `GET /owner/repo/info/refs` with `service=git-receive-pack`, and
     `POST /owner/repo/git-receive-pack`, only with `push = true`.
   - Compare owner and repository names in lower case. Every other path,
     including Git LFS, gets 403 `path`.
5. Injection: `authorization: Basic` of `basic_user:token`, after the gate of
   T-143.
6. Push check, before any request byte goes upstream:
   - Read pkt-lines from the body up to the flush packet, at most 64 KiB of
     decoded bytes (a design value). Accept identity or gzip encoding only.
   - A pkt-line length of 1 to 3, or of more than 65520, is refused.
   - Each command has two 40-hex object ids and a ref name. The first
     command carries capabilities after a NUL.
   - Refuse the `push-options` capability in v1.
7. Ref rule. Allowed refs start with `refs/heads/agent-box/<instance>/`, where
   `<instance>` is the instance name (T-020). The rest follows the
   `git check-ref-format` rules. Create, update and delete are allowed
   inside the namespace.
8. One command outside the namespace refuses the whole push with 403 and
   `x-agent-box-refusal: ref`. Name the ref in the body. Nothing goes
   upstream.
9. After the check, send the buffered bytes and then stream the rest. Flush
   each write.
10. Refuse an `Upgrade`, and do not follow a redirect (T-143).

## Decision

The forge nonce reaches git through `GIT_CONFIG_*` variables in kage's
environment, not through a credential helper. A credential helper lets a user
config add `credential.helper = store`, which writes the nonce to disk.

NOTE: an instance name can be used again after a delete. A new instance with
an old name can push to the branches the old instance left.

## Prove

```sh
cargo test -p box-broker forge::
cargo test -p box-broker --test forge_git -- --ignored
```

Expected: the unit tests refuse `refs/heads/main`, `refs/tags/v1` and a ref
of another instance. They accept `refs/heads/agent-box/NAME/fix`, and refuse
a truncated pkt-line. The second test serves a bare repository with
`git http-backend` behind a local TLS server and a test CA. A real `git push`
through the broker to the allowed ref succeeds. A push to `main` fails with
the ref named, and the bare repository is unchanged. Host: any host with git.

## Start condition

T-143 and T-144.

# T-147: Usage accounting

**Source:** Rule R-TRUSTED-CHANNEL of the planning session (2026-10-14). The
usage fields that kage reads, below.
**Category:** feature
**Milestone:** M6
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

Billing, rate-limit backoff and slot accounting need counts the guest cannot
forge. kage's own events are written by a process that the agent's tools can
reach. The broker sees every provider response, so it is the one trusted
source.

## Premise

- READ: in the guest, every process at kage's uid can write to kage's stdout
  through `/proc/<pid>/fd/1`, as in errand issue 2
  (https://github.com/QaidVoid/errand/issues/2). The kage stream is
  agent-reported data.
- READ: the usage fields as kage parses them:
  - Anthropic: `message_start` carries `message.usage`, and `message_delta`
    carries `usage`
    (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/anthropic.rs:515-531`);
  - the cache fields `cache_creation_input_tokens` and
    `cache_read_input_tokens`
    (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/anthropic.rs:568-569`);
  - OpenAI chat: kage sets `stream_options.include_usage`, and reads
    `prompt_tokens` and `completion_tokens`
    (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/openai.rs:273`,
    `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/openai.rs:722-731`);
  - OpenAI Responses: `response.completed` carries `response.usage`
    (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/openai_responses.rs:413`);
  - Gemini: each chunk carries `usageMetadata`
    (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/gemini.rs:554`).

## Approach

1. File: crates/box-broker/src/account.rs.
2. Count for each instance and binding: requests, responses by status class,
   bytes in each direction, and the tokens below.
3. Read a copy of the response bytes. Forward first and parse the copy, so
   accounting never delays or changes the stream.
4. Parse SSE incrementally, line by line. Cap one event at 1 MiB (a design
   value). A parse failure adds one to `unparsed` and never stops forwarding.
5. The fields for each kind:

| Kind | Where | Fields |
| --- | --- | --- |
| `anthropic` | `message_start`, then the last `message_delta` | `input_tokens`, `output_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens` |
| `openai`, chat | the chunk that has `usage` | `prompt_tokens`, `completion_tokens`, `prompt_tokens_details.cached_tokens` |
| `openai`, responses | `response.completed` | `input_tokens`, `output_tokens`, `input_tokens_details.cached_tokens` |
| `gemini` | the last `usageMetadata` | `promptTokenCount`, `candidatesTokenCount`, `thoughtsTokenCount` |

6. A non-streaming JSON response has the same fields at the top level.
7. Decision: for provider kinds, send `accept-encoding: identity` upstream, so
   the broker can read the response. The cost is more bytes on the wire.
8. Record 429 and 529 responses, with `retry-after` and the provider's
   rate-limit headers, for the backoff of T-252.
9. Write each request's counts to the event log of T-176, labelled
   host-observed. Keep running totals in the instance record.
10. Nothing in the daemon takes usage, billing or backoff input from the ACP
    stream (T-161).

## Prove

```sh
cargo test -p box-broker account::
```

Expected: exit 0. Recorded responses for each kind, streaming and not, give
the expected counts. A response split at every byte gives the same counts. A
malformed event adds one to `unparsed`, and the guest still gets every byte.
Host: any.

## Start condition

T-143.

# T-148: Short-lived forge credentials

**Source:** [broker](../docs/broker.md). The open agent-vault request below.
No reference implements this.
**Category:** feature
**Milestone:** backlog
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

A long-lived forge token in the store is valid for every instance and for its
whole life. A token minted for one instance, for its repositories only, ends
with the instance. It also expires if a revocation fails.

## Premise

- READ: agent-vault has an open request for a GitHub App credential type. It
  signs a JWT, exchanges it for an installation token, injects it and
  refreshes it. The agent never reads either
  (https://github.com/Infisical/agent-vault/issues/255).
- Not read in this session: GitHub's documentation of app JWTs and
  installation tokens. Read it before the work, and cite it with the date
  read.

## Approach

1. File: crates/box-broker/src/forge/app.rs. A new binding field,
   `github_app = { app_id, installation_id, key = "<store name>" }`, in place
   of `credential`.
2. The app's private key lives in the store (T-140). It never leaves the
   daemon.
3. For each instance, mint an installation token limited to the repositories
   of the binding and to `contents: write` (or `read` without `push`).
4. Cache the token for each instance. Refresh it before it expires, with a
   margin. The margin is set after the documentation is read.
5. Revoke the token at stop and at delete. A failed revocation is logged, and
   the token's expiry still ends it.
6. The guest sees no change: it still sends its forge nonce (T-146).

## Prove

```sh
cargo test -p box-broker forge::app::
```

Expected: exit 0, against a fake GitHub API on a local TLS server. A token is
minted for an instance with its repositories only. After the instance is
deleted, the fake API has a revocation, and a request with that token is
refused. Host: any. A live check against a test GitHub App is the operator's
step.

# T-149: Optional TLS interception for tools that cannot take a base URL

**Source:** [broker](../docs/broker.md), the later option. The certificate
authority rules of the earlier plan, and the CubeSandbox, agent-vault and
OpenSandbox findings below.
**Category:** feature
**Milestone:** backlog
**Priority:** P3
**Effort:** L
**Status:** open

## Problem

Some tools cannot point at a base URL. Only interception of their TLS can
give them a credential. A certificate authority that signs before it
authorizes is a signing oracle. A planted CA is also a new thing to protect.

## Premise

- READ: kage trusts only its compiled-in webpki roots. A CA in the guest trust
  store or in `SSL_CERT_FILE` does not reach it
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/http.rs:43-52`).
  So kage needs a CA option first (T-169).
- READ: at the CubeSandbox pin, the certificate is minted before any
  authorization. The cache key is the raw SNI, and the serial is not from a
  CSPRNG
  (`TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:CubeEgress/lua/cert_signer.lua:97-158`).
  The fixes, https://github.com/TencentCloud/CubeSandbox/pull/1427 and
  https://github.com/TencentCloud/CubeSandbox/pull/1455, closed without a
  merge.
- READ: agent-vault authorizes before any TLS work, validates the SNI before
  it signs, caches validated names only, and takes serials from a CSPRNG
  (`Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/ca/soft.go:395-503`).
- READ: OpenSandbox passes a TLS flow without SNI through untouched. Here
  such a flow is refused
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:components/egress/mitmscripts/system.py:44-50`).
- Reported by the relevance pass: installing the CA took about 0.6 s of
  1.6 s to the first TLS call in an OpenSandbox Firecracker guest. Not
  measured here.

## Approach

1. Off by default. A binding of kind `intercept` turns it on for its names
   only.
2. Files: crates/box-broker/src/ca.rs and crates/box-broker/src/intercept.rs.
   box-net sends TCP flows to port 443 of an intercepted name to the broker.
3. The order is the entry:
   1. the flow belongs to an instance with an `intercept` binding (from the
      channel), before any TLS work;
   2. read the ClientHello. Refuse a flow without SNI;
   3. validate the SNI against the binding's names;
   4. only then, mint or fetch a leaf for that validated name.
4. A budget for each instance of distinct names signed, and a rate limit on
   signing. Key the cache by the validated name only.
5. A fresh key for each leaf, and a serial from the CSPRNG.
6. The root key lives in the daemon, encrypted at rest with the store key
   (T-140). No path from any instance reaches it.
7. After the handshake, `Host` must equal the SNI, and the gate of T-143
   applies unchanged. Plaintext injection does not exist.
8. box-guest installs the root certificate at boot into the guest trust
   store. The certificate is specific to the host, so it cannot be in the
   shared image.

## Prove

```sh
cargo test -p box-broker ca::
```

Expected: exit 0, with named tests for each of these:

- a flow without an `intercept` binding causes zero signing operations,
  counted, not read from a log;
- a flow without SNI is refused;
- an instance over its name budget is refused;
- two spellings of one name hit one cache entry;
- two leaves for one name have different serials;
- with the feature off, no flow reaches the CA.

Host: any.

## Start condition

T-143, and an answer to the CA request of T-169 for kage.

# T-150: Narrow the bindings per task phase

**Source:** The agent-sandbox issue and pull request below. The
execution-scoped credentials entry of the earlier plan maps here.
**Category:** research
**Milestone:** backlog
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

A persistent instance runs one unattended task after another. Its bindings
live as long as the instance. A task that needed repository read can still
reach repository write in a later task. A prompt injection in between gets
the union of every grant.

## Premise

- READ: the issue names an authorization that outlives the execution that
  needed it. In a reused sandbox the effective grant is the union of the
  grants (https://github.com/kubernetes-sigs/agent-sandbox/issues/1580).
- READ: an open pull request holds three properties together. The key never
  enters the sandbox. The token reaches one process. The token is revoked at
  process exit (https://github.com/kubernetes-sigs/agent-sandbox/pull/1644).
- READ: flue's RFC proposes per-operation count limits and a separate token
  for each class of operation (https://github.com/withastro/flue/issues/1).
- READ: kage reads provider key variables one time, when its host starts
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/rpc/host.rs:86-106`).
  A new nonce for each task needs a kage restart, or `_kage/auth/set`, which
  writes `auth.json` to the guest disk.
- Design: brokering stops credential theft. It does not stop misuse through
  an allowed channel.

## Approach

1. Write a design page section in [broker](../docs/broker.md). Compare three
   routes, each with its cost:

| Route | What it gives | What it costs |
| --- | --- | --- |
| a binding set for each task phase | the strongest step without upstream help | somebody declares the phases; a wrong declaration blocks work |
| an operator approval before a widening operation | the operator sees each widening | the operator is in the loop |
| short-lived credentials for each task (T-148) | expiry without a revocation | the upstream must mint tokens; most API keys cannot |

2. Design the mechanism that all routes need: a binding set that a task start
   replaces. The generation of T-140 applies it to the next request, not to
   the one in flight.
3. Design a nonce for each task. box-guest gets it for the task, and the
   broker revokes it at task end. Say how kage gets it, given the premise.
4. Write a test plan that proves the limit. A task with an allowed write
   binding sends a private payload to an allowed host, and the broker permits
   it. The plan says this project does not claim otherwise.
5. Write the operator's decision in [decisions](../docs/decisions.md). Write
   new entries for the chosen route.

## Prove

```sh
cargo xtask check docs
cargo todo check
```

Expected: both exit 0. [broker](../docs/broker.md) has the design section, and
[decisions](../docs/decisions.md) has a dated decision. New entries exist for
the chosen route. Host: any.

# T-257: SSH from the guest through a host-held key

**Source:** The main-session ruling of 2026-10-14 on ssh in the guest; operator ruling D5 (ssh works in the guest); rule R-NO-SECRET-IN-GUEST (T-256).
**Category:** feature
**Milestone:** M6
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

Ruling D5 says that ssh works in the guest. An ssh login to a remote host
needs a private key, and R-NO-SECRET-IN-GUEST keeps every key out of the
guest. Until this entry lands, ssh from the guest works only to hosts that
need no credential.

## Premise

- Design (ruling of 2026-10-14): box-guest offers an ssh agent socket in the
  guest. It carries each request over vsock port 5005, guest to host, to an
  agent in the daemon.
- Design (same ruling): the daemon signs only with keys bound to that
  instance, and only for destinations that the policy allows. The key never
  enters the guest.
- Design: the netstack knows each flow of an instance, with its address, its
  port and the name that DNS answered (T-121, T-122, T-123).
- Design: the forge binding rewrites git over ssh to GitHub into broker HTTP
  (T-146). A push to GitHub does not need this entry.
- Design (docs/protocol.md): a v1 port set change needs a new version. Port
  5005 is in the v1 set (T-022), so this entry adds no version. Its first
  frame carries the egress token, so `PROVISION` gets no new field.
- Not read in this session: the ssh agent protocol draft
  (https://datatracker.ietf.org/doc/draft-miller-ssh-agent/). Read it before
  the code, and cite it with the date read.
- Not read in this session: the destination restrictions of OpenSSH and its
  `session-bind@openssh.com` extension
  (https://www.openssh.com/agent-restrict.html). They can tie a signature to
  one server host key.

## Approach

1. Files: crates/box-guest/src/ssh_agent.rs (the guest socket),
   crates/box-broker/src/ssh_agent.rs (the agent) and the port 5005 messages
   in box-proto (T-022).
2. box-guest listens on /run/agent-box/ssh-agent.sock, on a tmpfs, mode 0600,
   owner `agent`. It sets `SSH_AUTH_SOCK` for kage (T-046) and for PTY
   sessions (T-042).
3. Each client connection on that socket gets its own vsock connection to
   host port 5005.
4. Use the frames of docs/protocol.md: `AGENT_AUTH` with the egress token
   first, then `AGENT_READY`, then the bytes of the agent protocol.
5. The daemon checks the token in constant time, as on port 5003. A refused
   frame increments the refused-forgery counter.
6. The daemon bounds each agent message. Set the bound and the connection
   limit in this entry, and write them into docs/protocol.md.
7. The daemon binds the port 5005 listener with its other listeners
   (docs/protocol.md). It sends `AGENT_READY` only after the policy and the
   bindings are live (T-126).
8. Configuration: a table `[ssh_keys.NAME]` with `credential` (a private key
   in the store of T-140), `hosts` (`name:port` pairs) and `host_keys` (the
   pinned server keys).
9. A profile names the ssh keys of its instances. Only an ed25519 key is
   accepted in v1, a design value.
10. The agent answers a request for identities with the public keys bound to
    the instance only.
11. The agent signs only when all of these hold. A failed condition gives the
    agent failure answer and a host-observed event:
    - the key is bound to the instance of the channel;
    - the instance has an open TCP flow to an address that DNS answered for a
      name in `hosts`, on the port of that pair (T-123);
    - the data to sign parses as an ssh public-key user authentication
      request, and nothing else.
12. With `session-bind@openssh.com`, the agent also requires the bound server
    key to equal a key in `host_keys`. Decide in this entry whether v1
    requires the extension.
13. The agent refuses every other request: add, remove, lock, unlock and any
    other extension.
14. The private key stays in the daemon. No frame to the guest carries it, and
    no log line shows it (T-140).
15. Count the signatures and the refusals for each instance. T-175 shows the
    counts in the startup report.
16. Write in [network](../docs/network.md) and [limits](../docs/limits.md):
    without this entry, ssh works only to hosts that need no credential.
17. Write in [limits](../docs/limits.md): a signature lets the guest use the
    remote account for that session. The agent does not see what the session
    does.

## Prove

```sh
cargo test -p box-broker ssh_agent::
cargo test -p box-guest ssh_agent::
cargo xtask linux --kvm -- cargo test -p box-daemon --test ssh_live -- --ignored
```

Expected: the unit tests refuse a sign request for a key of another
instance, and a sign request with no allowed flow. They refuse data that is
not a user authentication request, and an add-identity request. The
identities answer lists only the bound keys. A scan of every frame to the
guest finds no byte of the private key.

The live test starts a local sshd with a test key on an allowed rule. `ssh -o
BatchMode=yes` from the guest logs in through the agent. The same command to
a host outside `hosts` fails, and the agent records the refusal. Host: any host
for the unit tests; a Linux KVM host for the live test.

## Start condition

T-121, T-123, T-140 and T-141.
