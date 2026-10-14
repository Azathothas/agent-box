# kage

This area covers kage in the guest. kage is the harness (operator ruling D2).
agent-box configures it, starts it and drives it, and does not change its
code. box-guest runs `kage serve` as uid 1000 on guest `127.0.0.1:7433`, and
bridges vsock port 5004 to it (T-046). The daemon speaks ACP to it. The design
is in [guest](../docs/guest.md). kage's provider calls go to the broker
endpoints of [broker](broker.md). A gap in kage goes to the maintainer through
the operator, by way of [upstream](upstream.md). No agent posts in another
repository.

Every kage fact here was read at
`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964`, the v0.3.0 era.
kage changes fast: five releases in 8 days before that commit. T-013 re-pins
it, and each entry checks its facts against the pinned release.

# T-160: kage configuration and approval mode

**Source:** [guest](../docs/guest.md), the kage pins. The kage reader of
2026-10-14 and the relevance pass. The trust and approval entries of the
earlier plan map here.
**Category:** decision
**Milestone:** M7
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

kage's behaviour comes from its config files, its environment and its ACP
client. A cloned repository can set some of those values. An unanswered
approval stalls an unattended run. A config that points kage at the real
provider puts the guest on the path of a real key. The values must be fixed
in one place, with a reason for each.

## Premise

- READ: kage has no OS sandbox. The shell tool says commands "run unsandboxed
  today"
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-tools/src/builtin/shell.rs:1-4`).
- READ: built-in tools run without a question unless `[permissions]` names
  them. MCP tools ask
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/guide/permissions.md:3-6`).
- READ: over ACP, every tool without a rule asks the client. When the last
  client of a session leaves with a request open, the request is denied
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/guide/permissions.md:119`).
- READ: a client can send `_meta.kage.unconfiguredTools: "allow"`, and the
  session option `mode` takes `allow`
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/kage-extensions.md:18-21`,
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/zed.md:166`).
- READ: every attached client gets each approval, and the first answer wins
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/remote.md:151-158`).
  Not tested: a tool at kage's uid can attach and approve its own request.
- READ: an untrusted project loses only its `mcp`, `permissions` and
  `plugins.capabilities` tables. `providers` and `acp` never apply from a
  project
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-core/src/trust.rs:110-126`).
  So a project's `[shell]` still applies.
- READ: `[shell]` has `program` and `scrub_env`
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-core/src/config.rs:806-822`).
  `kage init` writes `scrub_env = []`
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/init.rs:66`).
- READ: `KAGE_` environment variables merge last and win over both files
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-core/src/config.rs:275`).
  Not tested: figment parses an array value such as `["*_API_KEY"]`.
- READ: an environment variable name cannot hold a dash. A provider id with a
  dash, such as `openai-responses`, cannot be set through the environment
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/guide/providers.md:395-398`).
- READ: `base_url` for the `openai` kind must include `/v1`. For `anthropic`
  and `gemini` it is the host root
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/guide/providers.md:368-374`).
- READ: `gemini` is the kind, but the code registers the provider id
  `google`
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/providers.rs:12-13`).
  kage stops at startup when a `KAGE_PROVIDERS__` variable names an unknown
  id (the providers guide, lines 398-400). So the pin is
  `KAGE_PROVIDERS__GOOGLE__BASE_URL`.
- READ: kage reads provider keys from their variables one time, when its host
  starts
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/rpc/host.rs:86-106`).
- Not tested: a project `.kage/AGENTS.md` loads with no trust question when
  the project sets nothing risky
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/runtime_env.rs:109-133`).

## Approach

1. box-guest builds the environment of `kage serve` from nothing (T-046). It
   inherits no variable. The section "Configuration pins" of
   [guest](../docs/guest.md) holds the full list. The variables:

| Variable | Value | Why |
| --- | --- | --- |
| `HOME`, `USER`, `LOGNAME` | `/home/agent`, `agent`, `agent` | the agent user, uid 1000 |
| `PATH` | `/usr/local/bin:/usr/bin:/bin` | fixed |
| `LANG` | `C.UTF-8` | fixed |
| `XDG_RUNTIME_DIR` | `/run/user/1000` | a tmpfs that box-guest makes |
| `XDG_CONFIG_HOME` | `/run/agent-box/kage/config` | a tmpfs owned by root, read-only to uid 1000 |
| `XDG_DATA_HOME` | `/run/agent-box/kage/data` | a tmpfs: the serve token and connect.txt stay off the data disk |
| `SSH_AUTH_SOCK` | `/run/agent-box/ssh-agent.sock` (T-257) | ssh signs through the daemon, with no key in the guest |
| `KAGE_SHELL__PROGRAM` | `/bin/bash` | a project cannot change the shell |
| `KAGE_SHELL__SCRUB_ENV` | `["*_API_KEY"]` | shell commands do not see the provider nonces |
| `KAGE_PROVIDERS__<ID>__BASE_URL` | the broker endpoint | one for each bound provider id without a dash |
| the provider key variable | the nonce of that binding | for example `ANTHROPIC_API_KEY` |
| `GIT_CONFIG_COUNT`, `GIT_CONFIG_KEY_0`, `GIT_CONFIG_VALUE_0` | the forge nonce header (T-146) | git through the broker |

2. The provider map. The binding name is the kage provider id, so the
   endpoint is `<id>.broker.internal`:

| kage provider | Key variable | `base_url` | Pinned in |
| --- | --- | --- | --- |
| `anthropic` | `ANTHROPIC_API_KEY` | `http://anthropic.broker.internal` | environment |
| `openai` | `OPENAI_API_KEY` | `http://openai.broker.internal/v1` | environment |
| `openai-responses` | `OPENAI_API_KEY` | `http://openai.broker.internal/v1` | config file |
| `google` | `GEMINI_API_KEY` | `http://google.broker.internal` | environment |
| a catalog id, for example `zai-coding-plan` | `ZAI_CODING_API_KEY` | `http://zai-coding-plan.broker.internal/api/coding/paas/v4` | config file |

3. For a catalog provider, `base_url` is the endpoint plus the path of the
   provider's default base URL. The broker forwards the path unchanged
   (T-141). `openai` and `openai-responses` share the `openai` binding.
4. box-guest writes `/run/agent-box/kage/config/kage/config.toml` before each
   start of kage. It renders the file from the provisioning data (T-041). The
   directory and the file are owned by root, modes 0755 and 0644. The file
   holds:

```toml
[shell]
program = "/bin/bash"
scrub_env = ["*_API_KEY"]

[providers.openai-responses]
base_url = "http://openai.broker.internal/v1"
```

   It also holds one `[providers.<id>]` table for each bound catalog id with
   a dash.
5. Approval. The daemon's ACP client (T-161) sends
   `unconfiguredTools: "allow"` and sets the session option `mode` to
   `allow` after each `session/new` and `session/load`.
6. Never run `kage trust` in an instance. box-guest and the daemon have no
   code path that runs it. The image ships no `trust.json`.
7. The daemon never calls `_kage/auth/set`, `_kage/config/set`,
   `_kage/options/set` or `_kage/plugins/install` (T-161).
8. Write in [guest](../docs/guest.md) that a project's `.kage/AGENTS.md` is
   untrusted prompt input.

## Decision

Approval mode is `allow`. The instance and the host egress policy are the
boundary, not kage's prompts. Reasons:

- kage has no OS sandbox, and its tools run at kage's uid. A tool can read
  kage's files and can attach to `kage serve`. An approval inside the guest
  does not stop the model.
- An unattended run has nobody to answer. An open request is denied when the
  last client leaves, so the run goes on degraded and nobody sees why.
- Every credential is on the host, and every flow passes the host policy.
  What a tool can do in the guest, the instance contains.

The config file is on a tmpfs that root owns, and box-guest sets
`XDG_CONFIG_HOME` for each start of kage. So uid 1000 cannot change the file
that kage serve reads. No security property depends on that file anyway. A
wrong `base_url` sends a worthless nonce, and the request fails.

## Prove

```sh
cargo test -p box-guest kage_env::
cargo xtask linux --kvm -- cargo test -p box-daemon --test kage_live pinned_shell -- --ignored
```

Expected: the unit test builds the environment and the config file from a
provisioning message, and compares both with golden files. In the live test,
a project in the guest has `.kage/config.toml` with
`[shell] program = "/tmp/evil"` and `scrub_env = []`. A recorded provider
(T-161) returns one `shell` tool call, `env | grep -c _API_KEY`. The result
is `0`, and `/tmp/evil` never ran (it writes a marker file, and the file is
absent). The same run proves the array parse of `KAGE_SHELL__SCRUB_ENV`.
[decisions](../docs/decisions.md) holds the decision with its reasons. Host: a
Linux KVM host, and Windows 11 with Hyper-V as admin.

## Start condition

T-063 and T-141.

# T-161: ACP client in the daemon

**Source:** [guest](../docs/guest.md): one `kage serve` for each instance,
bridged over vsock. The kage reader of 2026-10-14. The harness adapter entry
of the earlier plan maps here.
**Category:** feature
**Milestone:** M7
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

`agent-box run`, `agent-box attach` and the later chat and web surfaces all
drive kage. They need one client in the daemon that speaks kage's protocol.
That client reads bytes that the guest wrote, so it must treat each one as
untrusted.

## Premise

- READ: `kage serve` speaks ACP over WebSocket at `/acp` on one TCP listener,
  default `127.0.0.1:7433`, with at most 16 connections
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/serve/mod.rs:57-60`).
- READ: its token is 64 hex characters in `$XDG_DATA_HOME/kage/remote-token`.
  A stored well-formed token is loaded as it is
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-remote/src/token.rs:37-58`).
- READ: serve writes the connect URL, with the token, to `connect.txt` beside
  the token file
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/serve/mod.rs:220-235`).
  So both files must be on a tmpfs (T-046), or the token reaches the guest
  disk.
- READ: the WebSocket head is capped at 16 KiB, read within 10 s
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-remote/src/lib.rs:32-35`).
  A JSON-RPC line is capped at 8 MiB
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-jsonrpc/src/lib.rs:49`).
- READ: the wire types are in the crate kage-acp-wire, licence MIT, version
  0.3.0, with serde and serde_json as its only dependencies
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-acp-wire/Cargo.toml:1-19`).
- READ: `session/new` takes `cwd` and `mcpServers`.
  `session/set_config_option` takes `sessionId`, `configId` and `value`
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-acp-wire/src/schema.rs:245-262`,
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-acp-wire/src/schema.rs:383-400`).
- READ: several clients can attach to one session and get a replay. Idle
  sessions close and reopen from their file
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/remote.md:137-166`).
- READ: in the microVM, any process at kage's uid can write kage's stdout
  through `/proc` (errand issue 2,
  https://github.com/QaidVoid/errand/issues/2). The ACP stream is
  agent-reported.
- READ: errand frames records on the byte 0x0A, because U+2028 and U+2029 are
  valid inside JSON strings
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/agent/framing.ts:1-60`).

## Approach

1. Files: crates/box-daemon/src/acp/mod.rs, crates/box-daemon/src/acp/ws.rs
   and crates/box-daemon/src/acp/events.rs.
2. Dependency: kage-acp-wire at exactly the kage version of images/pins.toml
   (T-063), from crates.io if that version is published, else by git at the
   pinned commit. A test fails when the two versions differ. Add MIT to the
   licence allowlist of `deny.toml` if it is absent.
3. Token: 32 bytes from the host CSPRNG, as 64 lowercase hex characters. It
   goes in the provisioning message on port 5000 (T-041).
   - box-guest writes it to `remote-token` in the tmpfs XDG_DATA_HOME of
     T-046, before each start of kage. It never reaches the data disk.
   - A new token for each boot.
   - The token is not a host secret. It proves the host to the guest only,
     and the agent can read it.
4. Transport: host-initiated vsock port 5004 (`Engine::connect`). box-guest
   bridges it to `127.0.0.1:7433` (T-046). On that stream, open the
   WebSocket with `GET /acp`, `host: 127.0.0.1:7433` and
   `authorization: Bearer <token>`.
5. On Windows, set `HVSOCKET_CONNECTED_SUSPEND` on the socket, so a pause
   keeps it (T-106).
6. Limits on received data: refuse a WebSocket message above 8 MiB and close
   the connection. Parse every frame as untrusted JSON. Ignore unknown fields
   and refuse invalid JSON-RPC.
7. The client sends only these methods: `initialize`, `session/new`,
   `session/load`, `session/prompt`, `session/cancel`,
   `session/set_config_option`, `session/list` and `session/close`. The
   module defines no other method name, so no other call is possible.
   T-258 adds `_kage/session/compact` in M10.
8. `initialize`: protocol version 1. `clientCapabilities`: no file system,
   no terminal, and `_meta.kage.unconfiguredTools: "allow"`.
9. `session/new`: the working directory from the caller, and
   `mcpServers: []` (T-162). Then set `mode` to `allow`, and set `model` when
   the profile names one.
10. Answer each `session/request_permission` with the allow option, and
    write an event that names the tool. Plan mode asks for commands even in
    `allow` mode (READ
    `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/guide/permissions.md:252-258`).
    The instance and the host policy are the boundary (T-160, Decision).
11. Every `session/update` goes to the instance's event log (T-176) as JSONL,
    labelled agent-reported. Nothing from this stream feeds authorization,
    billing, backoff or the instance state. The broker (T-147) and the event
    channel of T-031 are the trusted sources.
12. After a lost connection (a daemon restart, or a pause without the socket
    option), reconnect with backoff and send `session/load`. kage replays
    the session.
13. `session/cancel` for a cancel. Confirm in the guest, through box-guest,
    that the tool's process group is gone (T-042).

## Prove

```sh
cargo test -p box-daemon acp::
cargo xtask linux --kvm -- cargo test -p box-daemon --test kage_live recorded_prompt -- --ignored
```

Expected: the unit tests run against a fake kage serve that plays a
recorded ACP transcript. A prompt completes, the events land in the JSONL log
with the agent-reported label, and a 9 MiB message closes the connection.
A recorded permission request gets the allow option and an event. In the live
test, a test binding points at a local server that plays a recorded Anthropic
SSE response. `agent-box run NAME --prompt TEXT --json`
completes, and the recording server saw the real key, not the nonce. Host: a
Linux KVM host, and Windows 11 with Hyper-V as admin. A run against a real
provider is the operator's step, with the operator's key.

## Start condition

T-046 and T-143.

# T-162: MCP OAuth tokens in the guest

**Source:** The kage reader and the relevance pass, 2026-10-14: MCP OAuth is
a second credential path into the guest.
**Category:** decision
**Milestone:** M7
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

The rule is that no credential lives in the guest. kage's MCP OAuth login
stores real access and refresh tokens in the guest, and kage refreshes them
itself. agent-box must refuse this path or move it to the host.

## Premise

- READ: `kage mcp login` stores OAuth access and refresh tokens in
  `$XDG_DATA_HOME/kage/mcp-auth.json`, and kage refreshes them at the
  server's token endpoint
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/mcp_auth.rs:85`,
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/mcp_auth.rs:187`).
- READ: MCP OAuth discovery checks each endpoint with a resolver in the
  guest, and allows loopback only
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-mcp/src/oauth.rs:883-889`).
  A broker endpoint on `10.200.0.1` is a private address, so discovery
  refuses it.
- READ: the OAuth redirect lands on a one-shot listener on guest loopback, or
  on a URL that a person pastes
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-mcp/src/oauth.rs:1-22`).
  An unattended instance has no browser.
- READ: a project `[mcp]` table applies only after `kage trust`
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-core/src/trust.rs:110-126`).

## Approach

1. The daemon sends `mcpServers: []` in every `session/new` (T-161).
2. The config that box-guest generates has no `[mcp.servers]` table (T-160).
3. The default policy has no MCP server destination (T-163). An operator
   rule adds one on purpose.
4. Write in [limits](../docs/limits.md) and [guest](../docs/guest.md): do not
   run `kage mcp login` in an instance. Say where its tokens would land.
5. box-guest reports when `mcp-auth.json` exists. The report is
   agent-reported, and the startup report shows it as a warning.
6. A brokered MCP OAuth (the host logs in, the broker refreshes and injects)
   is later work. It needs the proxy-aware discovery request of T-169.

## Decision

v1 refuses MCP OAuth. An MCP server that needs a static token waits for a
broker binding of the `substitution` kind (T-145) and an operator rule.
Reason: the conservative choice keeps the rule "no credential in the guest"
without exceptions.

## Prove

```sh
cargo test -p box-daemon acp::new_session_has_no_mcp
cargo xtask linux --kvm -- cargo test -p box-daemon --test kage_live mcp_login_refused -- --ignored
```

Expected: the unit test asserts an empty `mcpServers` in `session/new`. In the
live test, a scratch kage config in the guest names a test MCP server, and
`kage mcp login test` exits non-zero under the default policy. No
`mcp-auth.json` exists after it, and the report names no warning.
[decisions](../docs/decisions.md) holds the decision. Host: a Linux KVM host.

# T-163: Default egress for kage's own destinations

**Source:** The kage reader of 2026-10-14: kage needs egress beyond the model
providers.
**Category:** feature
**Milestone:** M7
**Priority:** P2
**Effort:** S
**Status:** open

## Problem

kage reaches more than the provider endpoints: web search, web fetch, MCP
servers and the model catalogue. The default policy must decide each of them
on purpose. A silent default either breaks a tool or opens a channel nobody
chose.

## Premise

- READ: `web_search` uses `https://html.duckduckgo.com/html/` by default, with
  no key
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-tools/src/builtin/web_search.rs:195`).
- READ: the Brave engine calls a fixed URL,
  `https://api.search.brave.com/res/v1/web/search`
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-tools/src/builtin/web_search.rs:334`),
  with a key from `BRAVE_API_KEY` in the guest
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-tools/src/builtin/web_search.rs:21`).
  The URL is not configurable, so the key cannot go through a broker
  endpoint.
- READ: `web_fetch` takes any URL. Without a proxy, it refuses non-routable
  addresses after a lookup in the guest
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-tools/src/ssrf.rs:118-182`).
- READ: `kage models refresh` fetches `https://models.dev/api.json` only when
  a person runs it. kage never refreshes on its own
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/catalog/source.rs:17`).
- READ: the `[tools]` table is not trust-gated, so a project can switch the
  search engine and URL
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-core/src/config.rs:84-121`).
  The netstack policy still judges the destination.

## Approach

1. Ship two named policies in box-policy, in
   crates/box-policy/src/defaults.rs. Profiles name them (T-027).
2. Policy `default`, mode `allowlist`:
   - the broker endpoints of the instance's bindings (implicit, T-123);
   - no web search, no web fetch, no MCP server and no `models.dev`.
3. Policy `web`, mode `open`:
   - web search and web fetch work, because any public destination is
     allowed;
   - the reserved ranges stay refused (T-123).
4. Decide each kage destination:

| Destination | `default` | `web` | Reason |
| --- | --- | --- | --- |
| `html.duckduckgo.com:443` | refused | allowed | a search query carries data out |
| `api.search.brave.com:443` | refused | refused | its key would live in the guest |
| a `web_fetch` URL | refused | allowed | any URL is a channel out |
| an MCP server URL | refused | refused | T-162 refuses MCP OAuth in v1; an operator rule adds a server |
| `models.dev:443` | refused | allowed | kage has a built-in catalogue snapshot |

5. A refused flow gets NXDOMAIN at DNS (T-122). kage then reports a failed
   tool call, and the run goes on.
6. Write the table into [network](../docs/network.md) and into the comment of
   each policy.

## Prove

```sh
cargo test -p box-policy defaults::
```

Expected: exit 0. Named tests assert each row of the table against both
policies: the verdict for each destination name and port. A test fails when a
shipped policy allows `api.search.brave.com`. Host: any.

## Start condition

T-123.

# T-169: Requests to the kage maintainer, sent by the operator

**Source:** The kage reader and the adversarial review, 2026-10-14. The
vendor-CLI provisioning entry of the earlier plan maps here.
**Category:** research
**Milestone:** M7
**Priority:** P2
**Effort:** S
**Status:** blocked

## Problem

Some gaps between kage and agent-box can be closed only in kage. agent-box
does not patch kage, and no agent posts in another repository. Each request
must be written so that the operator can send it as it is, with its evidence.

## Premise

[upstream](upstream.md) is the one home of the requests, K1 to K7. Each one
has its evidence at `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964`
and its workaround. This table maps each one to the entries that wait:

| Request | Subject | Entries that wait |
| --- | --- | --- |
| K1 | a CA bundle or platform-verifier option | T-149 |
| K2 | MCP OAuth discovery through a proxy | T-162 |
| K3 | trust gating of the shell, web_search and rename settings | T-160 |
| K4 | a raw ACP listener on a Unix socket or vsock | T-046, T-161 |
| K5 | print mode reads the prompt from stdin | T-171 |
| K6 | plans for provider OAuth | T-250 |
| K7 | a settled event for a whole agent tree | T-222, T-251 |

## Approach

1. Keep each request in [upstream](upstream.md), one section for each. Each
   section has:
   - the request in one sentence;
   - the agent-box use, in two sentences or fewer;
   - the evidence as a citation at the pin;
   - what agent-box does until an answer.
2. Never describe the project or its maintainer. Ask, do not judge.
3. K4 also removes the token file from the guest (T-046). Say so in its
   section.
4. K2 matters less under ruling D5, because the guest has DNS through the
   netstack and v1 has no egress proxy. Keep it, and say so.
5. K7 is partly covered: an ACP prompt answers after every child agent ends.
   Keep it, and say so.
6. When an answer comes, write it under its request with the date. Then
   update the entries that wait on it, as the table gives.

## Prove

```sh
cargo todo check
cargo xtask check docs
```

Expected: both exit 0. [upstream](upstream.md) has seven sections, and each
one has a dated answer. Host: any.

## Blocker

The operator must act. The operator sends the requests in
[upstream](upstream.md) to the kage maintainer (QaidVoid) and records each
answer. Agents do not open issues or pull requests in another repository.
