# The CLI

This area covers the verbs that the operator types: the lifecycle verbs,
`run`, `attach`, profiles, `policy`, `forward`, `setup`, `uninstall`,
`doctor` and `logs`. [docs/cli.md](../docs/cli.md) is the reference for the
verbs, the states, the exit codes and the configuration file.
[docs/architecture.md](../docs/architecture.md) gives the processes behind
each verb.

Every verb is a client of the daemon control API (T-028). The exceptions are
`setup`, `uninstall`, and `doctor` when no daemon runs. Every verb takes
`--json`. The product crates do not exist yet. T-033 creates them, so each
crate path in this file is a planned path.

# T-170: Lifecycle verbs end to end

**Source:** brief section 8 (verbs, states, kinds); ruling D1; agent-sandbox KEP-119 and KEP-694; TEMPLATE lifecycle lessons.
**Category:** feature
**Milestone:** M7
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The engines (T-088 to T-091, T-105 to T-107) give the daemon its
operations. The operator needs verbs that drive them with one behaviour on
both OSes. A persistent instance holds a 30 GiB data disk by default and may
run an agent. A verb that shows a failed query as `stopped` or `running` gives
the operator a wrong picture of it.

## Premise

- Design (brief section 8): the verbs are `create NAME [--profile P]
  [--cpus N] [--memory M] [--disk 30G]`, `list`, `inspect NAME`, `start`,
  `stop`, `pause`, `resume` and `delete`. Each takes `--json`.
- Design (brief section 8): the states are `creating`, `stopped`, `starting`,
  `running`, `paused`, `stopping`, `deleting` and `unknown`. The state is an
  enum everywhere, never a boolean.
- Design (brief section 8): the kinds are `persistent` (the operator creates
  it) and `ephemeral` (created for one task and deleted at its end).
- Design (brief section 4.1): pause is in memory. When the VMM or the host
  goes away, the pause is lost and the instance shows `stopped`.
- READ: KEP-119 keeps an explicit Unknown state, because a failed reconcile
  does not prove that the workload is gone
  (`kubernetes-sigs/agent-sandbox@4b63868f0d67e4ee0fa690bb14cb5722b78d679d:docs/keps/119-sandbox-suspended-state/README.md:26-31`).
- READ: KEP-694 records an upgrade trap. A new state field defaulted to
  Running, so suspended sandboxes resumed after an upgrade
  (`kubernetes-sigs/agent-sandbox@4b63868f0d67e4ee0fa690bb14cb5722b78d679d:docs/keps/694-kep-for-suspend-and-resume-for-beta/README.md:99-113`).
- READ: a delete that printed success left multi-gigabyte disks behind. A hard
  interrupt skips cleanup. Measure free space before a disk is created
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:226-281`).
  TEMPLATE notes that others reported most of these, so they are inputs, not
  measurements.
- READ: `kage serve` cancels every run on SIGTERM and closes its session files
  before it exits
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/remote.md:46-48`).

## Approach

1. In crates/box-cli/src/cmd/lifecycle.rs, parse the eight verbs with the
   argument shapes of docs/cli.md. Each verb sends one JSON-RPC call to the
   daemon (T-028).
2. In crates/box-daemon/src/lifecycle.rs, keep one state machine per
   instance. docs/cli.md holds the allowed states of each verb. Test the table
   below against it row by row.

| verb | allowed from | transient state | end state | when already in the end state |
| --- | --- | --- | --- | --- |
| create | no record | creating | stopped | refuse: the name exists |
| start | stopped | starting | running | no change, success |
| pause | running | none | paused | no change, success |
| resume | paused | none | running | no change, success |
| stop | starting, running, paused, unknown | stopping | stopped | no change, success |
| delete | stopped; others only with `--force` | deleting | no record | refuse: no such instance |

3. Refuse every other pair with the state-conflict code of docs/cli.md. The
   message names the current state and the verb that leads out of it.
4. Map a failed engine query to `unknown`, never to a guess. From `unknown`,
   allow `inspect`, `stop` and `delete --force` only.
5. Store the state as a tagged enum in a versioned record (T-026). Treat a
   missing or unknown state value as a load error, never as a default.
6. `create`: validate the name (T-020) before any IO. Resolve the profile and
   the flags into an InstanceSpec, and store the resolved spec in the record.
7. A later profile edit does not change an existing instance. `inspect` names
   the profile that the spec came from.
8. `create`: check free space in the store before the helper makes the disk.
   Refuse below a floor of the base image size plus 1 GiB (design value).
9. Report a note when free space is below the disk size, because the sparse
   disk can grow past what the host has.
10. `create`: when a step fails, roll back every earlier step (record, disks,
    uid) and verify each absence, as `delete` does.
11. On daemon start, finish any instance found in `creating` or `deleting`:
    roll back a create, and continue a delete.
12. `stop`: on a paused instance, resume first, because a guest poweroff needs
    running vCPUs. Then send `POWEROFF` on port 5000 and wait `--grace`.
13. The design default of `--grace` is 30 s (not measured). Then kill through
    the engine.
14. box-guest sends SIGTERM to `kage serve` first and waits, so that the
    session files close (T-046).
15. `delete --force` on a live instance runs `stop` first, then the delete.
16. `delete`: print the Removal that the engine returns (brief section 4).
    It lists each verified absence. `--json` carries the same list.
17. A Removal with an item still present is a failure. The CLI names the item
    and exits with the failure code.
18. `list`: print NAME, STATE, KIND, PROFILE, CPUS, MEMORY and DISK. `--json`
    prints an array of records with a `schema_version` field.
19. `inspect NAME`: print the id, name, kind, state, spec, image digests,
    engine, forwards, policy generation, binding names and the refused-forgery
    count (T-031).
20. Never print a nonce, a token or a credential value in any verb.
21. Ephemeral: `create --ephemeral` stores the kind `ephemeral` and the id of
    the task that owns it (an ACP session id, T-171).
22. The daemon deletes an ephemeral instance when its task ends or when the
    instance stops for any reason.
23. After a daemon restart, the daemon re-adopts an ephemeral instance and
    looks for its task. If the task is gone, the daemon deletes the instance.
24. The per-instance lock (T-026) serializes operations. A verb that finds an
    operation in progress waits up to its timeout, then exits with the
    timeout code.

## Prove

```sh
cargo test -p box-cli --test lifecycle
cargo test -p box-daemon lifecycle::
cargo test -p box-cli --test lifecycle_live -- --ignored
```

The first two pass against the T-032 test double. Named tests cover every row
of the transition table and every refusal. Other named tests cover `unknown`
after a failed query and the rollback of a failed create. A record with no
state field is refused at load.

The live test runs create, start, pause, resume, stop, start, stop and delete
on one instance. It checks the state after each verb and every item of the
Removal.

Host: any host for the first two commands. The live test needs a Linux KVM
host (or `cargo xtask linux --kvm`, development only) and Windows 11 with
Hyper-V as admin.

## Start condition

T-029 and T-032 for the tests with the double. T-088 and T-105 for the live
test.

# T-171: `agent-box run`

**Source:** brief sections 5 and 8; rulings D2 and D7; kage ACP schema and serve documentation.
**Category:** feature
**Milestone:** M7
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

Agents run unattended by default (D7). The operator, scripts and later the
chat gateway need one headless call. It sends a prompt to kage in an
instance, follows the turn, and ends with a result and an exit code. A run
that can wait forever breaks scripts. A run that reports the agent's own words
as host facts breaks R-TRUSTED-CHANNEL.

## Premise

- Design (brief section 5): kage runs as `kage serve` on guest
  127.0.0.1:7433 as uid 1000. box-guest bridges port 5004 to it. The daemon is
  the ACP client (T-161).
- READ: the result of `session/prompt` carries `stopReason`, one of
  `end_turn`, `max_tokens`, `max_turn_requests`, `refusal` and `cancelled`
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-acp-wire/src/schema.rs:1208-1231`).
- READ: a session runs one prompt at a time. A prompt from another client
  fails with a busy message. `session/cancel` cancels the running prompt
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/remote.md:160-166`).
- READ: when a tool needs approval, `kage serve` asks every attached client,
  and the first answer wins
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/remote.md:151-158`).
- READ: kage events carry a session id and a per-session sequence number
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/reference/architecture.md:47-69`).
- Design (R-TRUSTED-CHANNEL): every byte from kage is agent-reported data. Any
  guest process at kage's uid can forge it.
- READ: every command that can wait forever has a hard time limit. A timeout
  and a kill are different facts from "not installed"
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/conventions/shell.md:422-437`).
- READ: watch a guest command with a heartbeat on silence, not on a timer
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:201-222`).

## Approach

1. Verb shape, as docs/cli.md gives it: `agent-box run NAME[:/path] --prompt
   TEXT [--json] [--timeout SECONDS] [--session ID] [--start]`.
2. `--prompt -` reads the prompt from stdin to the end of input. A long prompt
   in argv hits the Windows command-line limit and shows in process lists.
3. `run --ephemeral [--profile P] --prompt TEXT` takes no NAME. It creates an
   ephemeral instance (T-170), starts it, runs the prompt, and deletes it.
4. The ephemeral delete also happens on failure, on timeout and on Ctrl-C.
5. The CLI calls `run.start` on the daemon. The daemon refuses unless the
   instance is `running`. With `--start`, the daemon starts a `stopped`
   instance first.
6. The daemon refuses a `paused` instance, because the operator paused it on
   purpose.
7. The daemon opens or reuses the ACP connection over port 5004 with the
   provisioned kage token (T-161).
8. With `--session ID`, the daemon sends `session/load`. Without it, the
   daemon sends `session/new` in the work directory of the image (T-063).
9. The daemon sends `session/prompt`. It turns each `session/update` into an
   event record of T-176 with `source: agent-reported`.
10. The daemon adds host-observed records: state changes, the start and end
    of each broker request of this instance (T-147 counts, no bodies).
11. After 30 s with no record, the daemon adds a heartbeat record. It says
    whether a broker request of this instance is in flight.
12. The daemon answers each `session/request_permission` with allow and logs
    it with the tool name. Plan mode asks for commands even in `allow` mode
    (T-161). The instance and the host policy are the boundary (T-160).
13. Without `--json`, render agent text with every control character escaped
    except newline and tab. Agent text is guest data, and an escape sequence
    can set the clipboard or the window title.
14. With `--json`, print one event record per line on stdout. The last line
    has `"type": "end"`, the final `ok` and the `exit` code, as docs/cli.md
    gives. Nothing else goes to stdout.
15. The `end` line also carries the result: the stop reason (agent-reported),
    the elapsed time (host-observed) and the broker counts of the turn
    (host-observed).
16. Outcomes that need a code in docs/cli.md: `end_turn`; another stop
    reason; `cancelled`; timeout; the instance left `running`; the daemon or
    bridge connection lost; busy.
17. The test asserts the mapping from each outcome to its code. docs/cli.md
    is the one home of the numbers.
18. `--timeout` bounds the whole turn. The default is a design value of 6 h
    (not measured). A config key for it needs a row in docs/cli.md first.
19. At the bound, the daemon sends `session/cancel` and waits 10 s for
    `cancelled`. Then the CLI reports the timeout.
20. The first Ctrl-C sends `session/cancel` and waits for the stop reason. A
    second Ctrl-C exits at once. The turn goes on in the guest.
21. When the CLI process dies, the daemon keeps the turn. `agent-box logs
    NAME --follow` (T-176) shows the rest.
22. When a turn already runs in the session, the daemon refuses the prompt
    with the busy outcome. It does not queue in v1.
23. Bound every ACP message from kage before the daemon parses it (T-161 sets
    the cap). An oversize message ends the connection and makes a
    host-observed record.

## Prove

```sh
cargo test -p box-cli --test run
cargo test -p box-daemon run::
cargo test -p box-cli --test run_live -- --ignored
```

The first two use a recorded ACP peer (T-161). Named tests cover each stop
reason and its code, the timeout path, and the first and the second Ctrl-C.
Other named tests cover an approval ask answered with allow and logged, an
oversize message, `--prompt -`, and `--ephemeral` cleanup on failure. In the
`--json` test, every stdout line parses, and the last one has `"type": "end"`.

The live test completes one prompt against a provider through the broker
(T-143). It checks that the `end` line carries a broker request count above
zero.

Host: any host for the first two commands. The live test needs a Linux KVM
host and Windows 11 with Hyper-V as admin, with one provider credential set
through `agent-box secret set`.

## Start condition

T-046, T-143, T-161 and T-176.

# T-172: `agent-box attach`

**Source:** brief sections 7 and 8; ruling D7; kage serve documentation; TEMPLATE notes on watching a guest.
**Category:** feature
**Milestone:** M7
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The operator can attach a terminal when they want (D7). `vi`, `less`,
Ctrl-C and window resize must work on Linux and on Windows. The attach must
not give the guest more reach into the host terminal than an ssh session
gives.

## Premise

- Design (brief section 7): port 5001 carries the PTY and exec streams. T-042
  owns the guest side: allocation, resize, signals and exit codes.
- READ: `kage resume <id>` or `kage resume --last` attaches the TUI to a
  session that `kage serve` hosts. Its prompts and approvals go to serve
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/editors/remote.md:168-194`).
- READ: the TUI reaches serve over a unix socket that checks for the same
  user, with no token. It works on unix systems only (same lines).
- READ: when serve stops, the TUI says that the session detached and takes no
  more prompts (same lines).
- Design, not measured: the Windows console is not a Unix TTY. Raw input needs
  console modes. mintty, the Git Bash terminal, gives the program a pipe, not
  a console.

## Approach

1. Verb shape: `agent-box attach NAME [--shell] [--user agent|root]
   [--detach-keys KEYS]`.
2. Without `--shell`, the target is the kage TUI: `kage resume --last` as
   uid 1000.
3. With `--shell`, the target is `/bin/bash -l` as the chosen user. The
   default user is `agent` (uid 1000).
4. The CLI calls `instance.attach` on the daemon (T-028). The daemon opens a
   PTY session on port 5001 and relays its frames over the control
   connection.
5. The CLI never opens a vsock socket itself. The daemon owns every guest
   channel.
6. The open request carries the argv, the user, the window size, and `TERM`
   from the host (default `xterm-256color`). No other host environment
   variable crosses.
7. Unix host: put stdin in raw mode (cfmakeraw). Restore it on every exit
   path: normal exit, error, panic hook and SIGTERM.
8. Unix host: send a resize frame on SIGWINCH.
9. Windows host: set the input mode to ENABLE_VIRTUAL_TERMINAL_INPUT and
   ENABLE_WINDOW_INPUT.
10. Windows host: clear ENABLE_PROCESSED_INPUT, ENABLE_LINE_INPUT and
    ENABLE_ECHO_INPUT.
11. Windows host: set the output mode ENABLE_VIRTUAL_TERMINAL_PROCESSING, and
    set both console code pages to UTF-8 (65001).
12. Windows host: restore every console mode and code page on every exit
    path.
13. Windows host: read input with ReadConsoleInputW. Send key-down characters
    as UTF-8 bytes.
14. Windows host: turn WINDOW_BUFFER_SIZE_EVENT into a resize frame with the
    size of the visible window, not of the buffer.
15. Windows host: when GetConsoleMode fails on stdin, as under mintty, refuse.
    The message names Windows Terminal and `winpty agent-box attach NAME`.
16. In raw mode, Ctrl-C reaches the guest as byte 0x03. The guest line
    discipline signals the foreground process group.
17. Detach: the default key sequence is ctrl-p then ctrl-q, as other
    container tools use. The CLI holds a lone ctrl-p and sends it with the
    next key.
18. `--detach-keys` and a key in the config file change the sequence.
19. Detach from the kage TUI closes the TUI client only. The kage session and
    its run go on in the guest.
20. Detach from `--shell` closes the PTY. The shell gets SIGHUP, as with ssh.
21. Exit code: the remote exit status, or 128 plus the signal number. A
    detach exits 0 and prints one line.
22. A lost daemon connection exits with the code that docs/cli.md gives for a
    lost connection. A daemon restart ends the attach, and the CLI says so.
23. Pause and resume keep the session open (T-089, T-106).
24. Bytes from the guest reach the host terminal unchanged, as with ssh. Add
    this limit to docs/limits.md: a hostile guest can send terminal escape
    sequences to an attached terminal.

## Prove

```sh
cargo test -p box-cli --test attach
cargo test -p box-cli --test attach_live -- --ignored
```

The first test runs the CLI in a pseudo terminal (openpty on Unix,
CreatePseudoConsole on Windows) against the T-032 double. It checks raw mode
on, raw mode restored after a panic, resize frames, the detach sequence, a
held ctrl-p, and the exit code.

The live test drives `vi`, `less`, `sleep 100` with Ctrl-C, and a resize
that `stty size` reads back. On Windows it runs in Windows Terminal and in
conhost. It is also the live check of T-042, the guest side of the PTY.

Host: any host for the first command. The live test needs a Linux KVM host
and Windows 11 with Hyper-V as admin.

## Start condition

T-042, T-046, T-089 and T-106.

# T-173: Profiles, `agent-box policy` and `agent-box forward`

**Source:** brief sections 6.2, 6.3 and 8; bailey's rule on policy found in a work directory.
**Category:** feature
**Milestone:** M7
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

The operator sets what an instance gets in two places: a profile at create
time, and the egress policy at run time. A dev server in the guest needs a
host port. Each of these widens what the guest can reach. The verbs must make
every widening explicit and visible.

## Premise

- Design (brief section 8): profiles are named templates in the config: cpus,
  memory, disk size, image variant, egress policy and bindings. T-027 parses
  them, and an unknown key is a refusal.
- Design (brief section 6.2): the modes are `deny`, `allowlist` (the default)
  and `open`. Reserved ranges are refused in every mode unless the operator
  writes an explicit rule.
- Design (brief section 6.3): a wildcard does not match its apex.
- Design (brief section 6.2): `agent-box forward NAME
  [ADDR:]HOSTPORT:GUESTPORT`. The default bind is 127.0.0.1. A public bind
  needs an explicit flag and is reported. T-125 owns the netstack side.
- READ: Firecracker takes 1 or an even number of vCPUs, at most 32 (the
  research document, section 5.4, from the pinned API specification:
  [docs/research/microvm-tier.md](../docs/research/microvm-tier.md)).
- READ: a policy file found in a directory is not policy until the operator
  accepts it
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:docs/security/limitations.md:155-159`).
  So nothing inside the guest or the forge widens host policy.

## Approach

1. `create --profile P` copies the resolved profile into the record (T-170).
   Flags override single fields. The profile `default` applies when
   `--profile` is absent.
2. Validate at config load (T-027) and again at create: cpus per engine,
   memory above the image floor, and an image variant present in
   images/pins.toml.
3. Firecracker cpus: 1 or an even number, at most 32. HCS cpus: 1 up to the
   host's logical processors.
4. `policy show NAME [--json]` prints the mode, the rules, the policy
   generation, and each binding's name, upstream host, methods and path
   scope.
5. `policy show` never prints a nonce, a token or a credential value.
6. `policy set NAME` takes `--mode MODE`, `--allow NAME:PORT`, `--remove
   NAME:PORT`, or `--file PATH` with a TOML policy document. `--file -`
   reads stdin.
7. box-policy validates the result with the schema of T-123. An unknown key
   is a refusal.
8. The daemon applies a change atomically: one generation bump, then RST on
   revoked TCP flows (T-123). The record stores the new policy for the next
   start.
9. Mode `open` needs `--allow-open`. An explicit rule for a reserved range
   needs `--allow-reserved`.
10. `inspect`, `doctor` and the startup report name every instance in mode
    `open` and every reserved-range rule.
11. Policy input comes only from the operator: argv, a file that the operator
    names, or the config file.
12. `forward NAME SPEC` parses `[ADDR:]HOSTPORT:GUESTPORT`. ADDR is an IPv4
    literal or a bracketed IPv6 literal. The default ADDR is 127.0.0.1.
13. HOSTPORT 0 lets the daemon pick a free port. The CLI prints the port.
14. A non-loopback ADDR needs `--public`. `inspect`, `doctor` and the startup
    report name each public forward.
15. `forward NAME --list` lists the forwards. `forward NAME --remove
    HOSTPORT` removes one.
16. The record stores the forwards. The daemon opens them again at each start
    and after a re-adoption.
17. A host port in use is a refusal. The message names the holder when the
    daemon knows it.

## Prove

```sh
cargo test -p box-cli --test profiles
cargo test -p box-cli --test policy
cargo test -p box-cli --test forward
```

Named tests cover each refusal: an unknown profile key, three cpus on
Firecracker, and `open` without `--allow-open`. Other named tests cover a
public forward without `--public`, a malformed SPEC and a port in use. The policy tests check the
generation bump. A scan of `policy show --json` output finds no planted nonce.

Host: any host, with the T-032 double.

## Start condition

T-027, T-123 and T-125.

# T-174: `agent-box setup` and `agent-box uninstall`

**Source:** rulings D3 and D11; brief sections 4.1, 4.2 and 8; TEMPLATE lifecycle and Windows lessons.
**Category:** feature
**Milestone:** M7
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The helper needs root or LocalSystem once, at install time (D3). Everything
else runs as the operator. `setup` must install the helper, its socket or
service, the store and the user service. `uninstall` must remove all of it
and prove the removal. A remover that reports success while files stay
behind leaves a privileged helper on the host.

## Premise

- Ruling D3: admin or root rights are acceptable for a one-time install of a
  privileged helper. The CLI itself runs unprivileged.
- Design (brief section 4.1): the Linux helper is socket-activated by systemd
  and exits when idle. The socket admits the `agent-box` group, and the helper
  checks the SO_PEERCRED uid of the peer. T-081 builds it.
- Design (brief section 4.2): the Windows helper is a LocalSystem service with
  a named pipe whose DACL names the installing user. T-100 builds it.
- Design (brief section 8): `setup` can register the daemon as a user
  service: systemd --user with lingering on Linux, a logon task on Windows.
- Design (brief section 4.1): the jail uses hard links, so the store and the
  jail base must be on one filesystem.
- READ: a delete must read the state back, and a list before and after
  catches a skipped cleanup
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:226-281`).
- READ: PowerShell 5.1 and wsl.exe change argv in ways that broke tools. The
  safer default is a native API call
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:157-161`).
- Design inference, not measured: with the default KillMode=control-group,
  systemd kills what remains in a unit's cgroup when the unit stops. The
  helper exits when idle.
- So the VMM processes must not stay in the helper service's cgroup.
- READ (T-082): systemd delegates on service and scope units only, not on
  slices. T-082 proposes the service `agent-box-vmm.service` as the delegated
  cgroup root.
- Design (docs/engines.md): the engine binaries go to /usr/libexec/agent-box/,
  and the Windows store is %ProgramData%\agent-box\store.
- Design ([docs/cli.md](../docs/cli.md#setup-and-uninstall)): setup has two
  parts. `sudo agent-box setup --system` runs as root once per host. Then
  `agent-box setup` runs as the operator once per user.

## Approach

docs/cli.md is the home of the two parts and their steps. This entry builds
them, and adds the install manifest and the removal report.

Linux, system part (`sudo agent-box setup --system`):

1. Refuse without root. Take the user from `SUDO_USER`, or from `--user NAME`.
2. Run the probes of T-175 first. Refuse on a gap that makes the engine
   unusable, and name the gap.
3. Create the system group `agent-box` if it is absent, and add the user to
   it. Print that the membership needs a new login.
4. Copy the helper from beside the running binary to
   /usr/libexec/agent-box/agent-box-helper, owner root, mode 0755.
5. A release build checks the helper's sha256 against the digest embedded at
   build time (T-200).
6. Install the units that T-081 ships: agent-box-helper.socket
   (/run/agent-box/helper.sock, group agent-box, mode 0660) and the template
   agent-box-helper@.service.
7. Run `systemctl daemon-reload` and enable the socket.
8. Install and start agent-box-vmm.service, the cgroup root of T-082.
9. Write /etc/agent-box/helper.toml, owner root, mode 0644: the uid range,
   the jail base, the cgroup root and the store path.
10. Refuse a uid range that overlaps a passwd entry or a range in /etc/subuid
    or /etc/subgid.
11. Download and verify Firecracker and the jailer (T-083) into
    /usr/libexec/agent-box/. Keep the release's seccomp filter JSON
    files for the battery (T-180).
12. Create the store and the jail base of docs/engines.md on one filesystem.
13. Run `loginctl enable-linger` for the user.
14. Write the system install manifest /var/lib/agent-box/install.json. It
    lists every file, unit, group, membership and linger change, each marked
    created or found.

Linux, user part (`agent-box setup`, as the operator):

15. Check the system part first. When it is missing, exit 2 and print the
    exact command of step 1.
16. Create the state directory. Write
    ~/.config/systemd/user/agent-box.service with ExecStart=agent-box daemon,
    and enable it.
17. Pull the image of the default profile, unless `--no-pull` is given.
18. Write the user install manifest in the state directory. Then run
    `doctor` and print the report.

Windows, system part (`agent-box setup --system`, in an elevated terminal):

19. Refuse without an elevated token. Run the preflight of T-108, and refuse
    on a gap. Without a running hypervisor, exit 10.
20. Do not enable a Windows feature. Print the command for each missing
    feature.
21. Copy the helper to %ProgramFiles%\agent-box\agent-box-helper.exe. ACL:
    SYSTEM and Administrators full control, Users read and execute.
22. Create the service `agent-box-helper` with CreateServiceW: LocalSystem,
    demand start.
23. Set the service DACL so that the operator may start and query the
    service, and do nothing more.
24. Create the store %ProgramData%\agent-box\store with the layout of
    docs/engines.md. ACL: SYSTEM and Administrators, and traverse only for
    `NT VIRTUAL MACHINE\Virtual Machines`. The helper grants VM access per
    file (T-103).
25. Write the helper's settings with the operator's SID (T-100). Write the
    system install manifest %ProgramData%\agent-box\install.json.
26. Call native APIs only. Do not run powershell.exe, cmd.exe or wsl.exe.

Windows, user part (`agent-box setup`, in a normal terminal):

27. Check the system part first, as in step 15.
28. Create the state directory. Register a logon task for the operator that
    runs `agent-box daemon`. Use the Task Scheduler API.
29. Pull the image unless `--no-pull` is given. Write the user install
    manifest, then run `doctor`.

Both:

30. `agent-box uninstall` removes the user part. It refuses while any
    instance record exists, exits 4, and lists them.
31. `--delete-instances` deletes each instance with verified absence first
    (T-091, T-107).
32. With `--purge`, the user part goes with its state: the state directory,
    the image cache and the credential store. Without it, uninstall keeps
    them and prints where they are.
33. `agent-box uninstall --system`, as root or admin, removes the system part.
34. Each part removes what its manifest marks created, in reverse order. It
    never removes an item marked found, and never the config file.
35. Verify each absence and print a removal report. `--json` carries it. An
    item still present is a failure that names the item.

## Prove

```sh
cargo test -p box-cli --test setup
cargo test -p box-cli --test setup_live -- --ignored
```

The unit tests cover the manifest, the reverse order, the created and found
rule, and the uid overlap refusal. Another test runs the user part without
the system part: it exits 2 and prints the command. They run on any host.

The live test records the system state. It runs `setup --system`, `setup`, a
create, a delete, `uninstall --purge` and `uninstall --system`. Then it
records the state again, and the two records are equal.

On Linux the record covers /etc/systemd/system, the user unit directory,
/usr/libexec/agent-box, /etc/agent-box, /var/lib/agent-box, the group
database and the linger state. On Windows it covers the service list, the
task list, %ProgramFiles%\agent-box and %ProgramData%\agent-box.

Host: a disposable Linux KVM host with root, and a disposable Windows 11 with
Hyper-V as admin. The live test changes system state.

## Start condition

T-081, T-082, T-083, T-100 and T-108.

# T-175: `agent-box doctor` and the startup report

**Source:** brief section 8 (startup report); the research document, section 10; TEMPLATE and bailey probe rules.
**Category:** feature
**Milestone:** M7
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

An operator must see what this host gives an instance before they trust it
with an agent. A gap that only a log line mentions is a gap nobody reads.
One report must name the engine, the isolation facts, every absence and every
gap, from probes that ran.

## Premise

- Design (brief section 8): the startup report holds the engine name and
  version, jail or partition facts, and helper availability. It holds the
  guest kernel and image digests, and the disk settings.
- It also holds the egress topology as absences, where each credential lives,
  refused-forgery counts, and every gap by name.
- READ: a probe is not a gate. A missing tool is data. A probe exits 0 when it
  ran and 2 when it could not run. It probes by running, never by presence
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:scripts/doctor/README.md:30-37`).
- READ: probe by attempting the real setup in a throwaway child, not by
  reading sysctls
  (`QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:docs/reference/kernel.md:50-54`).
- READ: name three routes before something is recorded as not doable
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/methodology/sessions.md:243-303`).
- Design (research document, section 10): a change that is not in the report
  is not finished.
- Design (T-032): a gap stops the daemon. There is no fallback between
  engines.

## Approach

1. Division of work: T-092 (Linux) and T-108 (Windows) write the engine
   probes. This entry writes the shared probes, the report type, the verb and
   the rendering.
2. In crates/box-daemon/src/report.rs, define `StartupReport` with
   `schema_version` and these parts: host, engine, isolation, helper, images,
   disks, egress, credentials, instances, gaps and notes.
3. Engine: name, version, and whether the version is inside the supported
   window.
4. Isolation on Linux: the jail facts (uid, gid, cgroup path, applied limits)
   and the seccomp filter in force.
5. Isolation on Windows: the per-VM worker and the partition facts.
6. Both OSes: one sentence that states the isolation difference, in the
   words of SECURITY.md.
7. Egress as absences: no NIC device in the VM configuration, no shared
   filesystem, no host path in the guest, no credential in the guest.
8. Credentials: each credential name and its store backend. Never a value,
   never a nonce.
9. Instances: per instance, the refused-forgery count (T-031), the policy
   mode, and each public forward (T-173).
10. A gap has a name in the words of docs/limits.md, its evidence (the call
    and its result) and three routes.
11. Example gap: no KVM. Routes: a host with KVM, nested virtualization for
    development only, or the Windows engine.
12. Shared probes, each by running: free space in the store, the image cache
    state (T-066), and the helper greeting and version.
    More shared probes: the credential store opens, and the host is or is not
    itself a VM (a note).
13. Without a daemon, `agent-box doctor` runs the probes itself. With a
    running daemon, it adds the live instance data from `report.get` (T-028).
14. Exit 0 when the report was made, with gaps or without. Exit 2 when it
    could not run. `--strict` exits 1 on any gap.
15. The daemon writes the same report to its log at every start, before it
    accepts a client. It stops on a gap (T-032).
16. Human output: one line per fact, gaps first. `--json`: the serialized
    report type.

## Prove

```sh
cargo test -p box-daemon report::
agent-box doctor --json
agent-box doctor --strict
```

The unit tests build a report from planted probe results. They check each
part, a gap with three routes, and that no planted secret appears in the
JSON.

On a Linux KVM host and on Windows 11 with Hyper-V, `doctor --json` exits 0
and names every probe with its evidence. On a host without KVM it exits 0
and names the gap. `doctor --strict` exits 1 on that host.

Host: any host for the tests. Both OSes for the live runs.

## Start condition

T-092 and T-108.

# T-176: Events and `agent-box logs`

**Source:** brief section 3 (R-TRUSTED-CHANNEL); CubeSandbox and OpenSandbox audit findings; the teardown check of T-180.
**Category:** feature
**Milestone:** M7
**Priority:** P2
**Effort:** S
**Status:** open

## Problem

An unattended agent works while nobody watches. The operator needs a record
of what happened per instance. The record must say which facts the host saw
and which the agent reported, because only the first kind may decide
anything.

## Premise

- Design (R-TRUSTED-CHANNEL): authorization, billing, backoff and lifecycle
  come only from the broker or from the box-guest event channel. The kage ACP
  stream is agent-reported data.
- READ: one JSONL record per request, separate security event lines, secrets
  kept only as a fingerprint, and Authorization values redacted
  (`TencentCloud/CubeSandbox@eaddce3a0e2ed76b246ef42996cf5f9ed671aeac:docs/guide/security-proxy.md:244-294`).
- READ: "denied" (the policy did its job) and "failed" (an allowed lookup
  failed) are separate counters
  (`opensandbox-group/OpenSandbox@8f9b616ed54852a65fc0a1ef4443f81da632aaf7:docs/components/egress.md:289-314`).
- Design (T-180, teardown check): after a delete, no byte that the agent
  wrote stays in the state directory. The event log holds agent-reported
  text, so the delete removes it.

## Approach

1. In crates/box-daemon/src/events.rs, keep one append-only JSONL log per
   instance in the instance's state directory.
2. Record fields: `v` (format version), `ts` (RFC 3339, UTC), `seq` (per
   instance, never reset), `instance`, `source`, `kind` and `data`.
3. `source` has three values: `host-observed` (daemon, helper, broker,
   netstack), `guest-channel` (box-guest on port 5002 after the nonce check),
   and `agent-reported` (the kage ACP stream).
4. Reason for three: the nonce proves that box-guest sent a record. It does
   not prove that the record is true when the guest kernel is compromised.
5. No code reads an `agent-reported` record to make a decision.
6. First kinds: state changes, verbs and their outcome, boot handshake,
   helper results, broker request summaries, refused forgeries, forward open
   and close, and ACP updates.
7. Broker summaries carry counts and status only: no body and no header
   value.
8. Policy outcomes use two kinds: `denied` (the policy refused) and `failed`
   (an allowed flow or lookup failed).
9. Cut an `agent-reported` record at 64 KiB (design value) and mark it
   `truncated`.
10. Rotate at 8 MiB per file and keep at most 8 files per instance (design
    values, not measured). `seq` continues across files.
11. Write each record with append and a flush. Rotate by rename in the same
    directory.
12. A credential value has a type with no Display, Debug or Serialize output
    (T-140). Event types take no free-form header string.
13. `agent-box logs NAME [--follow] [--since TIME] [--tail N] [--source
    SOURCE] [--json]`.
14. Human output escapes every control character except newline and tab, as
    in T-171.
15. `delete` removes the event log with the instance. To keep it, write
    `agent-box logs NAME --json` to a file before the delete.

## Prove

```sh
cargo test -p box-daemon events::
cargo test -p box-cli --test logs
```

Named tests cover the record format, `seq` across a rotation, the three
sources, the 64 KiB cut, `--follow` across a rotation, and control-character
escaping. One test makes a brokered request with a planted credential, then
searches every log file: the value is absent.

Host: any host, with the T-032 double.

## Start condition

T-028 and T-031.
