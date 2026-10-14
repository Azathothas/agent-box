# MicroVM tier: engine selection and build specification

This page is the operator-supplied research document on the microVM tier, converted to ASD-STE100 Simplified Technical English.
It predates the pivot of 2026-10-14.
[docs/decisions.md](../decisions.md) records the rulings of that pivot (D1 to D14) and the decisions of the planning session.
[docs/architecture.md](../architecture.md) is the technical reference, and it wins every conflict with this page.

## What changed since this document was written

- The tier is now the product (D1, D12). The document treats it as an opt-in tier of a chat bot whose sessions share the host kernel.
- Instances are persistent. `pause` is now an in-memory vCPU freeze, and `hibernate` (snapshot to disk) is a later opt-in verb (planning decision P5, T-184).
- A daemon restart now re-adopts running instances (T-090, T-107). The document removes every running VMM as an orphan instead.
- The guest now has a virtual NIC. Its packets go over vsock to a userspace netstack in the host daemon, and policy applies per flow (D5).
- Credentialed traffic goes to explicit plain-HTTP broker endpoints on the guest gateway (P4). The loopback CONNECT proxy and the planted CA of the document are gone.
- The data disk uses `io_engine: Sync`, `cache_type: Writeback` and discard. The `Async` and `Unsafe` settings of the document are wrong for a persistent disk.
- Windows has its own engine: native Hyper-V through the Host Compute Service (D11). The statement "Windows keeps podman" is retired.
- Project files move only through a git forge (D4). The host seeds no workspace image, and the wire protocol has no `put` or `get`.
- [docs/roadmap.md](../roadmap.md) replaces the build sequence of section 8. This page renames those steps S0 to S12, because M0 to M12 now name the new milestones.
- No product crate exists yet (D14). T-033 creates the crates. The crate names in this page are the names of the old plan.

Each section that the pivot superseded starts with a `NOTE:` paragraph that names its replacement.
Each claim that the engine evaluation of 2026-10-14 or its adversarial review corrected has a `NOTE: Correction` paragraph in place.
The section [Corrections made in this conversion](#corrections-made-in-this-conversion) lists them all.

## Conversion notes

### Labels

The labelling convention of the document stays:

- MEASURED: the research session ran it on the research host, and section 2.3 has the output.
- READ: an upstream project's own document or artefact, cited at the point of use.
- No label: design, which is neither.

This page shortens upstream quotes to phrases, or paraphrases them.
Each paraphrase keeps its READ label and its citation.

### The research host

The document names no host and gives no date.
It cites a Kata issue that was opened on 2026-10-09, so the research happened between 2026-10-09 and 2026-10-14.
The research host had no `/dev/kvm`, `memfd_create` failed with `EPERM`, and `CapEff` was 0.

### Pins and citations

The document gave short pins.
This page cites upstream files in the form owner/repo@commit:path:line, with the full commit from the pins table of [docs/references.md](../references.md).

| Reference | Pin in the document | Pin in this page |
| --- | --- | --- |
| Firecracker | v1.17.0 | v1.17.0, cited at its full commit |
| Cloud Hypervisor | v53.0 | v53.0, cited at its full commit |
| agent-sandbox | 4b63868 | the same commit, cited in full |
| errand | issue 5 revision 3 at d067597 | not in the corpus (next section); the errand tree is cited at its pin |
| Kata Containers | 4.2.0 | not pinned; cited by tag URL; not checked again |
| agent-box | the old `main` at 44896d6 | the old main branch of the previous repository; its history is not carried over (D10) |

### Sources cited but not in the corpus

The document cites [errand issue 5](https://github.com/QaidVoid/errand/issues/5) at revision 3 (d067597).
It also cites [errand issue 17](https://github.com/QaidVoid/errand/issues/17) and [errand issue 18](https://github.com/QaidVoid/errand/issues/18).
The old references corpus holds only errand issues 1 to 3.
Nobody read these three sources again for this conversion, so the claims that rest on them are as the document gives them.
T-013 re-pins errand and checks these claims again.

The document uses these labels from errand issue 5: findings F1 to F12, Tasks B, E, F and G, and the numbered sections.

### Old entry ids

The document names entries of the old plan.
Old ids collide with new ids that mean other things, so this page writes only new ids in its prose.
This block maps each old id that the document uses:

```
old id        old title (old plan at 44896d6)                  new id or fate
T-001         Cargo workspace, strictly downward graph         T-001
T-002         Shared types in box-core                         T-020
T-003         The binary and its subcommands                   T-029
T-010         The backend trait and the capability report      T-032
T-011         A probe that attempts the real setup             T-092 (Linux), T-108 (Windows)
T-012         Per-session policy generation                    T-123
T-013         The bailey backend, locked to one broker         retired (D1)
T-015         Podman in a machine, Windows host checks         retired (D11)
T-017         Measure whether a session can run containers    T-185
T-020         The CONNECT proxy                                T-142 (broker endpoint server)
T-021         The injection gate                               T-143
T-023         The binding table                                T-141
T-024         The certificate authority                        T-149 (optional, backlog; P4)
T-025         Substitutions                                    T-145
T-026         The credential store, failing closed two ways    T-140, T-143
T-020..T-026  the broker, as a group                           T-140 to T-147
T-030         Never run a tool against agent-writable state    T-030
T-031         A trusted channel the session cannot forge into  T-031
T-032         Teardown and orphan discovery                    T-090, T-091, T-107
T-046         Pin the harness project-trust setting            T-160
T-047         The harness adapter and its framing              T-161
T-048         Pin the harness tool approval mode               T-160
T-054         cargo xtask check record                         T-003
T-064         The containment battery                          T-180
T-091         When a shared kernel stops being enough          the product (D1, D12)
```

Other renames in this page:

- Section 1.2 named its rows D1 to D6. D numbers now name operator rulings, so this page names those rows V1 to V6.
- Section 3.3 named its reasons C1 to C5, which collides with the claims C1 to C9 of section 1.1. This page names those reasons H1 to H5.
- Section 8 named its steps M0 to M12. This page names them S0 to S12.

## 1. Reconciliation

NOTE: This section compares errand issue 5 with the old agent-box plan. The pivot replaced that plan. The column "Now" says how each row stands after the pivot.

### 1.1 Settled by both documents, no dispute

| # | Claim | errand issue 5 | Old agent-box plan | Now |
| --- | --- | --- | --- | --- |
| C1 | To relax a sandbox boundary for nested containers is a regression, not a configuration change. | F1: reject it as a default, never a toggle on an existing boundary. | The nested-container entry, route 1. | Holds. T-185 measures containers inside the guest. |
| C2 | The credential is not in the session environment. | F10. Its section 2.7 records that the podman backend violates this today. | The old architecture page: the credential never leaves the broker process. | Holds as the rule R-NO-SECRET-IN-GUEST. |
| C3 | There is one route off the host, to the broker. | Sections 2.4 and 2.6. | The old architecture page, as its stated invariant. | Changed. The guest NIC reaches a host netstack with per-flow policy (D5). Credentials go only through broker endpoints (P4). |
| C4 | The probe attempts the real setup. It does not read configuration. | Section 10 lists the commands. | The probe entry: "Attempt, never read". | Holds (T-092, T-108). |
| C5 | A gap stops the daemon. There is no fallback between backends. | Sections 2.2 and 9.8. | The backend trait and probe entries. | Holds: no fallback between engines (T-032). |
| C6 | A sandbox that overstates itself is worse than one that does not. | F9, section 9.8. | The old limits page. | Holds ([docs/limits.md](../limits.md)). |
| C7 | End-to-end latency is the only performance number that matters. | Section 5. | The battery and kernel-tier entries. | Holds (T-181). |
| C8 | Warm pools and cluster orchestration are out of scope. | F11. | The old limits page. | Holds for warm instance pools and clusters. The provider pool of D8 is a different thing. |
| C9 | The daemon never runs a tool against agent-writable state. | The finding of [errand issue 2](https://github.com/QaidVoid/errand/issues/2). | T-030. | Holds (T-030, the rule R-HOST-NO-TOOLS). |

### 1.2 Where they diverge, and which reading applies

| # | errand issue 5 | Old agent-box plan | Resolution in the document | Now |
| --- | --- | --- | --- | --- |
| V1 | F5 comes last, not first. The decision tree ends at "is a shared kernel acceptable for this tenancy?", and "no" leads to F5. | Tenants are mutually untrusted (old TODO/RULES.md, line 196). The deployment is a public community channel. | errand's own terminal branch applies. The microVM tier is a tier, not the first isolation work. T-030, T-031, the harness pins, the backend trait and the broker all come first. | Superseded. The microVM is the product (D1). The P0 rules still come first in the roadmap. |
| V2 | The surface of a microVM backend includes a `toHostPath` mapping and a project volume. | The backend trait makes `to_host_path` a security boundary that returns `None` outside the project. | No host path crosses into the guest. `to_host_path` returns `None` for every input, by construction (section 5.1). | Superseded. There is no Sandbox trait and no host path. Files move through a git forge only (D4). |
| V3 | Nesting inside a shared-kernel sandbox needs `--cap-add` and a writable store (Task B). | The nested-container entry measures first and adds nothing. | In a microVM, the blocking primitives are guest-kernel primitives. The host boundary does not move (section 7). | Holds (T-185). |
| V4 | On Windows, the route is podman in a machine. | The Windows podman entry measures that path. bailey is refused on Windows. | The tier is Linux-only and needs `/dev/kvm`. A host that is itself a VM gives nested virtualization, which the gVisor platform guide calls poor and "not recommended for production". Windows keeps podman. | Superseded by D11: Windows uses Hyper-V through HCS. |
| V5 | F12 leaves the conflict between KVM and rootless operation open. | The bailey backend entry requires rootless operation. | Corrected by measurement: the jailer needs root plus capabilities (section 1.3). A small privileged helper is necessary. It is still less privileged than every alternative. | Holds (D3). The helper is socket-activated and has a fixed operation set ([docs/engines.md](../engines.md)). |
| V6 | The credential gap closes before a new backend claims a credential boundary. | The broker entries are the centre of the design. | The guest reaches the broker over vsock. The daemon holds the credential at all times. | Holds in a new form. The broker serves endpoints on the guest gateway, behind the netstack, over vsock (P4). |

### 1.3 The privileged helper, measured

errand F12 shape (b) applies: a privileged helper manages the VMM, and the session stays unprivileged.
errand's own verdict: shape (b) is the only shape that adds a privileged component.
So it is the shape that needs its own threat model.

MEASURED on the research host: the Firecracker v1.17.0 release verifies against its own `SHA256SUMS` file (19 non-debug files, all `OK`).

```
$ ./jailer-v1.17.0-x86_64 --id t2 --exec-file ./firecracker-v1.17.0-x86_64 \
    --uid 0 --gid 0 --chroot-base-dir <base> -- ./firecracker-v1.17.0-x86_64 --version
Failed to unshare into new mount namespace: Operation not permitted (os error 1)
Error: UnshareNewNs(Os { code: 1, kind: PermissionDenied, ... })
```

The exit code is 1.
On the same host, `/proc/self/status` shows `CapEff: 0000000000000000`.
uid 0 is not sufficient: the jailer needs the capabilities.

READ: the jailer documentation says that the project runs the jailer as root.
It says that the jailer needs a smaller set of capabilities, which is still "to be determined" (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:291`).

READ: the jailer does these steps in this order (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:111`):

1. Validate all provided paths and the VM id.
2. Close all open file descriptors except stdin, stdout and stderr.
3. Clean the environment.
4. Create `<chroot_base>/<exec_file_name>/<id>/root`. If that path exists already, the jailer does nothing at this step.
5. Copy the `--exec-file` binary into the jail.
6. Set `setrlimit` bounds.
7. Create the cgroup subfolder and write the current pid into `tasks`.
8. Call `unshare()` for a new mount namespace, then `pivot_root()` and `chroot()`.
9. Call `mknod` for `/dev/net/tun` and for `/dev/kvm`.
10. Call `chown` on the jail directory and on both device nodes.
11. Optionally join a network namespace.
12. Optionally call `setsid()` for `--daemonize`.
13. Optionally call `clone(CLONE_NEWPID)` and store the pid of the child.
14. Set the uid and gid, then call `exec`.

The privileged steps (the cgroup write, `unshare`, `pivot_root`, `mknod`, `chown`) all come before the privilege drop.
Only the privilege drop works without capabilities, because an unprivileged caller is already in that state.

The research session observed two consequences. It did not infer them:

- The jail directory and its root subtree exist before `unshare` fails. A helper that reports success on directory creation reports a jail that does not work.
- The jailer creates the jail directory itself. A stale directory from a crashed run looks the same as a new one until somebody verifies the pid. Section 5.5 verifies the pid for this reason.

The helper surface, enumerated, is a fixed set of syscalls over a path that the caller chooses:

| Operation | Capability | Input |
| --- | --- | --- |
| `unshare(CLONE_NEWNS)` | `CAP_SYS_ADMIN` | none |
| `pivot_root`, `chroot` | `CAP_SYS_ADMIN` | the jail directory |
| `mknod` `/dev/kvm` | `CAP_MKNOD` | a fixed node, with the major and minor numbers read from the host's `/dev/kvm` |
| `mknod` `/dev/net/tun` | `CAP_MKNOD` | a fixed node, 10:200 |
| `chown` | `CAP_CHOWN` | one directory tree |
| cgroup writes | a delegated cgroup v2 root | `memory.max`, `cpu.max`, `pids.max`, `io.max` |

The helper never parses data that the guest supplies.
That property makes the helper acceptable, and T-030 asks for that property.

NOTE: Correction (this conversion, 2026-10-14). `chroot` needs `CAP_SYS_CHROOT`, not `CAP_SYS_ADMIN`. The drop to the instance uid and gid needs `CAP_SETUID` and `CAP_SETGID`. [docs/engines.md](../engines.md#capability-lifecycle) lists the planned set, and T-081 measures the minimum.

NOTE: The new plan keeps this helper (D3) with a fixed operation set (T-023, T-081). The Windows engine has a helper service with the same rules (T-100).

Comparison against the alternatives.
READ: the rootless VMM guide of Kata at 4.2.0 says that only the VMM runs as a non-root user.
The Kata shim and virtiofsd still run as root ([how-to-run-rootless-vmm.md](https://github.com/kata-containers/kata-containers/blob/4.2.0/docs/how-to/how-to-run-rootless-vmm.md)).
The same guide names the jailer as the answer for Firecracker.
One narrow helper is better than a root shim plus a root filesystem daemon.

### 1.4 Gaps in both source documents

- Neither document names an engine. Both put the decision behind a gate. Sections 2 and 3 resolve the inputs of that gate.
- Neither document owns a guest image supply chain. errand Task G asks who owns it and leaves the answer open (section 6).
- Neither document has a negative control in its acceptance criteria (section 9, check 12).

## 2. Measurement inputs

### 2.1 The density and footprint study

READ: [agent-sandbox pull request 1279](https://github.com/kubernetes-sigs/agent-sandbox/pull/1279), "Add multi-runtime benchmark study for gVisor and Kata Containers", closed on 2026-08-03.
The document read it at the agent-sandbox pin.
The node type is `c4-standard-8`: 8 vCPU, 30 GB of physical RAM, about 28 GB allocatable (`kubernetes-sigs/agent-sandbox@4b63868f0d67e4ee0fa690bb14cb5722b78d679d:examples/gke-swap/runtimes/README.md:9`).
Density means the maximum stable pod density.
Chrome Ready is a headless-Chrome workload. It is a proxy for agent latency, not a measurement of it.

| Runtime | Host RSS per pod | Maximum stable density | Chrome Ready P50 | Chrome Ready P99 |
| --- | --- | --- | --- | --- |
| `runsc` (gVisor) | about 370 MiB | 80 without swap, 160 with NVMe swap | 17.59 s | 41.07 s |
| `kata-qemu` | about 1,130 MiB | 20 in both cases | 16.44 s | 24.92 s |
| `kata-clh` | about 1,128 MiB | 40 without swap, 50 with swap | 34.54 s | 60.99 s |

The table comes from lines 31 to 33 and 57 to 63 of the same README.

The component breakdown is in `kubernetes-sigs/agent-sandbox@4b63868f0d67e4ee0fa690bb14cb5722b78d679d:examples/gke-swap/runtimes/kata/README.md:31`:

- `qemu` 678.0 MiB and `cloud-hypervisor` 664.7 MiB;
- `virtiofsd` 421.0 to 426.8 MiB;
- the shim 35.0 to 36.0 MiB.

Sandbox Ready, from the same file: `kata-qemu` 4.86 s average and 8.40 s P99 at density 20 (line 48).
For `kata-clh` it is 7.00 s and 12.90 s at density 30 (line 64).

Consequences:

1. A microVM boundary costs 2 to 4 times in density against gVisor on the same hardware. 80 against 40 is 2 times, and 80 against 20 is 4 times. These ratios are the arithmetic of the document, not the words of the upstream.
2. Inside Kata, Cloud Hypervisor beats QEMU by 2 to 2.5 times: 40 against 20 without swap, and 50 against 20 with swap. This is also the arithmetic of the document.
3. The upstream README calls the result without swap "2.5x density over QEMU without swap" (line 90). That does not follow from 40 against 20. The error is upstream's, and this page does not repeat it.
4. The upstream attribution that holds: the async decoupling of vCPU threads cuts D-state stalls to 0.069 percent, 13 times fewer than QEMU (kata/README.md, line 86).
5. Both Kata configurations need node boot disks of 150 to 250 GB, against a default of 100 GB. Otherwise the host virtiofsd crashes with `SIGBUS`, exit code 135 (kata/README.md, lines 89 to 91). That is the cost of the shared-filesystem path, and the upstream measured it.
6. This study does not compare Firecracker against Cloud Hypervisor. Both VMMs sit under one container runtime, whose shim and virtiofsd are common costs inside the RSS figures. No conclusion about a VMM driven directly follows from it.

### 2.2 Firecracker's own specification, and its conditions

READ: SPECIFICATION.md says that integration tests enforce its figures on each pull request and each merge to main (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:SPECIFICATION.md:5`).

| Metric | Value | Conditions |
| --- | --- | --- |
| VMM start to API socket | 8 CPU ms | Wall clock is typically about 12 ms, in a range of 6 to 60 ms. |
| `InstanceStart` to guest `/sbin/init` | 125 ms or less | 1 vCPU, 128 MiB, a minimal kernel and rootfs, serial console disabled. |
| VMM memory overhead | 5 MiB or less | The same configuration. It excludes MMDS and depends on the workload. |

The header of SPECIFICATION.md puts the figures on M5D.metal and M6G.metal instance types (line 8).
The README table of tested platforms lists m5n, m6i, m6a, m7i, m7a, m8i, m6g, m7g, m8g and m9g instances (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:README.md:133`).
m5d.metal is not in that table.
CI enforces the 125 ms figure, and it is a number internal to the project.

The creation rate is not from SPECIFICATION.md, and CI does not enforce it.
READ: design.md states a steady rate of 5 microVMs per host core per second, for a minimal kernel, 1 vCPU and 128 MiB (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/design.md:26`).
Its example is 180 microVMs per second on a host with 36 physical cores.
That is a claim of a design document, with its own conditions.
It is a creation rate, not a session latency, so it stays out of the enforced table.

### 2.3 What this project has measured, and what it has not

MEASURED on the research host:

1. The Firecracker v1.17.0 release verifies against its own published checksums: 19 non-debug files, all `OK`.
2. `firecracker --version` prints `v1.17.0`.
3. `jailer --version` prints `Jailer v1.17.0`.
4. The jailer fails at `unshare(CLONE_NEWNS)` with `CapEff: 0`.
5. `memfd_create` returns `EPERM`.
6. `/dev/kvm` is absent.
7. A boot attempt with a two-device vsock configuration fails at `Vsock device error: ... cannot create a memfd`.

The boot failure and the absent `/dev/kvm` agree.
Firecracker builds the configured devices before it opens `/dev/kvm`, so the vsock device fails first and KVM is never reached.
The failing step is the observed one.

Not measured by this project:

- cold start to a usable session;
- container runtime behaviour in the guest;
- the end-to-end turn latency;
- the jailer on a host with capabilities.

The research host cannot produce any of them: no `/dev/kvm`, `memfd_create` returns `EPERM`, and `CapEff` is 0.
It is a valid negative control for the whole tier.
A plan that claims to have exercised the jail on that host describes an event that did not occur.
Steps S5 and later need an external KVM host (section 8).

NOTE: After the document, the planning session measured `/dev/kvm` inside a wsl-toolkit 6.0.0 job on the Windows 11 Pro host of the operator. That was one run on 2026-10-14, with WSL2 kernel 7.2.0 and `kvm_intel` nested=Y. That is nested virtualization. It is valid for development tests, never for a production claim. [docs/status.md](../status.md) records measurements. The spikes T-014 and T-015 come first.

## 3. Engine selection

NOTE: This section selects the Linux engine, and the selection still holds (D12), with the corrections below. The Windows engine is a separate decision (D11). [docs/research/engines-2026-10.md](engines-2026-10.md) records that evaluation and the engines that it rejected.

### 3.1 Requirements, each traced to a source

| # | Requirement | Source in the old plan | Now |
| --- | --- | --- | --- |
| R1 | The VMM process runs unprivileged, in a chroot and namespaces, with cgroup limits. | The bailey backend and probe entries. | Holds ([docs/engines.md](../engines.md)). |
| R2 | The boundary is a second kernel, not the host kernel. | The kernel-tier entry; the old sandbox-model page, "Where the guarantees end". | Holds: it is the product (D1). |
| R3 | A host-guest transport that is not the IP network of the guest. | The bailey backend entry: one route off the host, the broker. | Holds for control and data. The IP network of the guest now travels on vsock too (D5). |
| R4 | A small device model, because every device is host attack surface. | The old limits page: anything granted is granted. | Holds. It is the main reason that Firecracker wins (section 3.4). |
| R5 | Memory, cpu and pid limits per session that apply. | The probe and bailey backend entries. | Holds per instance (T-082). |
| R6 | Orphan discovery and removal by an identity that the host can see. | The teardown entry. | Holds, after re-adoption (T-090, T-107). |
| R7 | No shared writable filesystem with the host. | T-030. | Holds (D4). |
| R8 | The guest runs a container runtime, and the host boundary does not move. | The nested-container entry. | Holds; T-185 measures it. |
| R9 | A pinned, reproducible supply chain for the kernel and the rootfs. | errand Task G; the pin rules of the old containers page. | Holds (T-060, T-062, T-066). |
| R10 | x86-64 and aarch64 Linux hosts. | The reality of the deployment. | Met in part: aarch64 only on 4K-page hosts (section 3.2, T-094). |

### 3.2 Firecracker v1.17.0, driven directly, jailed

This mapping gives the artefact that each claim rests on.

R1, R5 and R6: the jailer.
The flag set comes from `jailer --help` on the verified release.
It agrees with the usage section of the jailer documentation (READ, `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:16`).

- Required flags: `--exec-file`, `--gid`, `--id`, `--uid`.
- Optional flags: `--cgroup`, `--cgroup-version`, `--chroot-base-dir`, `--daemonize`, `--netns`, `--new-pid-ns`, `--parent-cgroup`, `--resource-limit`.
- `--resource-limit` accepts `fsize` and `no-file` only.
- `--cgroup` takes `<cgroup_file>=<value>`, and it can repeat.
- `--cgroup-version` defaults to 1, so cgroup v2 needs an explicit request (line 39).

Section 1.3 covers the privilege requirement.
R1 is met only in the weak sense: the VMM runs unprivileged, but a privileged helper must put it there.
That is the honest reading.

R2: the guest kernel is the boundary.
READ: design.md treats all vCPU threads as malicious code from their start, and it contains them with nested trust zones (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/design.md:83`).

R3: vsock.
READ: the `Vsock` schema of the shipped spec requires `guest_cid` (minimum 3) and `uds_path` (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:1934`).

- For a host-initiated connection, Firecracker listens on `uds_path`. The host connects there and sends a forwarding request, such as `CONNECT 52\n` for guest port 52.
- Guest-initiated connections land on `uds_path_<PORT>`.
- CID 2 is the host.

NOTE: The document read the release asset firecracker_spec-v1.17.0.yaml. This page cites the in-tree swagger file at the same pin. The 26 paths and the 13 configuration properties below agree with the in-tree file.

R4: the device model, enumerated from the shipped spec.
READ: the `paths:` map has 26 paths. One of them is the root `/`, which carries no device.
The configuration-bearing subset, the properties of `FullVmConfiguration`, has 13 entries (the swagger file, line 1337):

```
balloon  boot-source  cpu-config  drives  entropy  logger  machine-config
memory-hotplug  metrics  mmds-config  network-interfaces  pmem  vsock
```

- The spec has no virtio-fs, no 9p, no virtio-scsi, no GPU, no console device other than `/serial`, and no input device.
- A case-insensitive grep over the shipped spec for virtio-fs, 9p and shared-directory names finds nothing.
- [Firecracker issue 1180](https://github.com/firecracker-microvm/firecracker/issues/1180) is closed. Its 2023 reply says that virtio-fs support is not on the roadmap.
- The only activity in 2026 is [issue 5687](https://github.com/firecracker-microvm/firecracker/issues/5687), labelled "Priority: Low" and "Status: Parked".
- Its [pull request 6147](https://github.com/firecracker-microvm/firecracker/pull/6147) is a draft.
- The virtualization design page of Kata at 4.2.0 records the same limit in its comparison table ([virtualization.md](https://github.com/kata-containers/kata-containers/blob/4.2.0/docs/design/virtualization.md)).

The seccomp policy.
READ: the research session parsed the shipped seccomp policy, and did not take the word of the docs for it.
The release file seccomp-filter-v1.17.0-x86_64.json has three per-thread allowlists, each with `default_action: trap`.

- The document gives their sizes as vmm 82, api 37 and vcpu 50 syscalls.
- These syscalls are trapped in all three threads: `unshare`, `clone`, `pivot_root`, `chroot`, `setuid`, `prctl`, `bpf`, `perf_event_open`, `userfaultfd`.
- So the guest cannot cause a host syscall that the VMM thread does not already allow.

NOTE: Correction (this conversion, 2026-10-14). 82, 37 and 50 are counts of filter rules, not of distinct syscalls. Some syscalls have several rules with different argument conditions. The in-tree x86_64 filter at the pin has 82, 37 and 50 rules, which name 51, 32 and 27 distinct syscalls (READ, `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:resources/seccomp/x86_64-unknown-linux-musl.json:1`). The list of trapped syscalls is correct for that file.

NOTE: Correction (engine evaluation, 2026-10-14). The vmm thread allows `io_uring_setup`, `io_uring_enter` and `io_uring_register` for every `io_engine` (READ, the same file, line 63). Firecracker accepts a custom filter through `--seccomp-filter` (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/seccomp.md:59`). Open question for T-084: does agent-box supply a filter without io_uring?

R9: the supply chain is explicit.
READ: the kernel policy lists the supported guest kernels (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:32`).
The document says that the guest needs an uncompressed kernel image and an ext4 rootfs (section 6).

NOTE: Correction (adversarial review, 2026-10-14). Firecracker v1.17.0 also boots a compressed bzImage on x86_64 (pull request 6037, `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:CHANGELOG.md:29`). Its docs still recommend vmlinux. The guest decompresses a bzImage at boot, which costs boot time and memory (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/rootfs-and-kernel-setup.md:13`). T-060 builds both formats. T-065 measures erofs against ext4 for the read-only base.

R10: x86-64 and aarch64 Linux.
READ: getting-started.md says that Firecracker supports x86_64 and aarch64 Linux (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/getting-started.md:22`).

NOTE: Correction (adversarial review, 2026-10-14). R10 is met only in part.

- Firecracker tests aarch64 only on AWS Graviton 2 to 5 (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:README.md:148`).
- Its kernel policy lists only 4K-page host kernels (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:24`).
- Raspberry Pi 5 and Asahi Linux use 16K pages by default (the review READ this in web sources).
- agent-box limits aarch64 support to 4K-page hosts until somebody measures a boot on a 16K-page host (T-094).
- QEMU under libvirt is the recorded fallback for those hosts.
- `SendCtrlAltDel` is x86 only, so a clean stop on aarch64 needs a poweroff inside the guest (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/actions.md:34`).

NOTE: Correction (adversarial review, 2026-10-14): a performance risk that the document does not state.

- One VMM thread emulates the net, block and vsock devices (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/design.md:74`).
- The `Sync` engine uses blocking syscalls (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-io-engine.md:3`).
- Discard works only with the `Sync` engine (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-discard.md:12`).
- So guest disk load, such as cargo or npm, shares the thread that carries every host channel. Expect head-of-line blocking.
- T-015 measures the vsock round-trip latency during a cargo build. The mitigations are vhost-user-blk (developer preview) or Cloud Hypervisor (T-093).

### 3.3 Why Cloud Hypervisor loses

Cloud Hypervisor (CLH) is the strongest rejected candidate.
It is the production choice of Azure: AKS Pod Sandboxing runs Kata on CLH.

What CLH does better, stated because it is real:

- It has a threat model document, which Firecracker does not have in that form. That document says that CLH is not hardened against denial of service from its guest kernel.
- It draws the management boundary in a useful way. An API client can tell CLH to overwrite its own memory, but a management stack can still act for untrusted parties. That is this use case.
- It has virtio-fs: `--fs tag=myfs,socket=/tmp/virtiofs,num_queues=1,queue_size=512` (`cloud-hypervisor/cloud-hypervisor@9ed824d6d08df3e96f7d5f50795d9449ac99f431:docs/fs.md:79`).
- It has SEV-SNP, with a measured-launch footnote that v52.0 added.

NOTE: Correction (engine evaluation, 2026-10-14). The file docs/threat-model.md is not in the v53.0 tag. It exists on the main branch ([threat-model.md on main](https://github.com/cloud-hypervisor/cloud-hypervisor/blob/main/docs/threat-model.md), READ 2026-10-14). The document cited it at the v53.0 pin, which is off by version. The first two points above come from the file on main.

Why it still loses, in order of weight:

H1. No jailer. This reason decides it.
The research session checked it four ways:

- no path that matches `jailer` in the v53.0 git tree;
- none on main;
- no `jailer` among the 24 workspace members of the root manifest (`cloud-hypervisor/cloud-hypervisor@9ed824d6d08df3e96f7d5f50795d9449ac99f431:Cargo.toml:16`);
- no `--jailer` option in `get_cli_options_sorted()` (`cloud-hypervisor/cloud-hypervisor@9ed824d6d08df3e96f7d5f50795d9449ac99f431:cloud-hypervisor/src/main.rs:187`).

What it offers instead:

- Seccomp filtering is on by default, because the project believes that "security should not be an option" (`cloud-hypervisor/cloud-hypervisor@9ed824d6d08df3e96f7d5f50795d9449ac99f431:docs/seccomp.md:23`).
- Landlock is opt-in behind `--landlock`, and it covers filesystem access only (`cloud-hypervisor/cloud-hypervisor@9ed824d6d08df3e96f7d5f50795d9449ac99f431:docs/landlock.md:1`).

In the terms of T-030, the VMM is a large parser of guest-controlled bytes on the host.
Under Firecracker, a chroot, a uid, a cgroup and per-thread seccomp confine it.
Under CLH, it runs as the user of the operator, in the namespaces of the operator, with seccomp.
These are different layers.
Only the first contains a VMM bug inside a directory that the attacker did not start in.

NOTE: Correction (engine evaluation, 2026-10-14). The Firecracker jailer cannot wrap CLH as it is. The jailer appends Firecracker-specific arguments such as `--id` and `--start-time-us` (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/jailer/src/env.rs:555`). So the CLH fallback (T-093) needs a jail that agent-box builds itself.

H2. Its virtio-fs is a separate daemon per session, and it costs guest memory isolation.
READ, from fs.md (`cloud-hypervisor/cloud-hypervisor@9ed824d6d08df3e96f7d5f50795d9449ac99f431:docs/fs.md:13`):

- The device uses the vhost-user protocol, so a dedicated host process, virtiofsd, does the device emulation.
- `--fs` needs `--memory shared=on`, so that the processes can share memory (line 67).
- DAX is not available, because the daemon side of DAX is not stable yet (line 97).
- The document says that `cache=never` is the default. The documented example passes `--cache=never` (line 35).
- The guest kernel must be at least v5.10 (line 54).

The cost, measured upstream in section 2.1: 421.0 to 426.8 MiB of RSS per pod.
That is 37.3 to 37.8 percent of the total of 1,128 MiB (the arithmetic of the document).
Guest pages are also mapped as shareable with the host process that holds the daemon.

H3. The project disclaims its only performance figure in the repository.
READ: performance_metrics.md shows sample data, such as `boot_time_pmem_ms` with a mean of 105.9 ms from v40.0, July 2024 (`cloud-hypervisor/cloud-hypervisor@9ed824d6d08df3e96f7d5f50795d9449ac99f431:docs/performance_metrics.md:109`).
A note says that the data is for illustration only and does not represent the real performance (line 118).
The public figure, boot to userspace in less than 100 ms with direct kernel boot, is on a marketing page.
That page states no host, no vCPU count and no image.

H4. Its Intel TDX path does not build at v53.0.
READ: the main binary contains `compile_error!("Feature 'tdx' is broken.")` (`cloud-hypervisor/cloud-hypervisor@9ed824d6d08df3e96f7d5f50795d9449ac99f431:cloud-hypervisor/src/main.rs:899`).
intel_tdx.md still says that a build with the `tdx` feature is all that CLH needs (`cloud-hypervisor/cloud-hypervisor@9ed824d6d08df3e96f7d5f50795d9449ac99f431:docs/intel_tdx.md:64`).
SEV-SNP works, and it is the path to use.

H5. Platform support is narrower than it reads.
CLH supports KVM on Linux and MSHV on Linux.
WHPX, the Windows host hypervisor, appears in no README, no docs file and no CLI argument list.
Every document about Windows guests assumes a host with KVM enabled (`cloud-hypervisor/cloud-hypervisor@9ed824d6d08df3e96f7d5f50795d9449ac99f431:docs/windows.md:7`).
The engine evaluation of 2026-10-14 agrees: the workspace depends only on the kvm and mshv crates (`cloud-hypervisor/cloud-hypervisor@9ed824d6d08df3e96f7d5f50795d9449ac99f431:Cargo.toml:58`).

### 3.4 The other rejections

NOTE: [docs/research/engines-2026-10.md](engines-2026-10.md) holds the current engine evaluation, with the Windows engines and the single-engine options. This table is the Linux view of the document at its date.

| Candidate | Reason |
| --- | --- |
| gVisor (`runsc`) | Not a microVM: a userspace kernel. The document kept it as the cheap tier. The pivot removes that tier (D1). |
| Kata + Firecracker | At 4.2.0, Kata pins Firecracker v1.12.1, which is past upstream support ([issue 12644](https://github.com/kata-containers/kata-containers/issues/12644), open). The upgrade to v1.17.0 is [issue 14016](https://github.com/kata-containers/kata-containers/issues/14016), open since 2026-10-09. It has no CI: it is absent from required-tests.yaml and from every GitHub Actions matrix in basic-ci-amd64.yaml. [Issue 13565](https://github.com/kata-containers/kata-containers/issues/13565) says so. `pause_vm`, `save_vm`, `resume_vm` and `resize_vcpu` each log "Not implemented". `get_hypervisor_metrics` is a bare `todo!()`, so one scrape of /metrics panics the sandbox ([issue 13708](https://github.com/kata-containers/kata-containers/issues/13708), open). |
| Kata + Cloud Hypervisor | It brings containerd, a shim, virtiofsd and a root shim to run one agent. The Azure docs record that Kata host-network access is not supported, and that arm64 is not supported. A pod VM defaults to 512 MiB whatever its limit, and fractional CPU limits round up to a whole vCPU. A limit that is not a reservation contradicts the old limits page. |
| Kata + Dragonball | Not released on its own. It is the built-in backend of Kata. |
| QEMU direct | The worst in the study of section 2.1: stuck at 20 pods under swap. The Big QEMU Lock lets vCPU threads in D-state block the main VMM event loop, which triggers containerd-shim keep-alive timeouts (kata/README.md, line 82). It has a large device model. |
| libkrun, crun krun | Its own security model says that the guest and the VMM share one security context (`containers/libkrun@b9f27258fe557d2970fdb0c57836b3b290d06d86:README.md:95`). So the boundary is the confinement of the caller, not of the engine. |
| Hyperlight, hyperlight-unikraft | Core Hyperlight has no guest kernel or OS (`hyperlight-dev/hyperlight@ad05110e17bf40bab2e5306d917ec468c1db064c:README.md:12`). Its README rules out full Linux guest workloads that need syscalls, networking or filesystem access (line 67). Unikraft has Python at Tier 1; C, Rust, Go and Node are smoke-tested only and experimental. It is before 1.0, and its CNCF metadata still has no list of adopters. |
| crosvm | A ChromeOS and Android niche. The document says that its multi-tenant and jailer-equivalent story is less developed, and that it offers nothing this design needs. The document did not check this itself. |
| rust-vmm | The library that both engines come from. To select it is to write a VMM. |
| Unikernels (Nanos, Unikraft, OSv) | Rejected on workload: a coding agent needs a full Linux userland, a PTY, package managers and a compiler toolchain. |
| firecracker-containerd | Zero GitHub releases. It is on the containerd 1.7 line. Its README lists CRI conformance as longer-term work and disclaims multi-tenant use. |
| Docker Sandboxes, Daytona, external providers | Proprietary, or they put the host and a third party in the trusted computing base. The verdict of errand F7 is "consent required". |

NOTE: Correction (adversarial review, 2026-10-14). Firecracker is not the only Linux VMM with a jail.

- crosvm puts each device in its own process under minijail, with namespaces and seccomp (`google/crosvm@113b02ce7e7092bae7b455bcc8c99c8f5e891b4a:docs/book/src/appendix/minijail.md:3`).
- Its main KVM process has no seccomp policy. The sweep of 2026-10-14 READ the policy directory jail/seccomp/x86_64 and found no policy for that process.
- Its vsock is the in-kernel vhost-vsock (`google/crosvm@113b02ce7e7092bae7b455bcc8c99c8f5e891b4a:devices/device_virtio_vsock/src/sys/linux.rs:15`).
- QEMU under libvirt runs as an unprivileged user with cgroups, seccomp and a per-VM sVirt label. This is an inference of the review, not READ.
- Firecracker wins on the smallest device model (R4), and on file-permission access control of its hybrid-vsock Unix sockets.
- With vhost-vsock, any local process can open AF_VSOCK to the CID of a guest.

### 3.5 Reopening condition

Firecracker is replaced if and only if both conditions hold:

1. A measurement shows that block I/O or memory pressure is the dominant cost per session.
2. No Firecracker release meets the requirement.

The document adds that this reopens R7, because the virtio-fs of CLH is a shared filesystem, which T-030 forbids.
Any such use must address T-030 explicitly.
The pull request to watch is Firecracker 6147.
The replacement is a decision with its own entry, not a configuration value.

NOTE: The reopening condition now has entries: T-015 (the spike) and T-093 (the measurement). The engine evaluation expects the condition to fire for persistent instances, because block throughput and space reclaim matter more. The fallback is CLH with a jail that agent-box builds. It uses the CLH block device, not virtio-fs, so R7 does not reopen (D4).

## 4. Tool shape

NOTE: Superseded. [docs/architecture.md](../architecture.md) gives the crate graph of the new plan. No crate exists yet (D14), and T-033 creates them. The old names map to the new crates as follows.

- box-vm-proto -> box-proto: framing, ports and messages, for host and guest.
- box-vm, modules engine and jail -> box-vmm (the firecracker module), driven by box-helper.
- box-vm, module image -> box-image.
- box-vm-init -> box-guest.
- box-sandbox and its Sandbox trait -> the Engine trait in box-daemon (T-032).
- box-cli -> box-cli, the binary `agent-box`.

The document specifies one binary, box-vm, that implements the backend trait of box-sandbox.
The graph goes strictly downward, as the old workspace entry requires (T-001).

```
box-core        no IO: identifiers, errors, cancellation, wire types.
box-vm-proto    framing and message types only, no IO, no platform code. Built for
                BOTH host and guest: it is the crate that makes the two sides of
                the protocol one definition.
box-vm          depends on core + proto. Three modules:
                  box_vm::engine    Firecracker API client over a Unix socket
                  box_vm::jail      jailer invocation, jail inventory, helper protocol
                  box_vm::image     kernel and rootfs build, pin verification
box-vm-init     depends on proto only. Static musl, PID 1 in the guest. Mounts,
                builds the environment, runs the harness adapter, holds the
                transcript nonce, speaks the protocol. Cross-compiled, not a
                workspace binary.
box-sandbox     the Sandbox trait; box_sandbox::vm is the backend.
box-cli         the binary; the only crate that knows all of the above.
```

- box-vm-proto is a separate crate because it has two callers, host and guest. That is the only reason this design permits a split.
- box-vm-init is separate from box-vm, because it compiles to a static musl target and must not inherit the host dependencies.
- Split any other module only when it gets a second caller.
- There is no plugin surface, no registry, no backend fallback and no unconfined mode.

Two artefacts are outside the workspace.
The first is box-vm-init, pinned by digest into the rootfs image.
The second is the images directory: the kernel and rootfs pipeline, which writes a pins file with versions and digests.

## 5. Design decisions

### 5.1 No shared filesystem, in either direction

NOTE: Partly superseded. "No shared filesystem" holds (D4), and files move through a git forge only. The host seeds no workspace image. Each instance has a sparse data disk, 30 GiB by default. box-guest formats it inside the guest on first boot ([docs/guest.md](../guest.md)). Delete with verified absence is T-091.

The host never mounts the filesystem of the guest. The guest never mounts the filesystem of the host.
This is T-030 in its strongest form.
R7 also forces it: the engine cannot offer a shared filesystem, even if the design wanted one.

- Base root: a read-only virtio-blk image, shared across sessions by digest, never copied. READ: the field `Drive.is_read_only` is for virtio-block and is omitted for vhost-user-block (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:1236`). So this is a virtio-blk configuration.
- Workspace: a writable ext4 image per session, sized to the budget of the session, in the directory of the session under the state directory. The host creates it before boot from a seed that the operator supplies, or from a new clone.
- The seed comes from the operator, so to build it is not to run a tool against agent state.
- `/tmp`: a tmpfs inside the guest.
- Egress of bytes: after boot, the only path out is a `get` on the wire protocol.
- `to_host_path` returns `None` for every input, by structure. The implementation holds no field from which a host path can come, so no code path can construct a `Some`.
- Step S3 asserts this against adversarial inputs. Without the test, a later mapping for the base-root image would change a security property, and no test would fail.
- The host writes the seed and never reads the workspace. After image creation, the host opens the workspace image zero times.
- Only the VMM holds a file descriptor on the workspace image, and only that descriptor is read-write. This is the requirement of T-030 in structural form, and step S6 traces it.

Workspace teardown is a named step, not a consequence.
The image is inside the jail directory, so jail removal takes it too.
The document states this, because the alternative is the failure that matters.
An image that survives teardown holds the private source of one session and everything that the agent wrote into it.
Anybody who can read the state directory can read it.
The old plan forbids a shared writable cache between mutually untrusted users.
Step S4 and check 9 of section 9 verify absence by content, not by listing.

Disk accounting becomes enforceable, which it is not on the container backends.
The image has a size, and the size is the budget.
To cross it gives `ENOSPC` in the guest, not a slow leak on the host.

NOTE: Correction (engine evaluation, 2026-10-14). A persistent data disk cannot live and die with the jail.

- The helper rebuilds the jail on each boot and hard-links the disks into it (T-086).
- Jail teardown removes the links and never deletes the data disk.
- Only `delete` removes the disk, and it verifies each absence (T-091).
- The host never parses the disk after creation: no fsck, mount, zerofree or resize2fs of an image that the agent wrote (T-087).
- Free space returns to the host through fstrim in the guest and discard in the VMM.

### 5.2 Egress: no interface, not a filtered interface

NOTE: Superseded by D5 and P4. The guest now has a virtual NIC: box-guest creates tun0. Its packets go as frames over vsock to a userspace netstack in the host daemon. The netstack has DNS on the gateway and a policy per flow ([docs/network.md](../network.md)).

NOTE: Credentialed traffic now goes to explicit plain-HTTP broker endpoints, with no TLS interception and no planted CA ([docs/broker.md](../broker.md)). Two ideas of this section survive. The VM configuration has no `network-interfaces` key (T-084). The egress token per instance survives as the first frame on the network vsock port (T-045, [docs/protocol.md](../protocol.md)).

The guest gets no virtio-net device.
READ: `FullVmConfiguration` in the shipped spec has no `required` list, and `network-interfaces` is an optional array (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:1337`).
So a configuration with vsock only is well formed.
errand F5 asks for egress enforcement at the TAP or the bridge.
The absence of the surface meets that requirement, not a rule kept against the surface.

```
guest   no virtio-net, no IP route beyond lo, no ARP, no DHCP, no DNS
        HTTP_PROXY/HTTPS_PROXY/NO_PROXY -> 127.0.0.1:<fixed>, set at build time
        loopback CONNECT proxy -> vsock 5004
host    vsock listener on uds_path_5004 -> broker (the old broker entries)
```

The broker must tell sessions apart, and vsock does not do that for it.
Every guest-initiated connection arrives at one host socket, `uds_path_5004`.
The vsock of Firecracker carries no peer identity other than the guest CID, which the host assigns per VM.
So the identifying material must come from the guest, on the first frame.
It must be a value that the host issued to that session:

- The host provisions a second, distinct secret at boot: the egress token. It goes with the transcript nonce of section 5.3: a different value, the same provisioning, never written to the workspace image.
- The guest proxy attaches the token to every forwarded CONNECT, as a header.
- The broker binds the token to the session id at provisioning. It refuses a request whose token is unknown, whose session is torn down, or whose destination is not in the bindings of that session.
- Tokens are per session and never shared. So the guest of tenant A cannot send a request that the broker attributes to tenant B, and it cannot borrow the bindings of B.
- That is the ruling on mutually untrusted tenants, meeting an authentication boundary. A shared broker endpoint without identification per session would be a hole in it.

The per-destination validation, the CLI naming and the credential injection of the broker stay as the old broker entries specify (now T-140 to T-147).
Only the identity of the caller is new here.

Consequences:

- The `RESTRICTED_NETWORK` question does not come up: no pasta, no TAP, no nftables rule set, no mapping of the host loopback.
- The pasta setting `pasta:--map-host-loopback,none,--map-guest-addr,none` fixes a problem that this configuration does not have.
- The image build plants the CA of the broker in the trust store of the guest (now T-149, optional).
- The host firewall is not part of the boundary of this tier, so the two cannot drift apart.
- The cost, stated: any protocol that the broker does not terminate is unavailable.
- The old limits page already records that SSH access to repositories and raw TCP are refused, not passed. Here there is no socket to use.
- A workload that needs a raw network is a second capability with its own gate: a TAP plus host nftables, per the network-setup.md of Firecracker. It is not in v1.

NOTE: Under D5 this cost is gone. DNS, ssh and any TCP or UDP work in the guest, subject to the flow policy (T-121, T-123). Raw network access goes through the netstack, never through a TAP. The guest holds no SSH key. Until the SSH agent of T-257 exists on port 5005, ssh reaches only hosts that need no credential.

NOTE: A lesson from bailey and pasta stays: never map the 127/8 view of the guest to the host loopback ([docs/network.md](../network.md)).

### 5.3 Wire protocol, over vsock

NOTE: Partly superseded. The port table below is the map of the document, and the v1 map of [docs/protocol.md](../protocol.md#ports) supersedes it. The v1 ports are 5000 control, 5001 PTY and exec, 5002 events and 5003 network frames. Then come 5004, the kage bridge, and 5005, the SSH agent of T-257. There is no `put` or `get` of files (D4).

NOTE: The framing, the two ceilings, the version rule and the nonce design survive (T-021, T-022, T-031). The trusted event channel moves from port 5003 to port 5002.

The ports are fixed by protocol version v1, as the document gives them.
The protocol refuses an unknown port.

| Port | Direction | Purpose |
| --- | --- | --- |
| 5000 | host to guest | control: version handshake, start, stop, stats |
| 5001 | host to guest | harness stdio: stdin, stdout, stderr, framed |
| 5002 | host to guest | bytes: `put` into and `get` out of `/workspace` |
| 5003 | guest to host | events and heartbeat, on a channel that the host owns (T-031) |
| 5004 | guest to host | egress: the guest proxy forwards here (section 5.2) |

Framing: a `u32` big-endian length, a `u8` type, then the payload.
Two ceilings are pinned as constants.
The guest controls the length prefix directly, and a `u32` can announce up to 4 GiB.

| Constant | Value | Rationale |
| --- | --- | --- |
| `MAX_HEAD` | 8 KiB | The errand broker caps its CONNECT head at 8192 bytes. Anything larger is not a request that this protocol serves. |
| `MAX_FRAME` | 8 MiB | The largest legitimate unit is a file `put` or `get`. 8 MiB is above any one source file that a build reads, and below what a guest can use as an allocation primitive. |

A length above either ceiling closes the connection before the receiver allocates a buffer of that size.
The acceptance of step S1 names both values, so the test checks the guard at the real number.
A version mismatch refuses the session. There is no negotiation down.

Two framing lessons come from errand: read the CONNECT head in increments, not in one fixed-size read, and cap it.
READ: the errand function `readRequestHead` reads one byte at a time and caps at 8192 bytes (`QaidVoid/errand@58a178b2a03dee289bb535e1fdc58676344050eb:src/sandbox/broker.ts:303`).
It is the reference shape.
The document names two errand defects in this area:

- issue 17: a CONNECT with no timeout and a single IP hangs;
- issue 18: a plain-HTTP request against a broker that does CONNECT only fails.

The research session did not read the two issues, so this page does not quote them.
The 4xx status differs between the refusal path of the broker and its provider path.

NOTE: errand issues 17 and 18 were not in the corpus either (T-013). The lessons carry over to the HTTP head parser of the broker (T-142).

Port 5003 is the trusted channel, and it carries the rule of T-031.
No path inside the session reaches the write end, except the intended one.
The acceptance is a test that forges an event and sees the channel reject it.

A vsock port alone does not meet that rule, and the reason is structural.
The vsock device is a guest device: the guest kernel exposes `/dev/vsock` to every process in the guest.
Any process in the session can call `socket(AF_VSOCK)` and connect to port 5003 directly, past box-vm-init.
vsock carries no peer credential and no authorisation per port.
So the port map alone gives every guest process the same access as the init process.
That is the shape of the exhibit of T-031, not its fix.

A nonce per session closes it.
The host provisions the nonce at boot, on a channel that the processes of the agent cannot reach:

| Element | Who holds it | Why the agent cannot get it |
| --- | --- | --- |
| the nonce | box-vm-init, in memory only | It arrives over vsock 5000 before any agent process exists. Nobody writes it to the workspace image. |
| the listener | box-vm-init, as the only process bound to 5003 | The agent runs as a different uid from box-vm-init inside the guest. |
| the bearer | the first frame of every 5003 message | An agent that connects to 5003 without it is refused before any frame is parsed. |
| rotation | started by the host, on the control channel | The agent has no channel on which to request it or to see it. |

The bearer comparison is constant-time, and the nonce is 32 bytes from the CSPRNG of the host.
A refused frame closes the connection and increments a counter.
The startup report carries the counter in its list of absences, because a session that forges events is a finding, not noise.

This guarantee is weaker than the guarantee of T-030, and the document states it as weaker.
A guest process that runs as the uid of box-vm-init would have the nonce.
The claim is narrower: the agent process, at a different uid, cannot forge a transcript event.
Nothing else follows from it.

NOTE: The new plan keeps this design. box-guest runs as root and holds the event nonce, and kage runs as uid 1000 (T-031, T-041). The image has no sudo and no setuid program. A guest process that gets root can still forge events for its own instance, so billing and backoff use broker data only (T-147).

### 5.4 Firecracker configuration

NOTE: Partly superseded and corrected. The data disk settings changed (the correction below). The base is read-only and shared by digest, and the data disk replaces the workspace image (T-084, T-087).

NOTE: [docs/engines.md](../engines.md#vm-configuration-fields) holds the current field table and corrects more rows. The column "Value now" below follows it.

Every field below is from the shipped spec.
The client does not set a field that is not listed.
The client refuses to send a key that is not in its own schema, so a later release cannot add a device without notice.

| Field | Value in the document | Value now | Source note (READ) |
| --- | --- | --- | --- |
| `machine-config.vcpu_count` | 1 or an even number | the same, from the profile | "either 1 or an even number"; minimum 1, maximum 32 |
| `machine-config.mem_size_mib` | the budget of the session | from the profile | required |
| `machine-config.smt` | `false` | the same | default `false`; x86 only |
| `machine-config.huge_pages` | `Transparent` on a THP host, `None` otherwise | `None`, because the balloon cannot reclaim memory that huge pages back | enum `None`, `Transparent`, `2M`; default `None`. `Transparent` enables `madvise(MADV_HUGEPAGE)`, which is a host property. |
| `boot-source.kernel_image_path` | pinned, uncompressed | pinned vmlinux (T-060) | the kernel policy; a bzImage also boots (section 3.2) |
| `boot-source.boot_args` | `console=ttyS0 reboot=k panic=1 pci=off` | the default list of Firecracker with the serial port off, built by the helper; never a secret (R-NO-SECRET-IN-GUEST, T-256) | the documented example of Firecracker |
| `drives[0]` | base root, `is_root_device: true`, `is_read_only: true` | the same: the read-only base image | `drive_id` and `is_root_device` are required |
| `drives[1]` | workspace, `is_read_only: false`, `path_on_host` per session | the data disk: 30 GiB by default, sparse, read-write | `path_on_host` is required for virtio-blk |
| `drives[*].cache_type` | `Unsafe`; step S11 measures `Writeback` | `Writeback` on the data disk | enum `Unsafe`, `Writeback`; default `Unsafe` |
| `drives[*].io_engine` | `Async` | `Sync` | `Async` needs a host kernel newer than 5.10.51; default `Sync` |
| `drives[1].discard` | not set | `true`, plus fstrim in the guest | only for writable drives on the `Sync` engine; new in v1.17.0 |
| `vsock.guest_cid` | per session | `3` for every instance, because hybrid vsock has no host-wide CID space | minimum 3; CID 2 is the host |
| `vsock.uds_path` | inside the jail | the same | required |
| `balloon.amount_mib` | the reclaim target | the same | required, with `deflate_on_oom` |
| `entropy` | present | the same | `EntropyDevice` exists in the spec |
| `network-interfaces` | absent | absent; a test refuses it (T-084) | section 5.2 |
| `pmem` | absent | absent | none |
| `memory-hotplug` | absent in v1 | absent; see section 5.7 | section 5.7 |
| `serial` | present, off unless diagnosing | not a configuration key; the helper never sends `PUT /serial` | none |

The field citations: `Drive` at line 1212, `MachineConfiguration` at line 1446, `Balloon` at line 902 and `Vsock` at line 1934 of the in-tree swagger file.

The start sequence is `PUT /actions` with `{"action_type": "InstanceStart"}`.
The verb is PUT, not PATCH: `/actions` defines only `put` (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:41`).
The enum is `FlushMetrics`, `InstanceStart`, `SendCtrlAltDel` (line 1376).

The reasoning of the document on `Drive.cache_type`:

- It made `Unsafe` a decision, not a default.
- It said that `Unsafe` keeps the image out of the host page cache, which is safer for a disk that an untrusted guest writes.
- It said that `Writeback` is faster, that the choice trades host memory against write amplification, and that step S11 measures both.

NOTE: Correction (engine evaluation, 2026-10-14). That reasoning is wrong on both settings.

- `Unsafe` does not keep the image out of the host page cache. It only stops advertising the virtio flush feature, so the device drops guest flushes (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-caching.md:16`).
- The block device code of Firecracker uses no O_DIRECT (READ, the block device sources at the pin). So all disk I/O goes through the host page cache in both modes.
- `Unsafe` gives up data integrity when the host loses its page cache, for example in a power outage. Firecracker recommends it for ephemeral storage (line 40).
- `Writeback` makes each guest flush an fsync of the backing file (line 23). A persistent disk needs it.
- `Async` (io_uring) is developer preview and not yet for production (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-io-engine.md:116`). It cannot discard.
- On host kernels before 5.12, such as 5.10, the io_uring workers start in the root cgroup, outside the VMM cgroup (the same file, line 142).
- Discard works only on writable drives with the `Sync` engine (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-discard.md:12`). Without discard, the sparse host file only grows.
- Firecracker takes no lock on a disk file. agent-box stops a double start with instance locks in the daemon and in the helper ([docs/engines.md](../engines.md#data-disk-settings), T-087).
- The settings now: `io_engine: Sync`, `cache_type: Writeback`, `discard: true`, and fstrim in the guest. T-015 and T-093 measure the cost.

### 5.5 Jail construction

NOTE: Partly superseded and corrected. Persistent instances need a stable uid and a stable cgroup per instance. The helper rebuilds the jail on each boot and hard-links the disks into it (T-086). A daemon restart re-adopts running instances (T-090). The pid verification of this section survives. [docs/engines.md](../engines.md#re-adoption-and-orphans) compares device and inode numbers in place of paths, and T-090 measures both forms.

```
jailer --id <session-id>
       --exec-file <chroot>/<binary>       copied in, per the jailer's own rule
       --uid <per-session uid> --gid <per-session gid>
       --chroot-base-dir <stateDir>/vm/jails
       --cgroup-version 2                 the default is 1; v2 must be explicit
       --parent-cgroup <delegated root>
       --new-pid-ns
       -- <firecracker> --api-sock <chroot>/run/fc.sock --id <session-id>
```

NOTE: Correction (this conversion, 2026-10-14). The jailer already passes `--id` to Firecracker (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:106`). The second `--id` after `--` duplicates it. T-086 does not pass it again.

READ: the user must hard-link or copy into the jailed root folder every resource that the API gives to the VM (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:275`).
So the kernel, both images and the API socket are inside the jail.
The uid of Firecracker needs read and write permission on the backing file of a read-write drive.

READ: the jailer considers all its inputs trusted, including the paths that it receives (line 266).
The operator who calls it is part of the trusted computing base.
This is why the helper validates every input (section 5.6).

cgroup v2 under a delegated root: `memory.max`, `cpu.max`, `pids.max`, and `io.max` where the delegation exposes it.
A missing `io.max` is a note.
A missing `memory.max`, or a missing delegated root, is a gap, and a gap stops the daemon (T-082, T-092).

`--resource-limit fsize` is the size of the workspace image plus one filesystem block.
So one file cannot grow beyond what the image can hold.
`fsize` caps one file, not the total usage.
The old plan records the same limit for the `fsize` ulimit of the podman backend.
The image size bounds the total disk instead.
The two together are not redundant, because a VMM bug can write outside the filesystem of the guest.

NOTE: [docs/engines.md](../engines.md#jailer-invocation) proposes `fsize` as the data disk size plus one 4096-byte block. T-086 decides it.

Orphan identity is a pid file plus a verification, not the existence of a directory.
The jailer writes its pid to `<chroot_dir>/<exec_file_name>.pid`, and the inventory reads that file.
The daemon kills a candidate only after it confirms the pid as the VMM of this tier:

1. `/proc/<pid>` exists.
2. `/proc/<pid>/root` resolves to the jail directory, or `/proc/<pid>/exe` resolves to the copy of the binary inside that jail.
3. The pid does not belong to a live session in the current table of the daemon.

Without step 2, the daemon could signal a recycled host pid.
READ: the jailer documentation gives both the hazard and the remedy (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:307`):

- With `--daemonize` and without a new PID namespace, Firecracker has a different pid from the jailer. A kill of the jailer does not stop Firecracker.
- So the jailer always stores the pid of the child in `<exec_file_name>.pid` in the jail root, with or without `--new-pid-ns`.

So the pid file is reliable in both modes.
This design passes `--new-pid-ns`, so a kill of the jail process reaches the VMM.
The verification above depends on neither mode.
It reads the pid file and confirms the pid, whether or not the jailer was killed correctly.

NOTE: That passage of the jailer documentation spells the flag `--new-ns-pid`. The real flag is `--new-pid-ns`.

A daemon restart turns every live VM into an orphan, by definition.
The document intends this.
A jail whose VM outlived its daemon is unreachable.
The daemon cannot re-adopt it, because the transcript nonce and the broker bindings were in memory.
So the inventory is the jail directory.
"Live" means a VMM whose pid passes step 2 and whose session id the table of the current daemon claims.
A running VMM that no session claims is an orphan, and the daemon removes it.
Check 8 of section 9 exercises this.

NOTE: Correction (engine evaluation, 2026-10-14). Persistent instances invert this rule.

- A daemon restart does not kill running instances.
- The daemon reads its encrypted secrets per instance again, asks the helper for the inventory, and verifies each VMM pid as above.
- Then it reconnects, with channels that it keys again (T-090).
- Only a VMM that no instance record claims is an orphan, and only that VMM is removed.

READ: the jailer does not clean up after itself. Cleanup is the job of the user (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:283`).
Removal takes the cgroup, the jail directory, the pid file and both images, and it releases the uid.
The test asserts all four absences.
A cgroup can go while the directory stays, and a surviving directory at a recycled uid is the collision that matters.

NOTE: In the new plan, jail teardown at each stop removes the hard links and never deletes the data disk. `delete` removes the jail, the cgroup, the disks, any snapshots and the uid, and it verifies each absence (T-091).

### 5.6 The privileged helper protocol

NOTE: Partly superseded. The helper now has a fixed operation set for persistent instances: create, start, pause, resume, stop, kill, delete, inventory and probe (T-023).

NOTE: The helper is socket-activated by systemd. It checks the peer with `SO_PEERCRED` against the agent-box group, and it exits when idle (T-081). The input rules of this section survive. [docs/engines.md](../engines.md) holds the helper protocol and its threat model (T-080).

It is not a general-purpose helper.
It has a fixed operation set, it takes no arguments from the guest, and it parses no guest data.

| Operation | Inputs | Outputs |
| --- | --- | --- |
| `create_jail` | session id, uid, gid, delegated cgroup path | the jail path, or a refusal |
| `destroy_jail` | session id | whether the cgroup, the directory, the pid file and the images are gone |
| `inventory` | none | the list of jail directories, with their verified pids |

A refusal is a refusal, and it names the syscall error.
The helper never reports a partial jail as a success.
Section 1.3 shows the jailer creating the jail tree and then failing at `unshare`.
A helper that reported success on directory creation alone would be wrong.
The helper verifies the jail with a real operation inside it, not by seeing that a directory appeared.

The input provenance, per field.
The claim that the helper never parses guest data is true, and it is why the helper is acceptable.
But it is a claim about the guest, and the untrusted input is one step earlier in the chain:

| Field | Provenance | Validation |
| --- | --- | --- |
| session id | the daemon, from a request that the user can see | It must match `[0-9a-f]{32}`. The helper refuses anything else before any syscall. |
| uid, gid | the daemon, from a uid allocator | It must be in the range that the allocator was given. |
| cgroup path | the daemon, composed from one configured root | It must be that root plus the session id. The helper refuses a path from any caller. |
| jail base | the daemon | It must equal the configured value. It is not a parameter at all. |

So no string that a caller supplies reaches a syscall.
The identifier validation is a test (step S4), not a code comment.

NOTE: The session id is now the instance id: 128 random bits, written as `[0-9a-f]{32}` (T-020). In the new helper protocol, the caller cannot express a path at all, because the helper derives every path (T-023).

The capability lifecycle.
The helper gets `CAP_SYS_ADMIN`, `CAP_MKNOD` and `CAP_CHOWN` for one operation, and drops them before it returns.
So it is idle at the identity of the daemon, with an empty `CapEff`.
A helper that kept `CAP_SYS_ADMIN` between sessions would have a much larger blast radius.
A test reads `/proc/<helper-pid>/status` while the helper is idle (step S4).

The helper is the only privileged component of the tool, so it gets its own threat model document.
Step S4 delivers that document with its own acceptance, as errand F12 requires.
Its exposure: root, called by the daemon, three fixed operations, no caller-supplied path, capabilities dropped between operations.
Each refusal condition above has a test in step S4, including the path refusal.

### 5.7 Snapshots are out of scope

NOTE: Superseded in part. `pause` is now `PATCH /vm` with the state `Paused`: an in-memory vCPU freeze with no snapshot (T-089). v1.17.0 is the minimum pin, because pull request 6100 fixes vsock delivery after a bare pause and resume (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:CHANGELOG.md:83`).

NOTE: Snapshot to disk is the later opt-in verb `hibernate` (T-184). It uses a Full snapshot, once only, with manifest checks. Its files are root-owned, mode 0600, outside the jail, with a verified digest. Cold boot stays the primitive of `start`.

READ: the snapshot documentation of Firecracker calls it insecure to resume the same state more than once (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/snapshotting/snapshot-support.md:549`).
The risk covers identifiers, random numbers and seeds, the entropy pool and cryptographic tokens.

- A resume from a snapshot also loses network and vsock state. The guest can lose network and vsock packets, and open vsock connections close (line 56).
- Diff snapshots are in developer preview (line 117). The release policy says that developer-preview features are not for production (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/RELEASE_POLICY.md:142`).
- Snapshot restore happens only before boot, and Firecracker checks only a 64-bit CRC of the state file (line 103).

So cold boot is the only primitive, which puts weight on the cold-boot time.
Both source projects already ruled pools out of scope, so this costs nothing today.

`memory-hotplug` exists in the spec: `total_size_mib`, `slot_size_mib` (default and minimum 128), and `block_size_mib` (default 2) (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:1867`).
The document does not use it in v1, by decision.
A hotpluggable region changes the memory accounting, and the accounting acceptance of step S9 assumes a fixed region.

NOTE: Open question, from the engine evaluation of 2026-10-14. The v1.17.0 documentation of virtio-mem hotplug has no developer-preview mark. It could unplug guest RAM from idle or paused instances, as a footprint control in place of the balloon. T-181 measures it with the other footprint controls.

## 6. Guest image supply chain

NOTE: Partly superseded. The base is Debian 13 slim (D6), built with mmdebstrap from a pinned snapshot.debian.org timestamp (T-062). The guest trusts no broker CA (P4). One kernel config carries the virtio and the Hyper-V drivers for both engines (T-060). [docs/guest.md](../guest.md) holds the current design.

This is the largest item that is not code, and errand Task G leaves it unanswered.

- Kernel. A pinned source and a minimal config, validated against the kernel policy of Firecracker. An uncompressed vmlinux plus a hash of the config. A named owner rebuilds it on a named cadence.
- Rootfs. A minimal Linux, reproducible from a pinned base by a pinned builder. It contains box-vm-init, the harness, the toolchains and the broker CA.
- The rootfs contains a container runtime only if the measurement of section 7 justifies it.
- No credential, ever, in any layer. Every session uses the image, so a secret in it is a secret given to every session that boots it.
- Two images, not one: a base and a variant per toolchain, so that a compiler toolchain is not resident in every session. Section 2.1 is the argument: 370 MiB per pod for gVisor against 1,128 MiB for kata-clh.
- No image is ever made writable after build. A session that needs a toolchain boots the variant that has it.
- There is no shared writable overlay between variants or between sessions. The tenancy ruling forbids a shared writable cache between mutually untrusted users, and a writable overlay is exactly that.
- A named approver bounds the number of variants. To add one is a supply-chain change with a diff of the pins file, so the verification tiers do not multiply without review.
- Verification, in three tiers, from the old containers page. The published sums file proves the transport. A digest that somebody obtains and reviews separately proves the exact bytes.
- A keyless signature against the identity of the publishing workflow proves provenance. Verify before you run. Delete a cache that does not match. Report which tier ran.
- Ownership. The document names one owner of the rebuilds and the patches, and writes that name beside the pin. Two rules: a release tag beats a commit, and a digest that nobody has to paste is a digest that nobody pastes wrong.
- Without a named owner, the answer to errand Task G is "no", and the tier is not scheduled.

NOTE: The new entries are T-060 (kernel), T-062 (base image), T-063 (kage in the image) and T-064 (variants). The others are T-066 (verification), T-068 (image CI with attestations) and T-069 (ownership and rebuild cadence).

NOTE: Each instance has its own overlay root, with the upper layer on its own data disk. No instance shares that overlay with another instance, so the rule above holds.

## 7. In-guest containers

NOTE: This section still holds, and T-185 is the measurement. The storage now lands on the data disk of the instance (30 GiB by default), not on a fixed-size workspace image.

The nested-container entry of the old plan says "measure it first and then decide", as the operator ruled on 2026-09-15.
errand Task E and Task F are the same rule.
This step gives a measurement, not an implementation, and a negative result closes it.

The structural change.
On a shared-kernel sandbox, these primitives block a nested container:

- `unshare` with `CLONE_NEWUSER`, `CLONE_NEWNS`, `CLONE_NEWPID` or `CLONE_NEWNET`;
- `mount` with `MS_BIND`;
- `CAP_SYS_ADMIN` in the target user namespace;
- a writable store location;
- cgroup v2 delegation.

In a microVM, each of these is a guest-kernel primitive.
The host seccomp allowlist traps `unshare`, `clone` and `pivot_root` in all three VMM threads.
The guest has its own kernel, which grants them to itself.

Consequences: no `--cap-add`, no capability allowlist, no relaxation in the shape of `--privileged`, no nesting mode, and no host capability given to anything.
The capability list of errand F2 is absent from this design by construction.

The storage trap does not go away. It moves.
Both Kata backends of section 2.1 need 150 to 250 GB of node boot disk.
A sparse `/tmp` cache over virtio-fs makes 80 GB of write pressure, and the host virtiofsd dies with `SIGBUS`.
Here the same pressure lands on one ext4 image inside one jail, bounded by the image size and removed with the jail.
It still fills without warning, and overlayfs in the guest on a fixed-size image looks like a broken build.
So it is measured.

## 8. Build sequence

NOTE: Superseded. [docs/roadmap.md](../roadmap.md) replaces this sequence. The document named its steps M0 to M12. Those ids now name the milestones of the new roadmap, so this page names the old steps S0 to S12, plus the gate S4.5. Each step names its successor entries.

The preconditions in the document, none of which this tier may come before:

- the workspace, types and binary entries;
- the backend trait, probe, policy and shared-backend entries;
- the broker entries;
- T-030 and T-031, the P0 shape decisions;
- the battery entry.

To build the tier before the trait and the broker exist is to build a second thing to redo.

S4.5 is a gate, not work.
Steps S5 and later cannot run on a host that is not a KVM host.
The host where the document was written is not one: no `/dev/kvm`, `memfd_create` returns `EPERM`, `CapEff` is 0 (section 2.3).
So S4.5 delivers the test host and names it: provisioning, owner, host model, kernel version, THP state, and whether the host is itself a VM.
Until S4.5 closes, S5 to S12 are blocked, and the record says so.
The record does not show them as pending work on a machine that cannot run them.
If the host is a VM, nested virtualization applies.
The gVisor platform guide calls it poor in performance and, in the past, a cause of security issues.
It is a gap until a real boot passes.

NOTE: In the new roadmap, the exit criterion of M3 needs a Linux KVM host. Development tests can use `cargo xtask linux --kvm` through wsl-toolkit on Windows, which is nested virtualization ([docs/development.md](../development.md)).

### S0: the decision record

- Delivers: a decision record and a new entry for this tier.
- Delivers, continued: an amendment to the kernel-tier entry that records trigger condition 1 as met, and route (d) of the nested-container entry.
- Acceptance: `cargo xtask check record` exits 0 once the record-check entry exists. Before that, count by hand and say so.
- Rollback: documents only.
- Now: [docs/decisions.md](../decisions.md) (D1, D12), and `cargo todo check` (T-003).

### S1: the protocol crate

- Delivers: box-vm-proto with framing, the port map, typed messages and the two ceilings.
- Acceptance: `cargo test -p box-vm-proto`. A frame over the 8 KiB head ceiling or the 8 MiB frame ceiling is refused before a buffer of that size is allocated.
- Acceptance, continued: an unknown type and an unknown port are refused. A truncated frame is an error, not a short read. A version mismatch refuses the session.
- Acceptance, continued: a mutation proves each guard.
- Rollback: a new crate that nothing uses; delete it.
- Now: T-021 and T-022 (box-proto).

### S2: the kernel and rootfs pipeline

- Delivers: the kernel and rootfs pipeline, the pins file, and the verification in `box_vm::image`.
- Acceptance: `cargo test -p box-vm image::` and the pipeline check, sh images/build.sh --check. A digest mismatch deletes the cache and refuses. A missing pin is an error, never a fetch of a branch.
- Rollback: artefacts, not code; revert the pins file.
- Now: T-060, T-062 and T-066.

### S2b: the guest init

- Delivers: box-vm-init, a static musl PID 1: mounts, the environment, the holder of the transcript nonce, the harness adapter.
- Acceptance: `cargo build -p box-vm-init --target x86_64-unknown-linux-musl`. A golden-frame conformance test proves that host and guest agree byte for byte on a corpus of frames.
- Acceptance, continued: the two sides are separate binaries and can drift otherwise. The shared protocol crate makes that test possible.
- Rollback: the guest side is unusable, and the tier cannot boot.
- Now: T-040 and T-041 (box-guest); the golden frames are in T-021.

### S3: the probe

- Delivers: `box_sandbox::vm::VmSandbox`, probe only, no launch.
- Acceptance: `cargo test -p box-sandbox vm::` and `cargo run -p box-cli -- doctor`. A missing delegated cgroup is one gap and nothing else. A host below the version floor is refused, and the message names both versions.
- Acceptance, continued: `to_host_path` returns `None` for `/`, `..`, an absolute host path, a path with a NUL, and an empty string.
- Acceptance, continued: the probe reports the THP availability and the host kernel version as notes, because `huge_pages: Transparent` and `io_engine: Async` depend on the host.
- Rollback: the backend stays unreachable until a tier selects it.
- Now: T-092 (the Linux probe) and T-175 (doctor). `to_host_path` has no successor (D4).

### S4: the jail and the helper

- Delivers: jail construction and the helper protocol; the threat model document of the helper; cgroup placement; jail inventory and pid verification.
- Acceptance: `cargo test -p box-sandbox vm::jail::`. The uid of the jailer is the configured one, not the uid of the daemon. The `CapEff` of the helper is empty while it is idle.
- Acceptance, continued: a session id that is not 32 hex characters is refused before any syscall. A cgroup path from a caller is refused.
- Acceptance, continued: the threat model document exists, and each refusal condition in it has a test.
- Acceptance, continued: a dead VMM is listed, and removed only after `/proc/<pid>/root` resolves to its jail. A VMM that a live session claims is never removed.
- Acceptance, continued: a missing `io.max` is a note, and a missing `memory.max` is a gap.
- Rollback: keep the backend and widen the gap list.
- Now: T-080, T-081, T-082, T-086 and T-090.

### S4.5: the KVM host gate

- See the paragraph at the start of this section.
- Now: the exit criterion of M3 needs a Linux KVM host ([docs/roadmap.md](../roadmap.md)).

### S5: boot

- Delivers: the workspace image, the read-only base, `PUT /actions InstanceStart`, and the handshake on port 5000.
- Acceptance: `cargo test -p box-sandbox vm::boot:: --ignored` on a KVM host. A handshake that misses its time bound stops and removes the session.
- Acceptance, continued: `stop` twice is safe. A dead VMM leaves an orphan that the inventory finds.
- Rollback: the tier is opt-in, and the shared tier does not change.
- Now: T-088.

### S6: transport

- Delivers: streams on 5001, bytes on 5002, the heartbeat on 5003, and the framing guards of S1 end to end.
- Acceptance: `cargo test -p box-sandbox vm::transport::`. The heartbeat fires on silence longer than 30 s with no inbound frame, not on a timer.
- Acceptance, continued: a guest that sends a frame every 100 ms, at a sustained 1 MiB/s, is disconnected. A flood that keeps the channel busy must not count as liveness.
- Acceptance, continued: a `get` outside `/workspace` is refused. A `put` cannot create a symlink that escapes.
- Acceptance, continued: the file-descriptor trace of the host shows zero opens of the workspace image outside the descriptors of the VMM.
- Rollback: go back to probe only.
- Now: T-042 (PTY and exec), T-022 (the heartbeat rule) and T-087 (the descriptor trace). `put` and `get` have no successor (D4).

### S7: egress

- Delivers: no virtio-net, the loopback proxy in the guest, and vsock 5004 to the broker.
- Acceptance: `cargo test -p box-sandbox vm::egress::`. The applied VM configuration has no `network-interfaces` key, and the jail has no TAP device. These host-side assertions carry the property.
- Acceptance, continued: a host listener on 0.0.0.0 is unreachable. A destination that the broker allows succeeds. The broker refuses a denied destination.
- Acceptance, continued: the guests of two concurrent sessions cannot act for each other. The broker refuses the egress token of one session for another session.
- Rollback: the broker path stays, and the bridge is one listener.
- Now: T-084 (no `network-interfaces`), T-121 to T-126 (the netstack) and T-186 (isolation between instances).

### S8: the trusted channel

- Delivers: the acceptance of T-031: the channel rejects a forged event.
- Acceptance: `cargo test -p box-sandbox vm::channel::`. A second guest process at the uid of the agent, not only the agent binary, opens `/dev/vsock`. It writes a well-formed transcript event without the nonce, and the channel refuses it.
- Acceptance, continued: a frame with a wrong nonce is refused, and a frame with the correct nonce is accepted. The refusal counter increments, and the report carries it.
- Rollback: the step stays blocked until it passes, and nobody can select the tier.
- Now: T-031.

### S9: accounting and the startup report

- Delivers: accounting and the startup report.
- Acceptance: `cargo test -p box-sandbox vm::accounting::` and `cargo run -p box-cli -- doctor`. To cross the disk budget stops the session. The report names every limit that applied and every absence.
- Rollback: reporting plus a stop; disable it without moving a boundary.
- Now: T-082 (limits) and T-175 (doctor and the startup report).

### S10: containers in the guest

- Delivers: the nested-container measurement for this tier. Does the guest run podman, docker or neither? Also the storage arrangement, the cost, and the disk-fill case.
- Acceptance: a page under docs/, per the old experiments method, with the host, the run count, the workloads and the date. `cargo xtask battery nested` runs.
- Acceptance, continued: a disk-fill run. The overlay of the guest on a fixed-size image fills, and the record says what the failure looks like from inside.
- Acceptance, continued: expect `ENOSPC`, and expect it to look like a broken build, not like an out-of-space error. A negative result is committed.
- Acceptance, continued: the page states the decision rule. A positive result licenses a container runtime in the image. That is a pins change under section 6, not a configuration flag.
- Rollback: a measurement cannot regress.
- Now: T-185.

### S11: the battery and the end-to-end number

- Delivers: the containment battery and the end-to-end number.
- Acceptance: all twelve checks of section 9, with the named mutant of the negative control on every invocation.
- Acceptance, continued: the `cache_type` comparison that section 5.4 defers. `Unsafe` against `Writeback` on the real workload, on the same host in the same session.
- Acceptance, continued: the comparison records the write amplification and the throughput difference, and it states a choice.
- Rollback: measurement only.
- Now: T-180 (the battery), T-181 (footprint and latency) and T-093 (block I/O). The `cache_type` question is closed: `Writeback` (section 5.4).

### S12: the default

- Delivers: a ruling: the tier becomes the default, or it stays opt-in.
- Acceptance: `cargo xtask check record` with corrected counts, and the rejected alternatives recorded.
- Rollback: a configuration default, which can be reversed.
- Now: D1 answers it. The microVM is the product.

## 9. Acceptance

NOTE: T-180 carries these checks and the negative control into the new plan ([TODO/assurance.md](../../TODO/assurance.md)). The second table below says how T-180 adapts each check.

No step closes on a claim.
Each check runs from outside the sandbox, against a live daemon, as a command with an expected exit code and recorded output.

| # | Check | Pass condition | A defect that would make it pass wrongly |
| --- | --- | --- | --- |
| 1 | Host reachability | A listener on the host is unreachable by TCP, by UDP and by name. | A typo in the address of the listener, so nothing was ever listening. |
| 2 | Broker | The guest reaches the broker, and the broker holds the only credential. | The guest reaches the provider directly. Check 1 would also catch that. |
| 3 | Egress | An outbound UDP packet to an external address does not arrive. | No route at all reads as success. Check 7 asserts the positive case for this reason. |
| 4 | Guest syscall surface | `unshare`, `clone` and `pivot_root` are trapped, as read from the shipped seccomp filter file, not from a running VMM. | The check parses a filter file for the wrong architecture. |
| 5 | Jail | `/proc/<vmm-pid>/root` resolves inside the jail, and the uid is the configured one. | The VMM does not run at all, so the lookups in `/proc` fail, but not for the correct reason. |
| 6 | Trusted channel | A second guest process at the uid of the agent forges an event onto port 5003 without the nonce, and the channel rejects it. With the nonce, the channel accepts it. | A test that forges from inside box-vm-init, which has the nonce, and so proves nothing. |
| 7 | Egress identity | Two concurrent sessions. Neither guest can send a request that the broker attributes to the other. The broker refuses the token of one session for the other. | A test with one session, which cannot tell tokens per session from one global token. |
| 8 | Orphans | Kill the daemon with three VMs running, then restart it. List exactly three jails, then remove all three. The jail directory, pid file, both images and cgroup are absent, and the uid is released. | Listing without removal, or removal of the cgroup while the directory survives. |
| 9 | Workspace teardown | After teardown, a content probe over the state directory finds no byte that the agent wrote. An absent path is not sufficient. | A check of path existence passes while the bytes live in a leftover image elsewhere. |
| 10 | Guest containers | `docker run --rm hello-world` inside the guest, with the result recorded either way. | Not applicable: this is a measurement. |
| 11 | Provenance | Every run records the host, the kernel, the Firecracker version, the guest kernel digest, the rootfs digest and the date. | Not applicable: this is provenance. |
| 12 | Negative control | The named mutant of section 9.1 is applied, the battery runs, and the battery fails with a non-zero exit. | The battery exits 2 because it could not run. The old containers page says that this is a different fact from a pass. |

How T-180 adapts each check:

| # | In the new plan |
| --- | --- |
| 1 | Holds. The netstack refuses the own addresses of the host and the other reserved ranges after resolution (T-123). Keep a positive control. |
| 2 | Holds. Credentials reach only broker endpoints, and the guest holds only nonces (T-143). |
| 3 | Changed. UDP is allowed per policy (D5). The check sends UDP to a destination that the policy does not allow, and the packet must not arrive. |
| 4 | Holds. |
| 5 | Holds, with the uid per instance (T-086). |
| 6 | Holds. The event channel is now port 5002, and box-guest holds the nonce (T-031). |
| 7 | Holds as egress identity across two instances. The egress token and the binding nonces are per instance (T-186). |
| 8 | Changed. Kill the daemon with three instances running, then restart it. All three are re-adopted, and a VMM with no record is removed (T-090). |
| 9 | Changed to `delete`. After `delete`, a content probe over the state directory finds no byte that the agent wrote (T-091). |
| 10 | Holds (T-185). |
| 11 | Holds. The digest of the base image takes the place of the rootfs digest. |
| 12 | Holds: the same mutant on every run (T-180). On HCS the planned mutant is a `NetworkAdapters` entry in the compute system document. It is not verified. |

### 9.1 The negative control, named

A defect class is not an instruction.
This is the mutant, so a session that did not attend this one can run it:

1. In the VM configuration builder, add the key `network-interfaces` with a loopback TAP.
2. Run the battery.
3. Expect check 1 and check 7 to fail, because the guest now has an interface that the host can reach. Expect a non-zero exit.
4. Revert the change.

It runs on every battery invocation, not once.
A control that runs once and passes looks the same as a control that nobody wired to the battery.
It is the cheapest mutant that exercises the whole egress path, so it was chosen over a jailer mutant or a seccomp mutant.

READ, from the old containers page: a check that needs a tool exits 2 when the tool is absent.
"Could not run" and "failed" are different facts.
A battery that has never failed has not been tested.

NOTE: In the new plan, the configuration builder itself refuses `network-interfaces` (T-084). So the mutant edits the builder, and that refusal is part of what it removes.

NOTE: For the HCS engine, T-180 plans the same mutant in another form: one `NetworkAdapters` entry in the compute system document. Nobody has run it, so it is not verified. T-180 also adds a second mutant: a netstack that maps the gateway onto the host loopback.

## 10. Startup report

NOTE: This section still holds, with new items (T-175, [docs/cli.md](../cli.md)). The egress topology is now stated as these absences: no shared filesystem, and no host path in the guest. The guest NIC reaches only the netstack.

The daemon emits the report before it touches the chat service, per errand section 9.8.
A change that is not in the report is not finished.

- The engine name and version, and whether the version is inside the supported window.
- The jail: present or absent, the uid, the gid, the cgroup path, and the limits that applied.
- The privileged helper: available or not, and its refusal conditions.
- The guest kernel digest, the rootfs digest, the image `cache_type` and the `io_engine`.
- The egress topology, stated as absences: no guest network device, no shared filesystem, `to_host_path` always `None`.
- Where the credential is: in the broker, outside the guest, for the life of the session.
- The count of refused forgeries on the transcript channel, per session.
- Every gap, by name, in the words of the limits page.

## 11. Out of scope

| Not here | Reason, in the document | Now |
| --- | --- | --- |
| An unconfined or degraded mode | No fallback between backends. If the jail cannot be built, the tier refuses. | Holds: no fallback between engines (T-032). |
| Snapshots, warm pools, diff snapshots | Firecracker calls a second resume of the same state insecure, and diff snapshots are developer preview. | Changed: `hibernate` is a later opt-in verb, with snapshots used once only (T-184). Warm pools and diff snapshots stay out. |
| A container runtime | One agent per VM; `to_host_path` returns `None`. | Holds on the host. Inside the guest, T-185 measures it. |
| A Windows host | Firecracker is Linux-only, on x86-64 and aarch64. Windows keeps the podman machine path. | Superseded: Windows uses Hyper-V through HCS (D11). |
| Raw network in the guest | No interface in v1; a capability with its own gate. | Superseded: TCP and UDP per flow through the netstack (D5). There is still no TAP. |
| A credential store | It is the job of the broker, in the daemon. | Holds (T-140). |
| An extension point | Two backends justify a trait; a third justifies a field. | Holds: two engines behind one trait (T-032). |

## 12. Limits

NOTE: [docs/limits.md](../limits.md) holds the limits of the new plan. Each item below says whether it still applies.

- Kernel maintenance moves to this project. A microVM moves the kernel boundary, and it also moves the patch cadence of the kernel. A shared-kernel sandbox inherits the cadence of the distribution; this tier does not. Still applies (T-069).
- A privileged helper exists (section 1.3). It is the only one, it uses a fixed syscall set, and it never parses guest data. But it is a root process, and it needs its own threat model. Still applies, on both OSes (T-080).
- Cold boot is the only primitive (section 5.7). Changed: `pause` is an in-memory freeze, and `hibernate` comes later (T-184).
- KVM is necessary, and nested virtualization is a real constraint. The gVisor platform guide calls nested virtualization poor and, in the past, a security problem. The probe attempts a real KVM ioctl, and a deeper probe attempts a real boot. A host that is itself a VM reports a gap until the deep attempt passes. Still applies on Linux (T-092).
- Disk I/O may be the dominant cost, because the workspace is a disk image and not a bind mount. Section 3.5 is the way out. Still applies, and more for persistent instances (T-015, T-093).
- A guest kernel defect loses that session to its own guest. That is the point: the defect no longer reaches the host or another tenant. Still applies.
- This project measured nothing in this document, except the seven items of section 2.3. Every other figure is the figure of an upstream project, cited where it is used.
- A nonce protects the transcript channel, not vsock. A guest process that runs as the uid of box-vm-init would have the nonce. Section 5.3 states the narrower claim of this design. Still applies (T-031).
- The broker is a shared endpoint, and a token that the host issued tells the sessions apart (section 5.2). If the token path were wrong, the failure would be cross-tenant attribution. So check 7 runs two concurrent sessions. Still applies in a new form (T-186).
- A daemon restart kills every session in flight (section 5.5). The document intends this, and it is a real cost for long work. Superseded: the daemon re-adopts running instances (T-090, T-107).
- Every protocol that the broker does not terminate is unavailable (section 5.2). Superseded by D5.
- New since the document: isolation is not the same on the two host OSes. Under HCS, the guest reaches kernel code of the root partition: the VMBus root side, storvsp, vhdmp and the hvsocket provider. Under Firecracker, the guest reaches KVM in the host kernel, and virtio-blk and vsock run in the jailed user-mode VMM ([docs/engines.md](../engines.md), T-182).

## Corrections made in this conversion

Corrections from the engine evaluation of 2026-10-14 and its adversarial review:

1. Data disk settings (section 5.4). `Unsafe` does not keep the image out of the host page cache, and `Async` is developer preview without discard. The settings are now `Sync`, `Writeback` and discard.
2. Jail lifecycle (sections 5.1 and 5.5). The data disk is hard-linked into a jail that the helper rebuilds on each boot. Teardown never deletes it. A daemon restart re-adopts running instances.
3. The Cloud Hypervisor threat model (section 3.3). It is not in the v53.0 tag; it exists on main.
4. The Firecracker jailer cannot wrap Cloud Hypervisor as it is (section 3.3), because it appends Firecracker-specific arguments.
5. aarch64 (section 3.2, R10). Firecracker tests aarch64 only on Graviton 2 to 5 and lists only 4K-page host kernels.
6. A performance risk (section 3.2). One VMM thread emulates block and vsock, so disk load can block every host channel (T-015).
7. The kernel format (section 3.2). Firecracker v1.17.0 also boots a bzImage on x86_64, and it still recommends vmlinux.
8. Other jailed VMMs (section 3.4). crosvm and QEMU under libvirt also confine their VMMs. Firecracker wins on device model size and on socket permissions for vsock.
9. The io_uring syscalls in the vmm thread allowlist (section 3.2), for every `io_engine`.

The other corrections of that review concern the Windows engine, which this document does not cover.
[docs/research/engines-2026-10.md](engines-2026-10.md) and [docs/engines.md](../engines.md) record them.

Corrections that this conversion found:

1. Seccomp counts (section 3.2). 82, 37 and 50 are filter rules. They name 51, 32 and 27 distinct syscalls.
2. The duplicate `--id` in the jailer command (section 5.5). The jailer already passes it.
3. Capabilities (section 1.3). `chroot` needs `CAP_SYS_CHROOT`, and the drop to the instance uid needs `CAP_SETUID` and `CAP_SETGID`.
4. Check numbers (sections 1.4, 5.1, 5.5 and 8). The document points to check 10 for the negative control, check 11 for teardown and orphans, and "all eleven checks". The table of section 9 has twelve checks: the negative control is check 12, teardown is check 9, and orphans are check 8. This page uses the numbers of the table.
