# Protocol

This page defines two protocols:

- the host-guest wire protocol between the daemon and box-guest, over vsock on six fixed ports;
- the helper protocol between the daemon and the privileged helper.

Both live in the crate box-proto (planned, entry T-033). The helper types are its module `helper`.
[engines.md](engines.md) gives the engines, the helper operations and the helper threat model.
[guest.md](guest.md) gives box-guest, [network.md](network.md) the netstack and [broker.md](broker.md) the broker.

NOTE: Every field layout on this page is design. T-021 and T-022 fix the exact bytes of the host-guest protocol.
T-023 fixes the helper protocol. The golden frames then make the bytes binding.
Nothing is built yet (D14).

## The host-guest protocol

The daemon speaks for the host. box-guest, PID 1 in the guest and running as root, speaks for the guest.
box-proto has no IO and no platform code. It builds for the host targets and for the static musl guest targets.

### Frame layout

```
offset  size  field
0       4     length   u32, big-endian: the number of payload bytes
4       1     type     u8: the message type
5       n     payload  exactly `length` bytes
```

- The length counts the payload only. It does not count the type byte. An empty payload has length 0.
- One connection carries a sequence of frames, except port 5004 after its preamble (see "Port 5004: kage bridge").

### Field encodings

| encoding | bytes |
| --- | --- |
| `u8`, `u16`, `u32`, `u64` | unsigned, big-endian |
| `i32`, `i64` | two's complement, big-endian |
| `bool` | one byte, 0 or 1; another value is an error |
| `[u8; N]` | exactly N bytes, no length prefix |
| `bytes` | a `u32` length, then the bytes |
| `string` | `bytes` that must be valid UTF-8 |
| `list<T>` | a `u16` count, then each item |
| `enum` | a `u8` tag; an unknown tag is an error |
| `raw` | the whole payload, with no inner length; only data messages use it |

Rules:

- Fields follow each other in the order of the message table. There is no padding.
- Bytes after the last field are an error.
- Each `string`, `bytes` and `list` field has its own ceiling in the message table. The decoder checks it before it allocates.

### Ceilings

The guest controls the length prefix, and a `u32` can announce 4 GiB. So each type has a class with a ceiling.

| constant | value | class |
| --- | --- | --- |
| `MAX_HEAD` | 8 KiB (8192 bytes) | head: every structured message |
| `MAX_FRAME` | 8 MiB (8388608 bytes) | data: stream bytes on port 5001 |
| `MAX_PACKET` | 65535 bytes | packet: one IP packet on port 5003 |

`MAX_PACKET` is a design addition. An IP packet cannot be longer, so a larger frame on port 5003 is a protocol error.

How a receiver reads one frame:

1. Read the 5 header bytes. An end of stream inside them, after at least one byte, is the error `truncated`.
2. Look up the type. An unknown type, or a type that the port does not allow, is an error.
3. Compare the length with the ceiling of the class of that type. A larger length closes the connection before any allocation of that size.
4. Read exactly `length` bytes. An earlier end of stream is the error `truncated`.
5. Decode the payload. A decode failure is the error `bad_payload`.

An end of stream between two frames is a normal close.

### Errors

Type `0x7F`, `ERROR`, may appear on every port and in both directions, except as noted for ports 5002, 5003 and 5005.

| field | encoding | ceiling |
| --- | --- | --- |
| `code` | `u16` | see the table below |
| `detail` | `string` | 256 bytes; never a secret |

The sender closes the connection after an `ERROR`. The receiver of a protocol violation never reads further on that connection.

| code | name | meaning |
| --- | --- | --- |
| 1 | `version_mismatch` | the peer speaks another protocol version |
| 2 | `unknown_type` | the type byte is not in the protocol |
| 3 | `type_not_allowed` | the type is not allowed on this port, or not in this state |
| 4 | `too_long` | the length is above the ceiling of the class |
| 5 | `bad_payload` | the payload does not decode, or it has trailing bytes |
| 6 | `not_provisioned` | the request needs provisioning first |
| 7 | `not_ready` | the bridge target does not accept a connection |
| 8 | `busy` | the session limit of the port is reached |
| 9 | `refused` | the request is valid but box-guest refuses it, for example an unknown signal |

### The version rule

- `PROTOCOL_VERSION` is 1, a `u16`.
- The first frame of every connection carries the version of its sender.
- The receiver compares it with its own. A mismatch refuses the session: `ERROR version_mismatch`, then close.
- There is no downgrade and no negotiation.
- The daemon then reports the instance with a gap that names both versions. It does not provision it, so the instance gets no egress and no kage.
- Any change to a layout, a type code, a ceiling, a port or a v1 constant needs a new version and new golden frames.

### Ports

The v1 port set is fixed. Any other numbering, for example the port map of the research document, is superseded.

| port | opened by | first frame | purpose |
| --- | --- | --- | --- |
| 5000 | host | `HELLO` | control: handshake, provisioning, time, rotation, stats, poweroff, fstrim |
| 5001 | host | `OPEN` | one PTY or exec session per connection (`attach`, `run`) |
| 5002 | guest | `EVENT_AUTH` | events and heartbeat |
| 5003 | guest | `NET_AUTH` | network packets |
| 5004 | host | `BRIDGE_OPEN` | the kage serve bridge to guest 127.0.0.1:7433 |
| 5005 | guest | `AGENT_AUTH` | the SSH agent of T-257 |

Port 5005 is in the v1 set, so T-257 needs no new protocol version. Until T-257 lands, nobody listens on it.

How an unknown port is refused:

- box-guest listens on 5000, 5001 and 5004 only. The daemon listens on 5002 and 5003, and on 5005 after T-257.
- A connection to any other port finds no listener and fails at the transport.
- Any guest process may bind a vsock port of 1024 or above. So box-guest binds its three ports before it starts any other process.
- The daemon creates its listeners on 5002 and 5003, and on 5005 after T-257, before it connects to port 5000.
- On 5003 and 5005 it reads nothing after the first frame until the policy and the bindings of the instance are live (T-126).

Connections per port:

| port | rule |
| --- | --- |
| 5000 | one at a time; a new connection from the host replaces the old one |
| 5001 | one session per connection; box-guest refuses with `busy` above its session limit (T-042 sets it) |
| 5002 | one at a time; a new authenticated connection replaces the old one |
| 5003 | one at a time; a new authenticated connection replaces the old one |
| 5004 | several; kage serve accepts at most 16 connections |
| 5005 | several; one connection per client of the guest SSH agent socket (T-257 sets the limit) |

READ for the kage serve limits (16 KiB head cap, a 10 s deadline, 16 connections): `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/reference/architecture.md:310`.

## Port 5000: control

Port 5000 is strictly request and reply.
The host sends one request and waits for its reply before it sends the next one. T-022 sets the reply deadlines.
The message codes below are design values.

| code | name | direction | fields | when sent |
| --- | --- | --- | --- | --- |
| `0x01` | `HELLO` | host to guest | `version: u16`, `instance_id: [u8; 16]`, `host_time_ns: u64`, `resume: bool` | first frame of each control connection |
| `0x02` | `WELCOME` | guest to host | `version: u16`, `agent_version: string(64)`, `kernel_release: string(64)`, `provisioned: bool`, `kage_state: enum` | reply to `HELLO` |
| `0x03` | `PROVISION` | host to guest | see "Provisioning" | after `WELCOME`, when `provisioned` is false |
| `0x04` | `PROVISIONED` | guest to host | none | after box-guest holds every value in memory |
| `0x05` | `READY` | host to guest | `policy_generation: u64` | when the policy and the bindings of the instance are live |
| `0x06` | `READY_ACK` | guest to host | none | after box-guest starts kage serve |
| `0x07` | `TIME_SET` | host to guest | `time_ns: u64` | after each resume, and when the daemon sees clock drift |
| `0x08` | `TIME_ACK` | guest to host | `offset_ns: i64` | after the guest clock is set; the offset is guest minus host before the step |
| `0x09` | `ROTATE` | host to guest | `kind: enum`, `binding: string(64)`, `value: bytes(256)` | see "Rotation" |
| `0x0A` | `ROTATED` | guest to host | `kind: enum` | after the new value replaces the old one in memory |
| `0x0B` | `STATS_REQUEST` | host to guest | none | on `agent-box inspect` and on a daemon schedule |
| `0x0C` | `STATS` | guest to host | see below | reply to `STATS_REQUEST` |
| `0x0D` | `POWEROFF` | host to guest | `reason: enum` | first step of `stop` and `delete` |
| `0x0E` | `POWEROFF_ACK` | guest to host | none | before box-guest stops kage, syncs and resets the guest |
| `0x0F` | `FSTRIM` | host to guest | none | on operator request |
| `0x10` | `FSTRIM_STARTED` | guest to host | none | at once; the result comes as an event on port 5002 |

Notes on the fields:

- `string(64)` means a `string` of at most 64 bytes. `bytes(256)` means at most 256 bytes.
- `instance_id` is the 128-bit instance id. It is for logs only and proves nothing about the peer.
- `kage_state` uses the values of `KAGE_STATE` on port 5002.
- `ROTATE.kind`: 1 event nonce, 2 egress token, 3 kage token, 4 provider key. `binding` is empty unless `kind` is 4.
- `POWEROFF.reason`: 1 stop, 2 delete, 3 host shutdown.
- `STATS` fields: `uptime_ms: u64`, `mem_total_kib: u64`, `mem_available_kib: u64`, `load1_milli: u32`, `cpu_count: u16`, `data_total_kib: u64`, `data_used_kib: u64`, `kage_restarts: u32`.
- `TIME_SET` and `HELLO.host_time_ns` are nanoseconds since the Unix epoch, UTC.

Why port 5000 carries no exec request:

- Every exec runs on port 5001, one connection per exec. A long command then cannot block a `POWEROFF`.
- One code path in box-guest checks every exec request. A second path would be a second gate to keep equal.

### The handshake

1. The daemon connects to port 5000 and sends `HELLO` with `resume` false after a cold boot, true after re-adoption.
2. box-guest checks the version and replies `WELCOME`. The handshake must end within a boot deadline that T-088 and T-105 set.
3. If `provisioned` is false, the daemon sends `PROVISION`.
4. box-guest connects to ports 5002 and 5003 with the new values (see "Boot order and deny first").
5. The daemon sends `READY` only when the policy of the instance is live.

### Provisioning

`PROVISION` carries every per-instance value that the guest holds:

| field | encoding | source | where box-guest keeps it | used for |
| --- | --- | --- | --- | --- |
| `event_nonce` | `[u8; 32]` | host CSPRNG, per instance | box-guest memory | the first frame on port 5002 |
| `egress_token` | `[u8; 32]` | host CSPRNG, per instance | box-guest memory | the first frame on port 5003 |
| `kage_token` | `[u8; 32]` | host CSPRNG, per instance | memory; handed to kage serve ([guest.md](guest.md)) | the WebSocket token of kage serve |
| `hostname` | `string(63)` | the instance name, a DNS label | the guest kernel hostname | |
| `time_ns` | `u64` | host clock | the guest clock | TLS needs a correct clock |
| `first_boot` | `bool` | the instance record | not kept | the first-boot format of the data disk ([guest.md](guest.md), T-043); design, T-041 adds it |
| `provider_keys` | `list` of at most 32 entries | the broker, per instance and binding | memory; the environment of kage serve | the broker swaps them for credentials |

Each `provider_keys` entry:

| field | encoding | rule |
| --- | --- | --- |
| `binding` | `string(64)` | the binding name in [broker.md](broker.md) |
| `env_name` | `string(64)` | matches `[A-Z][A-Z0-9_]*`; box-guest refuses `PATH`, `HOME`, `LD_*` and `KAGE_SHELL__*` |
| `value` | `string(256)` | a nonce in the key format of the provider |

The whole `PROVISION` payload stays under `MAX_HEAD`.

When provisioning happens:

- Over port 5000 only, after `WELCOME`, before `READY`, so before kage starts.
- Again after a reboot of the guest, because box-guest keeps nothing across a boot.

Where the values never go:

- the data disk, the base image or any file on a block device;
- the kernel command line, which every guest process can read in `/proc/cmdline`;
- MMDS, the VMM argv, the jailer argv or the HCS document;
- a log line on either side.

How box-guest keeps them:

- in its own memory, locked with `mlock` so that they cannot reach swap;
- the provider keys also in the environment of kage serve, because kage reads its key variables once at start;
- the kage token where kage serve reads it, without a write to the data disk.

NOTE: kage serve reads its token from `$XDG_DATA_HOME/kage/remote-token`, mode 0600.
READ: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/remote.md:67`.
box-guest sets `XDG_DATA_HOME` of kage serve to a tmpfs directory under /run, so the token file never reaches the data disk ([guest.md](guest.md)).

T-041 proves the rule: a test reads the data disk image after provisioning and finds no provisioned value.

### Rotation

The host starts every rotation on port 5000. The guest has no message that asks for one.

| kind | what box-guest does after `ROTATED` | what the daemon does |
| --- | --- | --- |
| event nonce | reconnects to port 5002 with the new nonce | closes the 5002 connection that used the old nonce |
| egress token | reconnects to port 5003 with the new token | closes the old 5003 connection; flows end and restart under the policy |
| kage token | restarts kage serve with the new token | reconnects its ACP client on port 5004 |
| provider key | restarts kage serve with the new environment | revokes the old nonce at the broker |

A rotated value is never used again. A restart of kage stops the running agent turn, so the daemon rotates provider keys only at a task boundary.

## Port 5001: PTY and exec

One session per connection. The host sends `OPEN`, and box-guest replies `STARTED` or `ERROR`.
A session runs as uid 1000 (`agent`) unless `OPEN` names root.
The daemon asks for root only on an explicit request of the operator, or for a battery check (T-180).
It logs each root session as host-observed.

NOTE: A root session can read every value that box-guest holds. A forgery from such a session is in the trust model already: guest root is not a boundary.

| code | name | direction | fields | when sent |
| --- | --- | --- | --- | --- |
| `0x20` | `OPEN` | host to guest | see below | first frame |
| `0x21` | `STARTED` | guest to host | `pid: u32` | after the process starts |
| `0x22` | `STDIN` | host to guest | `raw` (data class) | input bytes |
| `0x23` | `STDIN_EOF` | host to guest | none | the end of the input; exec sessions only |
| `0x24` | `STDOUT` | guest to host | `raw` (data class) | output bytes; a PTY sends all its output here |
| `0x25` | `STDERR` | guest to host | `raw` (data class) | error output; exec sessions only |
| `0x26` | `RESIZE` | host to guest | `rows: u16`, `cols: u16` | the terminal size changed; PTY sessions only |
| `0x27` | `SIGNAL` | host to guest | `signal: enum` | 1 INT, 2 TERM, 3 KILL, 4 HUP, 5 QUIT; another value is `refused` |
| `0x28` | `SILENT` | guest to host | `silent_ms: u32` | the session produced no output for 30 s; again after each further 30 s of silence |
| `0x29` | `EXIT` | guest to host | `outcome: enum`, `value: i32` | last frame; box-guest then closes |

`OPEN` fields:

| field | encoding | rule |
| --- | --- | --- |
| `version` | `u16` | `PROTOCOL_VERSION` |
| `kind` | `enum` | 1 PTY (`attach`), 2 exec (`run` and one-shot commands) |
| `user` | `enum` | 1 agent (uid 1000), 2 root |
| `argv` | `list` of `string(4096)` | at least one item; the first is the program |
| `env` | `list` of pairs `string(128)`, `string(4096)` | added to the fixed environment of the session user |
| `cwd` | `string(4096)` | an absolute guest path; empty means the home directory |
| `term` | `string(64)` | the `TERM` value; PTY only |
| `rows`, `cols` | `u16`, `u16` | the first terminal size; PTY only |
| `timeout_ms` | `u32` | 0 means no timeout; at the timeout box-guest sends KILL and reports `outcome` 3 |

`EXIT.outcome`: 1 exited (`value` is the exit code), 2 killed by a signal (`value` is the signal number), 3 timed out, 4 failed to start.

The whole `OPEN` payload stays under `MAX_HEAD`.
A stream reader on either side stops reading when the other side does not drain, so the vsock stream gives back pressure.

## Port 5002: events and heartbeat

box-guest opens this connection. Every frame on it comes from box-guest, which runs as root.
A process at the agent uid (1000) cannot read the event nonce, so it cannot forge an event (T-031).

| code | name | direction | fields | when sent |
| --- | --- | --- | --- | --- |
| `0x40` | `EVENT_AUTH` | guest to host | `version: u16`, `nonce: [u8; 32]` | first frame |
| `0x41` | `HEARTBEAT` | guest to host | `seq: u64`, `uptime_ms: u64` | after 30 s with no frame sent on this connection |
| `0x42` | `KAGE_STATE` | guest to host | `seq: u64`, `state: enum`, `exit: i32`, `restarts: u32` | on each change of the kage serve process |
| `0x43` | `GUEST_STATE` | guest to host | `seq: u64`, `kind: enum`, `value: u64` | on each guest event in the list below |

- `KAGE_STATE.state`: 1 starting, 2 listening on 127.0.0.1:7433, 3 exited (`exit` holds the code), 4 waiting to restart.
- `GUEST_STATE.kind`: 1 data disk formatted, 2 data disk mounted, 3 data disk above 90 percent full, 4 OOM kill.
- More `GUEST_STATE.kind` values: 5 clock stepped, 6 fstrim done (`value` holds the trimmed bytes), 7 network channel up, 8 network channel down.
- `seq` starts at 1 for each boot and grows by 1 for each frame after `EVENT_AUTH`. It continues across a reconnect.
- The daemon logs a gap in `seq`. A `seq` that does not grow is a protocol error.
- The host sends nothing on this port.

How the daemon reads the first frame, before it parses anything:

1. Read exactly 39 bytes: the 5-byte header and a 34-byte payload.
2. Type `0x40` and length 34, or close.
3. Compare `nonce` with the current event nonce in constant time. A mismatch closes the connection.
4. Compare `version`. A mismatch sends `ERROR version_mismatch` and closes.

The nonce check comes before the version check, so a peer without the nonce learns nothing, not even the version.
Each refused first frame increments the refused-forgery counter of the instance. The startup report shows it.

Labels: the daemon labels events from port 5002 "host-observed". It labels the kage ACP stream "agent-reported".
Only host-observed data and broker data feed authorization, billing, backoff and lifecycle decisions.

## Port 5003: network packets

box-guest opens this connection and carries the packets of `tun0` on it.

| code | name | direction | fields | when sent |
| --- | --- | --- | --- | --- |
| `0x60` | `NET_AUTH` | guest to host | `version: u16`, `token: [u8; 32]` | first frame |
| `0x61` | `NET_READY` | host to guest | `policy_generation: u64` | when the policy and the bindings of the instance are live |
| `0x62` | `PACKET` | both | `raw` (packet class): one IP packet | after `NET_READY` |

- The daemon reads the first frame the same way as on port 5002, with type `0x60` and the egress token.
- After `NET_AUTH`, the daemon reads nothing more from the connection until the policy is live. Then it sends `NET_READY`.
- box-guest keeps `tun0` down until `NET_READY` arrives. So no packet leaves before the policy exists (T-126).
- A `PACKET` above the MTU and at most `MAX_PACKET` is dropped and counted. It is not a protocol error.
- The address plan and the MTU are v1 constants of box-proto. [network.md](network.md) gives their values.
- The egress token identifies the instance as a second check. The primary identity is the vsock listener that the host made for that instance.

## Port 5004: kage bridge

| code | name | direction | fields | when sent |
| --- | --- | --- | --- | --- |
| `0x80` | `BRIDGE_OPEN` | host to guest | `version: u16` | first frame |
| `0x81` | `BRIDGE_READY` | guest to host | none | after box-guest connects to 127.0.0.1:7433 |

- box-guest connects to kage serve on guest loopback. If kage serve refuses or does not listen, box-guest sends `ERROR not_ready` and closes.
- After `BRIDGE_READY`, the connection carries raw bytes in both directions until either side closes. No more frames follow.
- The bytes are the WebSocket of kage serve. The `kage_token` authenticates the host to kage at that layer (T-161).
- Only the host can open port 5004, so the bridge adds no authentication of its own.
- The daemon treats every byte from kage as untrusted input. The ACP client bounds every message.

## Port 5005: SSH agent (T-257)

box-guest opens one connection to port 5005 for each client of its SSH agent socket ([broker.md](broker.md) gives the design).

| code | name | direction | fields | when sent |
| --- | --- | --- | --- | --- |
| `0xC0` | `AGENT_AUTH` | guest to host | `version: u16`, `token: [u8; 32]` | first frame; the token is the egress token |
| `0xC1` | `AGENT_READY` | host to guest | none | after the token check, when the agent of the daemon serves the instance |

- The daemon reads the first frame the same way as on port 5002, with type `0xC0` and the egress token.
- After `AGENT_READY`, the connection carries the bytes of the SSH agent protocol in both directions, as port 5004 carries kage bytes.
- The daemon parses those bytes as untrusted input and bounds every agent message. T-257 sets the bound.
- A separate token for this port would change `PROVISION`, so it needs a new protocol version.

## Boot order and deny first

| step | daemon | box-guest |
| --- | --- | --- |
| 1 | asks the helper to `start` the instance | boots; mounts; prepares the data disk |
| 2 | creates its listeners on 5002 and 5003, and on 5005 after T-257 | binds 5000, 5001 and 5004 before any other process starts |
| 3 | connects to 5000; retries until the boot deadline | |
| 4 | sends `HELLO` | replies `WELCOME` |
| 5 | sends `PROVISION` | stores the values in memory; replies `PROVISIONED` |
| 6 | checks the first frames | connects to 5002 with `EVENT_AUTH` and to 5003 with `NET_AUTH` |
| 7 | installs the policy and bindings; then sends `NET_READY` on 5003 | brings `tun0` up after `NET_READY` |
| 8 | sends `READY` on 5000 | starts kage serve as uid 1000; replies `READY_ACK` |
| 9 | | sends `KAGE_STATE` listening on 5002 |
| 10 | opens 5004 when it needs kage | bridges to 127.0.0.1:7433 |

Rules that follow from the order:

- No egress before the policy is live: `NET_READY` waits for it (R-DENY-FIRST).
- T-126 makes the order a type rule: only the step that installs the policy can open the network channel for packets.
- `AGENT_READY` on port 5005 waits for the same step (T-257).
- No kage before egress is decided: `READY` waits for it.
- No secret before kage: provisioning ends at step 5, and kage starts at step 8.
- T-126 tests the race: no packet leaves before `READY`.

## Re-adoption and rotation

A daemon restart leaves running instances alone ([engines.md](engines.md), section "Re-adoption and orphans").

1. The daemon reads its encrypted per-instance records, including the current nonces and tokens.
2. It asks the helper for `inventory` and keeps the verified instances.
3. It creates its listeners on 5002 and 5003 again, and on 5005 after T-257.
4. box-guest notices the lost connections and reconnects with backoff, with its current values.
5. The daemon connects to 5000 and sends `HELLO` with `resume` true. `WELCOME` shows `provisioned` true.
6. The daemon rotates the event nonce and the egress token, so the channels are re-keyed (T-090).
7. The daemon installs the policy and the bindings again. Only then does it send `NET_READY`.

If `WELCOME` shows `provisioned` false, the guest rebooted, and the daemon provisions it from the start.

## Nonce and token rules

These rules hold for the event nonce, the egress token and the kage token:

- Each is 32 bytes from the host CSPRNG: `getrandom` on Linux, `BCryptGenRandom` on Windows.
- Each instance gets its own values. No two instances share one.
- The values differ from each other within one instance.
- The host compares a presented value in constant time.
- A value that one instance holds is refused on the channel of another instance. It is never resolved to the other instance.
- A refused first frame closes the connection and increments the refused-forgery counter.
- The host rotates the values on port 5000. A rotated value is never accepted again.
- The daemon keeps the current values encrypted at rest for re-adoption ([broker.md](broker.md) gives the store).

The guarantee is narrow. A guest process at the uid of box-guest (root) would have the nonce.
The claim is only that a process at the agent uid cannot forge an event or open the network channel.

## Heartbeat on silence

- box-guest sends `HEARTBEAT` on port 5002 only after 30 s in which it sent no frame on that connection.
- A stream of events therefore needs no heartbeat between them.
- The daemon counts only authenticated frames on port 5002 as signs that box-guest lives.
- Traffic on ports 5001, 5003 and 5004 does not count. A flood from a guest process is not liveness of box-guest.
- The daemon marks the instance `unknown` after a silence longer than its liveness deadline. T-022 sets the deadline above 30 s.
- The deadline does not run while the instance is `paused`. It starts again at resume.
- On port 5001, `SILENT` tells the operator that a command is quiet. It does not end the session.

## Golden frames

The corpus lives in crates/box-proto/tests/golden/ (planned path, T-021 and T-022).

- One file per case: `<port>/<message>.hex` and `helper/<message>.hex`.
- Format: lowercase hex pairs; whitespace does not count; a line that starts with `#` is a comment. The files are ASCII text.
- A positive case: decoding the file gives the expected value, and encoding that value gives the file bytes.
- A negative case: decoding gives the named error.
- The negative cases: a truncated header, a truncated payload, a length above each ceiling, an unknown type and a type on the wrong port.
- More negative cases: trailing bytes, a bad `bool`, bad UTF-8, an unknown `enum` tag and a version mismatch.
- The same tests run for the host targets and the musl guest targets, so both sides agree byte for byte.
- Each ceiling check is mutation-proved: a planted off-by-one makes a test fail (T-021).
- A layout change edits the golden files and `PROTOCOL_VERSION` in the same commit.

## Transport on Firecracker: hybrid vsock

Firecracker maps each guest vsock port to Unix sockets on the host. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/vsock.md:52`.
The helper returns the host path of `v.sock` in the reply to `start` ([engines.md](engines.md) gives the jail layout).

Host-initiated connection (ports 5000, 5001 and 5004), crate box-transport (T-024):

1. Connect a Unix stream socket to `v.sock`.
2. Write exactly `CONNECT <port>\n` with the port in decimal, for example `CONNECT 5000\n`.
3. Read until `\n`, at most 32 bytes. Expect `OK <n>\n`.
4. `<n>` is the host-side port that Firecracker assigned, not the guest port. Do not compare it with the request.
5. If Firecracker closes the connection before the `OK` line, no guest process listens. Retry with backoff until the boot deadline.
6. After the `OK` line, frames start.

Facts behind the steps, all READ at the pin:

- Firecracker reads the `CONNECT` line into a buffer of 32 bytes and accepts `connect` in any case. Source: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/devices/virtio/vsock/unix/muxer.rs:449`.
- It writes `OK` with its own local port. Source: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/devices/virtio/vsock/unix/muxer.rs:683`.
- It assigns local ports with bit 30 set, so `<n>` is 1073741824 or more. Source: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/devices/virtio/vsock/unix/muxer.rs:618`.
- One vsock device holds at most 1023 connections. Source: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/devices/virtio/vsock/unix/mod.rs:20`.

Guest-initiated connection (ports 5002 and 5003):

1. The daemon listens on `v.sock_5002` and `v.sock_5003`, next to `v.sock`.
2. When the guest connects to CID 2 and port P, Firecracker connects to `v.sock_P`. No text line comes first.
3. With no listener, the guest gets a reset. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/vsock.md:87`.
4. Firecracker calls a blocking `connect` on the VMM thread. So the daemon accepts at once, on its own tasks.

The guest CID is 3 for every instance, because the hybrid vsock has no host-wide CID space.
OpenVMM uses the same `CONNECT` protocol, so this transport also serves the OpenVMM fallback.
READ: `microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:support/hybrid_vsock/src/lib.rs:18`.

## Transport on HCS: AF_HYPERV

The guest uses `AF_VSOCK` over `hv_sock`. The host uses `AF_HYPERV` sockets, crate box-transport (T-025).

| item | value |
| --- | --- |
| address family | `AF_HYPERV` = 34 |
| protocol | `HV_PROTOCOL_RAW` = 1 |
| address | `SOCKADDR_HV`: family, reserved 0, `VmId`, `ServiceId` |
| `VmId` | the runtime id of the compute system, from the reply to `start` |
| `ServiceId` | the port in the first field, then `facb-11e6-bd58-64006a7986d3` |
| guest CID 2 | maps to `HV_GUID_PARENT`, the host |
| valid ports | 0 to 0x7FFFFFFF |

READ: Windows SDK 10.0.22621.0, header shared/hvsocket.h, lines 135 to 169, and ws2def.h for `AF_HYPERV`.

Service GUIDs of the v1 ports:

| port | `ServiceId` |
| --- | --- |
| 5000 | `00001388-facb-11e6-bd58-64006a7986d3` |
| 5001 | `00001389-facb-11e6-bd58-64006a7986d3` |
| 5002 | `0000138a-facb-11e6-bd58-64006a7986d3` |
| 5003 | `0000138b-facb-11e6-bd58-64006a7986d3` |
| 5004 | `0000138c-facb-11e6-bd58-64006a7986d3` |
| 5005 | `0000138d-facb-11e6-bd58-64006a7986d3` |

Rules:

- Build each GUID from its fields, with the port as the first field. The first three fields are little-endian in memory, so a byte copy is wrong.
- Host-initiated ports: `connect` to the runtime id and the service GUID. The guest listens on the vsock port.
- Guest-initiated ports: `bind` to the runtime id and the service GUID, then `listen`.
- Never bind the wildcard `VmId` (all zeros) or `HV_GUID_CHILDREN`. Never set `AllowWildcardBinds`.
- Set `HVSOCKET_CONNECTED_SUSPEND` (0x04) to 1 at level `HV_PROTOCOL_RAW` on every listener, every accepted socket and every connected socket.
- Without that option a socket disconnects when the instance pauses ([engines.md](engines.md), section "HvSocket").
- Use `HVSOCKET_CONNECT_TIMEOUT` (0x01) on each connect. The header gives a maximum of 300000 ms.
- No text line comes first. Frames start at once.
- The `unsafe` code for these calls stays in one module, and each block carries a comment (T-025).

## The helper protocol

The daemon talks to the privileged helper with this protocol (T-023).
The shape of the helper, its operations per engine and its threat model are in [engines.md](engines.md).

### Endpoint and peer check

| item | Linux | Windows |
| --- | --- | --- |
| endpoint | Unix stream socket `/run/agent-box/helper.sock` | named pipe `\\.\pipe\agent-box-helper` |
| who may open it | root and the `agent-box` group (mode 0660) | SYSTEM and the operator SID (pipe DACL) |
| peer check | `SO_PEERCRED` uid equals the recorded operator uid | client token user SID equals the recorded operator SID |
| when | after accept, before the first frame is read | after connect, before the first frame is read |

On Windows the helper reads the client SID with `ImpersonateNamedPipeClient`, `OpenThreadToken` and `GetTokenInformation`, then reverts.
A failed peer check sends `REFUSAL` with `peer_refused` and closes.

### Framing and the version rule

- The same frame layout and field encodings as the host-guest protocol.
- `REQUEST` payloads stay under `MAX_HEAD`. `RESULT` payloads stay under `MAX_FRAME`, because an inventory can be long.
- `HELPER_VERSION` is 1, a `u16`, separate from `PROTOCOL_VERSION`.
- One operation per connection. The helper closes the connection after its reply.
- The helper drops a client that sends no complete frame within a deadline that T-081 sets.

| code | name | direction | fields |
| --- | --- | --- | --- |
| `0xA0` | `HELPER_HELLO` | daemon to helper | `version: u16` |
| `0xA1` | `HELPER_WELCOME` | helper to daemon | `version: u16`, `helper_version: string(64)`, `engine: enum`, `engine_version: string(64)` |
| `0xA2` | `REQUEST` | daemon to helper | `op: enum`, then the fields of the operation |
| `0xA3` | `RESULT` | helper to daemon | `op: enum`, then the result of the operation |
| `0xA4` | `REFUSAL` | helper to daemon | `code: u16`, `step: u8`, `os_error: i64`, `detail: string(256)` |

- `engine`: 1 Firecracker, 2 HCS.
- A version mismatch: the helper sends `REFUSAL` with `version_mismatch` and closes.
- `step` names the step of the operation that failed, 0 for none. T-023 lists the steps per operation.
- `os_error` is the errno on Linux or the HRESULT on Windows, 0 for none.

No `REQUEST` field is a string. Ids are 16 bytes, digests 32 bytes, and counts are integers.
So a caller-supplied path cannot be expressed in the request types (T-023).

### Operations

| op | name | request fields | result fields |
| --- | --- | --- | --- |
| 1 | `probe` | none | `facts: list` of `name: string(64)`, `status: enum`, `evidence: string(512)` |
| 2 | `create` | `id: [u8; 16]`, `disk_gib: u16` | `uid: u32` (0 on Windows) |
| 3 | `start` | `id: [u8; 16]`, `kernel: [u8; 32]`, `base: [u8; 32]`, `cpus: u8`, `memory_mib: u32` | `pid: u32`, `start_time: u64`, `endpoint: enum` |
| 4 | `pause` | `id: [u8; 16]` | none |
| 5 | `resume` | `id: [u8; 16]` | none |
| 6 | `stop` | `id: [u8; 16]` | none |
| 7 | `kill` | `id: [u8; 16]` | `absences: list` |
| 8 | `delete` | `id: [u8; 16]` | `absences: list` |
| 9 | `inventory` | none | `entries: list` |
| 10 | `image_add` (proposed) | `digest: [u8; 32]`, `kind: enum`, `size: u64`, plus a passed descriptor | none |

Result types:

- `probe` `status`: 1 ok, 2 gap, 3 note.
- `endpoint`: tag 1 is a Unix path, a `string(4096)` for Firecracker; tag 2 is a runtime id, a `[u8; 16]` for HCS.
- `absences` entry: `item: enum`, `absent: bool`.
- The Linux items: 1 jail, 2 cgroup leaf, 3 data disk, 4 instance directory, 5 uid.
- The other items: 6 compute system, 7 VM access grant, 8 save file, 9 snapshot files.
- `entries` item: `id: [u8; 16]`, `state: enum`, `claimed: bool`, `pid: u32`, `start_time: u64`, `endpoint: enum`.
- `state`: 1 running, 2 paused, 3 stopped, 4 unknown. `claimed` is false for a VMM or compute system with no helper record.
- On Windows `pid` and `start_time` are 0.
- `image_add` `kind`: 1 kernel `vmlinux`, 2 kernel `bzImage`, 3 kernel `Image` (aarch64), 4 raw base image, 5 VHDX base image.

NOTE: `image_add` is not in the T-023 list. It passes the open image file with `SCM_RIGHTS` on Linux.
On Windows the helper duplicates a handle from the client process. T-023 decides whether the operation exists.

### Refusals

| code | name | when |
| --- | --- | --- |
| 1 | `peer_refused` | the peer uid or SID is not the recorded operator |
| 2 | `version_mismatch` | `HELPER_HELLO` has another version |
| 3 | `malformed` | a frame or a field fails to decode, or bytes trail |
| 4 | `unknown_op` | the `op` tag is not in the table |
| 5 | `bad_id` | the id is all zeros |
| 6 | `unknown_instance` | the id has no helper record |
| 7 | `exists` | `create` for an id that has a record |
| 8 | `wrong_state` | for example `pause` of a stopped instance, or `delete` of a running one |
| 9 | `out_of_bounds` | cpus, memory or disk size outside the configured bounds |
| 10 | `instance_limit` | `create` above the configured instance count |
| 11 | `image_missing` | no image with that digest and a fitting kind |
| 12 | `image_digest_mismatch` | `image_add` bytes do not hash to the digest |
| 13 | `busy` | another operation holds the instance lock |
| 14 | `unsupported` | `stop` on aarch64 or on HCS |
| 15 | `cross_device` | the store and the jail base are on different filesystems |
| 16 | `verification_failed` | a pid, a cgroup or a configuration check failed after start; the VMM was killed |
| 17 | `gap` | a host capability that the operation needs is missing, for example `memory.max` |
| 18 | `syscall_failed` | a step failed; `step` and `os_error` name it |
| 19 | `absence_not_verified` | `kill` or `delete` removed an item but could not prove its absence |

Refusals 1 to 7, 9, 10 and 14 come from pure checks over the request, the helper configuration and the store list.
The helper reads that list before it decodes the request, so these refusals happen before any syscall that uses a request value (T-081).
Refusals 8, 11 to 13 and 15 to 19 come from a step of the operation, for example the instance lock or a hard link.
[engines.md](engines.md), section "Threat model of the helper", names the planned test for each one.

## Open questions

| question | entry |
| --- | --- |
| The reply deadlines on port 5000, the boot deadline and the liveness deadline. | T-022, T-088, T-105 |
| The session limit on port 5001. | T-042 |
| Does kage serve work with its data directory on a tmpfs and its sessions linked to the data disk? | T-046 |
| The bound of one SSH agent message on port 5005, and the connection limit. | T-257 |
| The MTU and the address plan constants. | T-045, T-120 |
| Whether the helper protocol gets `image_add`. | T-023 |
