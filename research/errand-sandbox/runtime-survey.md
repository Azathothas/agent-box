# Container runtimes and Sysbox alternatives, surveyed 2026-10-23

**This is a delegated survey and has not been checked.** A subagent ran it
with curl only, through web.pkgforge.dev and api.gh.pkgforge.dev, in two
passes. The first pass was thin: 9 tool calls. The second ran about 57 web
searches, read 86 pages and pulled metadata for 46 repositories. Its claims
are reported here as the subagent's, not as findings of this research.
`engines-shortlist.md` is where they are tested on the host.

The raw material is in `corpus/runtime-survey-2026-10-23/`: the subagent's
scripts (`srch.sh`, `meta.sh`, `rd.sh`), its search logs, and the GitHub
metadata, cut down to the fields cited. Copies of the third-party pages it
read (`rd/*.md`) and of PVM source were left out of this public tree and not
kept. Re-run `rd.sh` to read them again.

## Contradicted by measurement on this host

1. "Sysbox needs a systemd host." It runs here without systemd: its daemons
   were started by hand (`engines-shortlist.md` section 1.1).
2. "No statement of cgroup v1 support for Sysbox; untested." It is now tested
   on this cgroup v1 host and runs, after three patches
   (`engines-shortlist.md` section 1.2).
3. "`nested_pvm` not found in virt-pvm pvm.c, so its effect is unknown." This
   host's own kernel names the parameter in dmesg, and gives no /dev/kvm
   without it (README item 3). The platform's kernel is a fork, so the
   upstream tree is the wrong place to look.

## What the subagent says changed since its first pass

- cgroup v1 support was removed in Incus 7.0 (PR 3235, merged 2026-04-27),
  LXC 7.0 (PR 4671), systemd v258 and Podman 6.0.0.
- Docker 29 deprecates cgroup v1 and keeps it until at least May 2029.
- The podman here is 4.9.3. Podman 6 would refuse this host.

## Its ranked top 5 for this host

1. **Sysbox v0.7.1.** Docker, systemd and apt inside an unprivileged
   container, at near-native speed. Against it: maintenance is best effort
   (#979 "State of the project" has no maintainer reply; last comment
   2026-06-22). 0.7.0 broke on kernel 6.17 with containerd 2.x until 0.7.1
   fixed it (#1028). There are open seccomp-listener (#1046) and sysbox-fs
   hang issues.
2. **gVisor with Docker inside.** Docker 29.6.1 is listed as partially
   supported. It needs `--net-raw`, `--allow-packet-socket-write` and a tmpfs
   on /var/lib/docker. Against it: the costs measured here, 8x on spawning
   and 3.7x on git.
3. **PVM with Firecracker, CubeSandbox (TencentCloud) or AgentENV
   (kvcache-ai).** The only VM boundary without VT-x. Against it: the
   platform must boot with PVM on. The code is out of tree, on 6.12.33, and
   needs a guest kernel to match. The module refuses a host with KPTI, or
   one without FSGSBASE, RDTSCP or CX16.
4. **sandlock or nono.** Rootless Landlock and seccomp process sandboxes that
   need no cgroups. Against them: they are not containers, so there is no
   Docker inside.
5. **Rootless podman 4.9.3 with crun, hardened.** Against it:
   podman-in-podman is fragile (#15419; the stock examples use
   `--privileged`), and there are no cgroup limits without delegation.

## Ruled out, by its reading

| route | reason |
| --- | --- |
| Incus, LXC 7, systemd-nspawn 258+, kern, Podman 6 | need cgroup v2 |
| Kata, libkrun, crun-krun, microsandbox, smolvm, Docker `sbx` | need KVM |
| E2B, Cloudflare, Vercel, CodeSandbox | Firecracker underneath, so KVM |
| Edera | needs the host booted into Xen |
| QEMU without KVM (quicksand) | its own documentation says 10-50x slower |
| Daytona | its core became private in 2026-06 and the repository is archived |

Self-hostable and needing no KVM, by its reading: OpenSandbox (runc or
gVisor), the Docker and Podman drivers of NVIDIA OpenShell, the userns mode
of gominimal/minimal, and coder/envbox and ContainAI, which wrap Sysbox.

## What it could not reach

Pages that answered 404 or 403: Daytona's deployment docs, Runloop's
architecture, e2b's self-host page, Modal's gVisor fork, and edera-dev/edera.
The Sysbox maintainers' answer in #979 does not exist to read. For UML and
LiteShield it read docs only.
