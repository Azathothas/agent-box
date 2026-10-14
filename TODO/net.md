# Network

This area covers the network path of an instance. The guest has a tun device,
`tun0`. box-guest carries its IP packets as frames on vsock port 5003. The
daemon turns the frames into flows in a userspace netstack, one per instance.
The netstack also serves DNS and the broker endpoints on the gateway address.
The design is in [network](../docs/network.md). The frames and ports are in
[protocol](../docs/protocol.md). The guest side of the network is T-045 in
[guest](guest.md). Credentialed traffic goes to the broker, in
[broker](broker.md).

Every entry here follows two rules of the design. The
host never maps the guest's view of 127/8 to the host loopback. An instance has
no egress until its policy and bindings are live in the daemon (T-126).

# T-120: Choose the userspace TCP/IP stack

**Source:** Planning session 2026-10-14. [network](../docs/network.md) names
three candidates and leaves the choice to a measurement.
**Category:** measurement
**Milestone:** M5
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

The daemon turns guest IP packets into host flows. The stack that does this
parses every byte the guest sends, and it carries every flow of every
instance. A bad choice costs throughput, host CPU or a defect in guest-facing
code. T-121 builds on the choice, so a later change is expensive.

## Premise

- Design: the candidates are smoltcp, ipstack and netstack-smoltcp. Nobody
  measured them for this project. Their licences and maintenance state were
  not read.
- Design: frames have a u32 length, a u8 type and a payload of at most
  `MAX_FRAME` (8 MiB). The framing does not limit the MTU. The cost per frame
  decides it.
- Design: T-127 later moves the guest-facing parser into a confined process.
  A stack with a poll model (no sockets and no threads inside it) can move
  without a rewrite.
- READ: kage opens a new TCP and TLS connection for each provider request
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/http.rs:58-70`).
  Connection setup rate matters as much as bulk throughput.
- READ: a userspace netstack can map the host by accident. pasta forwards the
  namespace's 127/8 to the host loopback
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:crates/bailey/src/cli.rs:456-475`).

## Approach

1. Write the decision rules into this entry before any run:
   - Refuse a candidate whose licence `deny.toml` refuses.
   - Refuse a candidate that opens sockets or starts threads inside the
     stack (T-127 needs a poll model).
   - Refuse a candidate without a raw IP medium (tun gives IP packets, not
     Ethernet frames).
   - Among the rest, pick the highest throughput at equal CPU. A tie goes to
     the smaller dependency tree.
2. Read these facts for each candidate, at the version measured:
   - the licence, the last release date and the open issue count;
   - the count of `unsafe` blocks;
   - support for IPv4, UDP, TCP window scaling and ICMP error generation.
3. Write a benchmark harness under .work/netstack-bench/ (not shipped). It
   puts a Linux tun device in a network namespace and feeds its packets to
   each candidate through the box-proto frame codec.
4. Measure each candidate at MTU 1500, 9000 and 65520 (candidate values, not
   results):
   - one TCP stream, namespace to host and host to namespace (iperf3);
   - daemon CPU time for each GiB moved;
   - 1000 sequential connections, each sending 1 KiB, for the setup rate;
   - resident memory for each idle established flow.
5. Run each case 5 times. Record the median and the spread, with the host,
   kernel, CPU, candidate versions and date, in [status](../docs/status.md).
6. Write a Decision section in this entry: the stack, its version and the
   default MTU. Add a row to [decisions](../docs/decisions.md). Until then,
   the default MTU is 1500.

## Prove

```sh
sudo sh .work/netstack-bench/run.sh --runs 5 --mtu 1500,9000,65520
cargo xtask check docs
```

Expected: a results table in [status](../docs/status.md) with the conditions
of step 5. This entry has a Decision section, and `cargo xtask check docs`
exits 0. Host: a Linux host as root, with `/dev/net/tun`.

# T-121: Host netstack per instance

**Source:** Operator ruling D5 (2026-10-14) and [network](../docs/network.md).
The identity rule comes from the OpenSandbox and CubeSandbox findings below.
**Category:** feature
**Milestone:** M5
**Priority:** P1
**Effort:** L
**Status:** open

## Problem

The guest has a NIC, and every packet it sends ends in the daemon. The daemon
must turn packets into flows under policy, for each instance separately. A
shared table or a guest-supplied identity lets one instance act for another.
An unbounded buffer lets one instance exhaust the daemon.

## Premise

- Design: frames ride on vsock port 5003, guest to host. The first frame
  carries the per-instance egress token, 32 bytes from the host CSPRNG
  (T-022).
- READ: an identity that the guest supplies can be forged. The OpenSandbox
  Firecracker profile selects the subject by the guest source address
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:components/egress/pkg/fastsandboxnft/fastsandboxnft.go:36-41`).
  Its smoke test forges a sibling's address
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:components/egress/tests/smoke-fast-sandbox.sh:588-601`).
- READ: CubeSandbox gives every guest the same internal address and takes
  identity from the host-side device
  (`TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:docs/architecture/network.md:23-27`).
- READ: CubeSandbox times out a flow by its TCP state. SYN is 1 minute,
  ESTABLISHED is 3 hours, FIN_WAIT and TIME_WAIT are 2 minutes, UDP is 30 s
  unreplied and 180 s replied
  (`TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:docs/architecture/network.md:140-150`).
- READ: kage allows 600 s of silence while it reads a provider response body
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-provider/src/http.rs:51`).
  A flow idle timeout at or below 600 s cuts a live model call.
- READ: bailey gives the confined side a synthetic address, never the host's
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:crates/bailey/src/cli.rs:404-412`).

## Approach

1. Crate box-net (layer 2). Files: crates/box-net/src/lib.rs,
   crates/box-net/src/instance.rs, crates/box-net/src/flow.rs,
   crates/box-net/src/gateway.rs and crates/box-net/src/driver.rs.
2. Make the core sans-IO. `InstanceNet` owns the stack of T-120, the flow
   table, the DNS state of T-122 and the policy view of T-123.
   - Inputs: guest frames, host socket events and timer ticks.
   - Outputs: guest frames, host connect requests and host writes.
   - The tokio driver in crates/box-net/src/driver.rs does all IO.
3. Network channel. The daemon listens on port 5003 for each instance through
   box-transport. The first frame must carry the egress token.
   - Compare the token in constant time. A missing or wrong token closes the
     channel and adds one to the refused-forgery count.
   - The instance is the one whose listener accepted the connection. The
     netstack never takes identity from an IP address or a header.
   - One live channel for each instance. A new authenticated channel
     replaces the old one and resets its flows (box-guest reconnects after a
     daemon restart).
4. Address plan (Decision below). Drop and count a packet whose source is not
   the guest address. Drop and count every IPv6 packet in v1.
5. TCP. The netstack ends the guest's TCP connection and opens a host
   connection for it.
   - Judge each SYN with T-123. A refused SYN gets RST at once.
   - Send SYN-ACK only after the host connect of T-124 succeeds. A failed
     host connect gets RST.
   - Gateway port 53 goes to DNS (T-122). Gateway port 80 goes to the broker
     through a `GatewayService` trait. Every other gateway port gets RST.
   - box-daemon implements `GatewayService` with box-broker, so box-net does
     not depend on box-broker.
6. UDP. Gateway port 53 goes to DNS. Every other datagram goes to T-123.
   - Allowed: one host UDP socket for each guest port and destination pair.
   - Refused: ICMP port unreachable to the guest, so a QUIC client falls back
     to TCP at once. Count it.
7. ICMP. Answer an echo request to the gateway locally. Drop and count every
   other ICMP packet from the guest in v1. State this in
   [limits](../docs/limits.md).
8. Flow timeouts: the CubeSandbox values of the premise. A unit test fails
   when the established idle timeout is 600 s or less.
9. Bounds for each instance, all in the configuration:
   - `net.max_flows`, design default 4096, not measured. A SYN above it gets
     RST and is counted.
   - a byte budget for flow buffers. When the guest window is full, the
     driver stops reading the host socket.
10. No state is shared between instances. Each `InstanceNet` is its own value
    and its own task. There is no global flow table.
11. Counters for each instance: frames, bytes, flows by verdict, refused
    forgeries and spoofed sources. T-175 shows them in the startup report.
12. Stop and daemon restart reset every flow and close every host socket.
    After a restart the channel goes through T-126 again.

## Decision

The address plan, the same for every instance, because identity comes from
the channel:

| Item | Value |
| --- | --- |
| Guest address | `10.200.0.2/24` on `tun0` |
| Gateway | `10.200.0.1`: DNS on port 53, the broker on port 80 |
| DNS server in the guest | `10.200.0.1` only |
| IPv6 in the guest | none in v1 |
| MTU | 1500 until T-120 decides |

T-045 configures these values in the guest. They are v1 constants of
box-proto, as [docs/protocol.md](../docs/protocol.md) gives. `PROVISION` has
no field for them. A change of this table needs a new protocol version.

## Prove

```sh
cargo test -p box-net
cargo xtask linux --kvm -- cargo test -p box-daemon --test net_live -- --ignored
```

Expected: the unit tests pass, with named tests for each of these:

- a wrong egress token is refused and counted;
- a spoofed source address is dropped and counted;
- two instances with the same guest address have separate flows;
- a host connect that fails gives the guest RST;
- the established idle timeout is above 600 s.

The live test sets a resolver stub that answers `allowed.test` with a local
host address, and an operator rule for that address. In the guest,
`curl http://allowed.test:PORT/` returns the body. `curl http://denied.test/`
fails to resolve. Host: a Linux KVM host. On Windows the dev host runs it
nested through `cargo xtask linux --kvm`, for development only. Windows 11
with Hyper-V as admin runs the same test against the HCS engine.

## Start condition

T-120 and T-045.

# T-122: DNS on the gateway

**Source:** [network](../docs/network.md). The OpenSandbox, CubeSandbox and
errand findings below.
**Category:** feature
**Milestone:** M5
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The gateway is the guest's only resolver. It decides which names exist for an
instance. A resolver that forwards every name is an exfiltration channel
through the query names. Its answers are also the input of the
answered-address binding of T-123.

## Premise

- READ: OpenSandbox answers NXDOMAIN for a denied name at its DNS layer
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/components/egress.md:32-36`).
- READ: OpenSandbox gives an allowed address a lease of the DNS TTL plus a
  margin, clamped to 60 to 360 s
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/components/egress.md:44`).
- READ: its revalidation never adds an address the client did not receive.
  A failed or truncated answer does not renew. A policy replacement clears
  tracking and invalidates lookups in flight
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/components/egress.md:48`).
  Its bounds are 128 domains, 64 addresses per domain and 5 s per lookup.
- READ: CubeSandbox learns names and does not police them. In Track mode
  every name still resolves, so query names stay an exfiltration channel
  (`TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:docs/guide/network-policy.md:401-403`).
- READ: errand gives the session a synthetic resolver and never shows it the
  host's resolver configuration
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/policy.ts:74-93`).
- Reported by the relevance pass, not read here: concurrent A and AAAA
  queries on one socket failed in CubeSandbox
  (https://github.com/TencentCloud/CubeSandbox/issues/1406).
- Design: HTTPS and SVCB records (types 65 and 64) carry ECH configuration
  and other endpoints. The netstack never returns them.

## Approach

1. Files: crates/box-net/src/dns/mod.rs (server), crates/box-net/src/dns/wire.rs
   (query parser and answer writer), crates/box-net/src/dns/records.rs (the
   answered-address table).
2. Parse queries with a strict hand-written parser. T-127 fuzzes it.
   - QDCOUNT is 1. ANCOUNT and NSCOUNT are 0.
   - ARCOUNT is 0 or 1, and the one record is an EDNS0 OPT record.
   - QCLASS is IN. Anything else gets FORMERR.
3. Normalize the name: lower case, no final dot, at most 253 bytes. Each
   label has 1 to 63 bytes of letters, digits, `-` or `_`. Anything else gets
   FORMERR.
4. Serve UDP and TCP on gateway port 53. A UDP answer fits in 512 bytes, or
   in the EDNS0 size of the query capped at 1232. A larger answer sets TC.
5. Decide each query in this order:

   | Query | Answer |
   | --- | --- |
   | `<binding>.broker.internal`, a binding of this instance | A `10.200.0.1` |
   | any other name under `broker.internal` | NXDOMAIN |
   | a name the policy refuses (T-123) | NXDOMAIN |
   | A for an allowed name | the host lookup, filtered (step 6) |
   | AAAA for an allowed name | NOERROR, no records (no IPv6 in v1) |
   | HTTPS (65) or SVCB (64) | NOERROR, no records, logged as refused |
   | ANY (255) | NOTIMP |
   | any other type for an allowed name | NOERROR, no records |

6. Resolve allowed names on the host with hickory-resolver and the host's
   system configuration. The guest never sees the host resolver.
   - Remove every address in a reserved range of T-123, unless an operator
     rule names it. Log the name and each removed address.
   - Follow CNAME chains on the host. Answer the queried name with the final
     addresses, and never return a CNAME record.
   - A host lookup that fails or times out after 5 s gets SERVFAIL. Never
     answer from an expired record.
7. Records. For each address answered, store instance, name, address and
   expiry.
   - The lease is the upstream TTL plus 30 s (a design value), clamped to 60
     to 360 s.
   - The answer TTL is the lease minus 30 s. So the guest cache expires
     before the record does, as [docs/network.md](../docs/network.md#what-the-server-answers)
     gives.
   - Bounds: 1024 names and 64 addresses for each name, for each instance.
     The first number is a design default, not measured.
   - When the table is full, drop the oldest expired record. With none
     expired, answer SERVFAIL and count it.
8. No background lookups in v1. A new guest query renews a lease.
9. A policy replacement removes the records that the new policy refuses. A
   lookup started under an older generation writes no record (T-123).
10. Answer concurrent queries on one socket in any order. glibc sends A and
    AAAA together from one source port.
11. Log every query to the event log of T-176 as host-observed: time, name,
    type, verdict and answer count.

## Prove

```sh
cargo test -p box-net dns::
```

Expected: exit 0, with named tests for each of these:

- a refused name gets NXDOMAIN and writes no record;
- an allowed name gets A records and writes one record for each address;
- the answer TTL is 30 s shorter than the lease of its record;
- a reserved address in a host answer is removed and logged;
- HTTPS and SVCB queries get no records;
- a CNAME chain is answered as A records under the queried name;
- an unknown `x.broker.internal` gets NXDOMAIN;
- A and AAAA sent together on one socket both get answers;
- a malformed query (QDCOUNT 2, a 64-byte label) gets FORMERR;
- a lookup that started before a policy change writes no record.

Host: any.

# T-123: Flow policy engine

**Source:** [network](../docs/network.md). The CubeSandbox, agent-vault,
OpenSandbox and bailey findings below. The policy, destination-address and
bypass entries of the earlier plan map here.
**Category:** feature
**Milestone:** M5
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The policy decides which flows leave the host. Three classic holes exist. An
address that no allowed name resolved to. A reserved address reached through
an allow rule. A tightened policy that open flows ignore.

## Premise

- READ: CubeSandbox evaluates allow, then deny, then default-allow. Its
  private-range denies lose to any explicit or DNS-learned allow
  (`TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:docs/architecture/network.md:193-204`).
  Its list has no 100.64/10 and no IPv6.
- READ: a matching name does not prove that the destination address belongs
  to it. The CubeSandbox report is open
  (https://github.com/TencentCloud/CubeSandbox/issues/1697).
- Design: the gateway is the guest's only resolver, so the netstack knows
  every address it answered to each instance. A flow to any other address is
  refused. This answers the open question for this project.
- READ: CubeSandbox stamps each flow with a policy generation, bumped only
  after both planes hold the new policy. A revoked TCP flow gets RST
  (`TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:docs/guide/network-policy.md:280-296`).
  The merged change is https://github.com/TencentCloud/CubeSandbox/pull/1399.
- READ: an expired DNS entry must not reset an established flow. The unmerged
  https://github.com/TencentCloud/CubeSandbox/pull/763 describes that defect.
- READ: agent-vault blocks 169.254.169.254 and `fd00:ec2::254` even when
  allowlisted, and blocks private, loopback, link-local and CGNAT ranges
  (`Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/netguard/netguard.go:93-124`).
- READ: DNS filtering alone is not an egress boundary, because a direct
  address connect skips it
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/guides/credential-vault.md:20-25`).
- READ: a port rule cannot tell one host on 443 from another
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:docs/guide/network.md:101-104`).
- READ: the agent-sandbox default policy allows the public internet only. It
  blocks RFC 1918 ranges and cloud metadata
  (`kubernetes-sigs/agent-sandbox@4b63868f0d67e4ee0fa690bb14cb5722b78d679d:docs/security/threat_model.md:65`).

## Approach

1. Policy data in box-policy: crates/box-policy/src/egress.rs and
   crates/box-policy/src/reserved.rs. No IO. The TOML shape is the one of
   [docs/network.md](../docs/network.md#rules): a named table
   `[policies.NAME]` in the config file (T-027), and a profile names it.

```toml
[policies.dev]
mode = "allowlist"          # deny | allowlist | open

[[policies.dev.allow]]
host = "crates.io"
ports = [443]

[[policies.dev.allow]]
host = "*.crates.io"        # subdomains only, never crates.io itself
ports = [443]

[[policies.dev.allow]]
cidr = "192.168.1.20/32"    # the only way into a reserved range
ports = [8080]
proto = "tcp"               # tcp (default) | udp
```

2. Validate at load, with an error that names the field:
   - `ports` is required and not empty. Port 0 is refused.
   - A lone `*` host is refused. `*.example.com` matches subdomains only.
   - Names compare after lower case and without a final dot.
   - A `cidr` rule reports in the startup report, because it opens a
     reserved range.
   - A `cidr` rule inside a range that no rule can admit is refused. These
     ranges are loopback, the metadata addresses, multicast, 240.0.0.0/4,
     the unspecified address and the address plan.
3. Engine: crates/box-net/src/policy.rs. The verdict for a new flow takes the
   instance, protocol, destination address, port and generation. The steps
   and their order are those of
   [docs/network.md](../docs/network.md#evaluation-of-a-new-flow). The first
   refusal ends the check:
   1. The gateway: ports 53 and 80 only.
   2. Any other address of the address plan: refuse.
   3. Mode `deny`: refuse every flow that is not to the gateway.
   4. Port 53 or 853 at any address but the gateway: refuse.
   5. A reserved address: refuse, unless a `cidr` rule covers the address,
      port and protocol. Loopback and the metadata addresses stay refused
      under every rule.
   6. Answered-address binding, in every mode: find an unexpired record for
      this instance and address, with a name allowed under the current
      generation. None: refuse. A `cidr` rule needs no record.
   7. The mode. `allowlist`: a rule must match the record's name, the port
      and the protocol. `open`: allow.
4. Reserved ranges, checked after resolution and before every allow (table
   below). For an IPv4-mapped, NAT64 or 6to4 address, check the embedded
   IPv4 address too.
5. The host's own addresses count as reserved. Read them at daemon start and
   on each change (netlink on Linux, `NotifyIpInterfaceChange` on Windows).
   Read them again every 60 s as a fallback.
6. Generation. `policy set` builds the new policy, installs it in the DNS view
   and the flow engine, and then bumps the instance's generation.
   - Each flow keeps the generation it was admitted under.
   - After a bump, judge each flow again on its next segment, one time.
   - Refused: RST to the guest and close the host socket (TCP), or close the
     association (UDP).
7. DNS expiry gates new flows only. An established flow never ends because
   its record expired.
8. Count refusals by reason: gateway port, address plan, mode, resolver port,
   reserved, no record and rule.

| Range | What it is |
| --- | --- |
| `0.0.0.0/8` | this network |
| `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16` | RFC 1918 private |
| `100.64.0.0/10` | CGNAT shared space |
| `127.0.0.0/8` | loopback, no rule admits it |
| `169.254.169.254/32` | cloud metadata (IMDS), no rule admits it |
| `169.254.0.0/16` | link-local |
| `192.0.0.0/24` | IETF protocol assignments |
| `192.0.2.0/24`, `198.51.100.0/24`, `203.0.113.0/24` | documentation |
| `198.18.0.0/15` | benchmarking |
| `224.0.0.0/4` | multicast, no rule admits it |
| `240.0.0.0/4` | reserved, including `255.255.255.255`, no rule admits it |
| `10.200.0.0/24` | the address plan, no rule admits it; the gateway services are local |
| `::/128`, `::1/128` | unspecified and loopback, no rule admits them |
| `::ffff:0:0/96` | IPv4-mapped, check the embedded address |
| `64:ff9b::/96`, `64:ff9b:1::/48` | NAT64, check the embedded address |
| `100::/64` | discard only |
| `2001::/32` | Teredo |
| `2001:db8::/32` | documentation |
| `2002::/16` | 6to4, check the embedded address |
| `fd00:ec2::254/128` | cloud metadata on IPv6, no rule admits it |
| `fc00::/7` | unique local |
| `fe80::/10` | link-local |
| `ff00::/8` | multicast, no rule admits it |
| the host's own addresses | every address of every host interface |

The source of the table is the IANA special-purpose registries
(https://www.iana.org/assignments/iana-ipv4-special-registry/ and
https://www.iana.org/assignments/iana-ipv6-special-registry/), read on the
web 2026-10-14. The agent-vault list of the premise is a subset of it.

## Decision

Not decided here: question Q10 of [PROGRESS](PROGRESS.md) asks the operator.
The design refuses HTTPS and SVCB records for ECH reasons, but nothing reads
the TLS SNI of a flow. Until the operator rules, write the gap of Q10 in
[limits](../docs/limits.md).

## Prove

```sh
cargo test -p box-net policy::
cargo test -p box-policy egress::
```

Expected: exit 0, with named tests for each of these:

- a flow to an address that no allowed name resolved to is refused;
- an allow rule whose name resolves to `10.0.0.5` does not open it;
- a `cidr` rule opens one reserved address and port, and nothing more;
- a `cidr` rule for `127.0.0.1/32` or `169.254.169.254/32` is refused at load;
- `*.example.com` does not match `example.com`;
- `::ffff:127.0.0.1` is refused as loopback;
- a removed rule resets its open TCP flow on the next segment;
- an expired record does not reset an established flow;
- `deny` mode refuses everything except gateway ports 53 and 80;
- a policy without `ports` is refused at load, with the field named.

Host: any.

# T-124: Host-side connect

**Source:** [network](../docs/network.md). The agent-vault and errand
findings below.
**Category:** feature
**Milestone:** M5
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

Two callers open host connections for an instance: the netstack for guest
flows, and the broker for upstreams. A connect that trusts a second DNS
lookup, or that skips the reserved-range check, reaches host-local services.

## Premise

- READ: agent-vault resolves on the host and refuses when any resolved address
  is blocked. It then dials the validated address literal, with no second
  lookup, which defeats DNS rebinding
  (`Infisical/agent-vault@f0cdface45da8f7eca07f737a03537be75f20a71:internal/netguard/netguard.go:137-161`).
  At the pin it dials only the first address.
- READ: errand dials an allowed name without a check of the resolved address.
  A name that resolves to loopback or RFC 1918 reaches host-local services
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/broker.ts:161-199`).
- READ: CubeSandbox keeps the upstream connect timeout at 10 s
  (`TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:CubeEgress/nginx.conf:126-128`).
- READ: "connection refused" and "timed out" are different facts, and a check
  must record which it saw
  (`talaria0101/malaria@499e897e8372a5db0f921542184e3b66ef610e2b:research/podman-machine.md:56-62`).
- Design: happy eyeballs follows RFC 8305
  (https://www.rfc-editor.org/rfc/rfc8305). Its recommended connection
  attempt delay is 250 ms.

## Approach

1. File: crates/box-net/src/connect.rs. Type `HostConnector`, with two calls:
   - `connect_literal(addr, port)` for netstack flows. It checks the
     reserved ranges and the host's own addresses again at connect time.
   - `connect_name(host, port)` for broker upstreams. It resolves with the
     resolver of T-122.
2. For `connect_name`, refuse the whole name when any answer is reserved.
   Log the name and the address. Then dial only the validated literals.
3. Happy eyeballs across the validated literals: alternate the families,
   250 ms between attempts, the first success wins. Cancel the others.
4. Timeout: 10 s for the whole connect. Typed errors: `Refused`,
   `TimedOut`, `Reserved`, `Resolve`. Count each one.
5. Sockets: `TCP_NODELAY` on and keepalive on. No bind to a chosen local
   address.
6. Layering: box-broker defines a trait `UpstreamDialer` in
   crates/box-broker/src/dial.rs. box-daemon passes `HostConnector` into it,
   so box-broker does not depend on box-net.
7. The reserved-range table comes from box-policy (T-123). There is one
   table for both callers.

## Prove

```sh
cargo test -p box-net connect::
```

Expected: exit 0, with a resolver stub and named tests for each of these:

- a name that resolves to `127.0.0.1` is refused as `Reserved`;
- a name with one public and one private answer is refused;
- the dialed address is the validated literal, and no second lookup happens
  (the stub counts lookups);
- with the IPv6 path held without an answer, the IPv4 attempt starts after
  250 ms and wins;
- a closed port gives `Refused` and a held SYN gives `TimedOut` after 10 s
  (a test clock).

Host: any.

# T-125: Inbound forwards

**Source:** Operator ruling D5 (2026-10-14) and [network](../docs/network.md).
The bind-address entry of the earlier plan maps here.
**Category:** feature
**Milestone:** M5
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

An agent runs dev servers in the guest. The operator wants to open them from
the host. The guest has no address the host can route to, so the daemon must
carry the connection. A careless bind exposes the guest to the network.

## Premise

- Design: `agent-box forward NAME [ADDR:]HOSTPORT:GUESTPORT`. The daemon
  listens on the host and opens a TCP flow into the guest.
- Design: the default bind is `127.0.0.1`. A public bind needs an explicit
  flag and is reported.
- Design: the guest address is `10.200.0.2` (T-121). The netstack opens the
  flow from the gateway address.
- Not measured: how many dev servers bind loopback only. A server that binds
  only `127.0.0.1` in the guest is not reachable through a forward.

## Approach

1. Files: crates/box-net/src/forward.rs (the inbound flow) and
   crates/box-daemon/src/forward.rs (listeners and the control API methods
   `forward.add`, `forward.remove` and `forward.list`). T-173 adds the CLI
   verb.
2. Parse the spec in box-policy, type `ForwardSpec`:
   - `HOSTPORT:GUESTPORT`, `ADDR:HOSTPORT:GUESTPORT` or
     `[V6ADDR]:HOSTPORT:GUESTPORT`;
   - ports 1 to 65535;
   - an address other than loopback needs the public-bind flag. This entry
     proposes `--public`. The name in [cli](../docs/cli.md) wins.
3. Keep forwards in the instance record (T-026). They come back after a
   daemon restart and after re-adoption.
4. On accept:
   - instance not `running`: close the connection and log it;
   - else the netstack opens TCP from `10.200.0.1` to `10.200.0.2:GUESTPORT`
     and copies bytes in both directions.
5. An inbound flow does not pass the egress policy. It never opens a host
   connection to anything other than the accepted client.
6. Refuse a host port that another forward holds, with the owner named.
7. Report every forward in `agent-box inspect` and in the startup report. A
   public bind gets a `WARNING:` line.
8. Write in [limits](../docs/limits.md): every local user of the host can
   connect to a loopback forward.

## Prove

```sh
cargo test -p box-policy forward::
cargo xtask linux --kvm -- cargo test -p box-daemon --test forward_live -- --ignored
```

Expected: the unit tests refuse `0.0.0.0:8080:80` without the flag and
accept `8080:80`. The live test uses the `python` variant of T-064. The guest
runs `python3 -m http.server 8000 --bind 0.0.0.0`. A host GET to
`http://127.0.0.1:18000/` through `18000:8000` returns the listing. Host: a
Linux KVM host. Windows 11 with Hyper-V as admin runs the same live test.

# T-126: Deny first: no egress before policy, no kage before egress

**Source:** Planning session 2026-10-14, rule R-DENY-FIRST. The OpenSandbox
and CubeSandbox lifecycle findings below.
**Category:** feature
**Milestone:** M5
**Priority:** P0
**Effort:** S
**Status:** open

## Problem

A guest that sends packets before its policy is live gets the policy of
nothing, or of whatever default the code had. kage started before egress is
ready makes provider calls that fail or that go out unchecked.

## Premise

- READ: an OpenSandbox subject is deny-first from registration until its
  data-plane-ready hook succeeds, so policy delivery "can be late, never
  early-open"
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/components/egress.md:368-374`).
- READ: CubeEgress answers 403 while its policy bootstrap is pending, and
  serves no traffic without policies
  (`TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:CubeEgress/lua/access_phase.lua:44-63`).
- Design: provisioning on port 5000 carries the nonces, the egress token, the
  kage token, the time and the hostname (T-041).
- Design ([docs/protocol.md](../docs/protocol.md), "Boot order and deny
  first"): the daemon binds its port 5003 listener before the handshake.
  After `NET_AUTH`, it reads nothing more until the policy is live.
- Design (same section): box-guest keeps tun0 down until `NET_READY` on port
  5003. It starts kage only on `READY` of port 5000.

## Approach

1. Make the start order in crates/box-daemon/src/start.rs fixed, in the
   order of docs/protocol.md:
   1. the engine starts the instance, and the daemon binds 5002 and 5003;
   2. the handshake and `PROVISION` on port 5000;
   3. box-guest connects to 5002 and 5003, and the daemon checks the first
      frames;
   4. install the policy and bindings: the `InstanceNet` with generation 1,
      and the broker bindings for the instance;
   5. send `NET_READY` with the generation on port 5003;
   6. send `READY` on port 5000.
2. Make the order a type rule. `net_open` takes an `InstalledPolicy` value,
   and only step 4 makes one. Only `net_open` reads frames after `NET_AUTH`.
3. A failure in steps 2 to 6 stops the instance. The start reports the
   failed step by name.
4. box-guest starts kage only after `READY` and after its network channel is
   up (T-046).
5. Daemon restart with running instances (T-090, T-107): read the encrypted
   per-instance secrets, install policy and bindings, and only then send
   `NET_READY`. box-guest reconnects with backoff. kage keeps running, and its
   flows fail until then. State this in [limits](../docs/limits.md).
6. A policy change does not pass through this path. It moves the generation
   (T-123).
7. `AGENT_READY` on port 5005 (T-257) also waits for step 4.

## Prove

```sh
cargo test -p box-daemon deny_first::
```

Expected: exit 0. The test starts a fake guest that sends packets on port
5003 from the first moment. A recording `HostConnector` counts dials. Before
`NET_READY`, no packet frame is read and no dial happens. `NET_READY` comes
after the install. Planted defect: call `net_open` before step 4, and the test
fails. Host: any. The T-180 battery checks the same rule on both engines.

## Start condition

T-121 and T-041.

# T-127: Confine and fuzz the packet parser

**Source:** [network](../docs/network.md), defence in depth. The bailey
findings below.
**Category:** feature
**Milestone:** M8
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

The netstack parses frames, IP, TCP, UDP and DNS that the guest wrote. The
daemon holds the credential store. A parser defect in the daemon process
gives the guest the credentials.

## Premise

- READ: bailey's Isolated mode leaves a process loopback only, with every
  protocol failing. A process that needs egress can only be limited by TCP
  port
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:crates/bailey/src/backend/network.rs:45-59`).
  So split the parser (no network) from the process that holds credentials.
- READ: a seccomp denylist without an architecture check was bypassed through
  i386 and x32 system calls. bailey fixed this in 0.1.3
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:crates/bailey/src/enforce.rs:966-1027`).
- Design: T-121 keeps the netstack core sans-IO, so it can run in another
  process.

## Approach

1. Run the netstack core of each instance in its own child process, the
   hidden subcommand `agent-box netstack`. One process for each instance, so
   a compromise of one does not reach the flows of another (T-186).
2. IPC on a socket pair (Linux) or an anonymous pipe pair (Windows), with a
   small typed message set in box-proto:
   - the child sends `Resolve`, `Connect`, `Data` and `Close`;
   - the daemon applies the policy, makes the host connects and sends the
     results back;
   - the daemon passes the connected port 5003 stream to the child.
3. Linux confinement, before the child reads any guest byte:
   - `PR_SET_NO_NEW_PRIVS`;
   - a seccomp filter (seccompiler) with an architecture check that refuses
     x32 (bit 30). `socket`, `connect`, `bind` and every open of a path
     return `EPERM`;
   - a Landlock ruleset with no file access, and no TCP bind or connect
     where the ABI is 4 or later;
   - an empty environment, no core dumps and a small file descriptor limit.
   - Landlock missing: a note in the startup report. seccomp still applies.
4. Windows confinement: an AppContainer with no capabilities, so no network.
   Add a Job Object that refuses child processes and caps memory. If the
   AppContainer launch fails, use a restricted token at low integrity, and
   report it as a gap.
5. Fuzz targets with cargo-fuzz in crates/box-net/fuzz/: `frame_decode`,
   `ip_ingest`, `tcp_session` (a sequence of segments with a test clock),
   `udp_ingest` and `dns_query` (the T-122 parser).
6. T-183 runs every fuzz target in CI for a fixed time. Add these targets to
   that job. If T-183 is not done, add the job with 300 s for each target.

## Prove

```sh
cargo test -p box-net --test confine -- --ignored
cargo +nightly fuzz run ip_ingest -- -max_total_time=60
```

Expected: in the confine test, the child calls `socket(AF_INET)` and gets
`EPERM`, and it calls `open("/etc/passwd")` and gets `EPERM`. The fuzz run
exits 0. The CI job of step 6 runs each target for its fixed time. Host: a
Linux host for the first two commands. Windows 11 runs the confine test for
the AppContainer case.

## Start condition

T-121 and T-122.
