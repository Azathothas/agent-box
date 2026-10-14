# Upstream requests

This page holds the requests that agent-box has for other projects. The
operator sends each one. Agents never open an issue, a pull request, a
discussion or a comment in another repository.

An agent writes a request here when an entry needs a change in another
project. A request gives what, why, the evidence, the workaround and the
entries that wait. The evidence cites a pinned commit of the pins table in
`docs/references.md`.

| Status | Meaning |
| --- | --- |
| not sent | Written here. The operator has not sent it. |
| sent DATE | The operator sent it on that date, and gives the link to where it went. |
| answered DATE | The answer, as the operator relays it, and what it changes in the plan. |

[PROGRESS.md](PROGRESS.md) asks the operator which requests to send (Q5).

## kage

kage is the harness inside each instance (ruling D2). Its maintainer is
QaidVoid, who also maintains errand and bailey. T-169 tracks these requests.
It stays `blocked` until each request has an answer.

Each citation below reads kage at the commit of its pins row. Each one is
READ: opened on 2026-10-14. None of these requests blocks a milestone today;
each one names its workaround.

### K1. A CA bundle or platform-verifier option

- What: an option that makes the HTTP clients of kage trust a CA bundle
  file, or the trust store of the platform.
- Why: kage trusts only the Mozilla roots compiled into its TLS client. A CA
  in the guest trust store, or in SSL_CERT_FILE, has no effect on kage.
- Why it matters later: the optional TLS interception of T-149 cannot work
  for kage without this option.
- Workaround: none needed for v1. Credentialed traffic goes to plain-HTTP
  broker endpoints (planning decision P4).
- Evidence: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/http.rs:43-52`
  builds the ureq configuration with no root store setting.
- Evidence: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:Cargo.toml:91`
  enables the json feature of ureq 3.4.2 only.
- Evidence: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:Cargo.lock:4274`
  locks webpki-roots, the compiled-in Mozilla roots.
- Waits: T-149.
- Status: not sent.

### K2. MCP OAuth discovery that follows the egress proxy

- What: when an egress proxy is set, the MCP OAuth metadata check does not
  resolve the host in the guest. web_fetch already works this way.
- Why: the OAuth discovery check resolves each metadata host locally before
  the fetch. With a proxy and no guest DNS, discovery fails.
- Why it matters later: only if the operator brokers MCP OAuth through a
  proxy (T-162, Q2). agent-box v1 has no egress proxy.
- Workaround: none needed for v1. The netstack DNS answers allowed names.
- Value now: low. v1 has no egress proxy, and the guest resolves allowed
  names through the netstack (ruling D5). The request stays for a later
  brokered MCP OAuth.
- Evidence: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-mcp/src/oauth.rs:886-889`
  checks each metadata URL with a local resolve.
- Evidence: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-tools/src/ssrf.rs:100-102`
  resolves the host with to_socket_addrs.
- Evidence: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-tools/src/ssrf.rs:118-125`
  gives the proxy path that web_fetch uses.
- Context: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/mcp_auth.rs:187-189`
  refreshes MCP OAuth tokens inside kage. This is why T-162 exists.
- Waits: T-162.
- Status: not sent.

### K3. Trust gating for the shell, web_search and rename settings

- What: an untrusted project .kage/config.toml cannot set [shell] program and
  scrub_env, [tools.web_search] or [tools.rename].
- Why: kage removes only [mcp], [permissions] and [plugins.capabilities] from
  an untrusted project file. A cloned repository can choose the program of
  each shell tool call.
- Workaround: the guest pins KAGE_SHELL__PROGRAM and KAGE_SHELL__SCRUB_ENV in
  the environment of kage (T-160). The instance is the boundary, so this gap
  is not an escape.
- Evidence: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-core/src/trust.rs:110-126`
  is the function that filters an untrusted project file.
- Waits: T-160.
- Status: not sent.

### K4. A raw ACP listener on a Unix socket or vsock for kage serve

- What: `kage serve` takes ACP as newline-delimited JSON, as `kage rpc` does,
  on a Unix socket or an AF_VSOCK port.
- Why: `kage serve` listens on TCP with WebSocket only, on 127.0.0.1:7433 by
  default. box-guest must bridge vsock port 5004 to that address.
- Why it helps: a vsock listener removes the bridge process and the
  loopback port in the guest. It also removes the token file, which box-guest
  now keeps on a tmpfs (T-046).
- Workaround: the bridge of box-guest (T-046).
- Evidence: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/remote.md:3`
  says that serve speaks the same ACP as `kage rpc`.
- Evidence: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/remote.md:32-35`
  gives the bind address and the TCP port of serve.
- Waits: T-046, T-161.
- Status: not sent.

### K5. kage print mode reads the prompt from stdin

- What: `kage -p` reads the prompt from stdin, for example when the prompt
  argument is `-`.
- Why: print mode takes the prompt as an argument only. A large prompt hits
  the argument length limit of the guest kernel.
- Workaround: `agent-box run` drives kage over ACP (T-171), where the prompt
  goes in a message.
- Evidence: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:man/kage.1:14-15`
  documents `-p, --print <PRINT>` as the prompt argument.
- Waits: T-171.
- Status: not sent.

### K6. Plans for provider OAuth

- What: whether kage plans provider OAuth, so that a subscription account can
  serve as a provider.
- Why: kage model providers use API keys only. The writer of an OAuth
  credential record is compiled for tests only.
- Why it matters later: the provider pool (T-250) serves API keys until kage
  has a plan. With OAuth, the broker must hold each refresh token on the
  host.
- Workaround: API-key providers only.
- Evidence: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/auth.rs:305-310`
  marks `set_oauth` with `#[cfg(test)]`.
- Waits: T-250.
- Status: not sent.

### K7. A settled event for a whole agent tree

- What: one event when a run and every child agent of it have ended, as the
  `agent_settled` event of pi.
- Why: the provider pool and the chat gateway wait for the end of a turn to
  free a slot or to post a reply.
- Rule: agent-box labels this event agent-reported. It never feeds billing,
  backoff or authorization (R-TRUSTED-CHANNEL).
- Workaround: an ACP client that sends the prompt gets its answer after
  every child has sent its final state.
- Value now: partly covered. The daemon is the ACP client, so the end of a
  prompt already marks the end of the tree. The request stays for observers.
- Evidence: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-core/src/protocol.rs:135-142`
  defines `run_started` and `run_ended` for one session.
- Evidence: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/reference/architecture.md:294-295`
  says that an ACP prompt answers after every child sends its final state.
- Evidence: `earendil-works/pi@f9bcd351dc3cedf989bc5fc0f8aa012db5737df2:packages/coding-agent/CHANGELOG.md:828`
  adds `agent_settled` to pi.
- Not read: whether an attached observer, or the `kage -p --json` stream,
  gets one event for the whole tree.
- Waits: T-222, T-251.
- Status: not sent.

## Other upstreams

None yet. A request to another project gets its own section here, in the
same form as the kage requests.
