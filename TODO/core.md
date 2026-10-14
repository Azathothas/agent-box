# Core

This area holds the shared types, the host-guest protocol, the host transport,
the daemon and CLI skeletons, and the two P0 rules about guest-written bytes.
[docs/architecture.md](../docs/architecture.md) gives the crate graph and wins
any conflict. [docs/protocol.md](../docs/protocol.md) gives the ports, the
frames and the versions. [docs/cli.md](../docs/cli.md) gives the verbs, the
states and the exit codes.

No product crate exists yet (ruling D14). Every path under crates/ in this file
is planned and written without backticks. T-033 creates the crates, and every
other M1 entry starts after it.

# T-020: box-core: InstanceId, names, State, InstanceSpec, errors

**Source:** The planning brief of 2026-10-14, sections 8 and 11; agent-sandbox KEP-119 and KEP-694.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

Every crate names instances, states and errors. Two definitions of one thing
drift apart. A boolean "running" hides the case where nobody knows the state.

## Premise

- Design: brief section 8 gives the states creating, stopped, starting,
  running, paused, stopping, deleting and unknown.
- Design: a failed query to the VMM or to HCS gives `unknown`, never an assumed
  `running`. The state is an enum everywhere.
- Design: the kinds are persistent and ephemeral. box-core does no IO
  (section 11).
- Design: the helper accepts an instance id only in the shape `[0-9a-f]{32}`
  (section 4.1).
- READ: KEP-119 has an explicit Unknown state, because a failed query does not
  prove that the sandbox is gone
  (`kubernetes-sigs/agent-sandbox@4b63868f0d67e4ee0fa690bb14cb5722b78d679d:docs/keps/119-sandbox-suspended-state/README.md:26-31`).
- READ: KEP-694 records a trap. A new field that defaults to Running resumed
  suspended sandboxes after an upgrade
  (`kubernetes-sigs/agent-sandbox@4b63868f0d67e4ee0fa690bb14cb5722b78d679d:docs/keps/694-kep-for-suspend-and-resume-for-beta/README.md:130-137`).
- READ: the identity of a stored object is a stable opaque token, never a value
  parsed again from a mutable name
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/conventions/code.md:43-63`).

## Approach

1. In crates/box-core/src/id.rs, define `InstanceId` as 16 bytes.
2. Build an `InstanceId` only from bytes that the caller supplies. box-daemon
   supplies bytes from the OS random source.
3. Format the id as 32 lowercase hex characters.
4. Parse only 32 lowercase hex characters. Refuse uppercase, and refuse the
   all-zero id.
5. In crates/box-core/src/name.rs, define `InstanceName` with the DNS label
   rules.
6. Accept 1 to 63 characters of lowercase letters, digits and hyphens, with no
   hyphen at either end.
7. Refuse a name of 32 hex characters, so that a CLI operand is a name or an id,
   never both.
8. In crates/box-core/src/state.rs, define `State` with the eight states of the
   brief.
9. Give `State` no `Default`. Make a missing or unknown state an error on read.
10. Define the transitions as a function of `State` and the lifecycle verb.
11. Take the allowed states of each verb from the table of
    [docs/cli.md](../docs/cli.md), the one home of those rules.
12. Define `InstanceKind` with the values persistent and ephemeral.
13. In crates/box-core/src/spec.rs, define `InstanceSpec`: id, name, kind,
    cpus, memory in MiB, disk in GiB, image variant and digest.
14. Validate only the shape in box-core. Each engine refuses values outside its
    own limits with a typed error.
15. In crates/box-core/src/error.rs, define one error enum with a variant per
    refusal.
16. Add a small cancellation flag type with no IO. box-daemon bridges it to
    tokio.
17. Add property tests with proptest for id and name parsing.

## Prove

```sh
cargo test -p box-core
git grep -n -E "std::(fs|net|process)" -- crates/box-core/src
cargo xtask layering
```

The tests pass. Each 16-byte value survives format and parse. Each string that
is not 32 lowercase hex characters is refused. The grep finds nothing. The
layering check exits 0. Any host.

## Start condition

T-033.

# T-021: box-proto: framing, ceilings and golden frames

**Source:** The planning brief of 2026-10-14, section 7; the research document, sections 5.3 and 8.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The guest controls the length prefix of every frame it sends. A u32 length
announces up to 4 GiB. A decoder that allocates before it checks gives the guest
a memory lever on the host.

## Premise

- Design: a frame is a u32 big-endian length, a u8 type and the payload. The
  ceilings are MAX_HEAD 8 KiB and MAX_FRAME 8 MiB (brief section 7).
- Design: the decoder closes the connection on an oversize length, and it
  allocates nothing of that size first. A short frame at the end of input is
  an error.
- READ: the research document pins both ceilings as constants and asks for each
  guard to be mutation-proved
  ([docs/research/microvm-tier.md](../docs/research/microvm-tier.md),
  sections 5.3 and 8).
- READ: errand returns a partial head at its 8192-byte cap instead of refusing
  it
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/broker.ts:292-326`).
- READ: a format mismatch is an error. Never pad, guess or overwrite
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/conventions/code.md:43-63`).
- Design: box-proto has no IO and no platform code. It builds for the host and
  for the guest musl targets.

## Approach

1. In crates/box-proto/src/frame.rs, define `MAX_HEAD` as 8192 and `MAX_FRAME`
   as 8388608.
2. Decision: the length counts the payload bytes only. The type byte is not in
   it.
3. Give each frame type a class: head (ceiling MAX_HEAD) or data (ceiling
   MAX_FRAME).
4. Write a decoder with no IO. It takes bytes in and gives frames out.
5. Read the 5 header bytes first. Refuse an unknown type and a length above the
   ceiling of its class.
6. Reserve the payload buffer only after both checks.
7. At the end of input, report a partial header or payload as `Truncated`.
8. Write an encoder that refuses an oversize payload with the same rule.
9. Define `FrameError` with the variants Truncated, TooLarge and UnknownType.
10. Put a golden corpus in crates/box-proto/tests/golden/: one binary file and
    one text description per case.
11. Add golden cases: an empty payload, a head of exactly MAX_HEAD and a data
    frame of exactly MAX_FRAME.
12. Add refused cases: MAX_HEAD plus 1, MAX_FRAME plus 1, a length of
    0xFFFFFFFF, and a truncated header and payload.
13. Add a test with a counting global allocator. A header that announces 4 GiB
    is refused with no allocation above 5 bytes.
14. Mutation-prove each guard: change `>` to `>=` in a ceiling check, run the
    tests, see one fail, and revert.

## Prove

```sh
cargo test -p box-proto
cargo build -p box-proto --target x86_64-unknown-linux-musl
```

Both exit 0. For each planted off-by-one of step 14, `cargo test -p box-proto`
fails. Any host with the musl target from `cargo xtask setup`.

## Start condition

T-033.

# T-022: box-proto: v1 messages and the version handshake

**Source:** The planning brief of 2026-10-14, section 7; the research document, section 5.3.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The host and the guest are two binaries built at different times. Without one
shared definition of each message, they drift. A downgrade path lets a guest
choose an older and weaker protocol.

## Premise

- Design: the v1 port set is fixed (brief section 7, and the main-session
  ruling on ports). An unknown port is refused.
- Design: 5000 is control and 5001 is PTY and exec, host to guest. 5002 is
  events and heartbeat, and 5003 is network frames, guest to host.
- Design: 5004 is the kage serve bridge, host to guest. 5005 is the ssh agent
  channel, guest to host (T-257).
- Design: [docs/protocol.md](../docs/protocol.md) is the one home of the
  message names, the type codes, the fields and the first frame of each
  port.
- Design: a heartbeat goes out only after 30 s with no other frame, never on
  a fixed timer. A version mismatch refuses the session, with no downgrade.
- Design: the event nonce and the egress token are 32 bytes from the host
  CSPRNG. Port 5000 provisions them before kage starts.
- READ: the research document proposes to disconnect a guest that sends at
  1 MiB/s sustained, because a flood is not liveness. That value is not
  measured.

## Approach

1. In crates/box-proto/src/port.rs, define `Port` with the six values and
   their directions.
2. Make `Port::try_from(u32)` refuse every other number.
3. Port 5005 is in the v1 set now, so T-257 needs no new version. The daemon
   makes no listener for it until T-257 lands.
4. In crates/box-proto/src/version.rs, set `PROTOCOL_VERSION` to 1.
5. Put the version in the first frame of every connection.
6. On a mismatch, send `ERROR version_mismatch` and close. Write no code path
   that accepts another version.
7. Implement each message of docs/protocol.md with its code, its fields and
   its class. Where the page leaves a value open, set it here and write it
   into the page.
8. Port 5005 gets `AGENT_AUTH` and `AGENT_READY` of docs/protocol.md now.
   T-257 serves the port later.
9. Carry PTY bytes, bridge bytes and IP packets as raw data payloads.
10. Refuse a type that does not belong to the port of the connection, or to
    its state.
11. Add a silence timer with no IO: it fires after 30 s with no frame.
12. Set the reply deadline of each port 5000 request, and a liveness deadline
    above 30 s. docs/protocol.md asks this entry for both, as design values.
13. Add a rate ceiling for the event port as a named constant. Mark its value
    as not measured.
14. Add a golden frame for every message, and a test that compares each one
    byte for byte.

## Prove

```sh
cargo test -p box-proto
```

It exits 0. Each message has a golden frame. Tests refuse an unknown port, a
type sent on the wrong port, and versions 0 and 2. `Port::try_from(5005)`
succeeds and `Port::try_from(5006)` fails. Any host.

## Start condition

T-021 and T-033.

# T-023: box-proto: the helper protocol

**Source:** The planning brief of 2026-10-14, sections 4.1, 4.2 and 11; the research document, section 5.6.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

The helper runs as root on Linux and as LocalSystem on Windows. Each string that
a caller can pass to it is a way to make it act on a path of the caller's
choice. The safest input is an input that the types cannot express.

## Premise

- Design: the Linux helper validates each input before any syscall. The id
  matches `[0-9a-f]{32}`.
- Design: the cgroup path is the configured root plus the id, and the jail base
  is the configured value. The helper never parses guest data.
- Design: the Windows helper builds every HCS document itself. It accepts no
  schema fragment, and grants VM access only inside the agent-box store.
- Design: the helper protocol types live in box-proto, module `helper`, so that
  the helper and the daemon share one definition.
- READ: the research document gives a provenance table for each field. No
  caller-supplied string reaches a syscall
  ([docs/research/microvm-tier.md](../docs/research/microvm-tier.md), section 5.6).
- READ: bailey keeps privilege in a small helper with a fixed operation set and
  a versioned greeting
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:docs/security/privilege.md:26-48`).
- READ: a caller-supplied Plan9, VirtualSmb or NetworkAdapters entry would make
  the LocalSystem helper an escalation tool. That is an inference of the
  adversarial review.

## Approach

[docs/protocol.md](../docs/protocol.md#the-helper-protocol) is the home of
the helper protocol. This entry builds it and does not change it.

1. In crates/box-proto/src/helper.rs, define the five message types
   `HELPER_HELLO`, `HELPER_WELCOME`, `REQUEST`, `RESULT` and `REFUSAL`, with
   the codes and fields of docs/protocol.md.
2. Define the operations of its table: probe, create, start, pause, resume,
   stop, kill, delete and inventory. Each one has the request fields and the
   result fields of that table.
3. Give request fields only these types: a 16-byte instance id, small
   integers and a 32-byte image digest. `stop` has no field but the id.
4. Give no request field a `String`, `PathBuf`, `OsString` or byte-vector
   type. Strings appear only in results and refusals, with their ceilings.
5. Decision: the helper allocates the uid and records it in its own root-owned
   state. The request has no uid field. `create` returns the uid.
6. NOTE: the brief has the helper validate a uid from the daemon. Step 5 is
   stricter. The operator can rule otherwise.
7. `REFUSAL` carries `code`, `step`, `os_error` and `detail`. Use the refusal
   codes of docs/protocol.md, and list the steps of each operation for the
   `step` field.
8. Never report a partial result as `RESULT`.
9. The encoding is binary, so a serde attribute does not apply. The decoder
   refuses an unknown `op` tag (`unknown_op`) and trailing bytes or a bad
   field (`malformed`).
10. Use the frame codec of T-021. A `REQUEST` payload stays under `MAX_HEAD`.
    A `RESULT` payload stays under `MAX_FRAME`, because an inventory can be
    long.
11. Start every connection with `HELPER_HELLO` and `HELPER_VERSION` 1, which
    is separate from `PROTOCOL_VERSION`. A connection carries one operation.
12. Decide whether `image_add` exists, as the open question of
    docs/protocol.md asks. Record the decision in docs/protocol.md.
13. Add a test that reads helper.rs with `include_str!`.
14. Make that test fail when a request field has a string, path or
    byte-vector type.

## Prove

```sh
cargo test -p box-proto helper::
```

It exits 0. Each message and each operation has a golden frame. A request
with trailing bytes is refused with `malformed`, and an unknown `op` tag
with `unknown_op`. An all-zero id is refused with `bad_id`. A `REQUEST`
above `MAX_HEAD` closes the connection before any allocation. Any host.

## Start condition

T-033.

# T-024: box-transport on Linux: Firecracker hybrid vsock

**Source:** The planning brief of 2026-10-14, section 7; Firecracker docs/vsock.md at v1.17.0.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

On Linux, the daemon reaches each guest port through a Unix socket that
Firecracker serves. The connect handshake is a text line. A reader that reads
past the line eats the first bytes of the guest.

## Premise

- READ: for a host-initiated connection, the host sends `CONNECT PORT` and a
  line feed on the socket at `uds_path`.
- READ: Firecracker answers `OK PORT`, where PORT is the vsock port assigned to
  the host end, not the requested port.
- READ: with no guest listener, Firecracker closes the host connection
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/vsock.md:53-63`).
- READ: a guest-initiated connection reaches a host listener at `uds_path_PORT`
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/vsock.md:80-87`).
- READ: Cloud Hypervisor and OpenVMM implement the same hybrid vsock protocol.
  The transport serves the recorded fallback without change.
- Design: file permissions control access to these sockets. A vhost-vsock CID
  is open to any local process (brief section 4.1).
- READ: every wait has a hard limit. A timeout is a different fact from a
  failure
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/conventions/shell.md:422-437`).

## Approach

1. In crates/box-transport/src/unix.rs (cfg unix), give a synchronous API on
   std Unix sockets.
2. How the async daemon uses this API is the open question of T-032 (a sync
   or an async Engine trait). Keep the API small, so that either answer fits.
3. `connect(uds, port, timeout)`: connect, then write `CONNECT`, the decimal
   port and a line feed.
4. Read the answer one byte at a time, up to the line feed, with a cap of 32
   bytes.
5. Accept `OK`, a space, a decimal u32 and a line feed. Record the number. Do
   not compare it with the requested port.
6. Map other text to `Refused`, end of input to `NoListener`, and an expired
   limit to `Timeout`.
7. `listen(uds, port)`: bind `uds_path_PORT` with the decimal port.
8. Before the bind, remove an old entry at that path only when it is a socket.
   Never follow a symbolic link.
9. Take the socket directory from the caller. The helper creates that directory
   inside the jail (T-086).
10. Name each limit as a constant. The default of 10 s is a design value, not a
    measurement.
11. Write a fake hybrid-vsock peer in crates/box-transport/tests/fake_peer.rs.
12. Test a good handshake, an `OK` with another port, garbage, end of input, a
    slow answer and a 33-byte answer.
13. Test that bytes the peer sends right after `OK` reach the caller intact.

## Prove

```sh
cargo test -p box-transport
```

It exits 0 against the fake peer. No VM is needed. Any Linux host, or
`cargo xtask linux -- cargo test -p box-transport` on Windows. The live check
is `hybrid_vsock_live` in T-088.

## Start condition

T-033.

# T-025: box-transport on Windows: AF_HYPERV

**Source:** The planning brief of 2026-10-14, sections 4.2 and 7; the adversarial review, items 4 and 5.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

On Windows, the daemon reaches each guest port through a Hyper-V socket. A
wildcard bind accepts connections from every VM on the host. The default socket
drops its connection when the VM pauses.

## Premise

- READ: a Hyper-V socket service id embeds the port in the first field of
  00000000-facb-11e6-bd58-64006a7986d3
  (https://learn.microsoft.com/en-us/virtualization/hyper-v-on-windows/user-guide/make-integration-service,
  read 2026-10-14).
- READ: the per-VM DefaultBindSecurityDescriptor covers a bind to an unlisted
  service of that VM, not a wildcard bind. No registry entry is needed.
- READ: WSL binds per VM with the SDDL `D:P(A;;FA;;;SY)(A;;FA;;;SID)` for the
  user's SID (WslCoreVm.cpp, read 2026-10-14).
- READ: HVSOCKET_CONNECTED_SUSPEND is a socket option. By default a socket
  disconnects when the VM pauses (hvsocket.h:50-57, Windows SDK 10.0.22621.0).
- Design: VmId is the runtime id of the compute system. The broker binds one
  listener per VmId and never sets AllowWildcardBinds.
- Design: the workspace sets `unsafe_code` to deny (`Cargo.toml`). This module
  needs `unsafe` for the WinSock calls, so it allows unsafe code for itself
  only.

## Approach

1. In crates/box-transport/src/lib.rs, outside the `cfg(windows)` gate, write
   `service_id(port)`. It makes no OS call, so its tests run on any host.
2. Put the port in the first 32-bit field. Keep the other fields of the template.
3. Test the mapping: 5000 gives 00001388-facb-11e6-bd58-64006a7986d3.
4. Test the mapping: 5004 gives 0000138c-facb-11e6-bd58-64006a7986d3.
5. Take the address family, the protocol and the option numbers from the windows
   crate. A test asserts that each one exists.
6. `connect(vm_id, port, timeout)`: open an AF_HYPERV stream socket and connect
   with the connect timeout option of hvsocket.h.
7. `listen(vm_id, port)`: bind to that VmId and that service id. Never bind the
   wildcard VmId.
8. Set HVSOCKET_CONNECTED_SUSPEND to 1 on each connected and each accepted
   socket, before the caller gets it.
9. Map each WinSock error to a `TransportError` variant that names the error
   number.
10. Write a SAFETY comment over each `unsafe` block.
11. Start crates/box-transport/src/windows.rs with the inner attribute
    `#![allow(unsafe_code)]`. Decision P6 lists this file, and T-259 checks
    the list. No other file of the crate allows the lint.
12. Put the socket code in crates/box-transport/src/windows.rs behind
    `cfg(windows)`. `service_id` stays in lib.rs (step 1).
13. Open question: does tokio accept the socket as a TCP stream through
    `FromRawSocket`? The other route is a blocking thread per connection.
14. T-105 answers the question with its live test `hvsock_live`. Record the
    answer in docs/decisions.md.

## Prove

```sh
cargo test -p box-transport
```

It exits 0 on any host with the GUID mapping tests. On Windows, the same
command also builds and tests the socket module. The live check is
`hvsock_live` in T-105, on Windows 11 with Hyper-V.

## Start condition

T-033.

# T-026: Host state directory, instance records and locks

**Source:** The planning brief of 2026-10-14, sections 4.2 and 8; TEMPLATE docs/conventions/code.md and shell.md.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

An instance outlives the CLI, the daemon and on Windows its compute system. Its
record is the only lasting identity. A torn write or a guessed default turns one
instance into another, or a stopped instance into a running one.

## Premise

- Design: HCS compute systems are ephemeral. An instance is the agent-box record
  plus its disks (brief section 4.2).
- READ: every persisted format carries a version. A mismatch is an error
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/conventions/code.md:43-63`).
- READ: write a temporary file in the same directory, then rename it. A rename
  across volumes is a copy
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/conventions/shell.md:122-132`).
- READ: never default a missing state to running (KEP-694, see T-020).
- Design: the brief does not name the state directory. This entry decides it.

## Approach

1. In crates/box-daemon/src/state.rs, decide the state directory.
2. On Linux, use `$XDG_STATE_HOME/agent-box`, default ~/.local/state/agent-box,
   mode 0700.
3. On Windows, use %LOCALAPPDATA%\agent-box, with a DACL for the user only.
4. Record the decision in docs/architecture.md.
5. Keep disks out of this directory. They live in the helper's store (T-087,
   T-103).
6. Give each instance a directory instances/ID/ with record.json and a lock
   file.
7. Write `version: 1`, the id, the name, the kind, the spec, the engine and the
   creation time.
8. Store the desired state that the operator asked for. Never store the
   observed state as truth.
9. Show each instance as `unknown` after a daemon start, until the engine
   inventory answers.
10. Write a record atomically: a temporary file in the same directory, flush,
    rename, then flush the directory on Linux.
11. Delete files with the temporary prefix at start. They never were a record.
12. Refuse an unknown version, a missing field and an unknown field. Name the
    file in the error.
13. Build the map from names to ids at start. Report two records with one name;
    never pick one.
14. Hold the instance lock with `File::lock` of std for each lifecycle
    operation.
15. Let a second operation on a locked instance wait up to a limit, then fail
    with `Busy`.

## Prove

```sh
cargo test -p box-daemon state::
```

It exits 0. Tests cover a round trip, an unknown version, a missing state, a
stale temporary file, a duplicate name and lock contention. Any host.

## Start condition

T-033.

# T-027: The configuration file and profiles

**Source:** The planning brief of 2026-10-14, section 8; bailey docs/security/limitations.md.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

A configuration that accepts unknown keys accepts typos in silence. A
configuration that can hold a credential will hold one. A profile from the wrong
file gives an instance a policy that nobody chose.

## Premise

- Design: the Linux search order is `$XDG_CONFIG_HOME/agent-box/config.toml`,
  ~/.config/agent-box/config.toml, then /etc/agent-box/config.toml.
- Design: the Windows search order is %APPDATA%\agent-box\config.toml, then
  %ProgramData%\agent-box\config.toml.
- Design: an environment variable names one file outright. An unknown key is a
  refusal. The configuration holds no credential.
- Design: a profile names cpus, memory, disk size, image variant, egress policy
  and bindings (brief section 8).
- READ: bailey accepts a configuration found in a directory as policy only
  after the operator accepts its contents
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:docs/security/limitations.md:155-159`).
- Design: in agent-box, the configuration exists on the host only. No guest
  byte reaches it.

## Approach

1. In crates/box-daemon/src/config.rs, read the first file of the search order
   that exists. Never merge two files.
2. Decision: the variable is AGENT_BOX_CONFIG. A file that it names and that
   does not exist is an error, with no fallback.
3. Require `version = 1` at the top of the file.
4. Use serde with `deny_unknown_fields` at every level.
5. Read the tables `[daemon]`, `[defaults]`, `[profiles.NAME]`,
   `[policies.NAME]` and `[bindings.NAME]`. The section "The config file" of
   [docs/cli.md](../docs/cli.md#the-config-file) is the home of the shape.
6. Give a profile the keys `image`, `cpus`, `memory`, `disk`, `policy` and
   `bindings`, as the example of docs/cli.md does.
7. Parse `memory` and `disk` as a size: an integer and the unit `M` or `G`,
   for example `memory = "8G"` and `disk = "30G"`. Units are binary.
8. Parse the `[policies.NAME]` tables with the types of box-policy (T-123).
   Parse the `[bindings.NAME]` tables with the binding types of T-141.
9. Give no type a field that can hold a credential.
10. Point a refused key whose name looks like a credential at
    `agent-box secret set`.
11. Name the file, the line and the key in each error. A refused file exits 2.
12. Without a file, use the built-in profile `default` of docs/cli.md: image
    `base`, 2 cpus, memory `4G`, disk `30G`, policy `default` and no binding.
13. These are design values. T-181 measures the footprint and can change them.
    docs/cli.md is the one home of the defaults, and a change goes there in
    the same commit.

## Prove

```sh
cargo test -p box-daemon config::
```

It exits 0. Tests cover the search order with temporary directories and the
variable. They cover an unknown key with its line, a missing named file, and a
key `api_key`. Other tests parse the full example of docs/cli.md, refuse the
key `memory_mib`, and check each value of the built-in profile. Any host.

## Start condition

T-033.

# T-028: Daemon skeleton and the control API

**Source:** The planning brief of 2026-10-14, sections 2 (P1) and 8; errand src/lock.ts.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The daemon holds the instances, the credentials and the network of every guest.
Two daemons would both act on every instance. A control socket that another
local user can reach gives that user every instance.

## Premise

- Design: the control API is JSON-RPC 2.0 over a Unix socket in
  `$XDG_RUNTIME_DIR`. On Windows it is a named pipe with a DACL for the user
  (brief section 8).
- Design: the daemon holds a single-instance lock for its life, not a PID file.
  The CLI starts the daemon when it does not answer.
- Design: decision P1 puts one tokio runtime in the daemon.
- READ: errand locks with a PID file and a probe signal, so a reused PID can
  block a restart
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/lock.ts:1-98`).
- READ: errand exits 2 for a refused configuration, 3 for an unenforceable
  guarantee and 4 when it already runs
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:packaging/errand.service:33-40`).

## Approach

1. Put `run()` in crates/box-daemon/src/lib.rs. `agent-box daemon` calls it.
2. On Linux, put the socket at `$XDG_RUNTIME_DIR/agent-box/daemon.sock`, in a
   directory with mode 0700.
3. If `XDG_RUNTIME_DIR` is not set, refuse to start. Never fall back to /tmp.
4. On accept, read SO_PEERCRED and close a peer with another uid.
5. On Windows, name the pipe after the user's SID.
6. Give the pipe a DACL for the user only, and create it with
   FILE_FLAG_FIRST_PIPE_INSTANCE.
7. In the client, check that the user of the pipe server is the current user.
8. Take the lock with `File::try_lock` of std on a file in the state directory.
9. Hold the lock for the life of the process. A second daemon names the
   holder and exits 2, as docs/cli.md gives.
10. On Windows, also take the named mutex of docs/cli.md. `File::try_lock`
    uses LockFileEx there, which covers the other logon sessions.
11. Read one JSON-RPC object per line. Refuse a line above 8 MiB with error
    -32600, and close. [docs/cli.md](../docs/cli.md#the-control-api) is the
    home of that cap.
12. Map each CLI verb to one method, for example `instance.start`.
13. Answer an unknown method with -32601.
14. In the client, start the daemon when the connect fails.
15. On Linux, start it in a new session, with stdio to a log file.
16. On Windows, start it detached and outside the job object of the CLI.
17. Retry the connect with backoff for up to 10 s, a design value.
18. Use the daemon exit codes of docs/cli.md: 0 after SIGTERM, 2 when another
    daemon holds the lock, and 10 or 11 on a gap.
19. Decide whether the daemon exits when it is idle, the open question that
    docs/cli.md gives this entry. Record the answer in docs/cli.md.

## Prove

```sh
cargo test -p box-daemon api::
```

It exits 0. A test starts one daemon on a temporary runtime directory. A second
`run()` returns the already-running error. On Linux, a peer with another uid is
refused. Any host.

## Start condition

T-033.

# T-029: CLI skeleton: verbs, exit codes, --json

**Source:** The planning brief of 2026-10-14, section 8; TEMPLATE docs/conventions/shell.md on Git Bash.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

Scripts and the later chat gateway drive the CLI. They need a stable verb set,
stable exit codes and one JSON document per call. A verb that does nothing yet
must say so, and must not exit 0.

## Premise

- Design: the verbs are setup, uninstall, doctor, image, create, list, inspect,
  start, stop, pause, resume, delete and attach.
- Design: the other verbs are run, forward, logs, policy, secret and daemon.
  Each verb takes `--json` (brief section 8).
- Design: an unimplemented verb exits 70 and names its entry.
- READ: Git Bash rewrites an argument with a leading slash into a Windows path.
  Use `NAME:/path` operands
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/conventions/shell.md:249-297`).
- READ: with `--json`, the JSON document alone goes to stdout
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/common/check-remote-items.sh:52-56`).
- Design: the CLI is synchronous except for its daemon client (P1).

## Approach

1. In crates/box-cli/src/args.rs, parse the verbs with clap, derive style.
2. Give every verb the flags of docs/cli.md.
3. Validate instance names with `InstanceName` of box-core at parse time.
4. Write guest paths as `NAME:/path` operands.
5. With `--json`, print exactly one JSON document on stdout, and all other text
   on stderr.
6. Use the exit code table of docs/cli.md, the one home of the codes.
7. Map each unimplemented verb to its entry.
8. The lifecycle verbs map to T-170. run maps to T-171, and attach to T-172.
9. forward and policy map to T-173. setup and uninstall map to T-174.
10. doctor maps to T-175, logs to T-176, image to T-067 and secret to T-140.
11. Print "not implemented yet" with the entry id, and exit 70.
12. With `--json`, print the error document of docs/cli.md with the kind
    `not-implemented` and the entry id.

## Prove

```sh
cargo test -p box-cli
cargo run -q -p box-cli -- --help
```

The tests pass. Each verb parses with `--json`, and each unimplemented verb exits
70 with its entry id. The help text lists every verb. Any host.

## Start condition

T-033.

# T-030: The host never runs a tool against guest-written bytes

**Source:** [errand issue 2](https://github.com/QaidVoid/errand/issues/2), reported against a running deployment; the planning brief, section 3 (R-HOST-NO-TOOLS).
**Category:** feature
**Milestone:** M1
**Priority:** P0
**Effort:** S
**Status:** open

## Problem

The daemon runs unconfined and holds every credential. A tool that the daemon
runs on bytes the guest wrote is the guest's code on the host. git, package
managers and linters read configuration from the directory they run in.

## Premise

- READ: errand ran git on the host in the agent's project directory. A hook
  that the agent wrote ran on the host, as reported in issue 2.
- READ: at the pin, errand pushes from a fresh bare clone with hooks off. It
  still runs host git in the agent tree for three reads
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/pr.ts:97-169`).
- READ: errand reads and truncates a file through a symbolic link that the
  agent can plant
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/memory/store.ts:222-269`).
- READ: errand resolves a `gitdir:` line that the agent wrote on the host
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/session/pr.ts:158-184`).
- READ: an untrusted project file of kage still sets the shell program
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-core/src/trust.rs:110-126`).
- Design: ruling D4 moves project files only through a git forge. The host holds
  no project bytes.

## Approach

1. Write the rule in docs/architecture.md and SECURITY.md.
2. The rule: no host process interprets a guest-written byte as a path, a
   command, a configuration or a repository.
3. List the guest-written bytes that reach the host: frames on ports 5000 to
   5005, ACP messages and event records.
4. Also list broker request bodies, git pack streams and the serial console in
   diagnosis.
5. Treat each one as bounded data. T-146 parses only the ref-update commands of
   a push, never the pack.
6. Write a new check, planned file xtask/src/check/host_tools.rs, named
   host-tools.
7. Register it in `xtask/src/check/mod.rs`, so the gate runs it.
8. Scope: each .rs file under crates/box-daemon/src, crates/box-net/src and
   crates/box-broker/src.
9. Floor: all three directories exist. Otherwise the check exits 2.
10. Report each line with `Command::new`, `process::Command`, `tokio::process`,
    `CreateProcess`, or an exec call of libc or nix.
11. Skip a line whose file and exact text are on the allowlist of the check.
12. Give each allowlist row a reason. Report a row that matches no line, so that
    the list cannot go stale.
13. Report a dependency of these three crates on git2 or gix.
14. Start with an empty allowlist. The daemon reaches the helper by socket and
    needs no subprocess.
15. Name errand issue 2 in the header comment of the check.
16. Add a self-test that plants `Command::new("git")` in a fixture and expects a
    finding.

## Prove

```sh
cargo xtask check host-tools
cargo test -p xtask
```

Both exit 0. Then plant `let _ = std::process::Command::new("git");` in
crates/box-daemon/src/lib.rs. `cargo xtask check host-tools` exits 1 and names
the line. Revert the plant. Any host.

## Start condition

T-033.

# T-031: A trusted event channel the guest cannot forge into

**Source:** [errand issue 2](https://github.com/QaidVoid/errand/issues/2), secondary section; the planning brief, section 3 (R-TRUSTED-CHANNEL); the research document, section 5.3.
**Category:** feature
**Milestone:** M1
**Priority:** P0
**Effort:** M
**Status:** open

## Problem

Authorization, billing, backoff and lifecycle decisions need facts that the
agent cannot write. Every guest process can open a vsock connection to the host.
The kage stream is reachable by every process at the agent's uid.

## Premise

- READ: in errand, `/proc/1/fd/1` was writable, and forged events reached the
  transcript, as reported in issue 2.
- READ: in a microVM, any guest process at kage's uid reaches kage's stdin and
  stdout through /proc. The nonce does not protect the kage stream.
- READ: vsock carries no peer credential, so any guest process can connect to
  any port. A host-issued nonce is the mechanism
  ([docs/research/microvm-tier.md](../docs/research/microvm-tier.md), section
  5.3).
- READ: the nonce is 32 bytes from the host CSPRNG, compared in constant time. A
  refused frame closes the connection and increments a counter.
- READ: errand starts a global backoff from an agent-reported retry event, so
  one session can stall all
  (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/admission/scheduler.ts:214-240`).
- Design: in the brief, the guest connects to host port 5002, and the first
  frame carries the event nonce. The host is the listener.
- WARNING: a guest process that reaches uid 0 can read the nonce. The claim
  covers processes at the agent's uid only.

## Approach

1. In box-guest (T-041), hold the event nonce in memory only. It arrives in
   `PROVISION` on port 5000.
2. Make box-guest not dumpable, so that a process at uid 1000 cannot read its
   memory or environment.
3. Never put the nonce in argv, the environment, a file or the kernel command
   line.
4. After `PROVISION`, connect box-guest to host port 5002. Send `EVENT_AUTH`
   with the version and the nonce.
5. Then send the event frames of docs/protocol.md, with a sequence number that
   starts at 1 and grows by 1.
6. On the host, in crates/box-daemon/src/events.rs, read the 5-byte header of
   the first frame.
7. Refuse unless the type is `EVENT_AUTH` and the length is the fixed auth
   length. Read nothing more before that check.
8. Compare the nonce in constant time with the subtle crate.
9. On a mismatch, close, increment `refused_forgeries` of the instance, and log
   a host-observed event.
10. Accept one authenticated connection per instance. Refuse and count a second
    one.
11. Close the connection on a sequence gap or repeat, and count it.
12. In box-core, add `Source` with the values HostObserved, GuestAgent and
    AgentReported.
13. Label every event record with its source. Label all kage ACP data
    AgentReported.
14. Make accounting (T-147), backoff (T-252) and lifecycle refuse AgentReported
    input with a typed error.
15. Show the counter in the startup report (T-175) and in `agent-box inspect
    --json`.
16. Rotate the nonce with `ROTATE` kind 1 on port 5000. Accept the new nonce
    only after box-guest answers `ROTATED`.
17. Write the stated limit of the Premise into docs/limits.md.

## Prove

```sh
cargo test -p box-daemon --test events
```

It exits 0 with the T-032 double. Any host. A connection without the nonce is
refused before a payload read. A header that announces 4 GiB is refused with
no large allocation.

A wrong nonce is refused and counted. The right nonce is accepted. A second
connection and a sequence gap are refused. An AgentReported usage event changes
no counter.

The live check is `forged_event_refused` in T-088 and T-105: a process at uid
1000 in the guest is refused. T-180 repeats it as its check 6.

## Start condition

T-022 and T-033.

# T-032: The Engine trait, the capability report and a test double

**Source:** The planning brief of 2026-10-14, section 4; the research document, section 1.1 (C4, C5); TEMPLATE docs/conventions/code.md.
**Category:** feature
**Milestone:** M1
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

The daemon must drive two engines with one code path. M1 has no engine yet, so
tests need a double. A suite where every test uses the double never tests the
branch that ships.

## Premise

- Design: the brief gives the trait: probe, create, start, pause, resume, stop,
  delete, inventory, connect and listen.
- READ: the probe attempts the real setup and does not read configuration. A gap
  stops the daemon. There is no fallback between backends
  ([docs/research/microvm-tier.md](../docs/research/microvm-tier.md), section 1.1).
- READ: test the production default of every injectable seam
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/conventions/code.md:141-155`).
- Design: box-guest is a Linux binary, so the double runs on Linux only.
- Design: the trait of the brief is synchronous, and decision P1 makes the
  daemon async on tokio. This entry owns that open question.

## Approach

1. In crates/box-daemon/src/engine.rs, write the trait with the signatures of
   the brief.
2. Open question: the daemon calls a synchronous trait on the blocking pool
   of tokio, or the trait becomes async. Decide it with the double and the
   transport of T-024.
3. Record the answer in docs/decisions.md and in docs/architecture.md.
4. Define `CapabilityReport` with the engine name, its version, facts, notes and
   gaps.
5. Define `Unavailable` with the list of gaps.
6. Define `EngineError` with variants for a helper refusal, a transport error, a
   wrong state, not found, a timeout and IO.
7. Define `Removal` as a list of absences. Each absence names its item and
   whether a check verified it.
8. Define `Observed` with the id, the `State`, and the VMM pid or runtime id.
9. Take `Stream` and `Listener` from box-transport.
10. Write `engine_for_host()`: Firecracker on Linux, HCS on Windows. No flag
   selects another engine.
11. Let tests inject an engine with `Daemon::with_engine`. `Daemon::new` always
   calls `engine_for_host()`.
12. At start, call probe. On a gap, exit 10 (engine) or 11 (helper) with the
    gap list, as docs/cli.md gives. Print the notes.
13. Write the double in crates/box-daemon/src/engine/double.rs.
14. Double start: run box-guest as a child with `--test-transport unix:DIR`
    (T-040).
15. In the double, serve the hybrid-vsock handshake on Unix sockets, so that the
    code of T-024 runs unchanged.
16. Double pause and resume: send SIGSTOP and SIGCONT to the child.
17. Give the double a fault switch that makes inventory fail. The state is then
    `unknown`.
18. Test that `Daemon::new` selects the right engine for the OS of the test.

## Prove

```sh
cargo test -p box-daemon engine::
```

It exits 0 on any Linux host. On Windows, run
`cargo xtask linux -- cargo test -p box-daemon engine::`. The tests cover each
verb on the double, `unknown` on a failed inventory, and the production default.

## Start condition

T-033.

# T-033: Create the product crates with the edges of the crate graph

**Source:** Ruling D14 of 2026-10-14; the planning brief, section 11.
**Category:** chore
**Milestone:** M1
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

The planning session built only the tooling (D14). The implementation session
needs the crates, with the edges of the graph, before any code. A crate created
later with a wrong edge is harder to fix than one created right.

## Premise

- Design: today `Cargo.toml` has one member, xtask.
- Design: `xtask/src/layering.rs` already holds the layer table of every planned
  crate.
- Design: the helper protocol types live in box-proto, module `helper`.
- Design: `unsafe_code` is deny in the workspace lints of `Cargo.toml`, not
  forbid. So a named module can allow unsafe code for itself (main-session
  ruling on unsafe code).
- Design: the AF_HYPERV module of box-transport (T-025) and the hcs module of
  box-vmm (T-102, T-103) need `unsafe`. T-100 proposes a third module in
  box-helper.

## Approach

1. Create these crates under crates/: box-core, box-proto, box-policy,
   box-image, box-transport, box-vmm and box-net.
2. Create also box-broker, box-daemon, box-cli, box-helper and box-guest.
3. Add each crate to the members of `Cargo.toml`.
4. Name the binaries: agent-box for box-cli, agent-box-helper for box-helper,
   and box-guest for box-guest.
5. Give each crate a crate doc in STE that names its layer and its purpose.
6. Take the package fields and the lints from the workspace.
7. Add the path dependencies exactly as docs/architecture.md gives them.
8. Make each binary print "not implemented yet" with its entry, and exit 70.
9. agent-box names T-029, agent-box-helper names T-081 and T-100, and box-guest
   names T-040.
10. Keep `unsafe_code` at deny. Every crate takes the workspace lints with
    `[lints] workspace = true`. No crate allows unsafe code yet: decision P6
    in docs/decisions.md lists the files that may, and T-259 checks the list.
11. Run `cargo update --workspace`, so that `Cargo.lock` gains the new crates
    and no other version moves.
12. Add one test per binary that runs it and expects exit 70 and its entry ids.
13. Update every statement that says no product crate exists. Find them with
    the command below, and change each to the new state in the same commit:

    ```sh
    git grep -n -i -E 'no product (code|crate)|crates/ directory|not there yet|do not exist yet|does not exist yet' -- '*.md'
    ```

14. Run `cargo todo remap` on each edited document that entries cite.
15. Write no other code. The unsafe check is T-259, and the musl build of
    box-guest in the gate is T-260.

## Prove

```sh
cargo xtask layering
cargo xtask gate --strict
cargo run -q -p box-cli -- list; echo "exit $?"
```

The layering check exits 0 with every crate in the table. The gate exits 0. The
binary prints its entry id and `exit 70`. The grep of step 13 finds no stale
statement. Any host. On Windows, run the block in Git Bash, or run each command
in PowerShell and read `$LASTEXITCODE`.

NOTE: every other M1 entry starts after this one.

# T-256: No secret in the guest image, the disk, the kernel command line, MMDS or the VMM argv

**Source:** The planning brief of 2026-10-14, section 3 (R-NO-SECRET-IN-GUEST); the main-session ruling of 2026-10-14 that makes this entry the owner of the rule.
**Category:** feature
**Milestone:** M1
**Priority:** P0
**Effort:** S
**Status:** open

## Problem

Every guest process can read the kernel command line, MMDS and the files of
the image and the disks. The host process table shows the VMM argv. A nonce or
a token in any of these places is in the hands of the agent, or of another
local user. No entry owned this rule, and no Prove checked the command line,
MMDS or the argv.

## Premise

- Design (brief section 3): no credential ever goes into the guest image, the
  guest disk, the kernel command line, MMDS or the VMM argv.
- Design: nonces, the egress token and the kage token go only over port 5000,
  after boot and before kage starts. box-guest keeps them in memory. The kage
  token file is on a tmpfs, never on the data disk (T-046).
- READ: the guest reads its command line in /proc/cmdline. Firecracker appends
  the root device to `boot_args`
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/vmm_config/boot_source.rs:74-86`).
- Design ([docs/protocol.md](../docs/protocol.md), section "Provisioning"):
  the page lists the places where a provisioned value never goes.
- Design: in docs/architecture.md, box-vmm depends on box-core only. A type
  of box-proto is out of its reach, so a builder of box-vmm cannot take one.
- Design: the builders of the VMM configuration arrive in M3 (T-084) and M4
  (T-101). The image build arrives in M2 (T-062). Their checks are steps of
  those entries, and they name this entry.

## Approach

1. In crates/box-proto/src/provision.rs, define `ProvisionedValue`: at most
   256 bytes in a `Zeroizing` buffer.
2. Give it no `Display`, no `Serialize` and no `Clone`. Its `Debug` prints
   `[redacted]`.
3. Give it one read method, `expose()`. Only the codec of `PROVISION` and
   `ROTATE` (T-022) and the users of the value call it.
4. Use the type in box-daemon and box-guest for the event nonce, the egress
   token, the kage token and each provider nonce of `PROVISION`. The broker
   keeps its own redacted type (T-140).
5. Add a rule to the layering table of `xtask/src/layering.rs`: box-vmm never
   depends on box-proto. The builders then cannot name the type.
6. Add a test helper in crates/box-proto/src/testing.rs, behind a `testing`
   feature: `assert_absent(haystack, values)`.
7. The helper searches for each value in raw, hex and base64 form. The
   entries below call it.
8. T-041: `no_secret_on_disk` scans the files that box-guest wrote, and in
   T-088 and T-105 the raw data disk.
9. T-062: the image build scans rootfs.tar with the rules of
   `xtask/src/check/secrets.rs`.
10. T-084: the test `boot_args_carry_no_value` builds every input and scans
    each emitted body. The builder sends no MMDS call.
11. T-101: the test `kernel_cmd_line_carries_no_value` scans each golden HCS
    document.
12. T-088 and T-105: the live test `no_secret_in_cmdline` scans the guest
    /proc/cmdline and the VMM argv, or the HCS document.
13. T-180: the battery repeats the scan of step 12 from outside, on both
    engines.
14. Write the rule and this list into docs/architecture.md, beside the rule.

## Prove

```sh
cargo test -p box-proto provision::
cargo test -p box-proto --features testing testing::
cargo xtask layering
```

All three exit 0. A test shows that `format!("{:?}", value)` prints
`[redacted]`. A compile-fail test shows that `ProvisionedValue` has no
`Display` and no `Serialize`.

The helper finds a planted value in raw, hex and base64 form, and finds
nothing in a clean buffer. A planted edge from box-vmm to box-proto fails the
layering unit test. Any host.

## Start condition

T-022 and T-033.
