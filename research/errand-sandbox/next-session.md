# Notes for the errand-box session

Written 2026-10-23, at the end of the research on branch
`ephemeral-errand-sandbox-research` of https://github.com/Azathothas/agent-box.
It is for a session on another machine that has no memory of this one. It
has two parts: what that session must know before it starts, and the
operator's draft prompt with corrections.

Get the files with git, not with MCP tools:

```sh
git clone --depth 1 --branch ephemeral-errand-sandbox-research \
  https://github.com/Azathothas/agent-box.git agent-box-research
cd agent-box-research/research/errand-sandbox
```

Every path below is relative to that directory:

| path | what it is |
| --- | --- |
| `engines-shortlist.md` | the engines tried, in order, as a log. Sections 5 to 10 hold the numbers. Section 10 corrects section 9. |
| `incus-gaps-survey.md` | a delegated web survey of the Incus gaps and the errand-box groundwork; marks what was tested |
| `runtime-survey.md` | a delegated survey of container runtimes |
| `README.md` | the earlier phase: the errand findings, and the `bailey` CLI contract errand uses |
| `experiments/` | every script behind a number, numbered in run order; `lib/` holds the shared helpers |
| `results/` | raw output of the runs the documents cite |
| `patches/` | the Sysbox patches (Sysbox was not chosen) |
| `poc/bailey-gvisor/` | a worked example of the bailey contract over gVisor; not a base |

Nothing here assumes this session's machine. Where a number depends on that
host, the host is named: Linux 6.18 x86_64 on a cloud VM, 4 vCPUs, 16 GB,
ext4, no KVM, cgroup v1, and PID 1 not systemd.

## 1. Facts the next session must not re-derive

All were measured on the one host named above. The source of each is in
engines-shortlist.md, by section number.

### 1.1 The engine result, stated as it was measured

- Incus 6.0 does not win every benchmark. For agent work (spawn, files, git,
  cpu, compile) it ties runc and Sysbox, at 0.9-1.1x, and beats gVisor by
  1.8-8.2x (section 5.2).
- Start-up depends on what is started (section 9):
  - Launch from an image: 15.6 s on the dir pool, mostly unpacking
    (section 10.2).
  - On a btrfs pool: 12.4 s for the first launch, then 1.8 s for each later
    one.
  - Golden copy on btrfs (`incus copy` of a stopped, prepared instance):
    about 0.4 s with no rewrite (section 10.3). Use this.
  - Start an existing instance: about 0.27 s, with systemd settled about
    0.7 s later.
  - Sysbox, for comparison: about 0.69 s per fresh container.
- Memory per idle instance: about 59 MB (systemd inside), against 5 MB for
  Sysbox.
- Incus is the only tested engine with all of: Docker inside, podman inside,
  FUSE, network storage over FUSE, systemd as PID 1, no patches, and a path
  to VMs on KVM hosts (sections 2, 8 and 9).
- Isolation: a shared kernel, as with Sysbox. It is not a VM. On KVM hosts
  `incus launch --vm` gives a VM, but on QEMU, which is not the most hardened
  VMM.

### 1.2 Host conditions that broke something, and what fixed it

| condition | effect | fix |
| --- | --- | --- |
| no init on the host (PID 1 is the platform's) | nothing starts or restarts daemons | an idempotent `up` step (experiments/75-host-up.sh) |
| the machine restarted mid-session | device modes, sysctls and every daemon were lost; disk state was kept | run `up` again; it is idempotent (two runs checked) |
| /dev/fuse and /dev/net/tun boot at 0600 | rootless networking and FUSE in guests fail | chmod 666, at every boot |
| cgroup v1, no delegation | Incus 7.0+, Podman 6, LXC 7 and systemd 258+ refuse it | pin Incus 6.0 LTS on v1 hosts; podman inside needs `--cgroups=disabled` |
| Incus dir storage pool | `root.size` is accepted and silently not enforced (1.5 GB written past 1 GiB) | btrfs, zfs or lvm pool; btrfs enforced the quota (EDQUOT) |
| no lxcfs | a guest sees the host's RAM and CPUs in /proc | run lxcfs (a root FUSE daemon: say so in the threat model) |
| `df` in a guest on btrfs | shows the pool size, not the quota | lxcfs does not cover df: state it, or use a per-instance block volume |
| the egress proxy listens on 127.0.0.1 and allows CONNECT only | guests cannot reach it; plain-http apt gets 405 | relay onto each bridge; apt over https with the proxy's CA |
| Docker Hub, anonymous | 429 rate limit | pull once; move images with save and load, or a registry mirror |
| no AppArmor in the kernel | Incus loses a layer it normally adds | seccomp, idmap isolation; state it |
| root lacks CAP_SYS_RESOURCE | Sysbox fails without a patch | not an Incus problem |
| `security.idmap.isolated=true` | each instance gets its own uid range, so its rootfs must be rewritten | about 2 s per launch on btrfs (section 9.4); worth it |
| incusd killed by name (`pkill`, `pgrep -x incusd`) | every guest's monitor is also named incusd, so the guests die too | signal incusd only by pid, or use `incus admin shutdown`; killed by pid, guests survive (section 10.1) |
| `incus launch` with stdin open, from a background job | the client hung 482 s while the server had nothing to do | call the client with stdin closed and a timeout (section 9.6) |

### 1.3 Disk and memory, for sizing

- Disk is thin, not pre-allocated (engines-shortlist.md section 11):
  - A btrfs pool on a 30 GiB loop file used 1.2 GB on disk.
  - A new instance with a 20 GiB quota added nothing.
  - Writing 2 GiB inside it grew the file by 2.1 GB.
  - Deleting that data inside did not return space to the host until
    `fstrim` of the pool ran (3.3 GB back to 1006 MB). Run fstrim on a
    schedule.
- An idle session is small: about 59 MB for the instance with systemd, plus
  19 MB for `kage rpc`. A session's real size is what the agent builds. That
  was not measured.

### 1.4 How errand reaches any engine

errand knows only two backends: `bailey`, which it runs as a CLI, and
rootless `podman` (errand@4a8011cd3668, src/sandbox/mod.rs). To put errand
on Incus with no change to errand, write a `bailey` that drives Incus. The
contract errand uses is in README.md, section "The bailey shim phase": the verbs
`doctor`, `trust`, `untrust` and `run`, and the doctor lines errand parses.
The gVisor shim in poc/ is a worked example of that contract. It is not a
base to build on, because it shares host binaries with the guest.

### 1.5 Credentials, when live testing is needed

Do not paste credentials into the chat, and do not upload them as a file in
the conversation: both stay in the transcript. Put them in the cloud
environment's secrets (environment variables), where the session reads them
from its environment without showing them. Make every one of them disposable
and narrow:

- **Discord**: a new bot in a test server you own, with one channel and the
  Message Content intent enabled. Give errand that channel's id and your own
  user id. Rotate the token afterwards.
- **Model gateway (OpenAI-style)**: a key with a spend limit, and the base URL
  of the gateway. errand needs the base URL to broker the key
  (`providers.<name>.baseUrl`). Without it, errand gives the real key to the
  session (src/serve.rs:727-736).
- **GitHub, only if the GitHub trigger is tested**: errand reads
  notifications, which needs a classic token with `notifications`, or `repo`,
  on a throwaway bot account. That token enters the sandbox as GH_TOKEN, so
  keep it on an account that owns nothing of value.

## 2. The draft prompt, corrected

Changes made to the operator's draft:
- typos fixed;
- the benchmark acceptance test restated as measurable claims;
- the cgroup v1 conflict with "latest by default" stated;
- the bailey route named;
- the "daemon without a daemon" item tied to the measured restart;
- the credential channel changed to environment secrets.

```
Work on https://github.com/AvalynSouvlaki/errand-box, branch main, directly.
Do not create or use any other branch. This is a fork of
https://github.com/Azathothas/errand-box. When you finish, I will open the PR,
or I will ask you to. CI is disabled here: run the CI steps locally through a
stub runner, and add a caveat in the PR text that the merger must run CI on
their side.

Do not credit yourself anywhere: no authored-by, co-authored-by or
generated-by lines. Commit with my git identity as configured.

Start the Docker daemon and set up the container tooling first.

1. Set up the tree as the template says:
   https://raw.githubusercontent.com/Azathothas/TEMPLATE/main/ROUTE.md
   (0BSD). Leave out the PowerShell twins and the slow scripts. Adopt
   AGENTS.md and the core structure, and add nothing we do not use. No
   emojis. Write in ASD-STE100. No narrative or lore. Use
   https://github.com/pkgforge-dev/mirror-archlinux and
   https://github.com/pkgforge-dev/docker-archlinux as examples, but keep
   one AGENTS.md as the router document.

2. Clone with git, not with MCP tools, into a scratch directory:
   https://github.com/Azathothas/agent-box, branch
   ephemeral-errand-sandbox-research. Read
   research/errand-sandbox/next-session.md first, then
   engines-shortlist.md sections 5 to 9.

3. Reproduce the engine results on your host. Run
   experiments/75-host-up.sh (adapted), 81-engine-cost-interleaved.sh,
   lib/fuse-podman-probe.sh and the Incus checks of section 9. Then say
   whether these claims hold:
   - Incus costs 0.9-1.1x of runc on spawn, files, git, cpu and compile.
   - gVisor costs at least 3x on spawn and compile.
   - Incus runs Docker, podman (--cgroups=disabled on cgroup v1), FUSE and
     an rclone network mount, with systemd as PID 1.
   - A started-from-stopped Incus instance is up in under 1 s.
   If one does not hold on your host, stop, and show me the numbers and the
   host conditions before you build anything.

4. If they hold, build errand-box: one tool that sets up Incus, a guest
   image, kage (https://github.com/QaidVoid/kage) and errand
   (https://github.com/QaidVoid/errand) on any Linux host (glibc, musl and
   WSL2), with one command. For example:
   errand-box setup --config ./errand-box.toml --env ./errand-box.env --creds ./creds.env
   Requirements:
   - Idempotent and hermetic. The same command, run again, changes nothing.
     For the bootstrap technique, see
     https://raw.githubusercontent.com/Azathothas/ToolKit/main/tools/windows/wsl-toolkit/internal/toolkit/shipped/bootstrap.sh
   - Pinned or latest versions of every tool, latest by default. Exception:
     on a cgroup v1 host, Incus 7.0 and later do not run, so pin the 6.0 LTS
     line there and say so in the output.
   - Sane defaults, then a printed summary of what was set up and what is
     degraded.
   - Resources: 20 GB of disk per box by default (this needs a btrfs, zfs or
     lvm pool, because the dir pool does not enforce quotas), and 80% of the
     host's RAM and CPU, configurable or derived from the host.
   - A host with no init must still work. The host restarted during the
     research and lost every daemon and device mode. Never signal incusd by
     name: its guests' monitors share the name (section 10.1). Provide a supervisor
     and an idempotent `up` that also runs at boot where an init exists, and
     on first use where none does. Inside the guest, systemd stays the init.
     Evaluate these, with the subagent research in
     research/errand-sandbox/ as a start: aurae, airup, earlyoom, nanocl,
     immortal, daemonize, watchdogd, joker, adaptivemm, and s6, dinit and
     runit.
   - errand runs unchanged. Connect it to Incus through a `bailey` that
     drives Incus (the contract is in research/errand-sandbox/README.md), or
     through its podman backend if that proves better. Show me why.
   - Self-maintaining, safely: track upstream releases and drift with
     scheduled workflows that open issues and PRs, as mirror-archlinux does.
   - Handle the edge cases in next-session.md section 1.2 from day one (and run
     fstrim on the storage pool on a schedule, section 1.3), each
     with a fallback, and a test that proves the fallback.
   - Same guest environment on personal machines, remote containers and
     clusters.

   Compare against what agents in the current errand and bailey setup must
   work around: https://github.com/talaria0101/sandhome. errand-box must make
   all of that unnecessary.

   Use Go or Rust if they fit better than shell. Do not force a large tool
   into shell.

Tests: local tests and mocks must make it mechanically impossible to ship a
broken build. For the live tests that need credentials, I will put them in
the environment's secrets. Tell me exactly which tokens, scopes and
permissions you need before you need them.

Patching or vendoring: follow
https://github.com/Azathothas/TEMPLATE/blob/main/docs/methodology/vendoring.md
A purpose-built guest image (for example with
https://github.com/lxc/distrobuilder) is in scope. We will publish it on
GitHub so that it is fetched at runtime.

Tools: clone with git, or fetch with curl or wget. No MCP tools for that.
Research with https://web.pkgforge.dev/llms.txt and
https://api.gh.pkgforge.dev/<GH_API_PATH>, through curl.

Questions: ask with the question tool once, at the start, then work
unattended.
```
