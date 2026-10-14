# The CLI

agent-box is one binary, `agent-box`, with subcommands. This page gives its
verbs, its output and exit codes, the instance states, the config file, the
daemon, setup, and the startup report.

NOTE: Nothing on this page is built yet. The product crates do not exist.
Entry T-033 creates them, and T-029 creates the CLI skeleton in crate box-cli
(crates/box-cli, planned). Until an entry is done, its verb exits 70 and names
the entry.

The entries are in [TODO/cli.md](../TODO/cli.md) and
[TODO/core.md](../TODO/core.md). The guest side is in [guest.md](guest.md).
The engines and the privileged helper are in [engines.md](engines.md). The
egress policy is in [network.md](network.md). Bindings and the credential store
are in [broker.md](broker.md).

Three programs take part:

| Program | Runs as | Role |
| --- | --- | --- |
| `agent-box` | the operator | The CLI. Unprivileged. It talks to the daemon only. |
| `agent-box daemon` | the operator | The daemon: instance records, the netstack, the broker, the credential store. |
| `agent-box-helper` | root or LocalSystem | The privileged helper: a fixed operation set, installed once by `setup`. Nobody runs it by hand. |

## Verbs

Every verb takes `--json`. `agent-box --help` lists every verb, and
`agent-box --version` prints the version and the commit.

| Verb | Arguments | What it does | Entry |
| --- | --- | --- | --- |
| `setup` | `[--system] [--user NAME] [--no-pull]` | Installs what this host needs. `--system` is the root or admin part. See setup below. | T-174 |
| `uninstall` | `[--system] [--purge] [--delete-instances]` | Removes what `setup` installed. It refuses while any instance exists, unless `--delete-instances` deletes each one first. | T-174 |
| `doctor` | `[--strict]` | Runs every probe and prints the startup report. It does not start the daemon. | T-175, T-092, T-108 |
| `image pull` | `[VARIANT...]` | Downloads the kernel and the images whose digests the binary embeds, and verifies them. No argument pulls the default profile's variant. | T-067 |
| `image list` | none | Lists the cached images: variant, digest, format, size, and the instances that record each. | T-067 |
| `image verify` | `[VARIANT...]` | Hashes the cache again and reports each verification tier. | T-067 |
| `image gc` | none | Removes cached images that no instance records. | T-067 |
| `create` | `NAME [--profile P] [--cpus N] [--memory SIZE] [--disk SIZE]` | Creates a persistent instance: the record, the uid, the data disk. It does not start it. | T-170 |
| `list` | none | One row per instance: name, state, kind, profile, image variant, cpus, memory, disk. | T-170 |
| `inspect` | `NAME` | The full record, the state, the engine facts, the image digest, the policy generation, the forwards. | T-170 |
| `start` | `NAME` | Cold boot from the data disk. It returns when the handshake and provisioning finish. | T-170, T-088, T-105 |
| `stop` | `NAME [--grace SECONDS]` | Guest poweroff over port 5000, then a kill when the grace passes. | T-170, T-088, T-105 |
| `pause` | `NAME` | Freezes the vCPUs. Guest RAM stays in host memory. | T-170, T-089, T-106 |
| `resume` | `NAME` | Unfreezes the vCPUs. The daemon then sends the host time. | T-170, T-089, T-106 |
| `delete` | `NAME [--force]` | Removes the instance and verifies each absence. Without `--force` it needs a stopped instance. | T-170, T-091, T-107 |
| `attach` | `NAME[:/path] [--shell] [--user USER] [--detach-keys KEYS]` | A terminal in the guest: the kage TUI joined to the serve session, or a bash shell with `--shell`. | T-172 |
| `run` | `NAME[:/path] --prompt TEXT [--timeout SECONDS] [--session ID] [--start]` | Sends one prompt to kage over ACP and streams its events until the run ends. `--prompt -` reads stdin. | T-171 |
| `forward` | `NAME [ADDR:]HOSTPORT:GUESTPORT [--public]` | Listens on the host and opens a TCP flow into the guest for each connection. | T-125, T-173 |
| `forward` | `NAME --list` or `NAME --remove HOSTPORT` | Lists or removes the forwards of an instance. | T-125, T-173 |
| `logs` | `NAME [--follow] [--since TIME] [--tail N] [--source SOURCE]` | The instance event log. Each event names its source. | T-176 |
| `policy show` | `NAME` | The effective egress policy, its rules and its generation. | T-173 |
| `policy set` | `NAME POLICY`, or `NAME` with `--mode`, `--allow`, `--remove` or `--file` | Moves the instance to a named policy of the config file, or changes its policy, live. | T-173 |
| `secret set` | `NAME` | Reads one value from stdin, never from argv, and stores it encrypted. | T-140 |
| `secret list` | none | The credential names and the bindings that use each. Never a value. | T-140 |
| `secret rm` | `NAME` | Removes one credential. | T-140 |
| `daemon` | none | Runs the daemon in the foreground. | T-028 |

Details that the table does not hold:

- `NAME[:/path]`: the path is a directory in the guest. For `attach` it is the
  start directory. For `run` it is the kage session directory. Without it,
  both use `/home/agent/work`.
- `attach` without `--shell` runs `kage resume --last` as uid 1000. That TUI
  joins the session that `kage serve` hosts (see [guest.md](guest.md#kage-in-the-guest)).
  With no session yet, it runs `kage`.
- `attach --shell` runs `bash -l` as uid 1000. Only `--shell --user root`
  gives a root shell, and the daemon logs each root session
  ([protocol.md](protocol.md)).
- `attach` passes Ctrl-C to the guest. A detach key returns to the host shell.
- `attach` exits with the exit status of the guest program, or 128 plus the
  signal number. A detach exits 0.
- `run` answers every kage permission request with allow and logs it (see
  [guest.md](guest.md#approval-mode-allow-and-why)).
- `forward`: `ADDR` defaults to `127.0.0.1`. Any other address needs
  `--public`, and `inspect` and `doctor` report it.
- `forward`: HOSTPORT 0 lets the daemon pick a free port, and the CLI prints
  it.
- `policy set` stamps a new policy generation. New flows use it. A revoked TCP
  flow gets RST ([network.md](network.md)).
- `policy set` needs `--allow-open` for `open` mode, and `--allow-reserved`
  for a rule into a reserved range (T-173).
- `run --start` starts a `stopped` instance first. `run --session ID` loads
  that kage session.
- `secret set` reads stdin to the end and strips one final line feed. When
  stdin is a terminal, it turns echo off. An empty value is refused.
- `stop --grace` defaults to `[daemon] stop_grace_seconds` of the config file.
- `create` without `--profile` uses `[defaults] profile`, or else the built-in
  profile `default`.
- Without `--json`, `run` prints the reply text of kage on stdout and the tool
  calls on stderr.

Verbs for later milestones:

| Verb | Arguments | What it does | Entry |
| --- | --- | --- | --- |
| `hibernate` | `NAME` | Opt-in snapshot to disk, P3. It is not `pause`. It exits 70 until its entry is done. | T-184 |

## Output and --json

Rules for every verb:

- Without `--json`, results go to stdout as text for people. Progress, warnings
  and errors go to stderr.
- With `--json`, stdout holds exactly one JSON object. Streaming verbs (`run`,
  `logs --follow`) print one JSON object per line instead.
- stderr is never JSON. A script reads stdout and the exit code only.
- Field names are snake_case. Sizes are integer bytes. Times are RFC 3339 UTC
  strings. Durations are integer milliseconds.
- States and kinds use the lowercase names on this page.
- Each event names its source: `"source": "host-observed"`,
  `"guest-channel"` (box-guest on port 5002) or `"agent-reported"` (kage).
- Authorization, billing and backoff never read agent-reported data (T-176).
- No output, JSON or text, ever holds a secret value.
- A change that breaks the shape increments `schema`.

The shape of a single result:

```json
{"schema": 1, "verb": "create", "ok": true, "result": {"name": "web1", "state": "stopped", "kind": "persistent"}}
```

The shape of an error:

```json
{"schema": 1, "verb": "pause", "ok": false, "error": {"exit": 4, "kind": "wrong-state", "message": "web1 is stopped; pause needs running", "state": "stopped"}}
```

An unimplemented verb:

```json
{"schema": 1, "verb": "run", "ok": false, "error": {"exit": 70, "kind": "not-implemented", "message": "run is not implemented yet", "entry": "T-171"}}
```

A stream line has a `type`. The last line of a stream has `"type": "end"`, the
final `ok` and the `exit` code.

## Exit codes

| Code | Name | Meaning |
| --- | --- | --- |
| 0 | success | The verb did what it says. For `doctor`: the report was made, with or without a gap. |
| 1 | failure | The verb ran and the operation failed. For `doctor --strict`: at least one gap. |
| 2 | could not run | The verb could not start its work: no daemon, an invalid config file, or a missing runtime directory. |
| 3 | not found | No instance, image, policy, binding or credential has that name. |
| 4 | wrong state | The state of an instance does not allow the verb. The error names the state. |
| 5 | exists | `create` got a name that an instance already has. |
| 10 | engine unavailable | The capability report has an engine gap: no KVM, no hypervisor, no engine binary, or an unsupported page size. |
| 11 | helper unavailable | The privileged helper is not installed, does not answer, or refused this user. |
| 12 | state unknown | The engine could not observe the instance. Its state is `unknown`. |
| 13 | boot timeout | The guest did not finish the handshake on port 5000 within the bound. |
| 14 | connection lost | The connection to the daemon, or the channel of the daemon to the guest, ended during the verb. |
| 20 | run failed | `run`: kage ended the run with a stop reason other than the end of the turn. |
| 21 | run timed out | `run`: `--timeout` passed, and the daemon cancelled the prompt. |
| 22 | run busy | `run`: a prompt already runs in that kage session. The daemon does not queue in v1. |
| 23 | instance left running | `run` or `attach`: the instance left `running` while the verb ran. |
| 64 | usage | An unknown verb, flag or argument. Nothing ran. |
| 70 | not implemented yet | The verb parses, but its entry is not done. The error names the entry. |
| 130 | interrupted | SIGINT or Ctrl-C. `run` cancels the kage prompt first. |

Rules:

- The most specific code wins. A missing helper is 11, not 2.
- Every non-zero exit prints one line on stderr that names the cause.
- Codes 10 to 14 let a script tell host trouble from instance trouble.
- Read an exit code from the process itself, not through a pipe.

## Instance states

An instance has exactly one state. The state is an enum everywhere, never a
boolean.

| State | Meaning |
| --- | --- |
| `creating` | The daemon writes the record, and the helper allocates the uid and the data disk. |
| `stopped` | No VMM runs. The data disk holds everything. |
| `starting` | The VMM runs. The daemon waits for the handshake and provisioning. |
| `running` | The guest answered the handshake and got its provisioning. |
| `paused` | The vCPUs are frozen. Guest RAM stays in host memory. |
| `stopping` | The guest powers off, or the engine kills the VMM after the grace. |
| `deleting` | The daemon removes every resource and verifies each absence. |
| `unknown` | The engine query failed. The daemon never assumes `running`. |

```text
  create
    |
    v
 creating ---(failed)---------> deleting ---> removed (absence verified)
    |                              ^
    | disk and record ready        | delete
    v                              |
 stopped --------------------------+
   |   ^
   |   +------------------------------------------------+
   | start                                              |
   v                                                    |
 starting ---(boot timeout or failure)---> stopping ----+
   |                                         ^    ^      (guest off, or killed)
   | handshake and provisioning done   stop  |    |
   v                                         |    |
 running ------------------------------------+    |
   |    ^                                         |
 pause  resume                                    |
   v    |                                    stop |
 paused ------------------------------------------+

 running, paused or starting --(VMM or host went away)--> stopped
 any state --(engine query failed)--> unknown
 unknown --(next query succeeds)--> the observed state
```

| From | Verb or event | To | Note |
| --- | --- | --- | --- |
| none | `create` | `creating` | |
| `creating` | disk and record ready | `stopped` | |
| `creating` | a failure | `deleting` | The partial resources are removed and verified absent. |
| `stopped` | `start` | `starting` | Cold boot. |
| `starting` | handshake and provisioning done | `running` | |
| `starting` | boot timeout or VMM failure | `stopping` | Exit 13 for a timeout. |
| `starting` | `stop` | `stopping` | Cancels the boot. |
| `running` | `pause` | `paused` | |
| `paused` | `resume` | `running` | The daemon sends the host time. |
| `running` | `stop` | `stopping` | Guest poweroff over port 5000. |
| `paused` | `stop` | `stopping` | The engine resumes the guest first, then powers it off. |
| `stopping` | guest off, or a kill after the grace | `stopped` | |
| `running`, `paused`, `starting` | the VMM, the helper or the host went away | `stopped` | A pause does not survive this. The next start cold-boots. |
| `stopped` | `delete` | `deleting` | |
| `deleting` | every absence verified | removed | The record goes last. |
| any | engine query failed | `unknown` | |
| `unknown` | the next query succeeds | the observed state | |

A verb on an instance that is already in the end state of the verb does
nothing and succeeds. Examples: start on `running`, pause on `paused`, resume
on `running` and stop on `stopped`.

Which verbs each state allows (`list`, `inspect` and `logs` work in every
state):

| Verb | Allowed in |
| --- | --- |
| `start` | `stopped` |
| `stop` | `starting`, `running`, `paused`, `unknown` (a kill) |
| `pause` | `running` |
| `resume` | `paused` |
| `delete` | `stopped`; with `--force` also `running`, `paused` and `unknown` |
| `attach`, `run`, `forward` | `running` |
| `policy show`, `policy set` | every state except `creating` and `deleting` |

A verb in a state that does not allow it exits 4. A daemon restart does not
stop an instance. The new daemon re-adopts every running or paused instance
(T-090, T-107).

## Instance kinds

| Kind | Created by | Ends |
| --- | --- | --- |
| `persistent` | `agent-box create` | at `agent-box delete` only. It survives a daemon restart, and a host reboot as `stopped`. |
| `ephemeral` | the daemon, for one task | when the task ends, with success or failure. The chat gateway uses it (T-222). |

`list` and `inspect` show the kind. `delete` works on both kinds. How the CLI
starts an ephemeral instance is an open question for T-170 and T-171. The
proposal is `run --ephemeral --profile P --prompt TEXT` with a generated
name.

## Names and sizes

- `NAME` follows DNS label rules: 1 to 63 characters, lowercase letters, digits
  and hyphens, with a letter or digit at each end (T-020).
- The instance id is 128 random bits as 32 lowercase hex digits. Only
  `inspect` and the logs show it.
- A size is an integer and a unit, `M` or `G`. Units are binary: `30G` is 30
  GiB. Ruling D1 says "about 30 GB", and every specification uses GiB.
- Without a config file, the built-in profile `default` applies: variant
  `base`, 2 cpus, memory `4G`, disk `30G`, policy `default`, no binding.
- These defaults are design values. T-181 measures the footprint and can change
  them.
- The built-in policy `default` is `allowlist` with no rule, so only the broker
  endpoints of the instance's bindings are reachable.

## The config file

### Where it is

The daemon reads one file: the first that exists in this order.

| OS | Order | Path |
| --- | --- | --- |
| Linux | 1 | `$XDG_CONFIG_HOME/agent-box/config.toml`, skipped when the variable is unset or empty |
| Linux | 2 | `~/.config/agent-box/config.toml` |
| Linux | 3 | `/etc/agent-box/config.toml` |
| Windows | 1 | `%APPDATA%\agent-box\config.toml` |
| Windows | 2 | `%ProgramData%\agent-box\config.toml` |

- The environment variable `AGENT_BOX_CONFIG` names one file outright and skips
  the search. A missing file named by it is an error (exit 2).
- The working directory is never searched. A cloned project could otherwise
  plant a config file there.
- Files are never merged. The first file found is the whole config.
- With no file, the built-in defaults apply. `doctor` prints `config: none`.

### Rules

- An unknown key, at any depth, is a refusal. The error names the key and its
  table, and the verb exits 2. A value of the wrong type is a refusal too.
- The refusal happens at load, never at use. An operator who misspells a key
  must not believe the setting is in force.
- No key holds a credential. A binding names a credential, and
  `agent-box secret set` stores the value in the credential store.
- The daemon reads the file at start, and again for each request that uses it
  (`create`, `start`, `policy set`, `doctor`). A refused file refuses that
  request. Running instances stay as they are.
- `create` copies the profile values into the instance record. `start` resolves
  the policy and bindings again from the file.
- The helper never reads this file. Its own settings come from `setup`
  ([engines.md](engines.md)).
- The binding fields are defined in [broker.md](broker.md). The policy fields
  are defined in [network.md](network.md). This example shows their shape.

### A full example

```toml
# agent-box configuration. No credential goes in this file.
version = 1

[daemon]
log = "info"                 # error, warn, info or debug
stop_grace_seconds = 30      # default for `agent-box stop --grace`

[defaults]
profile = "rust"             # used by `create` without --profile

# A profile is a template for `create`.
[profiles.rust]
image = "rust"               # an image variant
cpus = 4
memory = "8G"
disk = "30G"
policy = "dev"
bindings = ["anthropic", "github"]

[profiles.quiet]
image = "base"
cpus = 2
memory = "4G"
disk = "30G"
policy = "locked"
bindings = ["anthropic"]

# A policy is the egress rule set of an instance.
[policies.dev]
mode = "allowlist"           # deny, allowlist or open

[[policies.dev.allow]]
host = "index.crates.io"
ports = [443]

[[policies.dev.allow]]
host = "static.crates.io"
ports = [443]

[policies.locked]
mode = "deny"                # broker endpoints only

# A binding is one broker endpoint. It names a credential; it never holds one.
[bindings.anthropic]
upstream = "https://api.anthropic.com"
kind = "anthropic"           # replaces the x-api-key header
credential = "anthropic-api-key"
routes = [
  { path = "/v1/messages", match = "exact", methods = ["POST"] },
]

[bindings.github]
upstream = "https://github.com"
kind = "git-basic"           # git smart HTTP; pushes only to the instance namespace
credential = "github-token"
basic_user = "x-access-token"
repos = [{ name = "example-org/example-repo", push = true }]
```

With this file, the guest reaches `http://anthropic.broker.internal` and
`http://github.broker.internal`. The credentials `anthropic-api-key` and
`github-token` exist only in the host credential store. The operator stores
them like this:

```sh
agent-box secret set anthropic-api-key
agent-box secret set github-token
```

Each command reads the value from stdin.

## The daemon

### Start on demand

- The CLI connects to the daemon's control endpoint. If nothing answers, it
  starts `agent-box daemon` in the background and retries. T-028 sets the
  retry bound.
- On Linux the CLI starts the daemon in a new session (`setsid`), with stdin,
  stdout and stderr on `/dev/null`. The daemon writes its log to the state
  directory.
- On Windows the CLI starts it with `DETACHED_PROCESS`,
  `CREATE_NEW_PROCESS_GROUP` and `CREATE_BREAKAWAY_FROM_JOB` (see the Windows
  notes).
- The CLI never decides that a daemon is absent from a file. The
  single-instance lock decides, so two CLIs cannot start two daemons.
- If the daemon does not answer within the bound, the verb exits 2.

### The user service

`agent-box setup` registers the daemon as a user service, so the start on
demand is rare:

| OS | Service | Effect |
| --- | --- | --- |
| Linux | a systemd user unit `agent-box.service` with `ExecStart` running `agent-box daemon`, plus lingering for the user | The daemon starts at boot and runs without a login session. |
| Windows | a Task Scheduler logon task that runs `agent-box daemon` | The daemon starts when the operator logs on. |

### The control API

The control API is JSON-RPC 2.0, one message per line, at most 8 MiB per line.
A longer line closes the connection. Each CLI verb is one method. T-028 fixes
the method names.

| OS | Transport | Access control |
| --- | --- | --- |
| Linux | Unix socket `$XDG_RUNTIME_DIR/agent-box/daemon.sock`, directory mode 0700, socket mode 0600 | Both sides check the peer uid with `SO_PEERCRED`. |
| Windows | named pipe `\\.\pipe\agent-box-daemon-<user SID>` | The pipe DACL grants the operator's SID only. The daemon creates it with `FILE_FLAG_FIRST_PIPE_INSTANCE` and `PIPE_REJECT_REMOTE_CLIENTS`. |

- On Windows, both sides check the peer's user SID through the process token
  of the other end. A pipe that another user created first fails the check.
- On Linux, an unset `$XDG_RUNTIME_DIR` stops the verb with exit 2. The CLI
  does not fall back to `/tmp`.

### The single-instance lock

- The daemon holds one lock for its whole life. It never writes a PID file.
- Linux: `flock` (exclusive, non-blocking) on `daemon.lock` in the state
  directory.
- Windows: the named mutex `Local\agent-box-daemon-<user SID>`, plus
  `LockFileEx` on `daemon.lock` in the state directory. A `Local` mutex covers
  one logon session only, so the file lock covers the rest.
- A second daemon finds the lock held, names the holder, and exits 2.
- The state directory is `$XDG_STATE_HOME/agent-box` on Linux (default
  `~/.local/state/agent-box`) and `%LOCALAPPDATA%\agent-box` on Windows. T-026
  fixes its layout.

### Lifetime

- The netstack and the broker live in the daemon. So the daemon does not exit
  while any instance is `starting`, `running`, `paused` or `stopping`.
- Whether the daemon exits when it is idle is an open question for T-028.
- If the daemon stops, the instances keep running without network or broker.
  The guests reconnect their channels when it returns (T-045).
- At start the daemon runs every probe and logs the startup report. A gap stops
  the daemon. There is no fallback from one engine to another (T-032).
- `agent-box daemon` exits 0 after SIGTERM, 2 when another daemon holds the
  lock, and 10 or 11 when the startup report has a gap.

## Setup and uninstall

`setup` has two parts. The system part needs root or admin once per host. The
user part runs as the operator once per user. `agent-box setup` with no flag
runs the user part. If the system part is missing, it exits 2 and prints the
exact command.

### Linux

System part: `sudo agent-box setup --system`. The user comes from `SUDO_USER`,
or from `--user NAME`.

1. Create the group `agent-box`, and add the user to it.
2. Install `agent-box-helper` under `/usr/libexec/agent-box/`, owned by
   root (T-081).
3. Install a systemd socket unit for `/run/agent-box/helper.sock` (group
   `agent-box`, mode 0660) and the service it activates (T-081).
4. Install the unit that holds the delegated cgroup v2 root. systemd
   delegates on service and scope units only, so T-082 proposes the service
   `agent-box-vmm.service` with `Delegate=yes`.
5. Reserve the per-instance uid range, and write the helper's own settings
   (T-082).
6. Download Firecracker and the jailer v1.17.0, verify their digests, and
   install them owned by root under `/usr/libexec/agent-box/` (T-083).
7. Create the jail base and the instance store on one filesystem. The jail uses
   hard links, which need one filesystem (T-086).
8. Enable lingering for the user (`loginctl enable-linger`).

The group applies at the user's next login. `setup` says so.

User part: `agent-box setup`.

1. Create the state directory.
2. Install and enable the systemd user unit for `agent-box daemon`.
3. Pull the image of the default profile, unless `--no-pull` is given.
4. Run `doctor` and print the report.

### Windows

System part: `agent-box setup --system`, from an elevated terminal.

1. Check the edition, `Win32_ComputerSystem.HypervisorPresent`, the
   Hyper-V-Hypervisor event log, and the Hyper-V or Virtual Machine Platform
   feature. Without a running hypervisor, setup stops with exit 10.
2. Setup does not turn on a Windows feature itself. It prints the command for
   the operator.
3. Install `agent-box-helper.exe` under `%ProgramFiles%\agent-box`, and register
   it as the LocalSystem service `agent-box-helper` (T-100).
4. Create the store `%ProgramData%\agent-box\store`. Its ACL grants SYSTEM
   and Administrators, and traverse only to `NT VIRTUAL MACHINE\Virtual
   Machines`. VMs get file access by grant (T-103).
5. Write the helper's own settings, with the operator's SID (T-100).

User part: `agent-box setup`.

1. Create the state directory.
2. Register the logon task for `agent-box daemon`.
3. Pull the image of the default profile, unless `--no-pull` is given.
4. Run `doctor` and print the report.

NOTE: Windows 11 Home is not verified. A third-party HCS caller on Home has no
evidence either way. T-109 runs the HCS spike on Home. Until then, `doctor`
prints a note on Home.

### Uninstall

- `uninstall` refuses while any instance record exists, and exits 4. Delete the
  instances first, or pass `--delete-instances`. That flag deletes each one
  and verifies each absence first.
- `agent-box uninstall` removes the user part: the user unit or the logon
  task.
- `--purge` also removes the state directory, the image cache and the
  credential store.
- `agent-box uninstall --system`, as root or admin, removes the system part.
  That is the helper, its units or service, and the store. On Linux it is also
  the unit of the cgroup root, the uid range, the group, the Firecracker
  binaries and the jail base.
- `uninstall` never removes the config file. The operator wrote it.
- T-174 proves it: `setup`, then `uninstall --purge` and
  `uninstall --system`, leave no file or service behind on both OSes.

## Doctor and the startup report

`agent-box doctor` and the daemon at start print the same report. `doctor`
runs the probes itself. It asks the daemon only for the lines that need it
(`forgery`, `instances`), when the daemon runs.

Each probe runs the real operation. A file that is present proves nothing until
the probe uses it. In JSON, each line names the probe that produced it as its
evidence.

### Format

- One line per fact: `KEY: VALUE`.
- A gap line ends with `[gap NAME]`. A note line ends with `[note NAME]`.
- The last line is `gaps: none`, or `gaps:` and every gap name.
- `doctor` exits 0 when it made the report, with or without a gap, and 2 if
  it could not run the probes. A probe is not a gate.
- `doctor --strict` exits 1 when the report has a gap.
- With `--json`, the result holds `lines` (key, value, status, name, evidence),
  `gaps` and `notes`.

### Every line

| Key | Linux value | Windows value | Gap or note |
| --- | --- | --- | --- |
| `agent-box` | version, commit, target triple | same | none |
| `config` | the file in use, or `none` | same | gap `config-invalid` |
| `host` | kernel release, architecture, page size | Windows build, edition, architecture | gap `page-size-unsupported`; note `windows-home-unverified` |
| `host-kernel` | the release against the Firecracker validated set (5.10, 6.1, 6.18) | line absent | note `host-kernel-unvalidated` |
| `virtualization` | the helper opened `/dev/kvm` and `KVM_GET_API_VERSION` answered | `HypervisorPresent` and the last Hyper-V-Hypervisor start event | gap `no-kvm` or `no-hypervisor`; note `nested-virtualization` |
| `engine` | Firecracker and jailer versions and digest tier | computecore present, the HCS schema version in use | gap `engine-missing` or `engine-digest-mismatch` |
| `helper` | socket path, probe answered, protocol version, peer check | pipe name, service state, probe answered, client check | gap `helper-missing` or `helper-refused` |
| `jail` or `partition` | chroot base, mount and PID namespaces, uid range, the Firecracker seccomp filters | per-VM worker `vmwp.exe`, HvSocket SDDL principals | gap `jail-unavailable` |
| `cgroup` | v2, the delegated root, the controllers | line absent | gap `cgroup-delegation-missing` or `memory-max-missing`; note `io-max-missing` |
| `thp` | the transparent huge page mode | line absent | none |
| `isolation` | the guest reaches KVM in the host kernel; virtio-blk and vsock run in the jailed VMM | the guest reaches VMBus, storvsp, vhdmp and the hvsocket provider in the root kernel, and `vmwp.exe` | none |
| `guest-kernel` | the file, its version, its sha256 and the verification tier | same, for `bzImage` | note `image-not-pulled` or `image-digest-mismatch` |
| `image` | one line per cached image: variant, sha256, format, each tier's outcome, users | same, plus the VHDX copy | note `image-not-pulled` or `image-digest-mismatch` |
| `disk` | raw sparse data disks, `io_engine` Sync, `cache_type` Writeback, discard on, hole punching in the store, host swap | dynamic VHDX on SCSI, the sparse-file setting | note `discard-unsupported` or `host-swap` |
| `egress` | absences: no NIC device in the VM configuration, no shared filesystem, no host path in the guest; the only route is vsock port 5003 | absences: no NetworkAdapters, no Plan9, no VirtualSmb; the only route is HvSocket port 5003 | gap `egress-device-present` |
| `credentials` | the store, its protection, each credential name and its bindings | same, with DPAPI | note `credential-missing` |
| `forgery` | refused event frames and refused broker nonces per instance since daemon start | same | note `forgery-refused` when a count is above zero |
| `daemon` | running with its pid and socket, or `not running` | same, with the pipe | gap `runtime-dir-missing` (Linux) |
| `instances` | the count per state | same | note `instance-state-unknown` |
| `gaps` | `none`, or every gap name | same | |

- The `isolation` line states a fact that differs by OS. The two engines do not
  give the same isolation ([engines.md](engines.md)).
- A missing or mismatched image is a note, not a gap. The daemon must run to
  pull it. `start` refuses an instance whose image is not verified (exit 3).
- `egress-device-present` means a live engine configuration has a NIC or a
  shared filesystem. agent-box never writes one, so this gap means tampering or
  a defect.
- The credential store location and protection are decided in T-140
  ([broker.md](broker.md)). The report never prints a value or a prefix of one.

| Gap | Meaning | What to do |
| --- | --- | --- |
| `config-invalid` | The config file has an unknown key or a value of the wrong type. | Correct the key that the line names. |
| `page-size-unsupported` | An aarch64 host with a page size other than 4K. | None in v1. T-094 measures such a host. |
| `no-kvm` | `/dev/kvm` is missing, or the KVM ioctl failed. | Turn on hardware virtualization in the firmware. |
| `no-hypervisor` | `HypervisorPresent` is false, or the hypervisor did not start. | Check the firmware settings for virtualization and NX. |
| `engine-missing` | Firecracker or the jailer is absent, or computecore cannot be used. | Run `setup --system`. |
| `engine-digest-mismatch` | An engine binary does not match its pinned digest. | Run `setup --system` again. |
| `helper-missing` | The helper socket or service does not answer. | Run `setup --system`. |
| `helper-refused` | The helper refused this user. | On Linux, log in again after `setup` adds the group. |
| `jail-unavailable` | The helper could not build a test jail. | Read the helper log. |
| `cgroup-delegation-missing` | There is no delegated cgroup v2 root. | Run `setup --system`. |
| `memory-max-missing` | The delegated root has no memory controller. | Turn on the memory controller for `agent-box-vmm.service`, or run `setup --system` again. |
| `egress-device-present` | A live engine configuration has a NIC or a shared filesystem. | Stop the instance and report a defect. |
| `runtime-dir-missing` | `$XDG_RUNTIME_DIR` is unset. | Log in through a session that sets it, or use the user service. |

| Note | Meaning |
| --- | --- |
| `windows-home-unverified` | Windows 11 Home: HCS for a third-party caller is not verified (T-109). |
| `host-kernel-unvalidated` | The host kernel is outside the set that Firecracker validates. |
| `nested-virtualization` | KVM runs inside another VM, for example WSL2. Valid for development tests, never a production claim. |
| `io-max-missing` | The cgroup has no `io.max`. Disk I/O is not limited. |
| `discard-unsupported` | The store filesystem cannot punch holes. A guest trim gives no space back (T-087, T-092). |
| `host-swap` | The host has swap on. Firecracker advises against host swap for guest memory (T-092). |
| `image-not-pulled` | No verified kernel or image is cached for a variant that a profile uses. |
| `image-digest-mismatch` | A cached file did not match its digest. It was deleted. |
| `credential-missing` | A binding names a credential that the store does not hold. |
| `forgery-refused` | The daemon refused a frame or a nonce. Read `logs` for the instance. |
| `instance-state-unknown` | The engine could not observe at least one instance. |

### An example

The values in angle brackets stand for real values. This is the Linux form.

```text
agent-box: <version> (<commit>), x86_64-unknown-linux-musl
config: ~/.config/agent-box/config.toml
host: Linux <release>, x86_64, page size 4K
host-kernel: <release> is outside the Firecracker validated set (5.10, 6.1, 6.18) [note host-kernel-unvalidated]
virtualization: helper opened /dev/kvm, KVM_GET_API_VERSION answered <value>
engine: firecracker v1.17.0, jailer v1.17.0, sha256 verified (tier 2)
helper: /run/agent-box/helper.sock answered probe, protocol <n>, peer check passed
jail: chroot under <jail base>, mount and PID namespaces, uid range <first>-<last>, Firecracker seccomp filters
cgroup: v2, delegated root system.slice/agent-box-vmm.service, controllers cpu memory pids io
thp: <mode>
isolation: the guest reaches KVM in the host kernel; virtio-blk and vsock run in the jailed VMM
guest-kernel: vmlinux <version>, sha256 <digest>, verified (tier 2)
image: rust, sha256 <digest>, erofs, tier 1 verified, tier 2 verified, tier 3 no verifier installed, used by <n>
disk: raw sparse data disks, io_engine Sync, cache_type Writeback, discard on, hole punching works, no host swap
egress: no NIC device, no shared filesystem, no host path in the guest; only route vsock port 5003; default policy allowlist
credentials: store <path>, <protection>; anthropic-api-key (anthropic), github-token (github)
forgery: refused event frames <n>, refused broker nonces <n>, since daemon start
daemon: running, pid <pid>, <socket path>
instances: <n> (running <n>, paused <n>, stopped <n>, unknown <n>)
gaps: none
```

## Windows notes

### Git Bash path rewriting

- Git Bash (MSYS2) rewrites an argument that starts with `/` into a Windows
  path before a native program sees it. `agent-box.exe` is a native program.
  The rewrite is silent.
- So agent-box takes a guest path only as a `NAME:/path` operand. The operand
  starts with the instance name, not with `/`.
- No flag of agent-box takes a guest path as its value.
- The `NAME:/path` form is not measured against every MSYS2 conversion rule.
  T-172 tests it in Git Bash.
- A prompt that starts with `/`, such as a kage slash command, is a
  leading-slash argument. Pipe it on stdin with `--prompt -`, or turn the
  rewrite off.
- `MSYS_NO_PATHCONV=1` turns off the leading-slash rewrite.
  `MSYS2_ARG_CONV_EXCL='*'` excludes every argument from conversion. Set both
  for one command:

```sh
MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' agent-box run web1 --prompt '/review'
printf '%s\n' '/review' | agent-box run web1 --prompt -
```

### Job Objects

- On Windows, agent-box controls child processes with Job Objects, not with
  process groups.
- Each child starts suspended, joins a Job Object with
  `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, and then resumes. So no grandchild
  escapes before the assignment.
- When the parent exits or crashes, the job handle closes and Windows kills the
  whole tree.
- The daemon is the exception, because instances outlive the CLI. The CLI
  starts it with `CREATE_BREAKAWAY_FROM_JOB` and `DETACHED_PROCESS`, outside
  its own job and console.
- A Ctrl-C in the terminal therefore does not reach the daemon.
- If the job of the terminal forbids breakaway, the start on demand fails with
  exit 2. The logon task from `setup` is then the way to run the daemon.
- On Linux, the same separation comes from `setsid`.

### The console

- `attach` puts the Windows console in virtual terminal input mode and restores
  the mode when it ends (T-172).
- While attached, Ctrl-C goes to the guest. The detach key returns to the host
  shell.

## Open questions

| Question | Entry |
| --- | --- |
| How does the CLI start an ephemeral instance? Proposals: `run --ephemeral --profile P --prompt TEXT` (T-171), and `create --ephemeral` (T-170). | T-170, T-171 |
| Does the daemon exit when it is idle? | T-028 |
| The JSON-RPC method names, and the retry bound of the start on demand. | T-028 |
| The detach key of `attach`. T-172 proposes ctrl-p then ctrl-q, as container tools use; `Ctrl-]` is the other candidate. | T-172 |
| Does a forward survive a stop and a start? Design: it ends at stop. | T-125 |
| Does `setup` work on Windows 11 Home? | T-109 |
| How does an instance move to a newer image? | T-069 |
