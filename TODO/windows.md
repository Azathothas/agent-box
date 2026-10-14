# Windows engine

This area builds the Windows engine: native Hyper-V through the Host Compute
Service (computecore.dll), driven by a LocalSystem helper service (ruling D11).
The design of this engine is in [docs/engines.md](../docs/engines.md), and each
entry below builds one part of it. [docs/protocol.md](../docs/protocol.md) gives the helper
protocol and the `AF_HYPERV` transport. The engine evaluation behind the choice
is in [docs/research/engines-2026-10.md](../docs/research/engines-2026-10.md).

No product crate exists yet (ruling D14), so each crates/ path here is a
planned path with no backticks. T-033 creates the crates. The
helper is crate box-helper. The schema types, the computecore calls and the
virtdisk calls are the hcs module of box-vmm. The daemon side is the planned
file crates/box-daemon/src/engine/hcs.rs.

The workspace lint is `unsafe_code = "deny"`. A named module may allow unsafe
code, with a comment on each block. The hcs module of box-vmm is one such
module (T-102, T-103).

Four HCS features have no Linux-guest reference user. They are a compute
system that outlives its creator, pause, save and restore, and a data disk
that shrinks after a trim. The spike T-014 tests them on Windows 11 Pro before this area depends
on them.

Hosts: the schema tests run on any host. The live tests need Windows 11 with
Hyper-V and the helper service, which an administrator installs once. The
daemon and the CLI run as the operator, unprivileged.

# T-100: Windows helper service: install, pipe ACL, client check, fixed operations

**Source:** The planning brief of 2026-10-14, section 4.2. The adversarial
review, item 12. docs/engines.md, section "The privileged helper", and
docs/protocol.md, section "The helper protocol".
**Category:** feature
**Milestone:** M4
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

HCS refuses a caller outside two administrator groups, and the daemon runs as
the operator. A LocalSystem service must make the HCS calls. Each input from a
caller can turn that service into a privilege-escalation tool. Examples are a
host share, a NIC, or an access grant on any file.

## Premise

- MEASURED in one run on 2026-10-14, on the Windows 11 Pro host of the
  operator (i7-12700H, 20 logical CPUs, 64 GB). With a non-elevated token,
  `hcsdiag list` exited 27 ([docs/status.md](../docs/status.md)).
- The refusal says that only administrators or members of the Hyper-V
  Administrators group may access virtual machines.
- READ (engine evaluation): the WSL service and a third-party VM service
  follow the same pattern, a LocalSystem service that an unprivileged program
  drives.
- READ: membership of Hyper-V Administrators lets a user manage Hyper-V
  virtual machines
  (https://docs.podman.io/en/latest/markdown/podman-system-hyperv-prep.1.html,
  read 2026-10-14). The group gives control of every VM on the host.
- Inference of the adversarial review, item 12: a Plan9, VirtualSmb or
  NetworkAdapters entry from the caller makes the service an escalation tool.
  `HcsGrantVmAccess` on a path of the caller's choice does the same.
- Design (docs/protocol.md): the pipe `\\.\pipe\agent-box-helper`, a DACL for
  SYSTEM and the operator SID, `PIPE_REJECT_REMOTE_CLIENTS` and
  `FILE_FLAG_FIRST_PIPE_INSTANCE`.
- Design (docs/protocol.md): the helper reads the client SID with
  `ImpersonateNamedPipeClient`, `OpenThreadToken` and `GetTokenInformation`,
  then reverts.
- Design (docs/engines.md): demand start, and the service stops itself when
  idle. Its privileges are declared after this entry measures them.

## Approach

1. Write the Windows helper in crates/box-helper/src/windows/mod.rs, gated by
   `cfg(windows)`. crates/box-helper/src/main.rs selects the OS module.
2. Put the Win32 calls of the service, the pipe and the token in one module,
   crates/box-helper/src/windows/sys.rs. Allow unsafe code there only, with a
   comment on each block.
3. Decision P6 in docs/decisions.md already lists this file, after the files
   of box-transport and box-vmm. The unsafe check (T-259) holds the list.
4. Install (T-174 calls it): copy the binary to
   `%ProgramFiles%\agent-box\agent-box-helper.exe`. The ACL gives SYSTEM and
   Administrators full control, and Users read and execute.
5. Create the service with `CreateServiceW`: the name `agent-box-helper`,
   `SERVICE_WIN32_OWN_PROCESS`, `SERVICE_DEMAND_START`, no account, so it runs
   as LocalSystem.
6. Call `ChangeServiceConfig2W` with `SERVICE_CONFIG_SERVICE_SID_INFO` set to
   `SERVICE_SID_TYPE_UNRESTRICTED`. The service then has its own SID for the
   store ACLs.
7. Call `ChangeServiceConfig2W` with `SERVICE_CONFIG_REQUIRED_PRIVILEGES_INFO`
   and the list that step 23 measures.
8. Set the service DACL with `SetServiceObjectSecurity`. Proposal:
   `D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;LCRP;;;<operator SID>)`.
9. That DACL lets the operator query the status and start the service, and
   nothing more. Check it with `sc.exe sdshow agent-box-helper`.
10. Write the configuration `%ProgramData%\agent-box\helper.toml`: the
    operator SID, the store root and the bounds. Its DACL is
    `D:P(A;;FA;;;SY)(A;;FA;;;BA)`.
11. At start, refuse a configuration file whose DACL lets another account
    write it (`helper::refuses_unsafe_config_acl`).
12. Create the pipe with `CreateNamedPipeW`: `PIPE_ACCESS_DUPLEX`,
    `FILE_FLAG_FIRST_PIPE_INSTANCE`, `FILE_FLAG_OVERLAPPED`, `PIPE_TYPE_BYTE`
    and `PIPE_REJECT_REMOTE_CLIENTS`.
13. Build its security descriptor with
    `ConvertStringSecurityDescriptorToSecurityDescriptorW` from
    `D:P(A;;GA;;;SY)(A;;GRGW;;;<operator SID>)`.
14. After `ConnectNamedPipe`, call `ImpersonateNamedPipeClient`,
    `OpenThreadToken` with `TOKEN_QUERY`, and `GetTokenInformation` for
    `TokenUser` and `TokenIntegrityLevel`. Then call `RevertToSelf`.
15. Refuse a user SID that differs from the configured operator SID with
    `peer_refused` (`helper::refuses_foreign_sid`).
16. Refuse a token below medium integrity (`helper::refuses_low_integrity_client`).
    A sandboxed process of the operator then cannot drive the helper.
17. In the daemon client, check the server side of the pipe before it sends a
    frame. `GetNamedPipeServerProcessId`, then the token of that process, must
    give SYSTEM (S-1-5-18).
18. When the connect fails with `ERROR_FILE_NOT_FOUND`, the daemon starts the
    service with `StartServiceW` and waits with `WaitNamedPipeW`, up to 10 s.
    That limit is a design value.
19. Read each frame with a deadline of 5 s, the same design value as on Linux
    (T-081). Use the box-proto codec and the pure check of T-023.
20. The operations are probe (T-108), create (T-103), start (T-105), pause and
    resume (T-106), kill (T-105), delete and inventory (T-107).
21. `stop` answers `REFUSAL` `unsupported`: the guest power-off on port 5000 is
    the soft stop. Until the entry of an operation lands, it answers
    `unsupported` with that entry in the detail.
22. The helper builds every HCS document itself (T-101). It grants VM access
    only to files inside the store (T-103), after the reparse and final-path
    checks of docs/engines.md (`helper::refuses_grant_outside_store`).
23. Measure the privileges that the HCS, virtdisk and ACL calls need. Run each
    operation with one privilege removed from the service token. Write the
    list into docs/engines.md.
24. Stop the service 60 s after the last connection, a design value. Running
    compute systems stay up, if spike T-014 (b) passed.
25. Log one line per operation to `%ProgramData%\agent-box\logs\helper.log`,
    SYSTEM only. Never log request bytes.

## Prove

```sh
cargo test -p box-helper
cargo test -p box-helper --test pipe_live -- --ignored
sc.exe sdshow agent-box-helper
```

Expected: the unit tests pass; each refusal runs as a pure test. The live test
needs the service installed and a second local standard account that the
operator made for the test.

- As the operator, `probe` returns a result.
- As the second account, `CreateFileW` on the pipe fails with
  `ERROR_ACCESS_DENIED`.
- A process of the operator at low integrity is refused.
- After 60 s with no client, the service state is `SERVICE_STOPPED`.

`sc.exe sdshow` prints the DACL of step 8. Host: any host for the unit tests;
Windows 11 with Hyper-V as administrator for the install and the live test.

## Start condition

T-023 and T-033. Spike T-014 must have results, because (b) decides whether the
service may stop while instances run.

# T-101: HCS schema types

**Source:** The planning brief of 2026-10-14, section 4.2. The HCS schema
reference, read 2026-10-14. The WSL and hcsshim sources, read 2026-10-14.
docs/engines.md, section "The compute system document".
**Category:** feature
**Milestone:** M4
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The windows crate has no schema types, and HCS takes a JSON document. A builder
that can write any JSON can write a NIC, a host share or a typo that HCS reads
in another way. The LocalSystem helper must not be able to express those
devices at all.

## Premise

- READ: the top-level fields are `Owner`, `SchemaVersion`, `HostingSystemId`,
  `HostedSystem`, `Container`, `VirtualMachine` and
  `ShouldTerminateOnLastHandleClosed`
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/schemareference,
  read 2026-10-14).
- READ (same page): `LinuxKernelDirect` has `KernelFilePath`, `InitRdPath`
  and `KernelCmdLine`, all new in 2.2. `GuestConnection` is optional; without
  it, the VM counts as started when the chipset is powered on.
- READ (same page): `VirtualMachine_Memory` has `SizeInMB`, `AllowOvercommit`
  and `EnableDeferredCommit` (2.1), and `EnableColdDiscardHint` (2.2).
  `VirtualMachine_Processor` has `Count` (2.0).
- READ (same page): `Devices` also has `NetworkAdapters`, `Plan9`,
  `VirtualSmb`, `ComPorts`, `VirtioSerial`, `VirtualPMem`, `FlexibleIov`,
  `SharedMemory` and `VirtualPci`. `HvSocket` is new in 2.1.
- READ (same page): `Scsi.Attachments` is keyed by LUN. An `Attachment` has
  `Type` (`VirtualDisk`, `Iso`, `PassThru`), `Path` and `ReadOnly`, and
  `AlwaysAllowSparseFiles` is new in 2.6.
- READ (same page): `HvSocketSystemConfig` has
  `DefaultBindSecurityDescriptor`, `DefaultConnectSecurityDescriptor` and
  `ServiceTable`. A service entry can set `AllowWildcardBinds`.
- READ (same page): `PauseOptions.SuspensionLevel` takes `Suspend`.
  `SaveOptions` takes `SaveType` `ToFile` and `SaveStateFilePath`.
  `SystemQuery` has `Ids`, `Names`, `Types` and `Owners`.
- READ (same page): `Properties` has `Id`, `Owner`, `RuntimeId` and `State`.
  Schema 2.3 is the Windows 10 SDK 2004, and 2.6 is the Windows 11 SDK 21H2.
- READ: WSL sets `SchemaVersion` 2.3, `ShouldTerminateOnLastHandleClosed` true
  and `StopOnReset` true. It sets all three memory flags
  (https://github.com/microsoft/WSL/blob/master/src/windows/service/exe/WslCoreVm.cpp,
  function `GenerateConfigJson`, read 2026-10-14).
- READ (same file): WSL attaches its system disks with `ReadOnly` true and
  `AlwaysAllowSparseFiles` true, in that 2.3 document.
- READ: for a SCSI root, hcsshim boots with `root=/dev/sda ro rootwait
  init=/init` and no `InitRdPath`. It appends
  `initcall_blacklist=virtio_vsock_init`, `8250_core.nr_uarts=0` and `pci=off`
  (https://github.com/microsoft/hcsshim/blob/main/internal/uvm/create_lcow.go,
  read 2026-10-14).
- Design: docs/engines.md says that `AlwaysAllowSparseFiles` has no reference
  user and needs schema 2.6. The WSL reading above shows a reference user on
  read-only disks, in a 2.3 document. A shrink after a guest trim still has no
  reference user.

## Approach

1. Write crates/box-vmm/src/hcs/schema.rs. The types are plain serde, with no
   OS call, so the tests build and run on any host.
2. Use `#[serde(rename_all = "PascalCase", deny_unknown_fields)]` on each
   document type. A golden document with one extra field then fails to parse.
3. Define only these types:

   ```rust
   pub struct ComputeSystem { owner: Owner, schema_version: Version,
       should_terminate_on_last_handle_closed: bool, virtual_machine: VirtualMachine }
   pub struct VirtualMachine { stop_on_reset: bool, chipset: Chipset,
       compute_topology: Topology, devices: Devices, restore_state: Option<RestoreState> }
   pub struct Chipset { linux_kernel_direct: LinuxKernelDirect }
   pub struct LinuxKernelDirect { kernel_file_path: StorePath, kernel_cmd_line: KernelCmdLine }
   pub struct Topology { memory: Memory, processor: Processor }
   pub struct Memory { size_in_mb: u64, allow_overcommit: bool,
       enable_deferred_commit: bool, enable_cold_discard_hint: bool }
   pub struct Processor { count: u32 }
   pub struct Devices { scsi: BTreeMap<ControllerKey, Scsi>, hv_socket: HvSocket }
   pub struct Scsi { attachments: BTreeMap<Lun, Attachment> }
   pub struct Attachment { r#type: VirtualDiskOnly, path: StorePath, read_only: bool,
       always_allow_sparse_files: Option<bool> }
   pub struct HvSocket { hv_socket_config: HvSocketSystemConfig }
   pub struct HvSocketSystemConfig { default_bind_security_descriptor: Sddl,
       default_connect_security_descriptor: Sddl }
   pub struct PauseOptions { suspension_level: SuspendOnly }
   pub struct SaveOptions { save_type: ToFileOnly, save_state_file_path: StorePath }
   pub struct SystemQuery { owners: [Owner; 1] }
   ```

4. `Owner` serializes only the constant `agent-box`. `VirtualDiskOnly`,
   `SuspendOnly` and `ToFileOnly` each have one value.
5. `StorePath` has no constructor from a string. Only the helper builds it,
   from the store root and a digest or an instance id.
6. `Sddl` has no constructor from a string. T-104 builds it from a SID.
7. `KernelCmdLine` has no constructor from a string. It joins a fixed list:
   `root=/dev/sda ro rootwait rootfstype=<erofs|ext4> init=/usr/sbin/box-guest`.
8. The list goes on with `reboot=k panic=1 nomodule 8250_core.nr_uarts=0
   pci=off initcall_blacklist=virtio_vsock_init`. docs/guest.md owns these
   parameters, and T-105 proves the boot.
9. This type has no string input. For T-256, the test
   `kernel_cmd_line_carries_no_value` scans each golden document with the
   helper of T-256.
10. Put the base disk at LUN 0 and the data disk at LUN 1 of controller `0`.
    hcsshim relies on LUN 0 as `/dev/sda`. T-061 measures the device names.
11. The result type `Properties` is lenient: it ignores unknown fields, because
    HCS can add result fields. It reads `Id`, `Owner`, `RuntimeId` and `State`
    only.
12. Write `min_schema_version(&ComputeSystem)` from a table of the
    `NewInVersion` of each field that the types can emit. A test checks that
    the document version is at least that value.
13. Set `SchemaVersion` 2.3, as WSL does. When `AlwaysAllowSparseFiles` is
    present, the table asks for 2.6.
14. The live test of step 18 checks whether HCS accepts that field in a 2.3
    document, as WSL sends it. Record the answer in docs/engines.md.
15. Format the compute system id as the 32 hex digits of the instance id. If
    HCS refuses that form, use the GUID form 8-4-4-4-12 of the same 16 bytes.
16. Keep golden documents in crates/box-vmm/tests/golden/hcs/: the compute
    system, the pause options, the save options and the query. The builder
    output must equal them byte for byte.
17. A test reads schema.rs with `include_str!`. It fails when the source names
    `NetworkAdapters`, `Plan9`, `VirtualSmb`, `ComPorts`, `VirtioSerial`,
    `VirtualPMem`, `VirtualPci`, `ServiceTable` or `AllowWildcardBinds`.
18. Write the live test `schema_accepted` in crates/box-vmm/tests/hcs_live.rs.
    It creates a compute system from the golden document with a test kernel,
    and terminates it.

## Prove

```sh
cargo test -p box-vmm hcs::schema::
cargo test -p box-vmm --test hcs_live -- --ignored schema_accepted
```

Expected: the first command passes on any host. Each golden document matches,
each document with an extra field is refused, and the source scan passes. The
live test creates and terminates a compute system from the golden document.

It records whether the id form and the 2.3 document with
`AlwaysAllowSparseFiles` were accepted. Host: any host for the first command;
Windows 11 with Hyper-V as administrator for the second.

## Start condition

T-033. The live test needs T-102 and a kernel: T-060, or a stock WSL kernel for
early runs.

# T-102: computecore calls

**Source:** The planning brief of 2026-10-14, section 4.2. The HCS API
reference, read 2026-10-14. The windows crate documentation of
`Win32::System::HostComputeSystem`.
**Category:** feature
**Milestone:** M4
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

Every HCS call is asynchronous: it starts an operation, and the caller waits
for its result. A wait with no limit hangs the helper. An error with only a
number hides the call that failed. A process that links computecore.dll at
load time does not start on a host without it, so the probe cannot report the
gap.

## Premise

- READ: the API has these operation calls: `HcsCreateOperation`,
  `HcsWaitForOperationResult`, `HcsCancelOperation` and `HcsCloseOperation`
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/reference/apioverview,
  read 2026-10-14).
- READ (same page): the compute system calls include `HcsCreateComputeSystem`,
  `HcsOpenComputeSystem`, `HcsStartComputeSystem`, `HcsPauseComputeSystem`,
  `HcsResumeComputeSystem` and `HcsSaveComputeSystem`.
- READ (same page): they also include `HcsTerminateComputeSystem`,
  `HcsGetComputeSystemProperties`, `HcsEnumerateComputeSystems`,
  `HcsSetComputeSystemCallback`, `HcsWaitForComputeSystemExit` and
  `HcsCloseComputeSystem`.
- READ: `HcsOpenComputeSystem(id, requestedAccess, computeSystem)` takes
  `GENERIC_ALL` as the only valid access
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/reference/hcsopencomputesystem,
  read 2026-10-14).
- READ: `HcsEnumerateComputeSystems(query, operation)` takes an optional
  `SystemQuery`. Its result document is an array of `Properties`
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/reference/hcsenumeratecomputesystems,
  read 2026-10-14).
- READ: the security descriptor argument of `HcsCreateComputeSystem` is
  reserved and must be NULL
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/reference/hcscreatecomputesystem,
  read 2026-10-14).
- READ: the sample opens a system by id, pauses with `SuspensionLevel`
  `Suspend`, and saves with `SaveType` `ToFile`
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/reference/computesystemsample,
  read 2026-10-14).
- READ: the windows crate exposes these functions in
  `Win32::System::HostComputeSystem`, with no schema types
  (https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/HostComputeSystem/index.html,
  read 2026-10-14).
- READ: third-party callers saw an opaque 0x80070005 at `Construct`. The
  reports concern a third-party product, and no maintainer confirmed the fix
  (https://github.com/anthropics/claude-code/issues/39285).

## Approach

1. Write crates/box-vmm/src/hcs/computecore.rs, gated by `cfg(windows)`. Allow
   unsafe code in this module only, with a comment on each block.
2. Load computecore.dll at run time with `LoadLibraryExW` and
   `LOAD_LIBRARY_SEARCH_SYSTEM32`. Resolve each function with
   `GetProcAddress` into a typed function pointer table.
3. A missing DLL or function is the gap `engine-missing` (T-108), not a crash
   at process start.
4. Wrap the handles in owners with `Drop`: `Operation` closes with
   `HcsCloseOperation`, and `System` closes with `HcsCloseComputeSystem`.
5. Write one function per call. Each creates an operation, starts the call,
   and waits with `HcsWaitForOperationResult` and a limit.
6. The limits are design values: create 60 s, start 60 s, pause and resume
   30 s, terminate 30 s, enumerate and properties 10 s.
7. On a timeout, call `HcsCancelOperation` and return `HcsError::Timeout`.
   Never return a result from a cancelled operation.
8. `create(id, &ComputeSystem)` passes the serialized T-101 document and a NULL
   security descriptor.
9. `pause(system)` passes `{"SuspensionLevel":"Suspend"}`. `resume(system)`
   passes no options.
10. `save(system, path)` passes `SaveOptions`. Only T-184 calls it, and only if
    spike T-014 (d) passed.
11. `open(id)` passes `GENERIC_ALL`. `enumerate()` passes
    `{"Owners":["agent-box"]}` and parses the array of `Properties`.
12. `properties(system)` returns `RuntimeId` and `State`. Confirm which query
    document returns `RuntimeId`, and record it in docs/engines.md.
13. `wait_exit(system, limit)` calls `HcsWaitForComputeSystemExit`. `kill`
    (T-105) uses it after `HcsTerminateComputeSystem`.
14. `HcsSetComputeSystemCallback` is used only inside one helper operation,
    to log `HcsEventSystemExited` while the helper waits. The helper is not a
    long-lived listener.
15. `HcsError` holds the call name, the HRESULT and the result document text,
    capped at 4 KiB. Its display reads, for example, "HcsStartComputeSystem
    failed: 0x80070005".
16. Map the `HCS_E_*` constants of the windows crate to their names in the
    error text. Print other HRESULTs in hex.
17. The live test `computecore_cycle` creates a compute system from the T-101
    document. It starts, queries, pauses, resumes, terminates and enumerates.

## Prove

```sh
cargo test -p box-vmm hcs::computecore::
cargo test -p box-vmm --test hcs_live -- --ignored computecore_cycle
```

Expected: the unit tests pass with a fake function table. A planted timeout
calls cancel, and a missing function gives `engine-missing`.

The live test sees `State` `Running`, then `Paused`, then `Running`, and a
`RuntimeId`. After terminate, `wait_exit` returns, and `enumerate` no longer
lists the id. A planted bad document gives an error that names
`HcsCreateComputeSystem` and its HRESULT. Host: Windows 11 with Hyper-V as
administrator.

## Start condition

T-101.

# T-103: VHDX: read-only base and dynamic 30 GiB data disk

**Source:** The planning brief of 2026-10-14, section 4.2. docs/engines.md,
section "VHDX disks". The measurement of 2026-10-14 in docs/status.md.
**Category:** feature
**Milestone:** M4
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

Each instance needs a data disk of 30 GiB that costs little host space until
the guest writes. The per-VM worker must read the kernel and the base and
write the data disk, and nothing else. A wrong ACL fails with an opaque error;
a wide one lets one VM read another instance's disk.

## Premise

- MEASURED on 2026-10-14 on the operator's Windows 11 Pro build 26200 host,
  one run with a filtered administrator token. A new dynamic VHDX of 30 GiB
  was a 4 MiB file ([docs/status.md](../docs/status.md)).
- READ: virtdisk.h defines `VIRTUAL_STORAGE_TYPE_DEVICE_VHDX` as 3 and
  `CREATE_VIRTUAL_DISK_VERSION_2`, whose parameters include `MaximumSize`,
  `BlockSizeInBytes`, `SectorSizeInBytes` and `PhysicalSectorSizeInBytes`.
- READ (same header): `CREATE_VIRTUAL_DISK_FLAG_NONE` makes a dynamic disk, and
  `CREATE_VIRTUAL_DISK_FLAG_FULL_PHYSICAL_ALLOCATION` a fixed one. The header
  is Windows SDK 10.0.22621.0, um/virtdisk.h, lines 68, 240, 265-268, 319 and
  323.
- READ: `HcsGrantVmAccess(vmId, filePath)` adds an ACL entry for the per-VM
  worker account, which HCS derives from the compute system id
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/reference/hcsgrantvmaccess,
  read 2026-10-14).
- READ: `HcsGrantVmGroupAccess(filePath)` grants the VM group read-only access
  to a file. `HcsRevokeVmAccess` removes a grant. The group call needs Windows
  10 version 2004 or later
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/reference/hcsgrantvmgroupaccess,
  read 2026-10-14).
- READ: third-party callers fixed an opaque 0x80070005 at `Construct` with
  ACLs for SYSTEM and `NT VIRTUAL MACHINE\Virtual Machines`, plus
  `HcsGrantVmAccess`. No maintainer confirmed it
  (https://github.com/anthropics/claude-code/issues/39285).
- READ (WSL, see T-101): WSL grants VM access on its disks and revokes the
  grants when it detaches them.
- Not known: whether two compute systems can attach one VHDX with `ReadOnly`
  true at the same time. T-065 measures it.

## Approach

1. Write crates/box-vmm/src/hcs/virtdisk.rs, gated by `cfg(windows)`. Allow
   unsafe code in this module only, with a comment on each block.
2. `create_data_disk(path, gib)`: call `CreateVirtualDisk` with
   `VIRTUAL_STORAGE_TYPE_DEVICE_VHDX` and `VIRTUAL_STORAGE_TYPE_VENDOR_MICROSOFT`.
3. Pass `VIRTUAL_DISK_ACCESS_NONE`, `CREATE_VIRTUAL_DISK_FLAG_NONE` and
   `CREATE_VIRTUAL_DISK_PARAMETERS` version 2. Set `MaximumSize` to `gib *
   1073741824`, and leave block and sector sizes 0 for the defaults.
4. The default makes `MaximumSize` 32212254720. Close the handle at once. The
   host never attaches the disk, never mounts it and never reads it.
5. Create the file in a new instance directory
   `STORE\instances\<id>\data.vhdx`, and refuse an existing file. The helper
   operation `create` runs this step.
6. Read `GetVirtualDiskInformation` with `GET_VIRTUAL_DISK_INFO_SIZE` once
   after create. Record the virtual size and the physical size.
7. Base image: the helper keeps one read-only VHDX per digest,
   `STORE\images\<sha256>.vhdx`. T-065 decides how the VHDX gets a digest
   that a reviewer can check.
8. Call `HcsGrantVmGroupAccess` on the base VHDX and the kernel once, at
   import. They are read-only for every VM, and no per-VM grant is needed for
   them.
9. Set the read-only attribute on the base and the kernel files. Attach the
   base with `ReadOnly` true (T-101).
10. Call `HcsGrantVmAccess(id, data.vhdx)` at each start, and
    `HcsRevokeVmAccess` at each kill and delete.
11. Before each grant, open the file with `FILE_FLAG_OPEN_REPARSE_POINT` and
    refuse a reparse point. Read the final path from the handle and refuse a
    path outside the store.
12. Store ACL, proposal: `D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)`, plus traverse
    only for `NT VIRTUAL MACHINE\Virtual Machines` (S-1-5-83-0).
13. File access for a VM comes only from the grants. Measure the narrowest ACL
    that avoids 0x80070005, and write it into docs/engines.md.
14. Verify a revoke with `GetNamedSecurityInfoW`: no ACE for the SID of that
    VM remains on the file.
15. Do not set `AlwaysAllowSparseFiles` on the data disk until spike T-014 (e)
    and T-110 decide it.

## Prove

```sh
cargo test -p box-vmm --test hcs_live -- --ignored vhdx_create
cargo test -p box-daemon --test windows_boot -- --ignored data_disk_size
```

Expected: the first test creates a data disk as LocalSystem. The file takes
about 4 MiB on the host, and the virtual size reads 32212254720.

The boot test starts an instance with the narrow store ACL and no 0x80070005.
In the guest, `blockdev --getsize64 /dev/sdb` prints 32212254720. After kill,
the data disk ACL has no entry for that VM.

Host: Windows 11 with Hyper-V as administrator.

## Start condition

T-100 and T-102. The boot test needs T-105.

# T-104: HvSocket configuration

**Source:** The planning brief of 2026-10-14, sections 4.2 and 7. The
adversarial review, items 4 and 5. docs/engines.md, section "HvSocket".
**Category:** feature
**Milestone:** M4
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

Every host channel to the guest is a Hyper-V socket. Without a per-VM
descriptor, any process that can bind the service id can listen for the guest.
A registry entry or a wildcard bind opens the service to every VM on the host.
A socket without the suspend option drops its session at each pause.

## Premise

- READ: `HvSocketSystemConfig` has `DefaultBindSecurityDescriptor`,
  `DefaultConnectSecurityDescriptor` and `ServiceTable`
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/schemareference,
  read 2026-10-14).
- READ (adversarial review, item 4): the default bind descriptor covers a bind
  to an unlisted service of that VM, not a wildcard bind. A wildcard bind needs
  `AllowWildcardBinds`.
- READ: WSL builds `D:P(A;;FA;;;SY)(A;;FA;;;` plus the user SID plus `)` for
  bind and connect
  (https://github.com/microsoft/WSL/blob/master/src/windows/service/exe/WslCoreVm.cpp,
  read 2026-10-14).
- READ: hcsshim uses `D:P(A;;FA;;;SY)(A;;FA;;;BA)`
  (https://github.com/microsoft/hcsshim/blob/main/internal/uvm/create_lcow.go,
  read 2026-10-14).
- READ: a service registered under
  `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Virtualization\GuestCommunicationServices`
  needs an administrator
  (https://learn.microsoft.com/en-us/virtualization/hyper-v-on-windows/user-guide/make-integration-service,
  read 2026-10-14).
- READ: `HVSOCKET_CONNECTED_SUSPEND` (0x04) keeps a socket connected across a
  VM pause. `HVSOCKET_CONNECT_TIMEOUT` (0x01) has a maximum of 300000 ms
  (Windows SDK 10.0.22621.0, header shared/hvsocket.h, lines 36-57).
- READ (same header, lines 76-120): `HV_GUID_WILDCARD` is the zero GUID.
  `HV_GUID_CHILDREN` and `HV_GUID_PARENT` have their own values.
- READ (same header, lines 135-156): the vsock template is
  `00000000-facb-11e6-bd58-64006a7986d3`, with the port in the first field.

## Approach

1. Build the descriptor in crates/box-vmm/src/hcs/sddl.rs:
   `D:P(A;;FA;;;SY)(A;;FA;;;<operator SID>)` for both bind and connect.
2. Take the operator SID from the token of the authenticated pipe client
   (T-100), never from a request field. Render it with
   `ConvertSidToStringSidW`.
3. Accept only a SID of the form `S-1-5-21-...` of a single account. Refuse a
   group or a well-known SID, for example S-1-1-0, S-1-5-11 or S-1-5-32-545.
4. The `Sddl` type of T-101 has one constructor, from that checked SID.
5. Never write `ServiceTable`. Never write a registry key under
   `GuestCommunicationServices`.
6. The daemon side is box-transport (T-025). It binds one listener per runtime
   id and port, for ports 5002 and 5003, with the service id of the port.
7. Never bind `HV_GUID_WILDCARD`, `HV_GUID_CHILDREN`, `HV_GUID_LOOPBACK` or
   `HV_GUID_BROADCAST`.
8. Set `HVSOCKET_CONNECTED_SUSPEND` to 1 at level `HV_PROTOCOL_RAW` on every
   listener, every accepted socket and every connected socket.
9. Set `HVSOCKET_CONNECT_TIMEOUT` on each connect, from the boot deadline of
   T-105.
10. Write the live test `hvsocket_access` in
    crates/box-daemon/tests/windows_boot.rs. It snapshots the subkeys of
    `GuestCommunicationServices` before and after a start.
11. Run the test as the operator. Then run its second half as the second local
    account of T-100.
12. Measure whether a process of the operator at low integrity can connect.
    Record the answer in docs/engines.md.

## Prove

```sh
cargo test -p box-vmm hcs::sddl::
cargo test -p box-daemon --test windows_boot -- --ignored hvsocket_access
```

Expected: the unit tests refuse a group SID, a well-known SID and a malformed
SID. In the live test, the daemon as the operator connects to port 5000 and
accepts on port 5002.

The second account gets `WSAEACCES` on a connect to port 5000 and on a bind of
port 5002 for that VM. The subkeys of `GuestCommunicationServices` are the same
before and after. Host: any host for the unit tests; Windows 11 with Hyper-V as
administrator for the live test.

## Start condition

T-025 and T-101. The live test needs T-105.

# T-105: Start, boot handshake, stop and kill on Windows

**Source:** The planning brief of 2026-10-14, sections 4.2 and 7.
docs/engines.md, sections "Pause, save and stop on Windows" and "Owner,
lifetime and re-adoption". docs/protocol.md, section "Boot order and deny
first".
**Category:** feature
**Milestone:** M4
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

A compute system is ephemeral: each start creates a new one from the
agent-box record. The boot must reach the guest agent within a bound, and a
stop must leave no compute system, no grant and an intact disk. Many earlier
entries prove their Windows live checks only once an instance boots here.

## Premise

- READ: compute systems are ephemeral. After a stop, the caller creates a new
  one to start the VM again
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/overview, read
  2026-10-14).
- READ (engine evaluation, HCS tutorial and sample): a VM keeps running after
  its creator closes the handle and exits. That holds when the VM is not set to
  terminate on the last handle.
- READ: WSL and hcsshim both set `ShouldTerminateOnLastHandleClosed` true. So
  a VM that outlives the helper has no Linux-guest reference user, and spike
  T-014 (b) tests it.
- READ: hcsshim boots a SCSI root through `LinuxKernelDirect` with no initrd
  (https://github.com/microsoft/hcsshim/blob/main/internal/uvm/create_lcow.go,
  read 2026-10-14).
- READ: WSL sets `StopOnReset` true, so a guest reset ends the compute system
  (WslCoreVm.cpp, see T-101).
- Design (docs/engines.md): agent-box does not use `HcsShutDownComputeSystem`.
  The guest power-off on port 5000 is the soft stop on both engines.
- Design (main-session ruling on live proves): M1 and M2 entries list their
  Windows live checks here, and this entry names them.

## Approach

1. The helper operation `start` in crates/box-helper/src/windows/start.rs
   checks the request (T-023). It loads the record and opens the id with
   `HcsOpenComputeSystem`.
2. An existing compute system for the id is `REFUSAL` `busy`. Two starts cannot
   race, because the helper holds the instance lock.
3. Build the T-101 document from the record and the request: cpus, memory, the
   kernel and base digests, and the descriptor of T-104.
4. Grant VM access to the data disk (T-103). Call `HcsCreateComputeSystem`,
   then `HcsStartComputeSystem`.
5. Read `RuntimeId` from the properties and reply with endpoint tag 2 and the
   runtime id. Close the handle; the system keeps running only if T-014 (b)
   passed.
6. In crates/box-daemon/src/engine/hcs.rs, bind the listeners for ports 5002
   and 5003 on that runtime id (T-104). Then connect to port 5000 with retries
   until the boot deadline.
7. Use the boot deadline of T-088: 30 s, a design value. T-181 measures the
   boot time on both engines.
8. Send `HELLO` with `resume` false, expect `WELCOME`, then provision (T-041).
   The state goes to `running`.
9. When the deadline passes with no `WELCOME`, the daemon calls the helper
   `kill`. The error names the missing handshake and the 30 s limit.
10. `stop(id, grace)`: on a paused instance, resume first. Send `POWEROFF` with
    `reason` 1, expect `POWEROFF_ACK`, and wait up to `grace` for the end of the
    control connection.
11. Always end with the helper `kill`. It is idempotent. It opens the id,
    calls `HcsTerminateComputeSystem` when the system exists, and waits with
    `HcsWaitForComputeSystemExit`.
12. `kill` then revokes the grant of the data disk and checks that
    `HcsEnumerateComputeSystems` no longer lists the id. It replies with those
    absences.
13. Measure what the guest power-off call does under HCS. With `StopOnReset`
    true, the reboot call ends the system. Use the same last call as on
    Firecracker (T-088).
14. Confirm the boot with no initrd and the root on SCSI LUN 0. Record the
    device names of the two disks for T-061.
15. Write the live tests in crates/box-daemon/tests/windows_boot.rs, marked
    `#[ignore]`. They open sessions on port 5001 directly. The CLI attach is
    T-172.
16. The file holds the boot test of this entry, `boot_handshake_stop`, and the
    Windows live checks of these entries:

    | entry | test | check |
    | --- | --- | --- |
    | T-025 | `hvsock_live` | connect and listen through a real compute system |
    | T-031 | `forged_event_refused` | a uid 1000 process without the nonce is refused, and the counter grows |
    | T-040 | `pid1_boot_poweroff` | box-guest boots as PID 1, answers the handshake and powers off |
    | T-041 | `no_secret_on_disk` | after provisioning, the raw VHDX bytes of a stopped instance hold no marker |
    | T-043 | `data_disk_survives_restart` | a marker survives stop and start, and the base digest stays |
    | T-044 | `boot_time_sync` | the guest clock is within 1 s of the host after boot |
    | T-061 | `vsock_transport` | hv_sock works with the T-060 kernel, with and without the blacklist |
    | T-062 | `boot_base` | the base VHDX reaches the handshake |
    | T-063 | `kage_version` | `kage --version` as uid 1000 prints the pin |
    | T-064 | `boot_variants` | each variant prints its toolchain version |
    | T-065 | `vhdx_shared_readonly` | two compute systems attach one base VHDX read-only |
    | T-256 | `no_secret_in_cmdline` | the guest /proc/cmdline and the HCS document hold no provisioned value |

17. The `no_secret_on_disk` scan reads the VHDX file as bytes. It never
    attaches or mounts the disk.

## Prove

```sh
cargo test -p box-daemon --test windows_boot -- --ignored
```

Expected: `boot_handshake_stop` reaches `WELCOME` within the deadline, stops
the instance, and finds no compute system with the id and no grant. A second
stop succeeds and does nothing. Every test of the table passes. A planted
kernel that never answers ends in `stopped` after the deadline.

Host: Windows 11 with Hyper-V as administrator for the one-time install; the
test itself runs as the operator.

## Start condition

T-060, T-062 (the VHDX copy, T-065), and T-100 to T-104. Spike T-014 (a) and
(b) must pass. The live checks of the table also need their own entries.

# T-106: Pause and resume on Windows

**Source:** The planning brief of 2026-10-14, section 4.2. The adversarial
review, item 5. docs/engines.md, section "Pause, save and stop on Windows".
**Category:** feature
**Milestone:** M4
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

A Hyper-V socket disconnects at a VM pause by default. A pause without the
socket option ends every session, the event channel and the kage bridge. No
Linux-guest reference user pauses an HCS compute system, so the behaviour
needs a test on this engine.

## Premise

- READ: `HcsPauseComputeSystem` takes `{"SuspensionLevel": "Suspend"}`, and
  `HcsResumeComputeSystem` takes no options
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/reference/computesystemsample,
  read 2026-10-14).
- READ: a non-zero `HVSOCKET_CONNECTED_SUSPEND` keeps a socket connected
  across a VM pause, and the default disconnects (Windows SDK 10.0.22621.0,
  header shared/hvsocket.h, lines 50-57).
- READ: WSL and hcsshim never call `HcsPause`. Spike T-014 (c) tests pause and
  resume with the option.
- Not verified: whether an accepted socket inherits the option from its
  listener (docs/engines.md, "Not verified on Windows").
- Design (docs/protocol.md): the liveness deadline does not run during a
  pause. After resume, the daemon sends `TIME_SET`.

## Approach

1. Write the helper operations `pause` and `resume` in
   crates/box-helper/src/windows/pause.rs. Each opens the id with
   `HcsOpenComputeSystem` and calls the T-102 function.
2. Read `State` after the call. Require `Paused` after a pause and `Running`
   after a resume. Anything else is `REFUSAL` `verification_failed`.
3. Pause of a stopped instance is `wrong_state`. A second pause succeeds and
   does nothing.
4. Pass no `HostedNotification`. The sample passes it only before a save.
5. In the daemon, set `paused` and stop the liveness deadline. On resume, set
   `running`, start the deadline, and send `TIME_SET` (T-044).
6. Check the suspend option on each live socket before the pause: each
   listener, each accepted socket and each connected socket. A socket without
   it fails the pause before any HCS call.
7. Write the live test `pause_keeps_sessions` in
   crates/box-daemon/tests/windows_boot.rs. It opens a PTY session on port
   5001, runs a read loop, pauses for 60 s and resumes.
8. After resume, the same PTY session must echo a new line. The event
   channel must show no new authentication.
9. Add `pause_time_sync`: pause 600 s and resume. The guest clock is within 1
   s of the host after `TIME_ACK`. This is the Windows pause half of T-044.
10. Add the control `pause_without_option`, which clears the option on one test
    socket. That socket must drop, so that the test can fail.

## Prove

```sh
cargo test -p box-daemon --test windows_boot -- --ignored pause_keeps_sessions pause_time_sync pause_without_option
```

Expected: after a 60 s pause, the PTY session still echoes, and no channel
reconnected. The clock offset after a 600 s pause is within 1 s. The control
socket without the option drops. Host: Windows 11 with Hyper-V as
administrator.

## Start condition

T-105. Spike T-014 (c) must pass; if it fails, T-111 decides.

# T-107: Inventory by owner, re-adoption, orphans, delete with verified absence

**Source:** The planning brief of 2026-10-14, section 4.2. docs/engines.md,
section "Owner, lifetime and re-adoption". docs/protocol.md, section
"Re-adoption and rotation".
**Category:** feature
**Milestone:** M4
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

A daemon restart must not stop running instances. The daemon must find each
compute system again, tie it to its record, and key its channels again. Three
leftovers leak state into the next instance. They are an unclaimed compute
system, a grant that outlives its VM, and a disk left after delete.

## Premise

- READ: `HcsEnumerateComputeSystems` takes a `SystemQuery` with `Owners`, and
  returns an array of `Properties` with `Id`, `RuntimeId` and `State`
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/reference/hcsenumeratecomputesystems,
  read 2026-10-14).
- READ: a VM that its creator left running can be opened again by id with
  `HcsOpenComputeSystem` (engine evaluation, HCS sample).
- READ: `HcsRevokeVmAccess` removes the ACL entry that a grant added (HCS API
  reference, see T-102).
- Design (docs/engines.md): `Running` maps to `running` and `Paused` to
  `paused`. A record with no compute system is `stopped`. A failed query is
  `unknown`.
- Design (docs/engines.md): the daemon terminates an orphan and keeps its VHDX
  for the operator.
- Design (main-session ruling on verbs): delete refuses a running or paused
  instance unless `--force` stops it first. In `unknown`, only stop and delete
  are accepted.

## Approach

1. Write the helper operation `inventory` in
   crates/box-helper/src/windows/inventory.rs. Call `HcsEnumerateComputeSystems`
   with `{"Owners":["agent-box"]}`.
2. For each listed id, open it and read the properties again. Join the result
   with the helper records in `STORE\instances\<id>\record`.
3. A compute system with no helper record is an entry with `claimed` false.
   The fields `pid` and `start_time` are 0 on Windows.
4. Owner is a free string, and any administrator can set it. That is inside
   the trust boundary, because only administrators can call HCS.
5. In crates/box-daemon/src/readopt.rs, the shared re-adoption code of T-090
   joins the daemon records with the inventory.
6. For each `running` or `paused` instance, bind the listeners on the runtime
   id again (T-104). Send `HELLO` with `resume` true, then `ROTATE` the event
   nonce and the egress token.
7. A compute system whose id has no daemon record is unclaimed. The daemon
   calls the helper `kill`, keeps its VHDX, and reports the id and the path.
8. Write the helper operation `delete` in crates/box-helper/src/windows/delete.rs.
   A running or paused compute system is `REFUSAL` `wrong_state`.
9. `delete` then revokes the grant of the data disk and deletes `data.vhdx`,
   the record and the instance directory. A save file goes too, once T-184
   makes one.
10. Verify each absence: the enumeration lacks the id. Before the file goes,
    `GetNamedSecurityInfoW` on the data VHDX shows no ACE for that VM.
    - The kernel and the base have a group grant from import (T-103), not a
      per-VM ACE. So the check reads the data VHDX only, as docs/engines.md
      gives.
11. The instance directory must not exist. Reply with the absence items 3, 4,
    6, 7 and 8 of docs/protocol.md.
12. In the daemon, delete the instance record, the encrypted per-instance
    secrets and the event log, as on Linux (T-091).
13. Write the live tests `readopt_three`, `orphan_removed` and
    `delete_verified_absence` in crates/box-daemon/tests/windows_boot.rs.
14. `delete_verified_absence` uses the marker scan of T-091. It scans
    `%LOCALAPPDATA%\agent-box` and the store, and reads files as bytes only.

## Prove

```sh
cargo test -p box-daemon --test windows_boot -- --ignored readopt_three orphan_removed delete_verified_absence
```

Expected: `readopt_three` starts three instances and ends the daemon with
`TerminateProcess`. After the restart, all three are `running`, and a PTY
session in each reads a larger `/proc/uptime`. A frame with the old event nonce
is refused.

`orphan_removed` sees the compute system of a fourth instance without a daemon
record terminated, and its VHDX kept and reported. After delete, the scan finds
no marker, the enumeration lacks the id, and no grant remains.

Host: Windows 11 with Hyper-V as administrator for the install; the tests run
as the operator.

## Start condition

T-105 and T-106. Spike T-014 (b) must pass.

# T-108: Windows probe and preflight

**Source:** The planning brief of 2026-10-14, section 4.2. The adversarial
review, items 10 and 11. docs/engines.md, section "Preflight facts", and
docs/cli.md, the gap and note tables.
**Category:** feature
**Milestone:** M4
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

On Windows the management services can answer while the hypervisor never
started. A probe that reads installed features then reports an engine that
cannot boot. The operator needs each fact with its evidence, and a named gap
when the engine cannot run.

## Premise

- READ: a third-party product saw 0x80370102, "Hyper-V is not installed", on
  hosts where the Hyper-V management service answered queries. One reporter
  found NX off in the firmware (https://github.com/anthropics/claude-code/issues/60371).
- MEASURED on 2026-10-14 on the operator's Windows 11 Pro build 26200 host, one
  run with a non-elevated token. `HypervisorPresent` was True, and
  `hcsdiag list` exited 27 ([docs/status.md](../docs/status.md)).
- MEASURED in the same run: the services vmcompute, vmms and hns ran, and
  computecore.dll and WinHvPlatform.dll were present.
- READ (adversarial review, item 11): no evidence exists either way for a
  third-party HCS caller on Windows 11 Home.
- Design (docs/cli.md): the gaps `no-hypervisor`, `engine-missing`,
  `helper-missing` and `helper-refused`, and the note
  `windows-home-unverified`.
- Design (main-session ruling on live proves): the Prove below uses the
  probe tests and the startup report of the daemon (T-028). `agent-box doctor`
  comes later (T-175).

## Approach

1. Split the probes by privilege. The daemon reads the facts that need none.
   The helper operation `probe` reads the rest. Write which side reads each
   fact into docs/engines.md.
2. Daemon, in crates/box-daemon/src/engine/hcs_probe.rs: query
   `SELECT HypervisorPresent FROM Win32_ComputerSystem` in `ROOT\CIMV2` through
   WMI COM. False is the gap `no-hypervisor`.
3. Daemon: query the System event log with `EvtQuery` for the provider
   `Microsoft-Windows-Hyper-V-Hypervisor`, for events after the last boot.
4. Record which event ids mean "started" and "failed to start". They were not
   read in this session. No start event after boot is `no-hypervisor`.
5. Daemon: read the edition with `GetProductInfo` and the build with
   `RtlGetVersion`. Home is the note `windows-home-unverified` (T-109).
6. Daemon: query `Win32_OptionalFeature` for Virtual Machine Platform and
   Hyper-V. Confirm the feature names with `dism /online /get-features` and
   record them.
7. Daemon: read the state of the service vmcompute with
   `QueryServiceStatusEx`. A stopped service is a fact, not a gap; the helper
   probe decides.
8. Daemon: connect to the helper pipe. No pipe after a service start is
   `helper-missing`. A refusal is `helper-refused`.
9. Helper, in crates/box-helper/src/windows/probe.rs: load computecore.dll and
   resolve every function (T-102). A failure is `engine-missing`.
10. Helper: call `HcsEnumerateComputeSystems` with no query. An error is the
    gap `engine-missing`, with the HRESULT as evidence.
11. Helper: read the store ACL and the free space of the store volume. A
    missing store is `engine-missing`, with the advice to run setup.
12. Map the facts to a `CapabilityReport` (T-032). The daemon prints it through
    the startup report of T-028. On a gap it exits with the code of
    docs/cli.md: 10 for the engine, 11 for the helper.
13. Write pure tests that map planted facts to gaps and notes.

## Prove

```sh
cargo test -p box-daemon engine::hcs_probe::
cargo test -p box-helper probe::
cargo test -p box-daemon --test windows_probe -- --ignored probe_report
```

Expected: each planted fact gives its gap or note in the unit tests. On the
Hyper-V host, the startup report names every fact of the Approach with its
evidence, and the daemon runs.

With the helper service removed, the report names `helper-missing` and the
daemon exits 11. Host: any host for the unit tests; Windows 11 with Hyper-V as
administrator for the live test.

## Start condition

T-028, T-032, T-100 and T-102.

# T-109: Windows 11 Home with Virtual Machine Platform

**Source:** The planning brief of 2026-10-14, section 4.2. The adversarial
review, item 11. The question Q6 in [TODO/PROGRESS.md](PROGRESS.md).
**Category:** measurement
**Milestone:** M4
**Priority:** P2
**Effort:** S
**Status:** open

## Problem

Many operators run Windows 11 Home. WSL2 runs there with Virtual Machine
Platform only, but that does not prove that a third-party service can call
HCS. agent-box must not promise Home support, or refuse it, without a
measurement.

## Premise

- READ: WSL2 runs on Windows 11 Home. It needs the Virtual Machine Platform
  component, not the full Hyper-V feature
  (https://learn.microsoft.com/en-us/windows/wsl/faq, read 2026-10-14).
- READ: the HCS overview names server releases and Windows 10 1809. It says
  nothing about client editions or third-party callers
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/overview, read
  2026-10-14).
- Inference of the engine evaluation: the HCS path of WSL probably works on
  Home. Whether the group Hyper-V Administrators exists there is not known.
- Design: spike T-014 has five items, (a) to (e), and runs on Windows 11 Pro.
  This entry runs the same program on Home.

## Approach

1. Get a Windows 11 Home host with Virtual Machine Platform on and the full
   Hyper-V feature absent. Record the build, the hardware, and whether the
   host is itself a VM.
2. A Home VM under Hyper-V needs nested virtualization. Record it as nested,
   and repeat on a physical Home host before a support claim.
3. Run `net localgroup "Hyper-V Administrators"`, and record whether the group
   exists.
4. Run the T-014 spike program as an administrator. Record (a) to (e), each
   with pass or fail and the HRESULT.
5. Install the helper service of T-100, and run the spike operations through
   it as LocalSystem. Record the result per call.
6. Write a table to docs/status.md: the edition, the build, the host and the
   date, and pass or fail per item.
7. Put the result before the operator for question Q6 in TODO/PROGRESS.md.
8. On a pass, Home can join the support claim by a ruling. On a fail, T-111
   decides the Home route.

## Prove

```sh
cargo run --manifest-path .work/hcs-spike/Cargo.toml --release -- all --json
cargo test -p box-helper --test pipe_live -- --ignored
```

Expected: the spike prints (a) to (e), each pass or fail with evidence, and
docs/status.md holds the table. The second command shows whether the helper
service works on Home. Host: Windows 11 Home with Virtual Machine Platform, as
administrator.

## Start condition

T-014. The helper part needs T-100.

# T-110: Memory reclaim and fstrim on Windows

**Source:** The planning brief of 2026-10-14, section 4.2. The engine
evaluation, the Windows reader. docs/engines.md, section "Memory and
processors".
**Category:** measurement
**Milestone:** M8
**Priority:** P2
**Effort:** S
**Status:** open

## Problem

Persistent instances idle for hours. Without reclaim, each one holds the
memory that its guest ever touched, and its data disk only grows. agent-box
copies the memory flags of WSL2, but nobody measured what they give to this
guest with this kernel.

## Premise

- READ: `AllowOvercommit` backs VM memory with the pagefile.
  `EnableDeferredCommit` charges commit on first access.
  `EnableColdDiscardHint` lets the guest trim pages that are not zero
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/schemareference,
  read 2026-10-14).
- READ: WSL sets all three flags (WslCoreVm.cpp, see T-101).
- Design: the guest kernel has `hv_balloon` with page reporting
  ([docs/guest.md](../docs/guest.md)).
- READ: `AlwaysAllowSparseFiles` is new in schema 2.6. WSL sets it on its
  read-only system disks in a 2.3 document (see T-101).
- Not measured: host memory per idle instance, and whether guest `fstrim` makes
  a dynamic VHDX smaller under HCS. Spike T-014 (e) gives the first answer.

## Approach

1. Boot one instance with 4096 MiB and record the working set and the private
   commit of its memory process after 5 minutes idle.
2. In the guest, allocate and touch 2 GiB, then free it. Record the same
   values after 1, 5 and 15 minutes.
3. Repeat with each memory flag off, one at a time, to see what each flag
   gives.
4. In the guest, write a 4 GiB file to the data disk, delete it and run
   `fstrim -v` on the data disk mount.
5. Record the file size and the allocated size of the VHDX before and after,
   with `GetFileSizeEx` and `GetCompressedFileSizeW`.
6. Repeat the fstrim case with `AlwaysAllowSparseFiles` true and false on the
   data disk.
7. Run 3 rounds per case. Write the host, the build, the guest kernel digest,
   the sample count and the date to docs/status.md.
8. Write the decisions into docs/engines.md: which flags stay, and whether
   the data disk sets `AlwaysAllowSparseFiles`.
9. If fstrim gives no space back, write that into docs/limits.md. Name the
   offline compaction of a stopped instance as an option for the operator.

## Prove

```sh
cargo test -p box-daemon --test windows_boot -- --ignored reclaim_bench
```

Expected: the test prints a table per case: the memory values after each wait,
and the VHDX sizes before and after fstrim. docs/status.md holds the table with
its conditions. Host: Windows 11 with Hyper-V as administrator.

## Start condition

T-105 and T-014.

# T-111: Windows fallbacks: Hyper-V VMMS through WMI, then OpenVMM

**Source:** The planning brief of 2026-10-14, section 4.2. The adversarial
review, missed options. docs/engines.md, section "Fallbacks and reopening
conditions".
**Category:** research
**Milestone:** backlog
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

HCS can fail a gate: a compute system that does not outlive the helper, a pause
that drops sockets, or no third-party access on Home. agent-box then needs a
recorded next step, not an improvised one. Each fallback costs isolation,
editions or features, and the operator must see the costs before a choice.

## Premise

- READ: Podman machine, among others, drives Hyper-V through the VMMS. Its
  preparation command creates registry entries in HKEY_LOCAL_MACHINE for
  Hyper-V vsock, and needs an administrator
  (https://docs.podman.io/en/latest/markdown/podman-system-hyperv-prep.1.html,
  read 2026-10-14).
- READ (adversarial review, missed options): VMMS VMs are persistent and
  registered, and Save-VM and Suspend-VM work with Linux guests. The worker is
  the same `vmwp.exe`.
- READ (same review): the costs are Pro, Enterprise or Education only, a Gen2
  UEFI boot with no `LinuxKernelDirect`, and one registry entry per hvsocket
  service.
- READ: OpenVMM speaks the hybrid vsock protocol of Firecracker, with the same
  vsock template
  (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:support/hybrid_vsock/src/lib.rs:16-22`).
- READ: the OpenVMM `SandboxProfile` trait has no implementation
  (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:support/mesh/mesh_process/src/lib.rs:283`).
- READ: its VHDX backend ignores unmap
  (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:vm/devices/storage/disk_vhdmp/src/lib.rs:825`).
  Its suspend and resume are not wired to a management interface
  (`microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:Guide/src/reference/openvmm/management.md:41-47`).
- MEASURED on 2026-10-14 on the operator's Windows 11 Pro build 26200 host, one
  run each. A WHP partition create worked at low integrity, and an AppContainer
  got `E_ACCESSDENIED` ([docs/status.md](../docs/status.md)).

## Approach

1. Start this entry only when HCS fails a gate: T-014 (b) or (c) on Pro, or
   T-109 on Home.
2. Option 1, VMMS through WMI v2: define a Gen2 VM with
   `Msvm_VirtualSystemManagementService` `DefineSystem`. Attach the VHDX disks
   on SCSI with `AddResourceSettings`, and add no network adapter.
3. Use `RequestStateChange` for start, pause and save. Map the states to the
   agent-box states, and record the codes from the WMI documentation.
4. The guest boots through UEFI. The image needs an EFI system partition with
   the kernel as an EFI stub, which changes T-060 and T-062.
5. Each guest port needs a registry entry under `GuestCommunicationServices`,
   written by setup as an administrator. List the entries and their removal.
6. VMMS gives `hibernate` through a save, if T-014 (d) fails on HCS.
7. Option 2, OpenVMM on WHP: it runs on Home with no administrator at run time.
   Its transport is the box-transport Unix module of T-024 over `AF_UNIX` on
   Windows.
8. OpenVMM has no sandbox, so agent-box must build one. The parts are a
   dedicated local account, a job object with kill-on-close and memory limits,
   a low-integrity token, and process mitigation policies.
9. List what OpenVMM lacks: memory reclaim, TRIM on VHDX, a save call in its
   API, signed binaries. agent-box would build and pin openvmm.exe itself.
10. Write both options with their isolation, editions, features and the work
    each needs, into docs/research/engines-2026-10.md.
11. Recommend one option to the operator in TODO/PROGRESS.md. The operator
    writes the decision into docs/decisions.md.

## Prove

```sh
cargo xtask check docs
cargo todo check
```

Expected: both commands exit 0. docs/research/engines-2026-10.md has the
comparison, and docs/decisions.md has the operator's decision with its date.
Host: any host.

## Start condition

A failed gate: T-014 (b) or (c), or T-109.
