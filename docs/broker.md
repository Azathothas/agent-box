# Broker

The broker is part of the daemon. It holds the credentials and injects them
into requests that the guest sends to explicit broker endpoints. The guest
holds only nonces, one per instance per binding. The network path to the
broker is in [network.md](network.md). What brokering does not do is in
[limits.md](limits.md). [architecture.md](architecture.md) is the technical
reference, and where the two differ, it wins.

NOTE: The broker is planned, not built. Ruling D14 in
[decisions.md](decisions.md) keeps this session to tooling and documents. The
crate box-broker comes after T-033, and [TODO/broker.md](../TODO/broker.md)
lists the broker entries.

Labels: READ and MEASURED are defined in [conventions.md](conventions.md).
Unlabelled text is design. "Proposal" marks a design default that its entry
can still change.

## What the guest holds

| credential | in the guest | in the daemon |
| --- | --- | --- |
| a model provider API key | a nonce in the provider's key format | the real key, in the credential store |
| a forge token | a nonce | the real token |
| any other credential | a nonce if a binding exists, else nothing | the real value |

- A nonce is worth only what the broker does with it, on the channel of the
  instance that holds it. On the channel of another instance it is refused,
  never resolved.
- A nonce is still a secret. The broker compares it in constant time and never
  logs it.
- A nonce in kage's environment is readable by every process at the agent's
  uid, through `/proc/<pid>/environ`. That is accepted, because the nonce
  works only through the broker endpoints of this instance.
- Docker Sandboxes has the same shape. The sandbox gets a sentinel, and a
  proxy swaps in the real credential on egress (READ:
  `earendil-works/pi@f9bcd351dc3cedf989bc5fc0f8aa012db5737df2:packages/coding-agent/docs/containerization.md:114-121`).
- errand shares one nonce per provider across all its sessions for a whole
  daemon run, so no request maps to one session (READ:
  `QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/serve.ts:94-106`).

## Why explicit endpoints (ruling P4)

Credentialed traffic goes to plain-HTTP broker endpoints on the gateway. The
broker does not intercept TLS for it. There are two reasons.

1. kage trusts only its compiled-in webpki roots. Every kage HTTP client uses
   the default TLS configuration of ureq, whose root store is
   RootCerts::WebPki. No kage crate sets a root store.
2. With an explicit endpoint, the guest never names the upstream. The binding
   fixes the scheme, the host and the port.

Evidence for reason 1 (READ):
`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/http.rs:42-52`
and `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:Cargo.lock:4023-4040`.
A CA planted in the guest trust store is ignored, so interception of kage's
calls fails.

Consequences:

- The guest image has no broker CA. The broker mints no leaf certificate and
  reads no SNI in v1.
- Traffic from the guest to the broker is plain HTTP, but it crosses no
  network. Its path is the guest kernel, tun0, box-guest, vsock and the
  daemon.
- A tool that cannot take a base URL is not brokered in v1. It reaches its
  upstream under the flow policy, with no credential. T-149 is the later
  option.
- errand's provider path is the nearest reference of this model. It is a
  plain-HTTP endpoint with a nonce check in constant time, longest-prefix
  routes and manual redirects (READ:
  `QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/broker.ts:160-199`).

## Endpoints (T-142)

- The guest reaches a binding at `http://<binding>.broker.internal/<path>`.
- The gateway DNS answers `<binding>.broker.internal` with the gateway
  address, in every policy mode. It answers only names of bindings of this
  instance.
- Any other name under broker.internal, and broker.internal itself, gets
  NXDOMAIN. A wildcard never matches its apex.
- TCP port 80 on the gateway is the broker. Other gateway ports are refused
  ([network.md](network.md)).
- The Host header selects the binding among the bindings of this instance. It
  must be exactly `<binding>.broker.internal`, with or without `:80`.
- The Host header selects nothing else. The upstream host comes from the
  binding, never from the request.

The upstream side:

- The broker opens TLS to the upstream itself, with rustls and the webpki
  roots. It verifies the certificate for the binding's fixed host.
- It offers HTTP/2 by ALPN and falls back to HTTP/1.1. The guest side is
  always HTTP/1.1.
- HTTP/2 to the upstream avoids a truncation that OpenSandbox measured. A
  large SSE event is cut when an HTTP/1.1 upstream closes with TCP RST.
- OpenSandbox found HTTP/2 upstreams unaffected (READ:
  `opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/components/egress-mitmproxy-sse-truncation.md:9-31`).
- The upstream Host header, or `:authority`, is the binding's host without a
  default port. agent-vault removes the default port so that signatures over
  Host still match.
- Evidence (READ):
  `Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/mitm/forward.go:127-129`,
  [PR 195](https://github.com/Infisical/agent-vault/pull/195), merged.
- Responses stream. The broker flushes after every chunk. A coalescing writer
  broke git clone and SSE in agent-vault until
  [PR 229](https://github.com/Infisical/agent-vault/pull/229), merged.
- Redirects are never followed with a credential. The broker passes a 3xx
  response to the guest without change.
- Credential components stay in one host process or on host-local sockets,
  never on a network that the operator does not control.
- oh-my-pi leaves that transport to the operator, which is the anti-pattern
  (READ:
  `can1357/oh-my-pi@6f2c14b3e4cc065139789da893e4f86f3d72958c:docs/auth-broker-gateway.md:8`).

## kage in the guest (T-160, T-063)

[guest.md](guest.md), section "Configuration pins", gives the whole kage
configuration. box-guest generates it at each boot from provisioning data, on
a tmpfs. This section keeps only what the broker relies on.

- Each provider `base_url` points at a broker endpoint, for example
  `http://anthropic.broker.internal`. The openai kind ends the base in `/v1`.
- kage appends the protocol path to `base_url`. Evidence (READ):
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/guide/providers.md:364-377`.
- kage reads provider settings only from the user file, never from a project
  file (READ:
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/guide/config.md:192-196`).
  `KAGE_*` variables override both files (READ:
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/guide/config.md:164-176`).
  An untrusted repository cannot move a base URL.
- The key variable of each bound provider holds the nonce of its binding, in
  the environment of `kage serve`. A provider with no binding gets no key
  variable, so kage does not register it.
- kage sends exactly one auth header per wire kind: `x-api-key` for
  anthropic, `Authorization: Bearer` for openai, and `x-goog-api-key` for
  gemini. Keys never go in the URL.
- Evidence (READ):
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/anthropic.rs:103`,
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/openai.rs:141`,
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/gemini.rs:92`.
- The kage configuration is not a control. The broker and the flow policy are
  the controls. A wrong `base_url` sends a nonce that is worth nothing
  elsewhere.

The forge nonce reaches git through the environment of `kage serve`, never
through a file (T-146):

```text
GIT_CONFIG_COUNT=1
GIT_CONFIG_KEY_0=http.http://github.broker.internal/.extraHeader
GIT_CONFIG_VALUE_0=Authorization: Bearer <github nonce>
```

- `KAGE_SHELL__SCRUB_ENV` keeps the provider nonces out of the environment of
  the shell tool. It is damage control, not a boundary: a process at the
  agent's uid can read kage's environment.
- The scrub pattern does not match the `GIT_CONFIG_*` variables, because the
  agent's git commands need them.

## git in the guest (T-146)

The system git configuration is part of the image and holds no secret (T-062,
T-146):

```ini
# /etc/gitconfig, in the image
[url "http://github.broker.internal/"]
    insteadOf = https://github.com/
    insteadOf = git@github.com:
    insteadOf = ssh://git@github.com/
```

- The brief gives the https line. The two SSH forms send SSH-form remotes
  through the broker too, so a git push needs no SSH key in the guest.
- The forge nonce travels in an extra header that git adds to each request
  for the broker URL. T-146 records this decision: a credential helper could
  write the nonce to disk through a user config.
- The header value comes from the `GIT_CONFIG_*` environment above. So the
  nonce is never written to the disk by this path.
- git sends the header on fetch and push. The broker removes it and sets the
  forge credential (gate condition 5).

Open question for T-146: the https rewrite sends every GitHub URL to the
broker. The forge binding allows only the repositories of the instance. So a
fetch of another public GitHub repository is refused, for example a git
dependency. T-146 chooses between two answers:

- a rewrite per allowed repository, so other GitHub URLs go out under the
  flow policy without a credential;
- an anonymous read route on the broker that never carries a credential.

## Bindings (T-141)

A binding is data in the config file, in the crate box-policy. It holds no
secret. A profile names its bindings ([cli.md](cli.md)).

| field | meaning | rule |
| --- | --- | --- |
| name (the table key) | the endpoint label, as in `<name>.broker.internal` | a DNS label, unique per instance |
| upstream | scheme, host and optional port, fixed | https only; a DNS name, no IP literal, no wildcard host |
| kind | the wire kind: `anthropic`, `openai`, `gemini`, `git-basic` or `substitution` | the kind fixes the headers that the broker removes and sets, and the usage parser |
| credential | the name of a credential in the store | must exist when the profile loads |
| routes | the allowed requests: a path, `exact` or `prefix`, and a method set | explicit and not empty; a `prefix` route of `/` alone is refused |
| basic_user | the user name of HTTP Basic | `git-basic` only |
| repos | the allowed repositories, each with a push flag | `git-basic` only; it derives the routes (T-146) |
| placeholders | placeholders with their surfaces | `substitution` only (T-145) |

The push namespace is fixed, not a field: `refs/heads/agent-box/<instance>/`
(T-146).

The syntax below is an illustration. T-027 and T-141 fix the real syntax.

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
repos = [{ name = "example-org/example-repo", push = true }]
```

- Read kage's provider crates for the exact paths of each kind. Then fix each
  route (T-141).
- Inside a binding, the longest path prefix wins. So a route nested under
  another route stays reachable.
- errand sorts its routes by prefix length for this reason (READ, the
  broker.ts lines above).
- The load refuses two bindings with one name, an empty method set, a route
  for the whole host, an unknown credential and a non-https upstream.

## Nonces (T-141)

- There is one nonce per instance per binding.
- A nonce is 32 bytes from the host CSPRNG, never a timestamp or a counter.
- Source for that rule (READ):
  `Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/conventions/code.md:116-117`.
- Format: the provider's documented key prefix, then the 32 bytes in the
  alphabet of that key format. T-141 keeps the table of prefixes.
- A nonce shaped like the real key passes the format checks of other clients.
  Docker Sandboxes uses such a sentinel for this reason (READ:
  `earendil-works/pi@f9bcd351dc3cedf989bc5fc0f8aa012db5737df2:packages/coding-agent/docs/containerization.md:127-136`).
- A search of kage's provider crates found no key format check (READ). The
  format copy is for other clients.
- Compare: the broker takes the nonce of this instance for the selected
  binding. It checks the length, then compares the bytes in constant time.
- The broker never looks a nonce up across instances. A nonce of instance A on
  the channel of instance B is a refusal.
- agent-vault never retargets a scoped session either (READ:
  `Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/brokercore/session.go:72-150`).
- Provisioning: the daemon mints the nonces at start and sends them on port
  5000 before kage starts. box-guest holds them in memory.
- A nonce never goes into the image, the disk, the kernel command line, MMDS
  or any argv. This is the rule R-NO-SECRET-IN-GUEST (T-256).
- errand's podman backend put the real provider key on argv, in the host
  process table (READ:
  `QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/podman.ts:124-131`).
- Proposal: new nonces at every start. The old ones stop working at once.
- A nonce that the agent writes to disk is still worth nothing outside the
  channel of this instance. The next start retires it.
- A nonce per task, revoked at the end of the task, is research (T-150).

## The injection gate (T-143)

The broker attaches a credential only when all eight conditions hold. If one
fails, the broker refuses the request. It never forwards a request without the
credential.

| # | condition | why | entry | upstream exhibit (READ) |
| --- | --- | --- | --- | --- |
| 1 | The caller is the instance whose vsock channel carried the flow. | The guest is root in its kernel and can forge any address or header. | T-143 | OpenSandbox selects the vault by guest source IP, and its smoke test forges a sibling's IP: `opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:components/egress/tests/smoke-fast-sandbox.sh:588-601`. |
| 2 | Exactly one binding matches the endpoint. | A match resolved by order picks a credential that nobody chose. | T-141 | OpenSandbox rejects an ambiguous match: `opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/guides/credential-vault.md:55-57`. |
| 3 | The binding names an explicit method set and path scope, and neither covers the whole host. | An omitted scope becomes everything. | T-141 | OpenSandbox's guard is opt-in, and default-on is still open: `opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/guides/credential-vault.md:305-310`. |
| 4 | The nonce is valid for that instance and that binding, and the compare runs in constant time. | An equality operator leaks timing. A shared nonce cannot be traced or revoked. | T-141 | errand shares one nonce across all sessions: `QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/serve.ts:189-214`. |
| 5 | Every header that carried a nonce, and every header the binding sets, is removed before injection. Substitution is one pass. | A forged header must not reach the upstream. A second auth header makes Anthropic refuse the request. | T-143, T-145 | CubeEgress strips injected headers before its gate: `TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:CubeEgress/lua/access_phase.lua:358-375`. |
| 6 | The upstream connection is TLS, and its certificate verifies for the binding's fixed host. | A credential sent to an unverified peer is a credential given away. | T-144 | OpenSandbox refuses a vault when upstream verification is off: `opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:oseps/0012-credential-vault.md:186`. |
| 7 | The resolved upstream address passes the reserved-range check. | A name that resolves to loopback or a private address reaches host-local services. | T-124, T-144 | errand dials an allowed name and never checks the address: `QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/broker.ts:240-248`. |
| 8 | The credential store answered freshly. | A stale or broken store state must not decide an injection. | T-140, T-143 | OpenSandbox fails every intercepted request closed when its vault fails: `opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/guides/credential-vault.md:85-97`. |

More exhibits for the table:

- Condition 2: in agent-vault, more than one grant and no hint is an error,
  never a pick (READ, the session.go lines above).
- Condition 3: OpenSandbox merged its guard as opt-in in
  [PR 1762](https://github.com/opensandbox-group/OpenSandbox/pull/1762).
  [Issue 1776](https://github.com/opensandbox-group/OpenSandbox/issues/1776),
  which makes it the default, is open.
- Condition 5: pi documents that Anthropic rejects a request with a second
  auth header (READ:
  `earendil-works/pi@f9bcd351dc3cedf989bc5fc0f8aa012db5737df2:packages/coding-agent/docs/containerization.md:124`).
- Condition 5: errand forwards every other client header unchanged. So a
  nonce in a second header reaches the upstream (READ, the broker.ts lines
  above).
- Condition 6: CubeSandbox injects over plaintext HTTP when only the Host
  header is present. The fix in PR 1411 closed without merge (READ:
  `TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:CubeEgress/lua/access_phase.lua:20-35`).
- Condition 6: on a gate failure, CubeEgress drops the injection and still
  forwards the request. This page forbids that shape.
- Condition 7: agent-vault resolves on the host, refuses if any answer is
  blocked, and dials the literal (READ:
  `Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/netguard/netguard.go:93-124`).

### Why the endpoint model needs fewer conditions

The old interception design had more conditions. It checked TLS from the
guest, the SNI against the binding scope, and the destination address. Those
existed because the guest named the destination:

- CubeSandbox: the plaintext path injected the credential toward an address
  the attacker chose, on the strength of a Host header
  ([issue 1410](https://github.com/TencentCloud/CubeSandbox/issues/1410)).
- OpenSandbox: binding selection used the Host header, which the sandbox
  controls. The SNI fix is an open PR (READ:
  `opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:components/egress/mitmscripts/system.py:566-568`).
- CubeSandbox: a matching Host or SNI does not prove the destination address
  ([issue 1697](https://github.com/TencentCloud/CubeSandbox/issues/1697),
  open).

In the endpoint model the guest names only a binding of its own instance. The
binding fixes the destination. The broker verifies the destination itself, by
conditions 6 and 7.

## Fail-closed shapes (T-143)

| when the refusal happens | the answer |
| --- | --- |
| before any byte went upstream, with the head and any body buffered | a local 503; proposal: 403 for a refusal by policy |
| after forwarding started, or for a streaming body of unknown length | the broker terminates both connections |

- Neither shape forwards the request without the credential. An
  unauthenticated request that reaches an upstream looks like an upstream
  fault, and the agent retries in a different way.
- Most refusals happen at the head, before the body. A refusal at the head can
  always use the first shape.
- The tests assert that injection happened, not only that the request worked.
  A request that reached the upstream without the gate fails, even with a 200
  answer.
- OpenSandbox uses the same two shapes (READ, the credential-vault.md lines
  85-97 above).

## Timeouts (T-144)

| bound | value | protects against |
| --- | --- | --- |
| read the request header | 10 s | a slow client that holds a connection open |
| write, streaming | 30 min | the end of a long stream: a model turn, a clone, a push |
| idle between requests | 2 min | keep-alive connections that nobody uses |
| upstream response header | 5 min | an upstream that never answers |
| upstream TLS handshake | 10 s | an upstream that never completes TLS |

- agent-vault uses the same values plus a 60 s read bound (READ:
  `Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/mitm/connect.go:137-145`
  and
  `Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/mitm/proxy.go:83-91`).
- Proposal: no bound on the total time to read a request body. A git push body
  can stream for longer than 60 s.
- NOTE: agent-vault sets these values as Go server deadlines, so its 30 min
  write bound applies to a whole response. kage sets no total bound on a
  stream (READ, the http.rs lines above).
- T-144 decides whether the 30 min bound is total or per write. A total bound
  cuts a longer stream.
- CubeSandbox shipped a 60 s read timeout that cut real model requests. PR
  1655, merged, raised it to 2 h (READ:
  `TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:CubeEgress/nginx.conf:126-128`;
  [issue 1650](https://github.com/TencentCloud/CubeSandbox/issues/1650)).
- The netstack keeps an established TCP flow for more than 600 s of silence
  ([network.md](network.md)). So the netstack never ends a stream before these
  bounds do.

## The substitution model (T-145)

- Header injection covers the providers that kage uses. Substitution covers an
  upstream that wants the credential somewhere else.
- A binding declares a placeholder, the credential it stands for, and the
  surfaces it may go into: path, query, header or body.
- The surface list is the security boundary (READ:
  `Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/brokercore/substitution.go:16-20`).
- A placeholder is a nonce of this instance for this binding.
- Substitution is one pass. The broker replaces every placeholder in one scan
  and never scans an inserted value again.
- agent-vault replaces each placeholder in turn, so a later placeholder can
  match text inside an earlier value. OpenSandbox uses one pass (READ:
  `opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/guides/credential-vault.md:226-228`).
- On any substitution error the request is not forwarded. Some changes can
  already be applied.

Encoding per surface (READ:
`Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/brokercore/substitution.go:36-87`
and
`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/guides/credential-vault.md:216-236`):

| surface | encoding | other rule |
| --- | --- | --- |
| path | path-escape into the wire-encoded path, so the value lands once | refuse a path with dot-segments, encoded separators or backslashes |
| query | query-escape | none |
| header | the raw value | refuse the whole request if the value holds CR or LF; never Host, Content-Length or forwarding headers |
| body, form-encoded | query-escape | set Content-Length and remove Transfer-Encoding |
| body, JSON | JSON string escape | set Content-Length and remove Transfer-Encoding |
| body, multipart | skipped | the placeholder goes upstream unchanged |
| body, compressed | skipped | the placeholder goes upstream unchanged |
| body, any other type | the raw value | set Content-Length |
| WebSocket frames | none in v1 | the broker refuses an Upgrade request |

More rules:

- The header surface runs before auth injection, as in OpenSandbox.
- The placeholder and every encoded form of the value join the redaction set
  for logs.
- A body held in memory for substitution has its own cap. That cap is far
  below the cap for a forwarded body.
- agent-vault uses 64 MiB and 1 GiB for the two caps (READ:
  `Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/brokercore/session.go:10-18`).
  T-145 sets the values here; none is measured.
- OpenSandbox streams a body above 1 MiB before its body hook runs, so it
  skips body substitution for it (READ:
  `opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:components/egress/mitmscripts/system.py:31-36`).
- agent-vault added a WebSocket surface in
  [PR 225](https://github.com/Infisical/agent-vault/pull/225), merged. It
  substitutes only unfragmented, uncompressed client text frames of 1 MiB or
  less.
- In agent-vault, permessage-deflate turns that WebSocket substitution off
  (READ:
  `Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/mitm/websocket.go:45-53`).

## The HTTP head parser (T-142)

The broker reads the head of each guest request itself. Each trap below gives
a corrupted or misrouted request, not an error, so a review does not see it.

1. Read the head incrementally, up to and including the blank line. Cap it at
   8 KiB, and refuse at the cap.
2. Keep the bytes read past the head. They belong to the body and go upstream
   in order.
3. Refuse a target that the broker would have to guess at. The list follows
   this one.
4. Strip hop-by-hop headers in both directions.
5. Set the upstream Host from the binding, never from the guest.

errand's reader returns the partial head at its 8192-byte cap, against its own
comment (READ:
`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/broker.ts:292-326`).
That is the defect that trap 1 names.

Targets and heads that the broker refuses:

- any form other than origin-form (`/path?query`): absolute-form,
  authority-form (CONNECT) and asterisk-form;
- a missing Host header, two Host headers, or a Host that names no binding of
  this instance;
- a request with both Content-Length and Transfer-Encoding, or with two
  different Content-Length values;
- obsolete line folding, a bare CR, and a bare LF line end (proposal; kage and
  git send CRLF);
- dot-segments, encoded slashes and backslashes in the path.

The hop-by-hop headers are connection and each header it names, keep-alive,
proxy-authenticate, proxy-authorization, te, trailer, transfer-encoding and
upgrade (READ:
`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/broker.ts:98-109`).
Forwarding proxy-authorization sends the caller's own proxy credential to the
origin.

Matching rules kept from the old plan:

- The longest prefix wins among the routes of a binding.
- A wildcard does not match its apex. `*.example.com` matches any name under
  example.com, never example.com. Names compare case-folded.
- Source for both rules (READ):
  `QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/broker.ts:30-43`.

T-183 fuzzes this parser.

## The forge binding and the push namespace (T-146)

git smart HTTP uses these requests, for each allowed repository OWNER/REPO,
with or without the .git suffix:

| request | method | git operation | the broker |
| --- | --- | --- | --- |
| /OWNER/REPO.git/info/refs?service=git-upload-pack | GET | fetch and clone | allows |
| /OWNER/REPO.git/git-upload-pack | POST | fetch and clone | allows |
| /OWNER/REPO.git/info/refs?service=git-receive-pack | GET | push discovery | allows |
| /OWNER/REPO.git/git-receive-pack | POST | push | allows after the ref check |
| any other path, LFS included | any | none | refuses in v1 |

The ref check on a push:

1. The body of a git-receive-pack POST starts with ref-update commands in
   pkt-lines. Each holds the old object id, the new object id and the ref.
2. The first command also carries the capabilities after a NUL byte. A
   flush-pkt ends the list, and the pack follows.
3. The broker reads the command list into memory, up to a fixed cap that
   T-146 sets.
4. Every ref must start with `refs/heads/agent-box/<instance>/`. One ref
   outside the namespace refuses the whole push.
5. Nothing goes upstream before this check. A refused push gets a local 403.
6. Proposal: the broker refuses push options and push certificates in v1. A
   push certificate wraps the commands, and some forges act on push options.
7. After the check, the broker forwards the commands and streams the pack.

More rules for the forge:

- The path scope is the list of allowed repositories. Other repositories on
  the same forge are refused.
- Proposal: `<instance>` is the instance name.
- Open question for T-146: a deleted instance's name can be used again, and
  the new instance then writes in the old namespace. The instance id avoids
  that.
- Proposal: the forge credential goes upstream as HTTP Basic, with the token
  as the password. T-146 checks the form against the forge.
- The broker binding carries git only. The daemon, not the guest, opens pull
  requests through the forge API later (T-223).

## Usage accounting (T-147)

- The broker counts requests and tokens per instance and per binding from
  provider responses, SSE streams included.
- It reads a copy of each response as it streams. It never changes the bytes
  that go to the guest.
- An unknown response shape still counts as a request, with tokens marked
  unknown.
- Broker accounting is the only trusted source for billing, rate-limit backoff
  and slot accounting (T-250, T-251, T-252).
- kage's usage events and its ACP stream are agent-reported. Any guest process
  at kage's uid can forge them. They are a second number to compare, never the
  meter.
- This is the rule R-TRUSTED-CHANNEL of [architecture.md](architecture.md).
- errand starts a global provider backoff from an agent-reported retry event
  (READ:
  `QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/session.ts:889-890`).
- In errand, admission then stops for every session (READ:
  `QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/admission/scheduler.ts:214-222`).
  So one forged event can stall every session.

## The credential store (T-140)

- The daemon holds the store. The values are encrypted at rest.
- Windows: DPAPI, in the scope of the operator's user.
- Linux: a key file with mode 0600 in the daemon's state directory, or the
  Secret Service when present. T-140 decides.
- `agent-box secret set NAME` reads the value from stdin only, never from
  argv. A value on argv is visible in the host process table.
- `agent-box secret list` prints names only. `agent-box secret rm NAME`
  removes one credential.
- The config file and the profiles hold no credential (T-027). A binding names
  a credential and never holds it.
- Every secret-shaped value goes through one redaction helper before any log,
  nonces included. A log records the fact, not the value.
- Source for the redaction rule (READ):
  `Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/security/secrets.md:23-103`.
- The store is a hard dependency of the gate (condition 8). After a daemon
  restart, the daemon loads it before any flow opens ([network.md](network.md)).

WARNING: The store protects against a copied disk or a backup. It does not
protect against a process that runs as the operator's user. Such a process can
call DPAPI or read the key file too.

## Optional TLS interception, later (T-149)

TLS interception is off by default and is not in v1. It exists for tools that
cannot take a base URL. kage first needs a CA option upstream (T-169),
because it ignores a planted CA.

Rules if it is built:

1. Authorize before signing. The instance and the binding scope are fixed
   before any TLS work.
2. Validate the SNI before signing. Cache a leaf only under a validated SNI.
3. Take serials from the CSPRNG.
4. Give each instance a signing budget, and limit the signing rate. A flood
   would otherwise evict valid leaves from the cache.
5. Refuse TLS without SNI. A flow that passes through untouched is egress
   that the broker never examined.
6. Use the SNI as the anchor. The upstream certificate must verify for the
   same SNI that selected the binding.
7. Refuse injection over plaintext.
8. Keep the CA private key in the daemon, encrypted at rest. No path from the
   guest reaches it.

Exhibits for the rules (READ):

- Rule 1: agent-vault authenticates the CONNECT and resolves the scope before
  the handshake that mints the leaf:
  `Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/mitm/connect.go:74-88`.
- Rule 1: CubeSandbox mints the certificate before any authorization, and PR
  1427 closed without merge:
  `TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:CubeEgress/nginx.conf:151-173`.
- Rule 2: agent-vault validates the SNI before signing:
  `Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/ca/soft.go:466-496`.
- Rules 2 and 3: CubeSandbox keys its cache on the raw SNI and builds serials
  from the clock and math.random:
  `TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:CubeEgress/lua/cert_signer.lua:97-194`.
- Rule 3: PR 1455, which moves CubeSandbox to CSPRNG serials, closed without
  merge.
- Rule 5: OpenSandbox passes a TLS flow with no SNI through untouched:
  `opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:components/egress/mitmscripts/system.py:44-50`.
- Rule 6: OpenSandbox's SNI anchor fix is
  [PR 1759](https://github.com/opensandbox-group/OpenSandbox/pull/1759), open.
- kage opens a new TLS connection for every model call, so a leaf cache per
  SNI matters (READ, the http.rs lines above).

## SSH agent for the guest (T-257, planned)

The guest holds no SSH key. ssh to a host that needs a key therefore fails
until T-257 lands. The design of T-257:

- box-guest offers an SSH agent socket (`SSH_AUTH_SOCK`) to uid 1000.
- box-guest bridges each client of that socket over vsock port 5005, guest
  to host. The first frame carries a token, as on port 5003
  ([protocol.md](protocol.md)).
- The daemon runs the agent. It signs only with keys that the operator bound
  to that instance.
- It signs only while the netstack holds a flow from that instance to an
  allowed destination host ([network.md](network.md)).
- The key stays in the credential store. The guest sees only the agent
  protocol, never the key.
- The agent parses bytes that the guest wrote. It bounds each message, as the
  HTTP head parser does.

## Short-lived forge credentials (T-148, open)

- Proposal: a GitHub App installation token, minted per instance. The broker
  injects it, and the guest never reads it.
- No reference implements this. agent-vault's request for it is open
  ([issue 255](https://github.com/Infisical/agent-vault/issues/255)).
- It matters more now, because project files move only through the forge
  (ruling D4).
- Open questions: where the App private key lives, the token lifetime and
  refresh, the repository scope, and revocation at delete.
- Clone and fetch through the broker need the streaming rules above.

## Evidence: what the old page called fixed

The old credential-brokering page said that nearly every rule came from a
defect somebody shipped and then fixed. At the pins, several of those defects
are still present. Each row gives the state in the corpus of 2026-09-15.
T-013 checks each row again at newer commits.

| item | state at the pin | what it means |
| --- | --- | --- |
| CubeSandbox [issue 1410](https://github.com/TencentCloud/CubeSandbox/issues/1410), [PR 1411](https://github.com/TencentCloud/CubeSandbox/pull/1411): opt-in before plaintext injection | issue closed; PR closed without merge | plaintext injection is still in the pinned tree |
| CubeSandbox [issue 1426](https://github.com/TencentCloud/CubeSandbox/issues/1426), [PR 1427](https://github.com/TencentCloud/CubeSandbox/pull/1427): authorize before signing | PR closed without merge | the pinned tree mints before authorization |
| CubeSandbox [PR 1455](https://github.com/TencentCloud/CubeSandbox/pull/1455): CSPRNG serials | closed without merge | serials still come from the clock and math.random |
| CubeSandbox [issue 1458](https://github.com/TencentCloud/CubeSandbox/issues/1458), [PR 1459](https://github.com/TencentCloud/CubeSandbox/pull/1459): type checks on match fields | issue open; PR closed without merge | a method of the wrong type passes admission and fails at request time |
| CubeSandbox [issue 1697](https://github.com/TencentCloud/CubeSandbox/issues/1697), [PR 1695](https://github.com/TencentCloud/CubeSandbox/pull/1695): destination binding | both open | no fix upstream; agent-box uses answered-address binding |
| CubeSandbox [PR 763](https://github.com/TencentCloud/CubeSandbox/pull/763): DNS expiry killed flows | closed without merge | PRs 1347 and 1399 removed the bug |
| CubeSandbox [PR 1399](https://github.com/TencentCloud/CubeSandbox/pull/1399): live policy re-evaluation | merged | the generation stamps are shipped |
| CubeSandbox [PR 1655](https://github.com/TencentCloud/CubeSandbox/pull/1655): proxy timeouts of 2 h | merged | fixed |
| OpenSandbox [PR 1759](https://github.com/opensandbox-group/OpenSandbox/pull/1759): SNI anchors the binding | open | the pinned tree selects by Host |
| OpenSandbox [PR 1762](https://github.com/opensandbox-group/OpenSandbox/pull/1762), [issue 1776](https://github.com/opensandbox-group/OpenSandbox/issues/1776): scoped bindings | 1762 merged as opt-in; 1776 open | an omitted scope is still everything by default |
| OpenSandbox [PR 1804](https://github.com/opensandbox-group/OpenSandbox/pull/1804): DNS revalidation rules | merged | shipped |
| agent-vault [issue 209](https://github.com/Infisical/agent-vault/issues/209), [PR 225](https://github.com/Infisical/agent-vault/pull/225): WebSocket substitution | merged | the old limit "frames are copied raw" is out of date for agent-vault |
| agent-vault [PR 229](https://github.com/Infisical/agent-vault/pull/229): flush per chunk | merged | fixed |
| agent-vault [PR 374](https://github.com/Infisical/agent-vault/pull/374): dial fallback across validated addresses | open | the pinned tree dials only the first address |
| agent-vault [issue 255](https://github.com/Infisical/agent-vault/issues/255): GitHub App tokens | open | no reference implements short-lived forge credentials |
| agent-sandbox [issue 1580](https://github.com/kubernetes-sigs/agent-sandbox/issues/1580), [PR 1644](https://github.com/kubernetes-sigs/agent-sandbox/pull/1644): authorization scoped to one execution | both open | the lifetime problem is open everywhere |

- Some line numbers on the old page came from the reporter's commit, not from
  the pinned tree. This page cites the pinned trees only.
- The old page's "client refuses the proxy and falls back" item came from
  agent-vault [issue 194](https://github.com/Infisical/agent-vault/issues/194).
  The netstack has no proxy, so the item does not apply.

## What brokering does not do

[limits.md](limits.md) has the full list. In short:

- Brokering stops credential theft. It does not stop misuse of a binding for
  what the binding allows.
- The broker reaches only HTTP requests to its endpoints. WebSocket frames
  and tools without a base URL get no credential.
- SSH gets no key until T-257 lands. Then the SSH agent of the daemon signs
  for the guest, and the key stays on the host.
- MCP OAuth tokens that kage stores in the guest are outside the broker until
  T-162 decides.

## Threat model of the broker (T-182 completes it)

| item | statement |
| --- | --- |
| attacker | every guest process, root in the guest included |
| assets | the credentials, the other refs and repositories on the forge, the operator's provider quota |
| boundary | the broker endpoint on the gateway, reached only through the network channel of the instance |
| controls | the eight gate conditions, fixed upstreams, nonces per instance and binding, the push namespace, deny first |
| residual risk | misuse through an authorized binding ([limits.md](limits.md)); the store in the context of the operator's user |

## Entries

| entry | title |
| --- | --- |
| T-140 | Credential store |
| T-141 | Bindings and nonces |
| T-142 | Broker endpoint server on the gateway |
| T-143 | The injection gate and the fail-closed shapes |
| T-144 | Upstream client |
| T-145 | Substitution model |
| T-146 | Forge binding and the push namespace |
| T-147 | Usage accounting |
| T-148 | Short-lived forge credentials |
| T-149 | Optional TLS interception for tools that cannot take a base URL |
| T-150 | Narrow the bindings per task phase |
| T-257 | SSH from the guest through a host-held key |
| T-160 | kage configuration and approval mode |
| T-162 | MCP OAuth tokens in the guest |
| T-169 | Requests to the kage maintainer, sent by the operator |
