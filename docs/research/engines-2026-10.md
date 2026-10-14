# Engine evaluation of 2026-10

This page is the research record of the engine evaluation that the planning session ran on 2026-10-14.
It gives the question, the scenario, the method, and each candidate with its verdict.
It also gives the corrections of the adversarial review, the rulings, the re-evaluation triggers and the open measurements.

The rulings that came out of the evaluation are D11, D12 and D13 in [docs/decisions.md](../decisions.md#product).
[docs/engines.md](../engines.md) holds the engine design that follows from them.
[docs/architecture.md](../architecture.md) wins every conflict with this page.

NOTE: Some parts of the evaluation assumed an egress model that later rulings replaced. The synthesis and the adversarial review kept an allowlisting CONNECT proxy for kage. An older design planted a CA in the guest for TLS interception. D5 and P4 supersede both. This page reports them as history, not as design. The section [Superseded by later rulings](#superseded-by-later-rulings) lists each item.

## Labels

- READ: a reader opened the source at the pin, or the page at the URL, and found the statement there.
- MEASURED: somebody ran it. The statement gives the host, the date and the conditions.
- INFERRED: a reader concluded it from READ facts or from general knowledge. Nobody checked it.
- No label: a verdict of the evaluation, or design.

The readers labelled their probes on the operator's host READ. This page labels them MEASURED and adds their conditions.

A citation of another project has the form `owner/repo@commit:path:line`, with the full commit of the pins table in [docs/references.md](../references.md#pins).
The web-only sources have no pin: microsoft/WSL, microsoft/hcsshim and the learn.microsoft.com pages. They are cited by URL, and each was read on 2026-10-14.

## The question

The goals of the operator, in order: isolation and security, then performance, then footprint.
The evaluation answered two questions.

1. Which engine runs a persistent Linux instance on a Windows 11 x64 host? Which engine runs it on a Linux x64 or aarch64 host?
2. Does one engine run natively on both host OSes with no loss of isolation? The operator asked this after the first answer.

## The scenario

The readers checked each candidate against one scenario.
The R-numbers are the requirements of [the microVM research](microvm-tier.md#31-requirements-each-traced-to-a-source).

| Requirement | What it means here |
| --- | --- |
| A second kernel (R2) | The guest runs its own Linux kernel behind hardware virtualization. |
| A persistent instance | Create, start, pause, resume, stop and delete. The instance outlives the CLI process, and its disk survives a host reboot. |
| A data disk of about 30 GiB | Sparse on the host. Space comes back to the host when the guest trims. |
| A full userland | Debian with kage, git and language toolchains. A container runtime only if a measurement allows it. |
| No shared filesystem (R7) | Project files move through a git forge only (D4). |
| A host channel that is not the guest IP network (R3) | vsock on Linux, hvsocket on Windows. |
| Credentials on the host | The guest holds nonces only. The broker in the host daemon injects each credential. |
| Privilege | A one-time install of a privileged helper is acceptable (D3). The CLI runs unprivileged. |
| A small device model (R4) | Each emulated device is attack surface on the host. |
| Host architectures (R10) | Windows 11 x64. Linux x64 and aarch64. |
| Integration | One Rust CLI drives the engine. |

## Method

The evaluation had four stages on 2026-10-14. Each stage kept the READ and INFERRED labels of the stage before it.

### Stage 1: parallel readers

Seven readers worked at the same time, each on one subject.
Each reader returned facts with a label and evidence, the fit to each requirement, a verdict and open questions.

| Reader | Subject | Facts |
| --- | --- | --- |
| 1 | The MXC backends microvm (Nanvix) and hyperlight (Unikraft) | 37 |
| 2 | LiteBox | 39 |
| 3 | MXC: the engine, the contracts and every backend | 45 |
| 4 | The MXC backends of the VM class on Windows, and the platform APIs under them: WSLC, IsolationSession, Windows Sandbox | 35 |
| 5 | kage v0.3.0 as the agent inside the guest | 48 |
| 6 | Linux engines for persistent instances: Firecracker v1.17.0, Cloud Hypervisor v53.0, OpenVMM | 57 |
| 7 | Windows engines: HCS, OpenVMM on WHP, a custom VMM on WHP | 42 |

The readers read clones at the pins and pages on the web.
They built and ran nothing from the clones.
Reader 7 ran small probe programs on the operator's host ([Measured on the operator's host](#measured-on-the-operators-host)).

### Stage 2: synthesis

One synthesis merged the seven reports.
It ranked twelve Windows options (W1 to W12) and seven Linux options (L1 to L7).
It gave a verdict on MXC, on LiteBox and on kage, defined the pause verbs, and listed 18 open items.
Where two readers disagreed, the synthesis said so and chose.

### Stage 3: adversarial review

A reviewer tried to break the synthesis and checked its claims against the sources again.
The two primary choices held: HCS on Windows, and Firecracker v1.17.0 with the jailer on Linux.
The review raised 14 objections, added 7 missed options and wrote a corrected recommendation ([Adversarial corrections](#adversarial-corrections)).

### Stage 4: cross-platform sweep

The operator then asked for one engine that is native on both OSes.
Four workers swept the candidates, and a ranking compared each one with the two-engine baseline.

| Worker | Subject | Facts |
| --- | --- | --- |
| 1 | crosvm: WHPX on Windows, KVM on Linux | 40 |
| 2 | Every other VMM, and a custom VMM | 39 |
| 3 | QEMU 11.1.2: WHPX on Windows, KVM on Linux | 37 |
| 4 | OpenVMM v0.2.0 inside confinement that agent-box builds | 27 |

NOTE: Some QEMU issue pages that the sweep fetched held text that addressed automated readers. The sweep treated that text as data and did not follow it.

### The trees

| Reference | How the readers read it |
| --- | --- |
| `firecracker-microvm/firecracker` | A sparse clone of the v1.17.0 tag, plus root files from the same tag |
| `cloud-hypervisor/cloud-hypervisor` | A sparse clone of the v53.0 tag (125 files: block, docs and the API), plus root files |
| `microsoft/openvmm` | A full clone |
| `microsoft/mxc` | A full clone |
| `microsoft/litebox` | A full clone, 6 days after the tag v0.1.0 |
| `QaidVoid/kage` | A full clone after v0.3.0 |
| `google/crosvm` | A clone |
| `qemu/qemu` | A sparse clone of the tag v11.1.2 |
| `containers/libkrun` | A clone of the main branch |
| `smol-machines/libkrun` | A clone of the fork that smolvm ships |
| `smol-machines/smolvm` | A clone |
| `hyperlight-dev/hyperlight` | A clone |

The pins table of [docs/references.md](../references.md#pins) gives the commit of each tree.

## Measured on the operator's host

MEASURED on 2026-10-14. The host is Windows 11 Pro build 26200 with an Intel Core i7-12700H, 20 logical CPUs and 64 GB of memory.
The probes ran without elevation. The user belongs to Administrators, but the filtered token marks that group deny-only.
Each probe ran once.
[docs/status.md](../status.md) holds the measured state of the project. This table is the view of the evaluation.

| Probe | Result |
| --- | --- |
| `Win32_ComputerSystem.HypervisorPresent` | True. |
| `hcsdiag list` | Refused with exit code 27. Only members of Administrators or Hyper-V Administrators may access virtual machines. |
| A WHP partition, non-elevated: WHvGetCapability, WHvCreatePartition, a processor count of 1, WHvSetupPartition, WHvDeletePartition | Each call returned S_OK. |
| The same probe at Low integrity level, in a token with the label S-1-16-4096 | Each call returned S_OK. |
| The same probe in an AppContainer with no capabilities | WHvGetCapability returned E_ACCESSDENIED (0x80070005). |
| A dynamic VHDX of 30 GiB through virtdisk, non-elevated | Created. The file took 4 MiB (4194304 bytes). Open and attach with NO_LOCAL_HOST worked. |
| `/dev/kvm` inside a wsl-toolkit 6.0.0 job | Present. The WSL2 kernel is 7.2.0-WSL2-STABLE, and kvm_intel has nested=Y. |

What the probes show:

- HCS needs a caller in Administrators or Hyper-V Administrators. So the Windows engine needs a privileged helper (D3, D11).
- WHP needs no administrator at run time, but it refuses an AppContainer. A WHP engine can at best run at Low integrity level or under its own account.
- Nobody tested a true standard user. The probes ran with a filtered administrator token.
- `/dev/kvm` in WSL2 is nested virtualization. It serves development tests only (T-010).

The jailer of Firecracker fails without root capabilities.
That fact is MEASURED by the operator's research document on its own research host, not on this host ([microvm-tier.md](microvm-tier.md#13-the-privileged-helper-measured)).

## Candidates and verdicts

| Candidate | Host OS | Verdict | Main reason |
| --- | --- | --- | --- |
| HCS through a helper service | Windows | Selected (D11) | Microsoft runs the VMM in a hardened worker per VM. Direct kernel boot, no NIC device, no shared filesystem, hvsocket with an SDDL per VM. |
| Hyper-V VMMS through WMI v2 | Windows | First fallback (T-111) | The same Microsoft worker, and registered VMs that persist and save. Only Pro and above, UEFI boot, a registry entry per hvsocket service. |
| OpenVMM on WHP | Windows | Second fallback (T-111) | Runs on Home with no administrator. No VMM sandbox, no memory reclaim, no TRIM, no save call. |
| WSL Containers (WSLC), called directly | Windows | Rejected; watch it | A kernel that Microsoft manages, a forced Consomme NAT, no pause, no hvsocket for our code. |
| Firecracker inside WSL2 | Windows | Rejected | Nested virtualization. All WSL distributions share one utility VM and its kernel. |
| Windows Sandbox | Windows | Rejected | A Windows guest, one per logon session, no state after stop. |
| IsolationSession | Windows | Rejected | Not a VM: a separate Windows user on the same kernel. The network cannot be restricted. |
| MXC backends | Windows, Linux | Rejected as engines (D13) | No persistent Linux guest with its own kernel. MXC is a source of design patterns only. |
| LiteBox | Windows, Linux | Rejected (D13) | A library OS inside the caller's process. No fork, and not a security boundary. |
| Firecracker v1.17.0 with the jailer | Linux | Selected (D12) | The smallest device model, a jail around the VMM, and vsock sockets under file permissions. |
| Cloud Hypervisor v53.0 | Linux | Fallback if block I/O dominates (T-093) | A better block stack. It has no jailer, so agent-box would build the jail. |
| QEMU under libvirt | Linux | Fallback on 16K-page aarch64 hosts (T-094) | libvirt confines it, but its device model is large. |
| Kata Containers | Linux | Rejected | A container runtime with containerd, a shim and virtiofsd. Linux hosts only. |
| crosvm | Windows, Linux | Rejected as the single engine | Upstream WHPX is not tested, the Windows sandbox is unusable, and the main process on Linux is unconfined. |
| OpenVMM as the single engine | Windows, Linux | Rejected | No confinement on either OS. agent-box would build both sandboxes and service the binary. |
| libkrun and the smolvm fork | Windows, Linux | Rejected; watch it | WHP is not released upstream, and the VMM runs as the user. |
| QEMU with WHPX and KVM | Windows, Linux | Rejected | On Windows: no vsock, no save, no reclaim, no TRIM and no sandbox. |
| A custom VMM | Windows, Linux | Rejected | The most expensive route. On Windows it still cannot match the sandbox of `vmwp.exe`. |
| VirtualBox 7.2 | Windows, Linux | Rejected | No virtio-vsock, no direct kernel boot, and the KVM backend is not released. |
| Hyperlight | Windows, Linux | Rejected | No guest kernel. A library for one vCPU. |

gVisor was not evaluated in this session. D1 makes the microVM the product.

## Windows candidates

### HCS through a helper service (selected)

The Host Compute Service (computecore.dll, service vmcompute) is the inbox API that creates Hyper-V utility VMs from a JSON document.
WSL2, hcsshim and Docker use it.
READ: https://learn.microsoft.com/en-us/virtualization/api/hcs/overview.

- READ: WSL2 creates its VM through HCS with schema 2.3 and LinuxKernelDirect, on x64 only. It puts its disks on SCSI and sets AllowOvercommit, EnableDeferredCommit and EnableColdDiscardHint. Its HvSocket SDDL grants SYSTEM and the user. Source: https://raw.githubusercontent.com/microsoft/WSL/master/src/windows/service/exe/WslCoreVm.cpp.
- READ: hcsshim boots its Linux utility VMs with LinuxKernelDirect. It adds `initcall_blacklist=virtio_vsock_init` to the command line to force hv_sock. Source: https://raw.githubusercontent.com/microsoft/hcsshim/main/internal/uvm/create_lcow.go.
- READ: NetworkAdapters and HvSocket are optional devices in the schema, so a compute system can have no NIC device. LinuxKernelDirect is new in schema 2.2. Source: https://learn.microsoft.com/en-us/virtualization/api/hcs/schemareference.
- READ: a compute system is ephemeral. Pause, resume, save and restore exist for VMs (the overview above).
- READ: HcsSaveComputeSystem with ToFile fails on a running system, so a save needs a pause first. Source: https://learn.microsoft.com/en-us/virtualization/api/hcs/reference/hcssavecomputesystem.
- READ: HcsGrantVmAccess adds an access entry for the worker account of one VM. Source: https://learn.microsoft.com/en-us/virtualization/api/hcs/reference/hcsgrantvmaccess.
- READ: the worker `vmwp.exe` runs with ACG and CIG on and without SeImpersonatePrivilege. Source: https://www.microsoft.com/en-us/msrc/blog/2019/09/attacking-the-vm-worker-process.
- READ: the worker runs under a virtual account per VM in the documentation of Windows Server 2012. Source: https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-server-2012-R2-and-2012/dn741279(v=ws.11). The same on Windows 11 is INFERRED.
- READ: Microsoft services the Hyper-V virtual machine boundary as a security boundary. The criteria have no row for WHP or for a third-party VMM. Source: https://www.microsoft.com/en-us/msrc/windows-security-servicing-criteria.
- READ: the HCS reference pages date from 2021-06-09. They name Windows Server 2019 and 2022 and Windows 10 1809, and say nothing about client editions.
- READ: the `windows` crate exposes every HCS function in `Win32::System::HostComputeSystem`, with no typed schema. Source: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/HostComputeSystem/index.html.
- INFERRED: the schema types, the AF_HYPERV code and the virtdisk calls take about 1 to 2 thousand lines of Rust.
- READ: WSL2 reaches a bash shell in under 2 s. No figure for the memory of one VM is published. Source: https://devblogs.microsoft.com/commandline/announcing-wsl-2.
- READ: a third-party desktop product that drives HCS from a LocalSystem service reports 0x80070005 at Construct. Reporters fixed it with ACLs for SYSTEM and `NT VIRTUAL MACHINE\Virtual Machines`. No maintainer confirmed the fix. Sources: https://github.com/anthropics/claude-code/issues/39285 and https://github.com/anthropics/claude-code/issues/38188.
- MEASURED: HCS refused a caller that was not elevated ([Measured on the operator's host](#measured-on-the-operators-host)).

Verdict: selected (D11).
On Windows, only the Hyper-V stack (HCS, or VMMS through WMI) runs the VMM in a worker per VM that Microsoft hardens and services.
With any WHP engine, agent-box launches the VMM process and must build its confinement.
Of the two Hyper-V paths, HCS boots the kernel directly and writes no registry entry.
INFERRED: like WSL2, HCS needs only the Virtual Machine Platform feature, not the Hyper-V role.
The review corrected three claims of the synthesis about HCS: corrections 1, 3 and 11 in [Adversarial corrections](#adversarial-corrections).

### Hyper-V VMMS through WMI v2 (first Windows fallback)

The synthesis missed this option, and the review added it.

- READ: the Hyper-V provider of Podman machine uses `Msvm_VirtualSystemManagementService` through libhvee. Source: https://docs.podman.io/en/latest/markdown/podman-system-hyperv-prep.1.html.
- The review names minikube, Vagrant and the older Hyper-V driver of Multipass as other users of this path. This page did not check them.
- READ (same source): each hvsocket service GUID needs a registry entry under HKLM, and the user needs Hyper-V Administrators or a helper.
- INFERRED: VMs are registered objects that persist. Save-VM and Suspend-VM work with Linux guests, and AutomaticStopAction=Save keeps the state over a host reboot.
- INFERRED: Gen2 VMs boot through UEFI, with no LinuxKernelDirect, so the boot is slower.
- The worker is the same sandboxed `vmwp.exe` as under HCS.
- INFERRED: the Hyper-V role exists on Pro, Enterprise and Education only, not on Home.

Verdict: the first fallback if HCS fails a gate (T-014, T-109). It also gives `hibernate` on Windows if the HCS save fails. T-111 decides.

### OpenVMM on WHP (second Windows fallback)

OpenVMM is the open-source Rust VMM of Microsoft. One `openvmm.exe` process runs one VM.

- READ: its development is focused on the OpenHCL paravisor (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:README.md:8-9`).
- READ: a release publishes source only. Prebuilt binaries and servicing are outside the release process (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:Guide/src/dev_guide/contrib/openvmm_source_release.md:21-22`). v0.1.0 came out on 2026-08-14 and v0.2.0 on 2026-10-02.
- READ: the management API is not compatible between releases (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:Guide/src/reference/openvmm/management/grpc.md:24-29`).
- READ: its vsock relays use the hybrid vsock protocol of Firecracker over AF_UNIX, on Windows too (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:support/hybrid_vsock/src/lib.rs:16-19`).
- READ: the `SandboxProfile` trait has no implementation, so `openvmm.exe` runs with no sandbox (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:support/mesh/mesh_process/src/lib.rs:283`).
- READ: on Windows the raw-file disk ignores unmap (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:vm/devices/storage/disk_file/src/lib.rs:197-208`). The VHDX path ignores it too (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:vm/devices/storage/disk_vhdmp/src/lib.rs:824-826`), so TRIM never shrinks the host file.
- INFERRED from a search of the tree: there is no balloon and no free-page reporting. READ: a maintainer wrote that memory reclaim has no concrete plans (https://github.com/microsoft/openvmm/issues/2118).
- READ: suspend and resume are not wired to a management interface (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:Guide/src/reference/openvmm/management.md:41-46`). A snapshot exists only in the interactive console and needs memory backed by a file.
- READ: WHP doorbells give virtio notifications a fast path (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:vmm_core/virt_whp/src/lib.rs:650`).
- MEASURED: WHP works non-elevated and at Low integrity level, and refuses an AppContainer.

Verdict: the second fallback, for Windows Home, if HCS fails there (T-109, T-111).
agent-box would need its own launcher: a dedicated account, a job object and mitigation policies.
INFERRED: a Low-IL token alone still lets an escaped VMM read the files of the user, because integrity levels block writes up and not reads.

### WSL Containers (WSLC), called directly

The readers disagreed. Reader 4 named direct use of WSLC the strongest Windows candidate. Reader 7 called it unusable for this design. The synthesis rejected it.

- READ: WSL Containers reached general availability in WSL 3.0.1 on 2026-09-29. WSL 3.0.3 is current. Source: https://github.com/microsoft/WSL/releases.
- READ: the public C SDK always creates the session VM with Consomme networking, and with no session flags. Source: https://github.com/microsoft/WSL/blob/master/src/windows/WslcSDK/wslcsdk.cpp, lines 447 and 453 on the date read.
- READ: the SDK has no pause, suspend or save function. Source: https://github.com/microsoft/WSL/blob/master/src/windows/WslcSDK/wslcsdk.h.
- READ: the WSL service builds a VM factory per session that recreates VMs after idle termination. Source: https://github.com/microsoft/WSL/blob/master/src/windows/service/exe/WSLCSessionManager.cpp, lines 290-296 on the date read. Microsoft Learn does not say that each session is one VM.
- READ: WSLC creates its VMs through HCS, and the guest kernel is the WSL2 kernel. Source: https://devblogs.microsoft.com/commandline/wslc-architecture-deep-dive/.
- READ: only dynamic VHDs are implemented, and their size can be set. Source: https://wsl.dev/api-reference/c/not-yet-implemented-apis/.
- INFERRED: the guest workload gets no documented hvsocket or vsock endpoint. The container mode NONE removes the NIC of the container only; the VM keeps Consomme.
- READ: in the WSL sources, only the COM interface of WSLCCompat.idl offers a session that outlives its creator, and a TTY.
- INFERRED: that COM interface is not documented for third parties.

Verdict: rejected; watch it.
The kernel is Microsoft's, the VM always has a NIC, there is no pause, and our code gets no hvsocket.

### Firecracker inside WSL2

No reader evaluated this path in depth. The synthesis recorded it as W11.

- READ: WSL builds its utility VM with NAT or Consomme networking and with GrantVmAccess on user files (WslCoreVm.cpp, above).
- READ: in WSL2, OpenVMM gets KVM as its hypervisor (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:Guide/src/dev_guide/getting_started/running_from_wsl.md:44-46`).
- INFERRED: an escape from the jailed Firecracker lands in the shared WSL VM, not in a worker per VM.
- INFERRED: all WSL distributions share one utility VM and one kernel, so every instance would share that layer.
- INFERRED: a broker on the Windows host would need a bridge to the Unix sockets inside WSL2. Nobody designed it.
- MEASURED: `/dev/kvm` works inside a wsl-toolkit job, through nested virtualization.

Verdict: rejected as an engine. It serves development tests only (T-010).

### Windows Sandbox

- READ: the guest is a Windows VM, and the host runs one Windows Sandbox VM per logon session (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:docs/backends/windows-sandbox/windows-sandbox.md:91-92`).
- READ: the sandbox is disposable, and it needs Pro, Enterprise or Education. Source: https://learn.microsoft.com/en-us/windows/security/application-security/application-isolation/windows-sandbox/windows-sandbox-overview.
- READ: the MXC backend talks to the guest over four TCP connections, each with a nonce per launch and a role byte (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:docs/backends/windows-sandbox/windows-sandbox-reference.md:10-31`).
- READ: the network lockdown is a firewall rule set inside the guest (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:src/mxc-sdk/src/bin/windows_sandbox_guest/firewall.rs:40-46`). INFERRED: an administrator in the guest could remove it.

Verdict: rejected. It runs no Linux userland and keeps no state.

### IsolationSession

- READ: MXC drives the private WinRT API `Windows.AI.IsolationSession.Preview`. It provisions a fresh Windows user in a separate session on the same kernel.
- READ: MXC itself states that a Win32k kernel exploit escapes such a session (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:docs/development/architecture/backends/isolation-session/state-aware-rust.md:262-265`).
- READ: the backend cannot restrict the network (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:schemas/stable/mxc-config.schema.1.0.0.json:441`).
- READ: the bindings come from a private WinMD (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:src/mxc-sdk/build/isolation_session_bindings/README.md`).
- READ: the API cannot start from Session 0, so a Windows service cannot drive it (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:docs/development/architecture/backends/isolation-session/state-aware-rust.md:52-55`).

Verdict: rejected. There is no hypervisor boundary, and the guest is Windows.

### MXC (microsoft/mxc)

MXC is the Rust SDK of Microsoft for untrusted code, version 1.0.0, MIT licence.
READ: its backends are listed at `microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:README.md:52-58`.

| Backend | Boundary | Own Linux kernel | Lifecycle |
| --- | --- | --- | --- |
| processcontainer | AppContainer or BaseContainer on the NT kernel | No | One run only |
| windows_sandbox | A Windows VM; experimental | No | Yes, one VM per logon session |
| wslc | One shared WSL session VM; containers inside it | No: one kernel for all sandboxes | Yes, through a per-user daemon |
| isolation_session | A separate Windows user on the NT kernel | No | Yes |
| microvm (Nanvix) | WHP or KVM; a research microkernel, not Linux | No | One run only |
| hyperlight (Unikraft) | WHP or KVM, inside the caller's process; a unikernel | No | One run only |
| bubblewrap, lxc | Namespaces on the Linux host kernel | No | One run only |
| seatbelt | The macOS sandbox | No | One run only |

- READ: all MXC wslc sandboxes share one session VM, and only container namespaces separate them (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:docs/backends/wslc/wslc-state-aware.md:100-102`).
- READ: the wslc exec of MXC forwards no stdin, so `kage rpc` and the kage TUI cannot run through it (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:src/mxc-sdk/src/bin/wslc_daemon/control_server.rs:588-595`).
- READ: the bindings carry the VHD size settings of the WSLC SDK, and MXC never calls them (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:src/mxc-sdk/src/backends/wslc/common/wslcsdk_sys.rs:194-201`).
- READ: the lifecycle has five phases and no pause. Snapshot, suspend and restore are deferred (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:docs/development/architecture/container-lifecycle.md:165-167`).
- READ: no container promises to survive a host reboot (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:docs/glossary.md:15`).
- READ: a search of the mxc-sdk sources finds no vsock, hvsocket or virtio transport.
- READ: microvm and hyperlight have no streaming path and no lifecycle (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:src/mxc-sdk/src/core/mxc_engine/dispatch.rs:12-22`). The CI marks microvm "Not working" and Windows Sandbox "Blocked" (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:docs/development/build-and-test/ci-validation-infrastructure.md:235-236`).
- READ: the hyperlight backend boots the VM inside the executor's own process (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:docs/backends/hyperlight/hyperlight-backend.md:138-142`). Its Unikraft guest has one vCPU, no fork and a root filesystem in RAM.
- READ: in MXC, Nanvix runs Python source only, in 256 MB of guest RAM (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:docs/backends/nanvix/nanvix.md:155`). Upstream calls Nanvix a prototype (https://raw.githubusercontent.com/nanvix/nanvix/dev/README.md).
- READ: the crate always pulls in tokio, nix, flatbuffers and, on Windows, the `windows` crate 0.62 (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:src/mxc-sdk/Cargo.toml:51-93`). The build script downloads files.
- MXC as a confinement layer around our own VMM adds nothing. HCS already sandboxes its worker, and WHP refuses an AppContainer (MEASURED).

The design patterns that agent-box takes from MXC:

- a lifecycle state machine with opaque, prefixed ids (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:docs/container-lifecycle.md:57-65`);
- a daemon that proves ownership by process id plus creation time (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:docs/backends/windows-sandbox/windows-sandbox.md:93-94`), with record directories that only the owner can read (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:docs/backends/windows-sandbox/windows-sandbox-reference.md:121`);
- host-guest channels that start with a nonce and a role (`microsoft/mxc@2366ee9b1e0bed3a0f950ec49a509f92285239cf:docs/backends/windows-sandbox/windows-sandbox-reference.md:24-31`);
- the refusal of a policy that the backend cannot enforce;
- request contracts in JSON with exact versions.

Verdict: not an engine on either OS (D13). A source of design patterns only.
Watch: Microsoft states that micro-VMs on the hypervisor and Linux containers through WSL are on the MXC roadmap (https://blogs.windows.com/windowsdeveloper/2026/06/02/windows-platform-security-for-ai-agents/).

### LiteBox (microsoft/litebox)

LiteBox is a library OS in Rust. On its userland platforms, the guest program runs inside one ordinary host process.

- READ: the guest shares the address space of the host process, on Linux and on Windows (`microsoft/litebox@c8eafc7a07725331113c5ec5d1ac56a95ac873b8:litebox_platform_linux_userland/src/lib.rs:6-7`, `microsoft/litebox@c8eafc7a07725331113c5ec5d1ac56a95ac873b8:litebox_platform_windows_userland/src/lib.rs:6-7`).
- READ: the syscall rewriter is not a security boundary, and it does not handle syscalls that a JIT generates (`microsoft/litebox@c8eafc7a07725331113c5ec5d1ac56a95ac873b8:litebox_syscall_rewriter/src/lib.rs:9-11`).
- READ: on Linux, a process-wide seccomp list still allows read-only `open` (`microsoft/litebox@c8eafc7a07725331113c5ec5d1ac56a95ac873b8:litebox_platform_linux_userland/src/lib.rs:369-383`). INFERRED: a missed syscall can read each host file that the user can read.
- READ: clone creates threads only, and fork is not supported (`microsoft/litebox@c8eafc7a07725331113c5ec5d1ac56a95ac873b8:litebox_shim_linux/src/syscalls/process.rs:600-641`). INFERRED: bash, git, cargo and the kage tools cannot start a child process.
- READ: the filesystem backend has no rename, link, symlink or fsync (`microsoft/litebox@c8eafc7a07725331113c5ec5d1ac56a95ac873b8:litebox/src/fs/backend.rs:47-169`). The root is an overlay in memory over a read-only tar.
- READ: networking on Windows is `unimplemented!()` and panics (`microsoft/litebox@c8eafc7a07725331113c5ec5d1ac56a95ac873b8:litebox_platform_windows_userland/src/lib.rs:1630-1646`).
- READ: the userland platforms build for x86_64 only (`microsoft/litebox@c8eafc7a07725331113c5ec5d1ac56a95ac873b8:litebox_platform_linux_userland/src/lib.rs:11`, `microsoft/litebox@c8eafc7a07725331113c5ec5d1ac56a95ac873b8:litebox_platform_windows_userland/src/lib.rs:11`).
- READ: the one hardware-isolated variant runs at VMPL2 in an AMD SEV-SNP guest. It needs a VMPL0 driver that is not in the repository (`microsoft/litebox@c8eafc7a07725331113c5ec5d1ac56a95ac873b8:litebox_platform_linux_kernel/src/host/snp/snp-sandbox.h:4`).
- READ: support for more than one process is an open RFC with no pull request (https://github.com/microsoft/litebox/issues/797).

Verdict: not an engine on either OS (D13).

## Linux candidates

### Firecracker v1.17.0 with the jailer (selected)

- READ: the jailer runs as root. It needs each resource hard-linked or copied into the jail, and leaves the cleanup to the user (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:266-292`).
- READ: v1.17.0 fixes vsock delivery from host to guest after a bare pause and resume (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:CHANGELOG.md:81-88`). So v1.17.0 is the minimum pin for a pause verb.
- READ: discard works only on writable drives with the `Sync` engine (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-discard.md:10-20`). It is new in v1.17.0 (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:CHANGELOG.md:59-62`).
- READ: the `Async` engine is a developer preview (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-io-engine.md:9-12`). Its measured gain on NVMe is 1.5 to 3 times in read efficiency (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-io-engine.md:92-94`).
- READ: `Unsafe` caching gives up data integrity on host power loss. `Writeback` turns each guest flush into an fsync (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-caching.md:16-48`).
- READ: a block device has one queue (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/devices/virtio/block/virtio/mod.rs:31`). The block code has no O_DIRECT, so data goes through the host page cache.
- READ: one VMM thread emulates the net, block and vsock devices (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/design.md:72-79`).
- READ: Firecracker validates the host kernels 5.10, 6.1 and 6.18, with 4K pages only (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:24-30`).
- READ: the tested aarch64 hosts are AWS Graviton 2 to 5 (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:README.md:135-153`).
- READ: a second resume of the same snapshot is insecure (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/snapshotting/snapshot-support.md:541-566`). The snapshot format is 12.0.0 (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/persist.rs:166`).
- READ: Firecracker exits on a CPU reset, and SendCtrlAltDel exists on Intel and AMD only (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/actions.md:34-40`).
- READ: for 1 vCPU and 128 MiB, the specification targets a VMM overhead of 5 MiB or less (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:SPECIFICATION.md:24-33`). It targets 125 ms or less from InstanceStart to init (same file, lines 37-40). A full userland was not measured.
- READ: each minor release gets support for at least 6 months (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/RELEASE_POLICY.md:50-51`).

Verdict: selected (D12).
Persistence changes the configuration, not the choice.
The data disk uses `Sync` with `Writeback` and discard.
Each instance keeps a stable uid and cgroup, the jail is rebuilt on each boot, and a daemon restart re-adopts running instances.
[docs/engines.md](../engines.md) gives that design.
crosvm and QEMU under libvirt also confine their VMMs (correction 6).
Firecracker wins on the smallest device model (R4).
It also wins on access control: file permissions guard its hybrid-vsock sockets.
INFERRED: with in-kernel vhost-vsock, any local process can open AF_VSOCK to the CID of a guest.

### Cloud Hypervisor v53.0 (Linux fallback)

- READ: virtio-blk does DISCARD and WRITE_ZEROES on raw and qcow2 images since v51, with `sparse=on` as the default (`cloud-hypervisor/cloud-hypervisor@9ed824d6d08df3e96f7d5f50795d9449ac99f431:release-notes.md:973-981`).
- READ: the disk options include O_DIRECT and more than one queue. The backend prefers io_uring, then AIO, then synchronous I/O.
- READ: OFD locks stop two instances from opening one image (`cloud-hypervisor/cloud-hypervisor@9ed824d6d08df3e96f7d5f50795d9449ac99f431:docs/disk_locking.md:1-19`).
- READ: seccomp per thread is on by default. Landlock is opt-in and covers mostly files.
- READ: there is no jailer. The jailer of Firecracker appends arguments of its own to the binary that it starts (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/jailer/src/env.rs:555-561`). So it cannot wrap Cloud Hypervisor without a shim.
- READ: the hypervisor crates are KVM and MSHV only (`cloud-hypervisor/cloud-hypervisor@9ed824d6d08df3e96f7d5f50795d9449ac99f431:Cargo.toml:58-62`). MSHV means Linux as the root partition (`cloud-hypervisor/cloud-hypervisor@9ed824d6d08df3e96f7d5f50795d9449ac99f431:docs/mshv.md:1-10`). So there is no Windows host.
- Not verified: whether v53.0 has VMGenID (correction 9).

Verdict: the Linux fallback if measured block I/O dominates the agent workload (T-015, T-093).
agent-box would build the jail: namespaces, chroot, uid, cgroup and a seccomp policy.
The hybrid vsock protocol is the same, so the transport stays.

### QEMU under libvirt (fallback for 16K-page aarch64 hosts)

- INFERRED: libvirt confines QEMU with an unprivileged user, cgroups, seccomp and an sVirt label per VM.
- INFERRED: it has `virsh managedsave`, iothreads, discard and O_DIRECT, and it works on any aarch64 page size.
- Its device model is large (R4), so it is not the primary engine.

Verdict: the fallback for aarch64 hosts with 16K pages, such as the Raspberry Pi 5 and Asahi (T-094). The alternative is a written limit to 4K-page hosts.

### Kata Containers

- READ: the hypervisors of Kata are Cloud Hypervisor, Firecracker, QEMU, Dragonball and StratoVirt (https://github.com/kata-containers/kata-containers/blob/main/docs/hypervisors.md).
- INFERRED: each of them runs on KVM or MSHV on a Linux host, so Kata has no Windows host.
- INFERRED: Kata serves container runtimes and has no lifecycle for a persistent instance.
- Design: it brings containerd, a shim and virtiofsd. virtiofsd shares a filesystem, which R7 forbids.

Verdict: rejected.

## Single-engine candidates

Four engines run natively on both OSes, with no nested virtualization: crosvm, OpenVMM, libkrun and QEMU.
Each one weakens isolation on at least one OS.

### crosvm

- READ: upstream WHPX and HAXM are "Tested upstream: no", and KVM is tested (`google/crosvm@113b02ce7e7092bae7b455bcc8c99c8f5e891b4a:hypervisor/README.md:16-33`).
- READ: in April 2026 the maintainers wrote that WHPX had decayed upstream, and they added only a lint check for it (https://github.com/google/crosvm/commit/c59ce5a137126ee434dc0a298ed3b431ff7a93d0).
- READ: the product code for Windows lives in a downstream repository of Google (https://github.com/google/crosvm/commit/c25eeb4a28360e2baf756c2de401f3d6c4dabddf).
- READ: the Windows `sandbox` feature is listed as functional only in future builds (`google/crosvm@113b02ce7e7092bae7b455bcc8c99c8f5e891b4a:Cargo.toml:468-479`). It links a prebuilt Chromium sandbox library of 2022 that only Google can rebuild.
- READ: even with the sandbox, the main process keeps a restricted token at Low integrity level, on the user's desktop (`google/crosvm@113b02ce7e7092bae7b455bcc8c99c8f5e891b4a:sandbox/src/policy.rs:41-56`).
- READ: on Windows, connections that the host starts are not implemented in vsock (`google/crosvm@113b02ce7e7092bae7b455bcc8c99c8f5e891b4a:devices/device_virtio_vsock/src/sys/windows/vsock.rs:1109-1114`).
- READ: on Linux, vsock is the in-kernel vhost-vsock device (`google/crosvm@113b02ce7e7092bae7b455bcc8c99c8f5e891b4a:devices/device_virtio_vsock/src/sys/linux.rs:15-19`).
- READ: on Linux, each device runs in its own minijail process (`google/crosvm@113b02ce7e7092bae7b455bcc8c99c8f5e891b4a:docs/book/src/appendix/minijail.md:3-12`). The main KVM process has no seccomp policy.
- READ: snapshots are "100% not supported" (`google/crosvm@113b02ce7e7092bae7b455bcc8c99c8f5e891b4a:docs/book/src/architecture/snapshotting.md:3-4`).
- READ: on Windows, sparse raw disks free space on discard (`google/crosvm@113b02ce7e7092bae7b455bcc8c99c8f5e891b4a:base/src/sys/windows/punch_hole.rs:22-48`).
- READ: crosvm has no releases and no Windows binaries (https://github.com/google/crosvm/releases).

Verdict: rejected. On Windows it is weaker than HCS and we would maintain it. On Linux its main process needs a jail that we write.

### OpenVMM with confinement that agent-box builds

The facts of [OpenVMM on WHP](#openvmm-on-whp-second-windows-fallback) apply. The sweep added these:

- READ: the KVM support on aarch64 calls itself "unsatisfactory" (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:vmm_core/virt_kvm/src/arch/aarch64/mod.rs:4-7`). CI has no aarch64 Linux KVM job.
- READ: a virtio device that does not opt in cannot be saved, and virtio-vsock does not opt in (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:vm/devices/virtio/virtio/src/device.rs:108-116`). So no configuration with vsock can be snapshotted today.
- READ: Microsoft closed its own microVM and snapshot effort as not planned on 2026-09-28 (https://github.com/microsoft/openvmm/issues/4421).
- READ: the ttrpc server reads only the memory size and ignores the overcommit and discard hints (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:openvmm/openvmm_entry/src/ttrpc/mod.rs:1090-1108`).
- INFERRED: a Linux jail for it is about 2 to 3 thousand lines of Rust, plus a seccomp list for each release. io_uring operations on the raw disk path are invisible to seccomp.

Verdict: rejected as the single engine. agent-box would become the packager, signer and security owner of a VMM on three targets.

### libkrun and the smolvm fork

- READ: upstream libkrun has a WHP backend on the main branch only. No release has it, and Windows comes with libkrun 2.0 (https://github.com/libkrun/libkrun/issues/798).
- READ: on Windows, an external kernel hits `unreachable!()`, so only the libkrunfw kernel bundle boots (`containers/libkrun@b9f27258fe557d2970fdb0c57836b3b290d06d86:src/libkrun/src/vmm/builder.rs:595-599`).
- READ: pause and resume return FeatureDisabled outside macOS (`containers/libkrun@b9f27258fe557d2970fdb0c57836b3b290d06d86:src/libkrun/src/api/vmm_builder.rs:195-214`).
- READ: smolvm ships Windows binaries today (https://github.com/smol-machines/smolvm/releases). Its libkrun fork is 256 commits ahead of upstream and 233 behind.
- READ: smolvm states that its CLI and VMM run with the permissions of the invoking user (`smol-machines/smolvm@8cf06018ff65db2196eba0fffab26c79d96995c4:docs/security-model.md:5`).
- READ: neither tree uses WHP doorbells. Each virtio-mmio access exits to user mode.
- READ: free-page reporting on Windows uses DiscardVirtualMemory (https://github.com/libkrun/libkrun/pull/716).

Verdict: rejected; watch it. It is the only single engine with Windows binaries today, through a fork, but its VMM runs as the user.

### QEMU with WHPX and KVM

- READ: vhost-user and vhost-kernel are not available on Windows (`qemu/qemu@4fc49f46dc95d4a27de2509e7fceb2931e91faeb:meson.build:227-235`). So QEMU on Windows has no vsock, and its VMBus code needs KVM.
- READ: WHPX always registers a migration blocker, so savevm fails (`qemu/qemu@4fc49f46dc95d4a27de2509e7fceb2931e91faeb:target/i386/whpx/whpx-all.c:2760-2769`).
- READ: guest RAM on Windows is committed in full at start (`qemu/qemu@4fc49f46dc95d4a27de2509e7fceb2931e91faeb:util/oslib-win32.c:61`). The balloon cannot give it back, and the Windows file driver has no discard.
- READ: `-sandbox` needs libseccomp, so QEMU on Windows has no sandbox (`qemu/qemu@4fc49f46dc95d4a27de2509e7fceb2931e91faeb:system/vl.c:3616-3623`).
- READ: on Linux, `-sandbox` is a deny list over a default of allow (`qemu/qemu@4fc49f46dc95d4a27de2509e7fceb2931e91faeb:system/qemu-seccomp.c:309`), applied to every thread of the process.
- READ: open WHPX bugs hit this setup: a virtio hang with a Debian 13 guest (https://gitlab.com/qemu-project/qemu/-/issues/4492) and a pipe chardev deadlock (https://gitlab.com/qemu-project/qemu/-/issues/2748).
- READ: in one test with 1 vCPU and 128 MB, Firecracker used 82 MB of RSS and QEMU 163 MB (https://docs.bell-sw.com/alpaquita-linux/latest/virtualization/firecracker-qemu-vm/).

Verdict: rejected as the single engine. On Linux it stays the recorded fallback under libvirt for 16K-page aarch64 hosts.

### A custom VMM

- INFERRED: about 15 to 30 thousand lines of Rust for KVM on x86_64 and aarch64 plus WHP on x86_64.
- INFERRED: a first version that boots Debian takes 3 to 6 engineer-months. The robustness of Firecracker takes 1.5 to 3 engineer-years, plus every CVE after that.
- READ (calibration): the Windows code of libkrun is about 9.5 thousand lines after about 7 months, and it is not released. The WHP backend of Hyperlight is 1,537 lines for one vCPU with no devices.
- INFERRED: on Windows it hits the same confinement ceiling as every WHP engine.

Verdict: rejected. agent-box would own a device model that the guest attacks, on two OSes, with no upstream security team.

### VirtualBox 7.2

- READ: VirtualBox has no virtio-vsock device (https://github.com/VirtualBox/virtualbox, src/VBox/Devices/VirtIO).
- READ: its KVM backend works in development builds only (https://phoronix.com/news/virtualbox-upstream-with-kvm).
- READ: on a host with Hyper-V on, it runs over WHP, which Oracle documents as experimental with a possible large slowdown (https://docs.oracle.com/en/virtualization/virtualbox/6.0/admin/hyperv-support.html).
- INFERRED: its processes run as the user, and its hardening on Windows is against injection, not a sandbox.

Verdict: rejected. It fails the vsock requirement.

### Hyperlight

- READ: there is no kernel or OS in the VM. Guests are `no_std` Rust or C programs (`hyperlight-dev/hyperlight@ad05110e17bf40bab2e5306d917ec468c1db064c:README.md:15`).
- READ: its hypervisor trait is private to the crate and serves one vCPU (`hyperlight-dev/hyperlight@ad05110e17bf40bab2e5306d917ec468c1db064c:src/hyperlight_host/src/hypervisor/virtual_machine/mod.rs:432-434`).

Verdict: rejected. It cannot run a Linux userland.

### The others

| Candidate | Finding | Label |
| --- | --- | --- |
| MSHV root | A Linux driver for Linux as the root partition, merged in Linux 6.15. Not an API of a Windows host. | READ: https://lkml.iu.edu/hypermail/linux/kernel/2503.3/01035.html |
| Docker VMM | A closed engine that does not name its hypervisor API. | READ: https://docs.docker.com/desktop/features/vmm/ |
| VMware Workstation | Closed source. It uses WHP when Hyper-V is on. | INFERRED |
| Multipass | HCS is the default driver on Windows; QEMU on Linux. | READ: https://canonical.com/multipass/docs//en/latest/explanation/driver/ |
| Lima | An experimental `hcs` VM type since Lima 2.3. | READ: https://lima-vm.io/docs/config/vmtype/hcs/ |
| Podman machine | WSL or Hyper-V on Windows; QEMU on Linux. | READ: https://docs.podman.io/en/latest/markdown/podman-machine.1.html |

The mature products split their backends by OS, as agent-box does. Multipass and Lima chose HCS on Windows.

### The ranking

The sweep ranked the options if the operator ever mandates one engine:

1. crosvm and OpenVMM, equal. Pick crosvm when Linux isolation and aarch64 matter most. Pick OpenVMM when the quality of the Windows backend matters most.
2. libkrun.
3. QEMU.
4. A custom VMM.
5. VirtualBox.

Each choice still needs a Windows launcher (a dedicated account, a job object, mitigation policies and ACLs) and a Linux jail with a seccomp policy.
The gain of one engine is small: the guest image, box-guest, the wire protocol and the broker are already shared.

## kage in the guest

kage is not an engine, but its facts changed the design around the engines.

- READ: the shell tool runs commands with no sandbox (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-tools/src/builtin/shell.rs:3-4`).
- READ: the permission fallback is allow (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/permissions.rs:105`).
- So the instance is the whole boundary, and the guest must hold no real secret.
- READ: kage makes its HTTP calls with ureq, which depends on webpki-roots (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:Cargo.toml:91`, `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:Cargo.lock:4039`). No kage crate sets its own trust roots.
- INFERRED from those facts: kage ignores the trust store of the guest, so a CA planted in the guest fails. This led to P4.
- The upstream requests that the reader found are in T-169. The pinned shell settings are T-160.

## Adversarial corrections

Each row is an objection of the review and where the correction lives now.

| # | The claim of the synthesis | The correction | Where it lives |
| --- | --- | --- | --- |
| 1 | WSL2 runs this exact path. | True for create, boot, VHDX, hvsocket and no NIC only. WSL and hcsshim set ShouldTerminateOnLastHandleClosed to true and never pause or save. | The four features are spike items: T-014. |
| 2 | Schema 2.3 or later. | AlwaysAllowSparseFiles is new in schema 2.6, the Windows 11 SDK 21H2 (10.0.22000.194). WSL uses 2.3. | T-101, T-014 (e). |
| 3 | A guest escape lands in the sandboxed worker. | Only for devices that `vmwp.exe` emulates. VMBus, storvsp, vhdmp and the hvsocket provider are kernel code of the root partition (INFERRED). | [engines.md](../engines.md#the-isolation-difference), T-182. |
| 4 | It is open whether HvSocket needs a registry entry. | Answered: the default bind descriptor of a VM covers binds that are not wildcards. One listener per VmId, never AllowWildcardBinds. | T-104, T-014 (a). |
| 5 | The pause design is complete. | HVSOCKET_CONNECTED_SUSPEND is an option of each socket, and the default disconnects on a pause. | T-025, T-106. |
| 6 | Firecracker is the only candidate with a jail. | False. crosvm jails each device with minijail (READ), and libvirt confines QEMU (INFERRED). | D12 now cites R4 and socket permissions. |
| 7 | (not stated) | One VMM thread emulates block and vsock, `Sync` blocks, and discard needs `Sync` (READ). Disk load can delay every host channel (INFERRED). | T-015. |
| 8 | R10 is met. | Graviton hosts only, and 4K-page host kernels only. The Raspberry Pi 5 and Asahi default to 16K pages. | T-094. |
| 9 | Cloud Hypervisor: no WHP and no VMGenID, READ. | The tree was sparse. No WHP holds through its Cargo.toml. VMGenID is not verified. | This page. |
| 10 | The HCS error evidence names 0x80070005. | 0x80370102 also occurs: NX was off in the BIOS, so the hypervisor never started, while vmms still answered. | The preflight of T-108. |
| 11 | WSL2 runs on every desktop edition, so HCS does. | That is a fact of WSL. No evidence exists for a third-party HCS caller on Home. | T-109. |
| 12 | The helper has a fixed operation set. | Not strict enough: the helper builds every HCS document itself, and grants VM access only inside the agent-box store. | T-100. |
| 13 | Open: one kernel image, and guest time with no NIC. | Firecracker v1.17.0 boots a bzImage, and HCS adds the vsock blacklist on the command line. Guest time can come from hv_utils or ptp_kvm (INFERRED). | T-060, T-061, T-044. |
| 14 | (confirmations) | The review confirmed the vsock fix of v1.17.0, discard on `Sync` only, the developer previews and the empty `SandboxProfile`. | This page. |

Sources of the review that the table does not cite elsewhere:

- READ: the WSL and hcsshim lifecycle flags: https://raw.githubusercontent.com/microsoft/WSL/master/src/windows/service/exe/WslCoreVm.cpp and https://raw.githubusercontent.com/microsoft/hcsshim/main/internal/uvm/create.go.
- READ: the per-socket suspend option: the header shared/hvsocket.h of the Windows SDK 10.0.22621.0, lines 50-57.
- READ: the cause of 0x80370102: https://github.com/anthropics/claude-code/issues/60371. Other reports have no diagnosis: https://github.com/anthropics/claude-code/issues/33784.
- READ: 16K pages on the Raspberry Pi 5: https://github.com/k3s-io/k3s/issues/7335.
- READ: Firecracker v1.17.0 boots a bzImage on x86_64 (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:CHANGELOG.md:29`).
- READ: Firecracker asks the host for its page size (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/vstate/memory.rs:166`). So a 16K-page host may work, but nobody tests it.

The kernel plan differs from correction 13. T-060 builds `vmlinux` for Firecracker and `bzImage` for HCS from one config per architecture. One bzImage for both engines is possible but is not the plan.

### Missed options that the review added

| Option | Outcome |
| --- | --- |
| Hyper-V VMMS through WMI v2 | The first Windows fallback (T-111). |
| crosvm on Linux KVM | Evaluated by the sweep and rejected. |
| QEMU under libvirt on Linux | The fallback for 16K-page aarch64 hosts (T-094). QEMU on Windows is rejected. |
| A WSL2 distribution per instance, the model of wsl-toolkit | Rejected. INFERRED: every distribution shares one utility VM, one kernel, the WSL NIC and Windows interop. |
| libkrun | Evaluated by the sweep and rejected. |
| VMware Workstation, VirtualBox on WHP, Kata, Dragonball | Rejected. |
| virtio-mem hotplug in Firecracker, and vhost-user-blk | virtio-mem is a footprint measurement in T-181. vhost-user-blk is a mitigation that T-015 compares. |

## Superseded by later rulings

The evaluation assumed that the guest has no network interface and reaches the host only through a proxy.
D5 gave the guest a virtual NIC whose frames go over vsock to a netstack in the daemon.
P4 sent credentialed traffic to explicit plain-HTTP broker endpoints.
The engine configuration still has no NIC device: no `network-interfaces` key, and no NetworkAdapters.

| Item of the evaluation | Where it appears | Replaced by |
| --- | --- | --- |
| An allowlisting CONNECT proxy in the broker for provider calls, web tools and git | The synthesis recommendation, the kage verdict, and the corrected recommendation of the review | D5: the netstack with policy per flow (T-045, T-121, T-123) |
| A choice between a guest loopback forwarder and CONNECT to a broker-only name | The kage reader and the open items of the synthesis | D5 and P4: the netstack DNS resolves each broker name to the gateway (T-122, T-142) |
| TLS interception with a CA planted in the guest trust store | The operator's research document and the old broker plan | P4: explicit broker endpoints. TLS interception for other tools is a later option (T-149). |
| No network interface in the guest; egress through a proxy over vsock only | The scenario of the readers and the WSLC analysis | D5: a tun device in the guest, frames over vsock |
| MCP OAuth fails because the guest has no DNS | The kage reader | D5: the netstack answers DNS for allowed names (T-122). MCP OAuth tokens in the guest stay a question (T-162). |
| A loopback proxy in the guest for the API calls of kage | The Linux reader | D5 and P4 |

## Corrections to the operator's research document

The evaluation corrected these claims of [the microVM research](microvm-tier.md#corrections-made-in-this-conversion).
That page marks each one in place.

1. The data disk settings. `Unsafe` drops flushes and does not keep the image out of the host page cache. `Async` is a developer preview with no discard. The settings are now `Sync`, `Writeback` and discard.
2. The jail lifecycle. Jail removal deleted the workspace image, and a daemon restart killed every VMM as an orphan. Persistent instances need the disk outside the jail lifecycle and the re-adoption of running VMMs.
3. The research cites the threat model of Cloud Hypervisor at v53.0. The file exists on the main branch only.
4. The Firecracker jailer cannot wrap Cloud Hypervisor, because it appends Firecracker arguments.
5. Firecracker is not the only jailed VMM (correction 6 of the review).
6. R10 holds on 4K-page hosts only (correction 8).
7. One VMM thread carries block and vsock (correction 7).
8. Firecracker v1.17.0 also boots a bzImage on x86_64.
9. The vmm thread allows the io_uring syscalls for every `io_engine` (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:resources/seccomp/x86_64-unknown-linux-musl.json:66-76`). A custom filter without them is an open question of T-084.
10. A CA planted in the guest fails for kage. P4 replaces the TLS interception of the research.

## Rulings

The operator ruled on 2026-10-14. [docs/decisions.md](../decisions.md#product) holds the text of each ruling.

| Ruling | What the evaluation gave it |
| --- | --- |
| D11: the Windows engine is HCS, driven by a privileged helper service | Microsoft hardens and services the VMM worker, and HCS boots the kernel directly. The sweep found four single engines, and each weakens isolation. |
| D12: the Linux engine is Firecracker v1.17.0 with its jailer, set up for persistent instances | The smallest device model and sockets under file permissions. Persistence changes the configuration only. |
| D13: MXC and LiteBox are not engines; MXC is a source of design patterns | No MXC backend runs a persistent Linux guest with its own kernel. LiteBox shares the address space of its host process. |

Later decisions replaced parts of the evaluation: D5 and P4 (above).
The planning session recorded the meaning of the verbs as decision P5.
`pause` is an in-memory freeze of the vCPUs, `stop` is a guest poweroff, and `hibernate` is a later opt-in verb (T-184).
The operator may override P5.

## Re-evaluation triggers

| Trigger | What it reopens | Where |
| --- | --- | --- |
| libkrun 2.0 ships with WHP, and Windows can boot an external kernel | libkrun as a single engine | This page |
| crosvm upstreams a sandbox that others can build, and runs WHPX in CI | crosvm as a single engine | This page |
| OpenVMM ships signed binaries, memory reclaim, a sandbox profile and a save call | OpenVMM as the single engine or the first Windows fallback | T-111 |
| A spike item of HCS fails on Windows 11 Pro | The Windows engine: VMMS through WMI, then OpenVMM | T-014, T-111 |
| HCS fails on Windows 11 Home | OpenVMM for Home | T-109, T-111 |
| Measured block I/O dominates, or vsock latency under disk load fails | Cloud Hypervisor with a jail that agent-box builds | T-015, T-093 |
| A boot on a 16K-page aarch64 host fails | QEMU under libvirt for such hosts | T-094 |
| WSLC gets a VM with no NIC, hvsocket for our code, and pause | WSLC as a Windows engine | This page |
| LiteBox lands support for more than one process (RFC 797) | LiteBox for single helper programs only, not as an engine | This page |
| MXC ships its hypervisor micro-VMs | MXC as an engine | This page |

The cheap hedge: the guest kernel keeps both the VMBus drivers and the virtio drivers ([guest.md](../guest.md)). A change of engine then needs no new guest image.

## Open measurements and questions

Nothing below is measured yet.

| Item | Entry |
| --- | --- |
| HCS: no NIC, a SCSI VHDX and a bind per VmId under SDDL, with no registry write | T-014 (a) |
| HCS: the VM outlives a helper restart, and HcsOpenComputeSystem re-adopts it | T-014 (b) |
| HCS: HcsPause and HcsResume keep a bridge connected with HVSOCKET_CONNECTED_SUSPEND | T-014 (c) |
| HCS: HcsSave to a file, then RestoreState, for a LinuxKernelDirect VM | T-014 (d) |
| HCS: AlwaysAllowSparseFiles and guest fstrim make the VHDX smaller | T-014 (e), T-110 |
| HCS on Windows 11 Home, and whether Hyper-V Administrators exists there | T-109 |
| Firecracker: vsock round-trip latency during a cargo build on the data disk | T-015 |
| Firecracker `Sync` against `Async` against Cloud Hypervisor io_uring, and host file growth without discard | T-093 |
| A boot on a 16K-page aarch64 host | T-094 |
| One kernel with virtio_vsock and hv_sock under both engines | T-061 |
| Guest time after a pause: ptp_kvm under KVM, hv_utils under Hyper-V | T-044 |
| Host memory per instance (the Firecracker VMM; `vmwp.exe` plus vmmem), boot to the first vsock connect, kage serve RSS | T-181 |
| Whether free-page reporting is generally available in Firecracker v1.17.0, and virtio-mem as a footprint control | T-181 |
| Memory reclaim and fstrim on Windows | T-110 |
| Guest S4 hibernation as an alternative to VMM snapshots; whether a Full snapshot memory file is sparse | T-184 |
| Containers inside the guest | T-185 |
| A NetworkAdapters entry in the HCS document as the negative control of the battery (planned, not verified) | T-180 |
| Whether `vmwp.exe` on Windows 11 runs under a virtual account per VM | T-182 |
| An AppContainer capability that grants WHP; the plans of OpenVMM for binaries, save and a sandbox | T-111 |
| The citations of this page at their pins | T-013 |

Open items of the synthesis that need no measurement now:

- The privilege model: a helper service on Windows (D11, T-100) and a root helper with socket activation on Linux (T-081). Membership in Hyper-V Administrators would give the user control of every VM on the host.
- The host kernel policy on Linux: the probe reports a kernel outside the validated set as a note (T-092).
- The kage requests to its maintainer: T-169. The array parse of `KAGE_SHELL__SCRUB_ENV`: T-160.
- The WSLC questions (one VM per session, Consomme off, reopen after a reboot) matter only while WSLC is a watch item. No entry owns them.
