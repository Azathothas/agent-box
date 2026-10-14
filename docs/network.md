# Network

This page specifies how an instance reaches the network. It covers the guest
side, the host netstack, DNS, the flow policy, host connect, inbound forwards
and the deny-first rule. The broker has its own page, [broker.md](broker.md).
What the network does not do is in [limits.md](limits.md). If this page and
[architecture.md](architecture.md) disagree, architecture.md is correct.

NOTE: This page describes planned code. Under ruling D14 in
[decisions.md](decisions.md), the netstack does not exist yet, and T-033
creates its crate first. Each section names the entry that builds it, and
[TODO/net.md](../TODO/net.md) holds those entries.

The labels READ and MEASURED follow [conventions.md](conventions.md). A
statement with no label is design. The word "proposal" marks a default that
the named entry confirms or changes.

## The path of a packet

```text
guest process
  -> tun0 (owned by box-guest)
  -> box-guest: one IP packet per frame
  -> vsock port 5003 (Firecracker hybrid vsock, or AF_HYPERV on Windows)
  -> daemon: the netstack of this instance
       -> DNS server        10.200.0.1:53
       -> broker endpoints  10.200.0.1:80
       -> flow policy -> host connect -> host socket -> upstream
```

Facts that follow from this path:

- The guest has no virtual NIC on a host switch. The Firecracker
  configuration has no network-interfaces key, and the HCS document has no
  NetworkAdapters entry. [engines.md](engines.md) has both.
- The netstack sees every packet the guest sends. No proxy setting exists that
  a client could ignore.
- The host firewall is not part of the boundary. malaria found that the
  Hyper-V firewall decides what crosses a vmNIC, and that a host setting
  changes it (READ:
  `talaria0101/malaria@499e897e8372a5db0f921542184e3b66ef610e2b:docs/windows.md:49-73`).
  With no vmNIC, that firewall decides nothing here.
- If a NIC is ever added, the startup report names the host firewall as a gap.

## Identity of an instance

- The daemon knows which instance sent a frame from the listener that accepted
  the connection.
- On Linux, each instance has its own uds_path_5003 socket inside its own jail.
- On Windows, the daemon binds one listener per VmId and never a wildcard.
- Every instance uses the same guest address plan. The guest address carries
  no identity, because the guest is root in its own kernel and can send any
  source address.
- Exhibit: OpenSandbox's Firecracker profile selects the subject, and so the
  credential vault, from the guest source IP. Its smoke test forges a
  sibling's address (READ:
  `opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:components/egress/pkg/fastsandboxnft/fastsandboxnft.go:36-41`).
- CubeSandbox keys each flow on the host-side TAP ifindex and gives every guest
  the same internal address (READ:
  `TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:docs/architecture/network.md:23-27`).
  agent-box keys on the vsock listener in the same way.

## The guest side (T-045)

box-guest runs as PID 1 and as root. At boot, before kage starts, it does
these steps:

1. It creates tun0 as a TUN device. TUN works at layer 3, so there is no MAC
   address, no ARP and no DHCP.
2. It gives tun0 the guest address and a default route through the gateway.
3. It writes `/etc/resolv.conf` with the gateway as the only name server.
4. It connects vsock port 5003 to the host and sends the egress token as the
   first frame.
5. It copies IP packets between tun0 and the network channel, one packet per
   frame.
6. After a daemon restart, it connects again and sends the token again.

### Address plan (v1 constants of box-proto)

| item | value | note |
| --- | --- | --- |
| guest address | 10.200.0.2/24 | the same in every instance |
| gateway | 10.200.0.1 | the netstack serves it; it is not a host address |
| DNS server | 10.200.0.1, port 53, UDP and TCP | the only resolver the guest knows |
| broker | 10.200.0.1, port 80, TCP | reached as `<binding>.broker.internal` |
| IPv6 | none in v1 | the DNS server returns no AAAA records |

Rules for the plan:

- The plan is synthetic. The guest never sees a host address, a host MAC, a
  host route or the host's resolver.
- bailey gives its confined side 10.0.2.15 and fd00::2 for the same reason
  (READ:
  `QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:crates/bailey/src/cli.rs:404-412`).
  A process that sees host interfaces can read the IP, the MAC and the routes.
- A host LAN in 10.200.0.0/24 causes no conflict. The netstack serves the
  gateway locally and refuses every other address of the plan.
- Open question for T-045: IPv6 in the guest. Without it, a destination with
  no IPv4 address is out of reach.

### MTU (v1 constant of box-proto)

- tun0 uses MTU 1500 in v1.
- The netstack terminates guest TCP and opens its own host sockets. So the
  guest MTU never reaches the Internet path. It sets only the segment size
  between the guest and the netstack.
- A 1500-byte packet gives a frame far below MAX_FRAME, which is 8 MiB.
  [protocol.md](protocol.md) has the framing.
- A larger MTU, up to 65535, can lower the cost per packet. T-120 measures it
  before any change.

### The network channel and the egress token

- Port 5003 carries frames from guest to host. The first frame carries the
  egress token. Every other frame carries one IP packet.
- The egress token is 32 bytes from the host CSPRNG. The daemon sends it on
  port 5000 before kage starts.
- box-guest holds the token in memory only. The host rotates it.
- The daemon compares the token in constant time with the token of the
  instance whose listener accepted the connection.
- A wrong token closes the connection. It also adds one to the refused-forgery
  count in the startup report.
- The token does not identify the instance; the listener does. The token shows
  that box-guest opened the channel, and not another guest process.
- That matters because vsock has no access control per port inside the guest.
  Any guest process can connect to port 5003 (research document, section 5.3,
  in [research/microvm-tier.md](research/microvm-tier.md)).
- Proposal: one network channel per instance at a time. A new channel with a
  valid token replaces the old one, and the flows of the old one end.
- Proposal: a rotated token takes effect at the next connect.

## The host netstack (T-120, T-121)

- The daemon runs one netstack per instance. Instances share no flow table, no
  buffer and no DNS record (T-121, T-186).
- A userspace TCP/IP stack turns frames into flows. The candidates are
  smoltcp, ipstack and netstack-smoltcp.
- T-120 chooses by measurement: throughput and CPU through the vsock framing,
  licence and maintenance. Nothing is measured yet.

### TCP, UDP and ICMP (proposal, T-121)

- TCP: the netstack terminates the guest connection and opens a separate host
  socket.
- The netstack answers the guest SYN only after the host connect succeeds. A
  refused host connect becomes a RST to the guest.
- A host connect timeout also becomes a RST, but the log records "timeout".
  "Refused" and "timed out" are different facts, and malaria's battery shows
  why a record needs both (READ:
  `talaria0101/malaria@499e897e8372a5db0f921542184e3b66ef610e2b:experiments/results/06-default-network.json`).
- UDP: one host UDP socket per flow. The netstack accepts a reply only from
  the destination of that flow.
- A refused UDP datagram gets an ICMP port-unreachable reply from the
  netstack, so a QUIC client falls back to TCP at once (T-121).
- ICMP: the netstack answers an echo request to the gateway and drops every
  other ICMP packet. A userspace stack cannot send ICMP from the host without
  privilege.

### The flow table (proposal, T-121)

Each flow-table entry holds these fields:

- the protocol;
- the guest address and port;
- the destination address and port;
- the allowed name that the destination was answered for;
- the policy generation the flow was admitted under;
- the state and the time of the last packet.

Each instance has a maximum flow count and a maximum buffer size. T-121 sets
the values; none is measured. A full table refuses new flows and keeps the
established ones.

### Timeouts

The idle timeout for an established TCP flow stays above 600 s. kage allows
600 s of silence on a provider response and sets no total bound (READ:
`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/http.rs:19-52`).
A shorter netstack timeout kills a slow model call that is still alive.

Proposal: start from CubeSandbox's flow-table timeouts. T-121 records the
final values.

| state | CubeSandbox value (READ) |
| --- | --- |
| TCP SYN | 1 min |
| TCP established | 3 h |
| TCP FIN_WAIT | 2 min |
| TCP CLOSE_WAIT | 1 min |
| TCP LAST_ACK | 30 s |
| TCP TIME_WAIT | 2 min, or 10 s if the guest closed |
| UDP with no reply yet | 30 s |
| UDP with a reply | 180 s |

Source:
`TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:docs/architecture/network.md:134-154`.
Its reaper runs every 5 s and warns at 80 percent occupancy.

## DNS on the gateway (T-122)

### Why DNS is an exfiltration channel

- The host resolves allowed names upstream. Each name it resolves reaches the
  upstream resolvers and the authoritative servers of its zone.
- So a guest can send data in the labels of a name, with no TCP or UDP flow to
  the receiver. An example is a query for `<data>.receiver.example`.
- The rule: the server decides on the name first. A name that the policy does
  not allow gets NXDOMAIN, and the host does not resolve it.
- CubeSandbox's Track mode lets every name resolve and only learns the
  configured names. Its query names stay an open channel (READ:
  `TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:docs/guide/network-policy.md:401-403`).
- OpenSandbox answers NXDOMAIN for denied names (READ:
  `opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/components/egress.md:32-42`).
- A wildcard rule opens the channel again for its zone. With `*.example.com`
  allowed, the guest can put data in labels under example.com.
- `open` mode resolves every name. DNS is an open channel in that mode.

### What the server answers

The name check comes first, for every query type. A name that the policy does
not allow gets NXDOMAIN.

| query, for an allowed name | answer |
| --- | --- |
| A | the host's A answer without reserved addresses; each address is recorded |
| A, for `<binding>.broker.internal` | the gateway address, for a binding of this instance only |
| AAAA | no records (NOERROR, empty) while the guest has no IPv6 (proposal) |
| HTTPS (type 65) or SVCB (type 64) | no records (NOERROR, empty) |
| any other type | no records in v1 (proposal) |

Rules for the answers:

- HTTPS and SVCB records carry ECH configuration and other endpoints. ECH
  hides the real SNI from any SNI check. With no such record, the client
  connects without ECH.
- An answer drops each reserved address unless an address rule admits it. If
  no address remains, the guest gets NXDOMAIN and the log says why.
- Dropping reserved addresses also stops DNS rebinding. An allowed name that
  resolves to 127.0.0.1 gives the guest nothing.
- Proposal: the host follows a CNAME chain itself. The guest gets A records
  under the name it asked for, and the record keeps that allowed name.
- Proposal: the record expiry is the upstream TTL plus a margin, with a lower
  and an upper clamp. OpenSandbox clamps its lease to 60-360 s (READ, same
  file as above).
- The guest gets a TTL that ends before the record expiry. So the guest cache
  expires first.
- T-122 adds a query type only when a real workload needs it.

### Queries and transport

- The server answers over UDP and TCP.
- The server answers concurrent A and AAAA queries on one socket, as glibc
  sends them. CubeSandbox has an open report where concurrent lookups fail
  ([issue 1406](https://github.com/TencentCloud/CubeSandbox/issues/1406)).
- Proposal: the host resolves through its own system resolver, so split DNS
  and VPNs on the host work. Open question for T-122: a configured upstream
  resolver as an option.
- The guest never learns which resolver the host uses. errand gave its
  sessions a synthetic resolver for this reason, because the host's file names
  the operator's ISP or private network (READ:
  `QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/policy.ts:74-93`).

### Records and logs

- For every answer the server stores a record: instance, name, address and
  expiry. The flow policy reads these records.
- Records belong to one instance. An answer to one instance never admits a
  flow of another.
- The server logs every query: instance, name, type, verdict, the addresses
  answered and the time.
- Proposal: the log has a size bound per instance. T-122 sets it.
- The server never renews a record in the background. A record ends at its
  expiry, and the guest asks again.
- OpenSandbox renews its leases in the background and needed fail-closed rules
  for that, merged in PR 1804 (READ:
  `opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/components/egress.md:46-48`).

### Other resolvers

- A flow to port 53 or port 853 at any address other than the gateway is
  refused, in every mode.
- OpenSandbox drops DNS over TLS on port 853 in the same way (READ:
  `opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/components/egress.md:406-413`).
- DNS over HTTPS to an allowed name looks like any other HTTPS flow. The
  netstack cannot tell it apart.

## The flow policy (T-123)

### Modes

| mode | what a new flow can reach |
| --- | --- |
| `deny` | the gateway services only: DNS and the broker endpoints |
| `allowlist` (the default) | the gateway services, plus destinations that match a rule |
| `open` | the gateway services, plus any public address that the gateway DNS answered |

NOTE: `open` mode still applies the answered-address rule and the reserved
ranges. A flow to an address that DNS did not answer is refused in `open` mode
too.

### Rules

The syntax below is an illustration. T-027 and T-123 fix the real syntax. A
policy is a named table of the config file, and a profile names its policy
([cli.md](cli.md)).

```toml
[policies.dev]
mode = "allowlist"

# an exact name: matches crates.io only
[[policies.dev.allow]]
host = "crates.io"
ports = [443]

# a wildcard: matches every name under crates.io, never crates.io itself
[[policies.dev.allow]]
host = "*.crates.io"
ports = [443]

# UDP must be named; a rule without proto is TCP
[[policies.dev.allow]]
host = "time.example.org"
ports = [123]
proto = "udp"

# an address rule: the only way to an address the guest did not resolve
[[policies.dev.allow]]
cidr = "192.168.1.20/32"
ports = [5432]
```

Rule semantics:

- A rule names a destination by DNS name (`host`), or by address or CIDR
  (`cidr`). It also names a port set and a protocol.
- A rule names a destination, not only a port. Port rules cannot tell one
  host on 443 from another (READ:
  `QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:docs/guide/network.md:101-104`).
- A rule without a protocol is TCP. So QUIC and other UDP need a rule that
  names UDP.
- Names compare case-folded. A wildcard `*.example.com` matches any name under
  example.com and never example.com itself.
- A name rule admits a flow only to an address that the gateway DNS answered
  for a matching name.
- An address rule admits a flow to that address with no DNS answer. It is the
  only way into a reserved range.
- No rule can admit loopback or a metadata address (see the reserved ranges).
- The default policy decides kage's own destinations: html.duckduckgo.com for
  web_search, Brave, web_fetch targets, MCP servers and models.dev (T-163).
- kage needs those destinations beyond the model providers (READ:
  `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-tools/src/builtin/web_search.rs:195`).

### Evaluation of a new flow

The netstack runs these steps in order on the first packet of a flow. The
first refusal ends the evaluation.

1. Identity: the instance is the one whose listener carried the frame.
2. Parse: a packet that does not parse is dropped.
3. Gateway: a flow to the gateway reaches DNS on port 53 or the broker on TCP
   port 80. Any other gateway port is refused.
4. Plan: a flow to any other address of the address plan is refused.
5. Mode `deny`: every flow that is not to a gateway service is refused.
6. Resolver ports: a flow to port 53 or 853 elsewhere is refused.
7. Reserved ranges: a reserved destination is refused unless an address rule
   names it. Loopback and metadata addresses are always refused. This step
   runs after resolution and before every allow.
8. Answered address: the destination must be in a live DNS record of this
   instance, or match an address rule. Otherwise the flow is refused.
9. Allow: in `allowlist` mode, the record's name or the address rule must
   match a rule for this port and protocol. In `open` mode, any name passes.
10. Admit: the netstack stamps the flow with the current policy generation and
    the name. Then host connect starts (T-124).

If an address is in the records of more than one name, one allowed name is
enough. The flow stores that name.

### Why answered-address binding closes the old question

The old design asked how to bind a destination address that the guest chose
to an allowed name. CubeSandbox reported that a matching Host or SNI does not
prove the address belongs to the name. With `curl --resolve` or a hosts entry,
all four HTTP and HTTPS cases returned 200 ([issue
1697](https://github.com/TencentCloud/CubeSandbox/issues/1697) and [PR
1695](https://github.com/TencentCloud/CubeSandbox/pull/1695), both open).

The netstack closes it by construction:

- The netstack is the guest's only resolver. It knows which addresses it
  answered, to which instance, for which name.
- A hosts entry or `curl --resolve` keeps the allowed name and moves the
  address. The new address has no record, so the flow is refused.
- The guest and the netstack share one view of DNS. So the old cost of a
  membership check on the proxy side does not arise. That cost was false
  denials under CDN rotation and split DNS.
- For credentialed traffic the guest chooses no address at all. The binding
  fixes the upstream host, and the broker resolves it on the host
  ([broker.md](broker.md)).

What it does not close, so that nobody records the question as fully solved:

- Shared addresses. A flow to an answered address can carry any SNI or Host.
  If an allowed name shares a CDN address with another name, the guest
  reaches both.
- The netstack works at layer 4 and does not read SNI in v1.
- A guest that keeps an address after its record expires gets new flows
  refused. Established flows go on.
- DNS policy alone is not an egress boundary, because a flow to a literal
  address skips DNS. OpenSandbox refuses to start its credential vault with
  DNS filtering only (READ:
  `opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/guides/credential-vault.md:20-25`).
  Here the answered-address rule is the IP-layer half.
- T-123 keeps three alternatives: a membership check by the proxy, a dial to
  the original address, and a host lookup per flow.
- A new host lookup per flow gives a new address, which must pass the whole
  policy again.

### Reserved ranges

The netstack refuses these destinations after resolution and before every
allow. The last column says whether an address rule can admit the range.

| range | what it is | an address rule can admit it |
| --- | --- | --- |
| 0.0.0.0/8 | "this network" | yes |
| 10.0.0.0/8 | RFC 1918 private | yes |
| 100.64.0.0/10 | CGNAT shared space | yes |
| 127.0.0.0/8 | loopback | no |
| 169.254.169.254/32 | cloud metadata service (IMDS) | no |
| 169.254.0.0/16 | link-local | yes |
| 172.16.0.0/12 | RFC 1918 private | yes |
| 192.168.0.0/16 | RFC 1918 private | yes |
| 224.0.0.0/4 | multicast | no |
| 240.0.0.0/4 | reserved, broadcast included | no |
| ::/128 and ::1/128 | unspecified and loopback | no |
| ::ffff:0:0/96 | IPv4-mapped | judged by the embedded IPv4 address |
| 64:ff9b::/96 | NAT64 | yes |
| fd00:ec2::254/128 | cloud metadata service on IPv6 | no |
| fc00::/7 | unique local | yes |
| fe80::/10 | link-local | yes |
| ff00::/8 | multicast | no |
| each address of each host interface | the host itself | yes, and the startup report names the rule |
| 10.200.0.0/24 | the address plan | no; the gateway services are local |

The IPv6 rows apply to host connect now, and to the guest when it gets IPv6.
T-123 adds the other ranges of the IANA special-purpose registries, for
example the documentation, benchmarking, Teredo, 6to4 and discard-only
ranges.

Sources for the list:

- The brief lists loopback, link-local with IMDS, RFC 1918, CGNAT, the host's
  own addresses and multicast.
- agent-vault adds 0.0.0.0, fc00::/7 and fe80::/10, and keeps IMDS blocked even
  when an allowlist names it (READ:
  `Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/netguard/netguard.go:93-124`).
- CubeSandbox's private denies have no 100.64/10 and no IPv6. Its allow
  entries come before its deny entries, so any allow overrides them (READ:
  `TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:docs/architecture/network.md:193-204`).
- The agent-sandbox default policy blocks RFC 1918 and cloud metadata as its
  floor (READ:
  `kubernetes-sigs/agent-sandbox@4b63868f0d67e4ee0fa690bb14cb5722b78d679d:docs/security/threat_model.md:65`).

Design decision of this page, for T-123 to record: no rule can admit loopback
or a metadata address. The brief allows an explicit rule for every reserved
range. This page narrows that, for two reasons:

- the 127/8 lesson in the section below;
- agent-vault's rule that IMDS stays blocked even when an allowlist names it.

More rules for the list:

- The host's own addresses change with DHCP and VPNs. The netstack reads them
  at start and again on each address change the host reports (T-124).
- A cloud host can have platform services outside these ranges. The operator
  can add ranges, and the startup report prints the active list.

### Generation stamps and revocation

- Each instance has a policy generation, a counter. A policy change increments
  it after the netstack and the broker both hold the new policy.
- Each flow carries the generation it was admitted under.
- On the next packet of a flow with an older generation, the netstack
  evaluates the flow again, once, with its stored name.
- An allowed flow takes the new generation. A refused flow ends.
- A refused TCP flow gets a RST toward the guest, and its host socket closes.
  A refused UDP flow is dropped.
- A changed verdict ends a flow. It never moves a flow to another
  destination.

Source: CubeSandbox [PR 1399](https://github.com/TencentCloud/CubeSandbox/pull/1399),
merged (READ:
`TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:docs/guide/network-policy.md:280-296`).
The PR contrasts this with a firewall chain that accepts established
connections first. In such a chain a tighter policy does not touch open
connections.

### DNS expiry gates new flows only

- A record expiry stops new flows to that address. It never resets an
  established flow.
- CubeSandbox [PR 763](https://github.com/TencentCloud/CubeSandbox/pull/763)
  describes the bug where an expired entry killed long-lived TCP connections.
  It closed without merge. Verdict caching per session in PRs 1347 and 1399,
  both merged, removed the bug (READ:
  `TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:docs/architecture/network.md:134-154`).
- A policy change is different. It evaluates established flows again, with
  the name each flow stored.

## Host connect (T-124)

- One connect function on the host serves guest flows and broker upstreams.
  So every path out reaches the same checks.
- TEMPLATE records that a control on one of several paths into an operation
  is the most frequent hole (READ:
  `Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/conventions/code.md:11-22`).
- A guest flow dials the one literal address that the guest chose, after the
  checks above. There is no second lookup.
- A broker upstream resolves the binding's fixed host on the host. If any
  answer is reserved, the broker refuses the whole name.
- Then it dials a validated literal. agent-vault's dial guard works this way
  and so defeats DNS rebinding (READ, the netguard file above).
- agent-vault dials only its first validated address. Its open [PR
  374](https://github.com/Infisical/agent-vault/pull/374) adds a fallback.
- Happy eyeballs (RFC 8305) applies where the host has more than one validated
  address for a name. That is the broker upstream case.
- The reserved-range check runs again at connect time, on the literal that the
  host dials. The host's own addresses can change between admission and
  connect.
- Proposal: a connect timeout of 10 s. CubeSandbox keeps 10 s for its proxy
  connect (READ:
  `TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:CubeEgress/nginx.conf:126-128`).
  Not measured here.
- Every instance leaves from the host's own address ([limits.md](limits.md)).

## Inbound forwards (T-125)

```sh
agent-box forward NAME [ADDR:]HOSTPORT:GUESTPORT
```

- The daemon listens on ADDR:HOSTPORT on the host. ADDR is 127.0.0.1 when the
  operator gives none.
- For each connection it accepts, the netstack opens a TCP flow into the guest
  to 10.200.0.2:GUESTPORT.
- Proposal: the guest sees the gateway as the peer. It never sees the address
  of the host client.
- A bind to an address other than loopback needs an explicit flag. The startup
  report and `agent-box inspect` list such a bind.
- A forward is not egress. It gives the guest no new outbound destination.
  Replies inside a forwarded flow pass.
- Proposal: forwards are part of the instance record. The daemon arms them at
  start and after its own restart, and removes them at delete.

WARNING: A forwarded port serves content that the guest controls. Every local
process can connect to it. A page from the guest that the operator opens in a
browser runs in a localhost origin, next to other local services.

## Deny first (T-126)

At each start, and after each daemon restart, the steps run in this order.
[protocol.md](protocol.md), section "Boot order and deny first", gives the
messages of each step.

1. The daemon provisions the egress token, the nonces and the event nonce on
   port 5000.
2. box-guest connects to port 5003 with the egress token. The daemon checks
   the token, then reads nothing more from that channel.
3. The daemon loads the policy and the bindings of the instance into the
   netstack and the broker. It loads the credentials from the store.
4. The daemon sends `NET_READY` on port 5003, and box-guest brings `tun0`
   up.
5. The daemon sends `READY` on port 5000, and box-guest starts kage.

Rules:

- An instance has no egress until its policy and bindings are live. This is
  the rule R-DENY-FIRST of [architecture.md](architecture.md).
- The daemon reads no packet from port 5003 before step 3 is complete. Only
  the step that installs the policy can open the channel for packets (T-126).
- After a daemon restart every running instance goes through the steps again.
  kage is still running. It sees the network go away and come back, and its
  provider calls retry.
- T-126 proves the order with a test that races the network channel against
  provisioning. No packet leaves before "ready".

## SSH from the guest (T-257)

- ssh in the guest is a TCP flow like any other. The flow policy decides
  whether it reaches its host.
- The guest holds no SSH key, by the rule R-NO-SECRET-IN-GUEST. So until
  T-257 lands, ssh reaches only hosts that need no credential.
- T-257 gives the guest an SSH agent socket. box-guest bridges it over vsock
  port 5005 to an agent in the daemon ([broker.md](broker.md)).
- The daemon signs only with a key bound to the instance. It signs only
  while the netstack holds a flow from that instance to an allowed host.
- git remotes in SSH form go to the broker over HTTP instead, through the
  `insteadOf` rules of [broker.md](broker.md).

## Never map the guest's 127/8 to the host loopback

- pasta forwards a namespace's own 127.0.0.0/8 to the host loopback. bailey
  installs a default-drop output chain that drops 127/8 and ::1, so a session
  cannot reach host services (READ:
  `QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:crates/bailey/src/cli.rs:456-475`).
- podman's default pasta mapping answered at 169.254.1.2 (READ, malaria
  06-default-network.json at the pin above).
- Rule: the netstack has no implicit host mapping. No guest destination
  reaches a socket on the host loopback.
- The gateway serves DNS and the broker inside the netstack. It forwards
  nothing to the host.
- The guest's own 127/8 stays inside the guest kernel. A packet to 127/8 that
  arrives on tun0 anyway is refused.
- T-180 checks this: host reachability by TCP, UDP and name, with a positive
  control so that a dead listener does not pass.

## Parser confinement and fuzzing (T-127, T-183)

- The netstack parses frames, IP, TCP, UDP and DNS from the guest. This is the
  largest guest-controlled input on the host.
- Until T-127, that parser runs in the daemon process, next to the credential
  store. A parser defect that the guest can exploit would reach the
  credentials. [limits.md](limits.md) lists this.
- T-127 moves the parser into a separate process with no network. On Linux it
  uses Landlock plus seccomp; on Windows, a restricted token.
- The process that holds credentials and dials upstream stays separate from
  the parser.
- bailey shows why the split matters. A process with no network gets an
  isolated mode where every protocol fails. A process with egress can only be
  limited by TCP port (READ:
  `QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:crates/bailey/src/backend/network.rs:45-59`).
- A seccomp filter checks the architecture and refuses the x32 ABI. bailey
  measured i386 and x32 bypasses of a denylist and fixed them (READ:
  `QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:crates/bailey/src/backend/enforce.rs:980-988`).
- cargo-fuzz targets cover frames, IP, TCP and DNS (T-127). The HTTP head
  parser of the broker has its own target (T-183). CI runs each target for a
  fixed time.

## Threat model of the netstack (T-182 completes it)

| item | statement |
| --- | --- |
| attacker | everything in the guest: the agent, its tools, the guest kernel, root in the guest |
| assets | host loopback services, the host LAN, other instances, the credential store, host CPU and memory |
| boundary | the network channel on port 5003; every frame on it is untrusted |
| controls | listener identity, the egress token, deny first, answered-address binding, reserved ranges, generation stamps, bounds per instance |
| residual risk | shared CDN addresses, DNS labels in wildcard zones, `open` mode, and the parser in the daemon process until T-127 |

## Entries

| entry | title |
| --- | --- |
| T-045 | Guest network: the tun device and the network channel |
| T-120 | Choose the userspace TCP/IP stack |
| T-121 | Host netstack per instance |
| T-122 | DNS on the gateway |
| T-123 | Flow policy engine |
| T-124 | Host-side connect |
| T-125 | Inbound forwards |
| T-126 | Deny first: no egress before policy, no kage before egress |
| T-127 | Confine and fuzz the packet parser |
| T-163 | Default egress for kage's own destinations |
| T-180 | The containment battery and its negative control |
| T-186 | Instance-to-instance isolation |
| T-257 | SSH from the guest through a host-held key |
