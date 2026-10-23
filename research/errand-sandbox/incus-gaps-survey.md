# Incus gaps and errand-box groundwork: a delegated survey, 2026-10-23

A subagent wrote this, with curl only, through web.pkgforge.dev and
api.gh.pkgforge.dev:
- 46 searches, 64 pages read, 77 raw files fetched, 255 GitHub API calls;
- 14 issue or PR threads read with their comments, and 9 repository issue
  lists.

**Not checked here, except where it says "tested".** Its tool was refused
when it tried to write its report, so the text below was transcribed from
its answer and trimmed. Its raw notes and its mount probe (`idmap_probe.c`)
were in session scratch and are not in this tree. Every claim it made came
with a source URL; the key ones are kept below.

## Claims since tested here (engines-shortlist.md section 10)

| claim | result |
| --- | --- |
| Guest monitors are also named `incusd`, so a name-based kill takes the guests too; killing only the daemon's pid leaves them running | **confirmed** (10.1) |
| The cold start on the dir pool is unpacking, because the dir driver has no optimized image storage; it is not the rewrite | **confirmed**: init 11.2 s, first start 0.8 s (10.2) |
| A local `incus copy` keeps `volatile.last_state.idmap`, so a golden copy skips the rewrite | **confirmed**: about 0.4 s per new instance on btrfs (10.3) |
| `incus info` "idmapped_mounts: true" is a generic kernel probe, and the real per-path check is CanIdmapMount | read from source, not tested |
| The same open_tree, userns and mount_setattr sequence succeeds from a root shell on these ext4 and btrfs paths | measured by the subagent; the cause of incusd's fallback is still unknown |

## Gap (a): incusd is root

- Upstream's position: Incus "remains running as fully privileged root". For
  fully unprivileged use, they point to LXC, "with many limitations on
  storage, networking and device passthrough".
  https://github.com/lxc/incus/issues/923
- incus-user gives each user a restricted project, but the daemon stays
  root.
- Unprivileged LXC 6.0.x works partly on cgroup v1. It needs `pam_cgfs` or
  cgroups pre-created by root. It has no Incus API. LXC 7.0 dropped v1.
  https://github.com/lxc/lxc/pull/4671
- Recommendation: treat incusd as a root component, and shrink who can
  reach it:
  - unix socket only, no `core.https_address`;
  - a restricted project for sessions;
  - `security.idmap.isolated`;
  - 6.0.7 for its security fixes.

## Gap (b): Incus 7.0 removed cgroup v1

- 7.0 requires cgroup v2 (PR 3235, merged 2026-04-27). Its minimums are
  Go 1.25, Linux 6.12, QEMU 8.2, LXC 6.0.0 and nftables.
  https://github.com/lxc/incus/pull/3235
- 6.0 LTS is supported until June 2029, with security-only fixes for its
  last 3 years. v6.0.7 was tagged 2026-10-17, with 94 commits since 6.0.6,
  mostly hardening. https://linuxcontainers.org/incus/docs/main/support/
- Where to get 6.0.x:

  | host | source | version |
  | --- | --- | --- |
  | glibc with apt | Zabbly `lts-6.0` repo | its index showed 6.0.6 |
  | musl | Alpine 3.23 | 6.0.6 |
  | Nix | `incus-lts` | 6.0.6 |
  | Ubuntu 24.04 | distro package | 6.0.0 only |
  | Debian 13 | distro package | 6.0.4 |

- **Incus ships no daemon binary.** Its releases carry client tools only, so
  incusd comes from a package, from Nix, or from a source build (cgo with
  liblxc).
- The exit route: ask the platform for a cgroup2-only boot
  (`cgroup_no_v1=all`).

## Gap (c): cold start (settled by tests, see above)

- Filesystems that accept idmapped mounts include ext4 (5.12), btrfs (5.15),
  overlayfs (5.19) and tmpfs (6.3). FUSE is not on the list, so a
  `shift=true` disk on an rclone mount cannot use them (INFERRED).
  https://man7.org/linux/man-pages/man2/mount_setattr.2.html
- Steps that would settle why incusd falls back:
  - run incusd with `--debug`, and read its idmapped-mounts line;
  - strace `open_tree`, `mount_setattr` and `clone` in incusd during a launch;
  - check `INCUS_IDMAPPED_MOUNTS_DISABLE` in the daemon's environment;
  - probe the source path of the disk device.

## Gap (d): no host init

- Incus docs: SIGTERM leaves instances running, SIGPWR shuts them down
  cleanly, and on start incusd restarts instances recorded as running.
  https://linuxcontainers.org/incus/docs/main/daemon-behavior/
- Init-less packagings to copy from: Gentoo OpenRC, Void runit, Artix s6,
  and cmspam/incus-docker, whose start script just runs incusd.

Supervisors, ranked for an init-less, cgroup v1 host with no compiler:

| rank | project | why |
| --- | --- | --- |
| 1 | s6, from the static just-containers/s6-overlay tarball (v3.2.3.2, about 730 KB) | supervises by pid, readiness fd, s6-log; works as PID 1 or as a normal process |
| 2 | ochinchina/supervisord (Go, v0.7.6, prebuilt 14 MB) | INI file, no compiler |
| 3 | dinit (v0.22.1) | ordering among lxcfs, dnsmasq and incusd |
| 4 | nitro (v0.8.1, 0BSD) | young; open issues #32 (overrun) and #33 |
| 5 | immortal (v0.24.7) | Go build needed |

- Companions: tini or catatonit as the reaper, and earlyoom with `--avoid`
  for incusd.
- Rejected, with reasons in the original answer:
  - aurae: early development, "do not run in production";
  - nanocl: needs Docker;
  - joker: its restart was removed;
  - daemonize: no restart;
  - watchdogd: there is no /dev/watchdog;
  - adaptivemm: a memory tuner, not a supervisor;
  - airup: source only.
- Against s6: no upstream binaries, and its docs site was reported down
  (https://github.com/skarnet/s6/issues/43).

## Gap (e): no AppArmor

- Without AppArmor, the LXC profile that denies /proc/sys and /sys/kernel
  writes and constrains mounts is simply absent.
- The default seccomp still denies kexec_load, open_by_handle_at, the module
  calls and forced umount.
- On this host, `security.nesting=true` only adds two proc and sys mounts,
  because the layer it would relax is missing anyway.
- Tunables: `security.syscalls.deny`, `.allow`, `raw.seccomp`, and
  `security.guestapi=false`.
- SELinux and BPF LSM are compiled into this kernel, but only the platform
  can enable them.
- Landlock is not used by Incus for instances. Run bailey or Landlock inside
  the guest.

## Gap (f): podman inside on cgroup v1

- Container root is uid 0, so podman acts as root and tries to create
  cgroups; only the guest's own cgroup is writable. Podman's maintainer:
  cgroup v1 cannot be safely delegated.
  https://github.com/containers/podman/issues/9259
- Fix: ship a `containers.conf` in the guest with `cgroups="disabled"`,
  `cgroup_manager="cgroupfs"` and `events_logger="file"`. Untested, and
  possibly cleaner: rootless podman as a normal user inside the guest.
- Podman 6 removed v1, so pin 5.x inside.

## Gap (g): VMs on KVM hosts

- Incus runs QEMU with `-sandbox on,obsolete=deny,elevateprivileges=allow,spawn=allow,resourcecontrol=deny`,
  an AppArmor profile, and a user drop. Its default device set is large.
- Upstream refuses a second VMM (issues 2167, 2518 and 2905). A community
  `microvm` machine-type profile exists:
  https://discuss.linuxcontainers.org/t/a-fun-project-microvms-with-incus/26889
- Firecracker v1.17.0 has the smallest device model, per-thread seccomp,
  and a jailer that supports cgroup v1. It has no Incus integration.
- PVM needs a PVM guest kernel (`CONFIG_PVM_GUEST`), so stock Incus VM
  images would not boot. Firecracker builds with PVM support exist:
  https://github.com/kvcache-ai/firecracker-next
- Recommendation: Incus VMs where one tool is the rule; Firecracker plus
  jailer where hardening is the rule.

## Prior art: coi (Code on Incus)

https://github.com/coipond/coi: v0.15.0 (2026-10-12), 758 stars, Go, MIT.
- An AI-agent sandbox on Incus, with a `curl | bash` installer.
- It installs Incus from Zabbly and makes a ZFS pool, with btrfs as the
  fallback.
- Read its issues #659 (start-up time split), #791 (hardened VM mode) and
  #716 (raw.idmap and systemd credentials) before building errand-box.

## Part B: errand-box groundwork

- **ROUTE.md** is a router, not a tree spec: probe, ask only what cannot be
  measured, classify the job, then read by table. The implied tree is
  ROUTE.md, AGENTS.md, ADOPT.md, MAINTAIN.md, docs/methodology,
  conventions, security and templates, scripts/common and doctor, and a
  bootstrap/ that is deleted after bootstrap.
- **mirror-archlinux and docker-archlinux**, the self-maintenance pattern:
  - an upstream-watch workflow every 15 min (ETag and Last-Modified);
  - a daily watchdog that tests the outcome, not the run ("a run that is
    green and promotes nothing" is stale);
  - `track-issue`: one issue per problem, found by listing rather than
    search, keyed by an HTML comment, closed on clear;
  - a weekly freshness check of every pin, with one tracked issue or PR per
    drift;
  - bot-merge only for diffs that move just a hash or version;
  - `permissions: {}` and actions pinned by commit.
- **wsl-toolkit bootstrap.sh** (1735 lines):
  - POSIX sh, needing only the shell and the package manager;
  - libc detected by its loader files;
  - privilege as root, passwordless sudo, or none, and never a prompt;
  - one table of package names per manager;
  - a bulk install, then a one-at-a-time retry that names failures;
  - upstream tarballs verified by sha256, refusing unverified bytes;
  - chains of hash tools and fetchers;
  - `--dry-run`, `--json`, and exit codes 0, 1 and 2.
- **sandhome**, what agents under bailey lack:
  - writable and executable paths outside project and state;
  - an executable HOME;
  - TCP bind;
  - egress other than TCP 443;
  - a real pty (sandhome ships LD_PRELOAD shims: fakepty, fakepwd);
  - devices;
  - ptrace;
  - limits without cgroup delegation.

  An Incus system container removes most of these by construction.
- **distrobuilder** v3.4.0:
  - `build-incus` gives `incus.tar.xz` and `rootfs.squashfs`.
  - It needs root on a host or VM, not in an Incus container.
  - Stock image sizes: Alpine about 3.5 MB squashfs, Debian 13 about
    105 MB.
  - GitHub Release URLs cannot serve `incus image import <URL>`. Use a
    download plus a local import, or a static simplestreams tree on Pages.
- **vendoring.md**:
  - fix vendored code in place;
  - the record is technical fact only, with a reproduction command;
  - delete a patch when a release carries the fix;
  - a verbatim copy has a pin file with sha256 per file;
  - never vendor agent instruction files or build output.
- **Language**: a POSIX sh bootstrap for install, and one static binary for
  the long-lived wrapper. errand, bailey and kage are Rust and ship static
  musl builds, so Rust matches. Go is equally valid (Incus, coi and
  distrobox v2 use it).

## What it could not reach

- 404: pyke369/sincus, and the v6.0.7 Release page (the tag exists).
- 403: the Zabbly pool listing.
- Not fetched: skarnet.org.
- Not evaluated: process-compose and runit.
