# Engines

This page is the design of the two engines and of the privileged helper.
The Linux engine is Firecracker with its jailer.
The Windows engine is Hyper-V through the Host Compute Service (HCS).
[protocol.md](protocol.md) gives the bytes on the wire, including the helper protocol.
[guest.md](guest.md) gives the kernel and the image that both engines boot.
[research/engines-2026-10.md](research/engines-2026-10.md) records why these two engines won.

NOTE: Nothing on this page is built yet.
The operator ruled on 2026-10-14 (D14) that this session prepares only the tooling and the documents.
Entry T-033 creates the product crates, so every crate path on this page is a planned path.

Labels on this page:

- READ: somebody opened the cited source.
- MEASURED: somebody ran it; the text names the host, the date and the conditions.
- No label: design. An implementation entry confirms it or corrects it.

## The Engine trait

The daemon runs as the operator and calls one narrow trait (T-032, planned file crates/box-daemon/src/engine.rs).
Each step that needs privilege goes to the helper as one fixed operation.
Data moves over the vsock transport (crate box-transport, T-024 and T-025) and never through the helper.

```rust
pub trait Engine {
    fn probe(&self) -> Result<CapabilityReport, Unavailable>;        // attempts the real setup
    fn create(&self, spec: &InstanceSpec) -> Result<(), EngineError>; // disks, uid, records
    fn start(&self, id: &InstanceId) -> Result<(), EngineError>;      // cold boot
    fn pause(&self, id: &InstanceId) -> Result<(), EngineError>;      // in-memory vCPU freeze
    fn resume(&self, id: &InstanceId) -> Result<(), EngineError>;
    fn stop(&self, id: &InstanceId, grace: Duration) -> Result<(), EngineError>; // guest poweroff, then kill
    fn delete(&self, id: &InstanceId) -> Result<Removal, EngineError>; // verified absence
    fn inventory(&self) -> Result<Vec<Observed>, EngineError>;        // includes State::Unknown
    fn connect(&self, id: &InstanceId, port: u32) -> Result<Stream, EngineError>;  // host-initiated vsock
    fn listen(&self, id: &InstanceId, port: u32) -> Result<Listener, EngineError>; // guest-initiated vsock
}
```

Rules for both implementations:

- `probe` attempts the real setup. It never decides from a configuration file or from the presence of a binary.
- A gap in the capability report stops the daemon. No engine falls back to another engine.
- `inventory` reports `State::Unknown` when a query fails. It never assumes that an instance runs.
- `delete` returns a `Removal` that names each item and says whether its absence was verified.
- A test double runs box-guest as a local process for the M1 tests (T-032).

### Operations per engine

| operation | Firecracker (Linux) | HCS (Windows) |
| --- | --- | --- |
| `probe` | helper `probe`: KVM ioctls, a throwaway jail, cgroup delegation, binary digests, host kernel, page size | helper `probe`: hypervisor present, hypervisor event log, HCS enumeration, edition, build |
| `create` | helper `create`: uid, store directory, sparse raw data disk, record | helper `create`: store directory, dynamic VHDX data disk, ACLs, record |
| `start` | helper `start`: jail, jailer, API configuration, verification, `InstanceStart` | helper `start`: HCS document, `HcsCreateComputeSystem`, `HcsStartComputeSystem` |
| `pause` | helper `pause`: `PATCH /vm` with state `Paused` | helper `pause`: `HcsPauseComputeSystem` with `SuspensionLevel` `Suspend` |
| `resume` | helper `resume`: `PATCH /vm` with state `Resumed`, then the daemon sends host time | helper `resume`: `HcsResumeComputeSystem`, then the daemon sends host time |
| `stop` | daemon sends `POWEROFF`, helper `stop` sends `SendCtrlAltDel` on x86_64, helper `kill` after the grace | daemon sends `POWEROFF`, helper `kill` calls `HcsTerminateComputeSystem` after the grace |
| `delete` | helper `delete`: jail, cgroup, disks, records, uid, each absence verified | helper `delete`: compute system, VHDX, grants, records, each absence verified |
| `inventory` | helper `inventory`: cgroup leaves, records, `/proc` verification, `GET /` | helper `inventory`: `HcsEnumerateComputeSystems` by owner, then open and query each |
| `connect` | hybrid vsock: `CONNECT <port>` on the instance socket | `AF_HYPERV` connect to the runtime id and the service GUID of the port |
| `listen` | Unix listener on `<uds_path>_<port>` | `AF_HYPERV` bind to the runtime id and the service GUID of the port |

The control messages (`POWEROFF`, `TIME_SET`) and both transports are in [protocol.md](protocol.md).

## Linux engine: Firecracker and the jailer

The pin is Firecracker v1.17.0 at commit `95f868c8e345b1cc8faccd1a3c910b4989dc3f58` (D12).
v1.17.0 is the minimum version for two reasons:

- It fixes host-to-guest vsock delivery after a bare pause and resume (#6100). READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:CHANGELOG.md:83`.
- It adds discard for writable drives on the `Sync` engine (#6142). READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:CHANGELOG.md:59`.

`agent-box setup` installs the pinned `firecracker` and `jailer` binaries root-owned under `/usr/libexec/agent-box/` (T-083, T-174).
It checks each file against the release `SHA256SUMS` and against a digest reviewed into this repository.
A wrong digest deletes the file and stops the setup.

### Why a root helper

The jailer needs root capabilities.
MEASURED in the operator's research ([research/microvm-tier.md](research/microvm-tier.md), section 1.3): the v1.17.0 jailer failed at `unshare(CLONE_NEWNS)` on a host with `CapEff` 0.
Upstream runs the jailer as root and has not fixed its minimal capability set.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:291`.
The jailer also treats all its inputs as trusted, including every path it gets.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:266`.
So the helper derives every jailer argument itself, and no caller string reaches the jailer.

### Jailer invocation

The helper runs this command for each boot. Values in angle brackets come from the helper's own records.

```
/usr/libexec/agent-box/jailer \
    --id <instance id, 32 lowercase hex> \
    --exec-file /usr/libexec/agent-box/firecracker \
    --uid <instance uid> --gid <instance gid> \
    --chroot-base-dir <jail base> \
    --cgroup-version 2 \
    --parent-cgroup <delegated root, relative>/<instance id> \
    --new-pid-ns \
    --daemonize \
    -- --api-sock /run/fc.sock
```

Each flag and the reason for it:

| flag | reason |
| --- | --- |
| `--id` | The jailer accepts letters, digits and hyphens up to 64 characters, so the 32-hex instance id fits. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:32` |
| `--exec-file` | The jailer copies this binary into the jail, so the jail directory name is `firecracker`. |
| `--uid`, `--gid` | A stable pair per instance from the helper's allocator. The jailer drops to it before exec. |
| `--chroot-base-dir` | The configured jail base. The caller never names it. |
| `--cgroup-version 2` | The default is 1, so version 2 must be explicit. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:39` |
| `--parent-cgroup` | Names the leaf that the helper made. With no `--cgroup` flag the jailer only moves the process into it. |
| `--new-pid-ns` | The VMM is init of its own PID namespace, so a kill of the jail process reaches it. |
| `--daemonize` | `setsid()` and standard streams to `/dev/null`, so guest serial output cannot fill a host log. |
| `--api-sock /run/fc.sock` | The API socket lives in the jail directory `run/`, which only root and the VMM uid can open. |

Open question for T-086: `--resource-limit fsize`, set to the data disk size plus one 4096-byte block.
With it, one file cannot grow past the data disk. A wrong value stops the VMM at its first write past the limit.

The helper never passes `--netns`, `--cgroup`, `--no-seccomp` or `--seccomp-filter`.
Firecracker then applies its built-in per-thread seccomp filters.
READ: the shipped x86_64 filter has 82 rules for the `vmm` thread, 37 for `api` and 50 for `vcpu`.
Each thread has the default action `trap`.
The rules name 51, 32 and 27 distinct syscalls.
Source: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:resources/seccomp/x86_64-unknown-linux-musl.json:2`.
The `vmm` filter allows `socket` only for `AF_UNIX` stream sockets.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:resources/seccomp/x86_64-unknown-linux-musl.json:448`.
The same filter allows the io_uring syscalls for every `io_engine`, and the `Sync` engine does not need them.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:resources/seccomp/x86_64-unknown-linux-musl.json:65-75`.
A custom filter without them is an open question for T-084.

Why the helper does not pass `--cgroup`:

- With any `--cgroup` flag, the jailer writes `+<controller>` into `cgroup.subtree_control` of every ancestor up to the root.
- READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/jailer/src/cgroup.rs:384`.
- Those writes reach cgroups outside the delegated root. The helper writes the limits itself, inside the delegated root only.

CAUTION: With `--cgroup-version 2`, no `--cgroup` flag and a missing parent cgroup, the jailer continues without error.
The VMM then runs outside every agent-box cgroup.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/jailer/src/env.rs:237`.
So the helper creates the leaf first and reads `/proc/<pid>/cgroup` after the jailer exits.
A pid outside the leaf is a refusal, and the helper kills it.

### Jail layout per boot

The jail is rebuilt on each boot and removed after each stop.
The data disk lives in the store and is hard-linked into the jail, so the store and the jail base must be on one filesystem.
The jailer documents the hard-link rule.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:275`.

Host view, design values (T-086 fixes the default base paths):

```
/var/lib/agent-box/                      root:root 0755
  store/                                 root:root 0711
    images/<sha256>.img                  root:root 0444  base images, by digest
    kernels/<sha256>                     root:root 0444  kernels, by digest
    instances/<id>/                      root:root 0700
      record                             root:root 0600  uid, gid, disk size, vmm pid and start time
      lock                               root:root 0600  operation lock (flock)
      data.img                           <uid>:<gid> 0600  sparse raw data disk
  jail/                                  root:agent-box 0750  the jail base
    firecracker/                         root:agent-box 0750
      <id>/                              root:agent-box 0750
        root/                            the chroot; sealed to root:<gid> 0751 before InstanceStart
          firecracker                    copy made by the jailer
          firecracker.pid                written by the jailer
          kernel                         hard link of kernels/<sha256> (vmlinux or Image)
          base.img                       hard link of images/<sha256>.img
          data.img                       hard link of instances/<id>/data.img
          dev/kvm                        device node made by the jailer
          run/                           root:<gid> 0750 after the seal
            fc.sock                      API socket, made by the VMM
          vsock/                         root:agent-box 02771 after the seal
            v.sock                       host-initiated vsock, <uid>:agent-box 0660
            v.sock_5002  v.sock_5003     daemon listeners, mode 0666
            v.sock_5005                  daemon listener after T-257, mode 0666
```

Rules for the jail:

- Only root and members of the `agent-box` group can traverse the jail base. Other local users cannot reach any socket in it.
- The VMM reaches its jail through its chroot, so the parent modes do not apply to it.
- The seal: after configuration and before `InstanceStart`, the helper makes every jail directory root-owned and not writable by the VMM uid.
- After the seal, a process at the VMM uid cannot create, rename or remove any entry in the jail.
- The seal stops a compromised VMM from replacing `fc.sock` or `v.sock` with a symbolic link that root or the daemon would follow.
- The seal also stops a compromised VMM from filling the host filesystem with new files.
- The daemon's listener sockets are mode 0666 because the VMM uid is not in the `agent-box` group. The directory chain limits who reaches them.
- `root/` is 0751 after the seal, so the daemon, which is not in the instance group, can traverse it to `vsock/`. The parent directories keep other users out (T-086).
- The jailer also makes `dev/net/tun`. No configuration uses it, so the seal removes it (T-086).
- Teardown unlinks the jail entries with `openat2` and `RESOLVE_BENEATH` plus `RESOLVE_NO_SYMLINKS`. It never follows a link.
- Teardown removes the hard link `data.img` in the jail only. It never deletes the store copy.
- After teardown, the helper checks that the link count of the store data disk is 1.

NOTE: The seal and the socket modes are design, not measured.
T-086 proves them on a KVM host: a process at the VMM uid fails to create, rename or unlink in the jail.

### The boot sequence

The helper operation `start` runs these steps in order. The steps match [protocol.md](protocol.md), section "The helper protocol".

1. Check the peer, the protocol version and every request field. No syscall on a request value happens before this step ends.
2. Take the instance lock: `flock(LOCK_EX | LOCK_NB)` on `store/instances/<id>/lock`. A held lock is the refusal `busy`.
3. Load the record. An unknown id is a refusal.
4. Refuse when a verified live VMM exists for the id (see "Re-adoption and orphans").
5. Remove a stale jail for the id. The jailer does nothing when the chroot directory already exists, so a stale tree must go first.
6. Create the cgroup leaf and write its limits (see "cgroup v2 delegation").
7. Create the chroot and its directories `run/` and `vsock/`, owned by the instance uid.
8. Hard-link the kernel, the base image and the data disk into the chroot. `EXDEV` is a refusal: the store is on another filesystem.
9. Run the jailer with umask 0007, wait for it to exit, then read `firecracker.pid`.
10. Read the start time of that pid from `/proc/<pid>/stat`. Check the pid against the jail (see "Re-adoption and orphans") and against the cgroup leaf.
11. Write the pid and its start time into the root-owned record. Guest code has not run yet, so this pid is trustworthy.
12. Configure the VMM over the API socket: `PUT /machine-config`, `/boot-source`, `/drives/base`, `/drives/data`, `/vsock`, `/balloon`, `/entropy`.
13. Read `GET /vm/config` and compare it with the document that the builder made. Any difference kills the VMM and refuses.
14. Seal the jail. Set `v.sock` to `<uid>:agent-box` mode 0660.
15. Send `PUT /actions` with `InstanceStart`.
16. Reply with the pid, its start time and the host path of `v.sock`.

Firecracker binds the vsock Unix socket when `PUT /vsock` runs, so step 14 can set its mode before boot.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/devices/virtio/vsock/unix/muxer.rs:335`.

The helper configures through the API, not with `--config-file`.
A configuration file starts the guest at once, and step 13 needs a check before boot.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/getting-started.md:343`.

After `start` returns, the daemon binds its listeners on `v.sock_5002` and `v.sock_5003`, and on `v.sock_5005` after T-257.
Then it connects to port 5000. The boot order on the wire is in [protocol.md](protocol.md).

### uid allocator

- `agent-box setup` reserves one contiguous range of uids and gids and records it in the helper configuration (T-082, T-174).
- The range must not overlap any account in `/etc/passwd` or `/etc/group`, or any range in `/etc/subuid` and `/etc/subgid`. Setup checks this.
- The helper allocates at `create`. Each instance gets one uid and the gid with the same number.
- The helper records the pair in `store/instances/<id>/record`. Later operations read the pair from the record.
- The caller never supplies a uid or a gid. A range check alone cannot stop two instances from sharing a uid.
- The helper still checks that a recorded uid is inside the configured range before it uses it.
- `delete` frees the uid only after two checks. No process runs as that uid, and no file in the store or the jail base belongs to it.

### cgroup v2 delegation

`agent-box setup` creates one delegated cgroup v2 root for every VMM (T-082).
Each instance gets one leaf below it: `<delegated root>/<instance id>`.

Requirements for the delegated root:

- It survives the exit of every helper process. It is not inside the cgroup of the helper unit.
- If it were inside the helper unit, systemd would stop every VMM when the helper process exits.
- systemd never moves or kills processes inside it.
- Its `cgroup.subtree_control` enables `memory`, `cpu` and `pids`, and `io` where the host has it.

The helper writes these files in the leaf before the jailer runs:

| file | value | when the controller is missing |
| --- | --- | --- |
| `memory.max` | the profile memory plus the VMM overhead allowance (T-082 fixes the allowance) | gap: the daemon stops |
| `memory.swap.max` | `0` (Firecracker advises against host swap for guest memory) | note |
| `cpu.max` | the profile cpu count as a quota per period | note |
| `pids.max` | a fixed small number, set by T-082 | gap |
| `io.max` | the profile I/O limit, when the profile has one | note |

Firecracker production guidance is to disable host swap, because swap puts guest memory on storage.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/prod-host-setup.md:274`.

systemd offers delegation on scope and service units only, not on slices.
READ: https://systemd.io/CGROUP_DELEGATION/ (read 2026-10-14).
So a service unit carries the delegated root. T-082 proposes `agent-box-vmm.service` with `Delegate=yes`, whose cgroup is the delegated root.
T-082 also confirms the behaviour on the target systemd versions.

### Data disk settings

Each instance has one sparse raw data disk of about 30 GiB and shares one read-only base image by digest.
The helper creates the data disk at `create` with `ftruncate`. Firecracker reads the capacity from the file size.
The host never formats, mounts, checks or resizes the data disk. box-guest formats it inside the guest on first boot ([guest.md](guest.md)).

| setting | data disk | base image | reason |
| --- | --- | --- | --- |
| `io_engine` | `Sync` | `Sync` | `Async` is developer preview and has no discard |
| `cache_type` | `Writeback` | default (`Unsafe`) | `Writeback` turns each guest flush into `fsync`; the base image is never written |
| `discard` | `true` | absent | Discard works only on writable `Sync` drives |
| `is_read_only` | `false` | `true` | |

Why not `Async`:

- Firecracker marks the io_uring engine developer preview and not suitable for production. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-io-engine.md:11`.
- On host kernel 5.10 its io_uring workers run in the root cgroup, outside the VMM cgroup. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-io-engine.md:142`.
- It cannot discard, so the sparse file only grows. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-discard.md:12`.

Why not `Unsafe` on the data disk:

- `Unsafe` does not offer the virtio flush feature to the guest. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-caching.md:18`.
- Upstream says it sacrifices data integrity when the host loses its page cache. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-caching.md:37`.
- A persistent ext4 journal then has no barrier on host power loss.
- `Writeback` performs `fsync` on each guest flush. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-caching.md:26`.

The research document chose `Async` and `Unsafe` for an ephemeral workspace. Persistent instances reverse both choices.
Space comes back only when the guest trims: box-guest runs `fstrim` weekly ([guest.md](guest.md)).

Firecracker takes no lock on a disk file. agent-box prevents a double start in two places:

- The daemon holds one lock per instance for as long as the instance runs (T-026).
- The helper holds `store/instances/<id>/lock` during each operation and refuses `start` while a verified VMM for the id is alive.

NOTE: The brief asks for an agent-box `flock` on the disk itself.
No agent-box process can hold one for the VMM lifetime: the jailer closes inherited descriptors, and the helper exits after each operation.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/jailer.md:114`.

### VM configuration fields

The configuration builder (T-084, planned file crates/box-vmm/src/firecracker/config.rs) is typed from the pinned `firecracker_spec-v1.17.0.yaml`.
It refuses to send a key that its own schema does not have.
Each API object of Firecracker refuses an unknown field, for example the drive configuration.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/vmm_config/drive.rs:35`.
The top-level type of `--config-file` does not refuse one.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/resources.rs:85`.
So the main risk is a known key that adds a device, such as `network-interfaces`. Only the builder refuses that key.

This table corrects the table in [research/microvm-tier.md](research/microvm-tier.md), section 5.4.
Rows marked "corrected" differ from it.

| key | value | source or reason |
| --- | --- | --- |
| `machine-config.vcpu_count` | profile cpus: 1 or an even number, 1 to 32 | READ: swagger `MachineConfiguration` |
| `machine-config.mem_size_mib` | profile memory | required |
| `machine-config.smt` | `false` | default; x86 only |
| `machine-config.track_dirty_pages` | `false` | only diff snapshots need it, and they are developer preview |
| `machine-config.huge_pages` | `None` (corrected) | the balloon cannot reclaim memory backed by huge pages |
| `machine-config.cpu_template` | absent | |
| `boot-source.kernel_image_path` | `/kernel` | hard link of the kernel, by digest: `vmlinux` on x86_64, `Image` on aarch64 |
| `boot-source.initrd_path` | absent | box-guest is init in the base image |
| `boot-source.boot_args` | see "Kernel command line" | |
| `drives[0]` | `drive_id` `base`, root device, read-only, `/base.img` | |
| `drives[1]` | `drive_id` `data`, read-write, `/data.img`, `Sync`, `Writeback`, `discard` | corrected: was `Async` and `Unsafe` |
| `drives[*].rate_limiter` | absent | `io.max` in the cgroup bounds disk use instead |
| `vsock.guest_cid` | `3` (corrected) | the hybrid vsock has no host-wide CID space, so every instance can use 3 |
| `vsock.uds_path` | `/vsock/v.sock` | |
| `balloon` | `amount_mib` 0, `deflate_on_oom` true, `free_page_reporting` false | see the open question on reporting |
| `entropy` | present, no rate limiter | virtio-rng for the guest |
| `network-interfaces` | absent | the builder refuses the key, and a test proves it (T-084) |
| `mmds-config` | absent | MMDS needs a network interface; no secret goes to MMDS |
| `pmem` | absent | |
| `memory-hotplug` | absent in v1 | T-181 measures virtio-mem as a footprint control first |
| `logger`, `metrics` | absent in v1 (corrected) | the guest can drive their volume |
| `cpu-config` | absent | |
| serial | not a key (corrected) | serial is `PUT /serial`, which the helper never sends |

Sources for the corrections:

- Swagger at the pin, `MachineConfiguration` and `Drive`: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:1446` and `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:1212`. READ.
- Huge pages: the balloon reports free pages at 4 KiB granularity and cannot drop huge-page backing. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/hugepages.md:81`.
- Serial: the configuration type skips the serial field. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/resources.rs:100`.
- Logs: the guest can influence the log volume. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/prod-host-setup.md:50`.

Open: `free_page_reporting` has an unclear status.
The v1.14 changelog calls it developer preview, and no later entry makes it generally available.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:CHANGELOG.md:340`.
It stays off until its status is confirmed. T-181 measures host memory per idle instance.

### Kernel command line

The helper builds the whole command line. The caller supplies no part of it.
No secret, nonce, token or hostname goes on it, because the guest can read `/proc/cmdline`.

The Firecracker part repeats Firecracker's default list, because a custom `boot_args` replaces the default:

```
reboot=k panic=1 nomodule 8250.nr_uarts=0 i8042.noaux i8042.nomux i8042.dumbkbd swiotlb=noforce
```

READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:191`.

- The research document used `console=ttyS0 ... pci=off`. Both parts are dropped (corrected).
- The helper puts no `root=` in `boot_args`. For the root drive, Firecracker appends `root=/dev/vda` and `ro` itself. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/vmm_config/boot_source.rs:74-86`.
- `8250.nr_uarts=0` turns the guest serial port off. Upstream does not advise serial in production. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/prod-host-setup.md:38`.
- Firecracker adds `pci=off` itself when PCI is off. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:206`.
- The root device and the init path come from [guest.md](guest.md).
- A diagnostic boot with a serial console is a later, explicit option. It is not in v1.

### Pause, stop, start and hibernate

| verb | Firecracker action | what survives |
| --- | --- | --- |
| `pause` | `PATCH /vm` with `{"state": "Paused"}` | RAM and vsock connections; lost if the VMM or the host goes away |
| `resume` | `PATCH /vm` with `{"state": "Resumed"}`, then host time over port 5000 | |
| `stop` | `POWEROFF` over port 5000, then box-guest resets the guest; `SendCtrlAltDel` on x86_64; then kill | the data disk |
| `start` | a new jail, a new VMM and a cold boot | the data disk |
| `hibernate` | later and opt-in (T-184, P3) | see below |

- `PATCH /vm` is valid only after boot. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/firecracker/swagger/firecracker.yaml:832`.
- A paused instance keeps all its guest RAM on the host.
- When the VMM or the host goes away during a pause, the instance shows `stopped`. The next `start` cold-boots from the data disk.
- Firecracker exits on a guest CPU reset. `SendCtrlAltDel` exists on Intel and AMD only. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/actions.md:34`.
- Firecracker does not implement guest power management, and a `reboot` in the guest ends it. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/getting-started.md:336-338`.
- So box-guest ends the guest with the reboot call ([guest.md](guest.md), "Poweroff sequence"). T-088 measures what the other call does to the VMM.
- On aarch64 the `POWEROFF` request over port 5000 is the only soft stop.
- `stop` twice is safe. A stop of a stopped instance does nothing and succeeds.

Rules for `hibernate`, when T-184 builds it:

- A Full snapshot only. Diff snapshots are developer preview.
- Single use: the restore consumes the snapshot. Firecracker calls a second resume of one state insecure. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/snapshotting/snapshot-support.md:548`.
- A cold boot of the data disk invalidates the snapshot.
- A manifest records the Firecracker version, the snapshot format, the host kernel and the CPU model. Any mismatch falls back to a cold boot.
- Firecracker trusts snapshot files. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/snapshotting/snapshot-support.md:94`.
- So the files move out of the jail, root-owned 0600, with a digest that the helper checks before load.
- A snapshot resets the vsock device. Every host channel must reconnect and authenticate again.
- Guest S4 hibernation to the data disk is an engine-neutral alternative. T-184 measures it first.

### Re-adoption and orphans

A daemon restart does not stop running instances (T-090).
The daemon reads its encrypted per-instance secrets, asks the helper for `inventory`, and reconnects each verified instance.
Then it rotates the nonces and the egress token ([protocol.md](protocol.md), section "Re-adoption and rotation").

How the helper verifies one VMM:

1. Read the pid and its start time from the root-owned record. The jail pid file is not trusted after boot.
2. `/proc/<pid>/stat` exists and its start time equals the recorded start time. This stops a reused pid.
3. `stat` of `/proc/<pid>/exe` gives the device and inode of the jail copy `root/firecracker`.
4. `stat` of `/proc/<pid>/root` gives the device and inode of the chroot directory.
5. The pid is listed in `cgroup.procs` of the instance leaf.
6. `GET /` on the API socket answers. Its `state` field gives `running` or `paused`. READ: swagger `InstanceInfo`.

Why the checks compare device and inode, not paths:

- The VMM runs in another mount namespace after `pivot_root`.
- A path read through `readlink` may show the path inside that namespace. This is not measured.
- The research document compared paths. T-090 measures both forms on a KVM host and keeps the one that works.

Result per instance:

| checks | state |
| --- | --- |
| 1 to 6 pass | `running` or `paused` |
| 1 to 5 pass, 6 fails | `unknown` |
| the pid is gone | `stopped`; the next `start` or `kill` removes the stale jail |
| any of 2 to 5 fails while the pid exists | `unknown`; never signalled |

Orphans:

- The helper also scans the leaves under the delegated root and the directories under the jail base.
- A leaf or a jail with no helper record is reported as unclaimed. Its processes are verified by checks 3 to 5.
- An instance with a helper record that the daemon does not claim is also unclaimed.
- The daemon removes an unclaimed VMM with `kill`. That stops the process and removes the jail.
- The daemon does not delete the data disk of an unclaimed instance. It reports it, and the operator decides.

### Delete with verified absence

`delete` refuses while a verified VMM runs. Otherwise it removes, then verifies each item:

| item | verified by |
| --- | --- |
| the jail | `<jail base>/firecracker/<id>` does not exist |
| the cgroup leaf | the directory does not exist |
| the data disk | `store/instances/<id>/data.img` does not exist |
| the record and the lock | the instance store directory does not exist |
| the uid | no process and no file under the store or the jail base belongs to it; the allocator lists it as free |
| snapshots (later) | no file under the snapshot directory of the id |

T-091 adds a content probe over the daemon state directory: no byte that the agent wrote survives.

### Performance risk: one VMM thread

Firecracker runs one VMM thread that emulates the block and vsock devices.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/design.md:74`.
The `Sync` engine uses blocking system calls.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/api_requests/block-io-engine.md:4`.
Discard works only with `Sync`.

So guest disk load (cargo, npm) shares one thread with every host channel: the broker, the kage bridge and the guest agent.
Expect head-of-line blocking. Nothing is measured yet.

A second detail adds to the risk.
A guest-initiated vsock connection makes the VMM thread call a blocking `connect` on the daemon's listener.
READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/vmm/src/devices/virtio/vsock/unix/muxer.rs:640`.
The daemon must accept on ports 5002 and 5003 at once, on its own tasks, or block I/O also stalls.

T-015 measures vsock round-trip latency during a cargo build on the data disk (M0).
The mitigations are vhost-user-blk (developer preview) or Cloud Hypervisor (see "Fallbacks and reopening conditions").
T-093 compares the block engines on real agent work.

### aarch64 hosts

- Firecracker tests aarch64 only on AWS Graviton 2 to 5. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:README.md:133`.
- Its kernel policy lists only 4K-page host kernels. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:26`.
- Raspberry Pi 5 and Asahi Linux default to 16K pages.
- aarch64 support is limited to 4K-page hosts until a boot on a 16K-page host is measured (T-094).
- The probe reports a page size other than 4096 bytes as a gap on aarch64.
- QEMU under libvirt is the recorded fallback for 16K-page hosts.
- aarch64 has no `SendCtrlAltDel`, so the `POWEROFF` request is the only soft stop.

### The validated kernel set

READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:26`.

| role | page size | validated versions |
| --- | --- | --- |
| host kernel | 4K | 5.10, 6.1, 6.18 |
| guest kernel | 4K | 5.10, 6.1, 6.18 |

- Upstream validates Amazon Linux microVM kernels built from its own configs. It does not promise that upstream kernels built from those configs work. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:53`.
- Desktop hosts often run other host kernels. The probe reports a host kernel outside the set as a note, not a gap.
- The guest 6.1 line had its minimum end of support on 2026-09-02, and the 6.18 line has it on 2028-06-01.
- Design recommendation for T-060: take the guest kernel from the 6.18 LTS line.

### Development hosts

`/dev/kvm` works inside wsl-toolkit jobs on the operator's Windows machine.
MEASURED on 2026-10-14: WSL2 kernel 7.2.0, `kvm_intel` with `nested=Y`.
That is nested virtualization. It is valid for development tests, never for a production claim.
[development.md](development.md) shows `cargo xtask linux --kvm`.

## Windows engine: Hyper-V through HCS

Each running instance is one HCS compute system, driven by a LocalSystem helper service (D11).
Compute systems are ephemeral: an instance is the agent-box record plus its disks.
READ: https://learn.microsoft.com/en-us/virtualization/api/hcs/overview (read 2026-10-14).

### What the evidence covers

WSL2 and hcsshim use HCS to boot Linux with `LinuxKernelDirect`, SCSI disks and HvSocket.
That covers create, boot, the VHDX path, HvSocket and the absence of a NIC.

WSL2 and hcsshim both set `ShouldTerminateOnLastHandleClosed` to true.
Neither calls `HcsPause`, `HcsSave` or `RestoreState`.
READ: https://github.com/microsoft/WSL/blob/master/src/windows/service/exe/WslCoreVm.cpp and https://github.com/microsoft/hcsshim/blob/main/internal/uvm/create.go (read 2026-10-14).
So four features have no Linux-guest reference user:

- a VM that outlives the process that created it;
- pause and resume;
- save to a file and restore;
- a data VHDX that shrinks after a guest trim.

They are spike items (T-014), not READ facts.
T-101 records a READ of the WSL source: WSL sets `AlwaysAllowSparseFiles` on its read-only system disks, in a 2.3 document.
No reference user trims a data disk through it.

### The compute system document

The helper builds this document itself and accepts no schema fragment from the caller.

NOTE: The field names are design. T-101 fixes them against the schema reference and the WSL and hcsshim types.

```json
{
  "Owner": "agent-box",
  "SchemaVersion": { "Major": 2, "Minor": 3 },
  "ShouldTerminateOnLastHandleClosed": false,
  "VirtualMachine": {
    "StopOnReset": true,
    "Chipset": {
      "LinuxKernelDirect": {
        "KernelFilePath": "<store>\\kernels\\<sha256>",
        "KernelCmdLine": "<built by the helper>"
      }
    },
    "ComputeTopology": {
      "Memory": {
        "SizeInMB": 4096,
        "AllowOvercommit": true,
        "EnableDeferredCommit": true,
        "EnableColdDiscardHint": true
      },
      "Processor": { "Count": 2 }
    },
    "Devices": {
      "Scsi": {
        "0": {
          "Attachments": {
            "0": { "Type": "VirtualDisk", "Path": "<store>\\images\\<sha256>.vhdx", "ReadOnly": true },
            "1": { "Type": "VirtualDisk", "Path": "<store>\\instances\\<id>\\data.vhdx" }
          }
        }
      },
      "HvSocket": {
        "HvSocketConfig": {
          "DefaultBindSecurityDescriptor": "D:P(A;;FA;;;SY)(A;;FA;;;<operator SID>)",
          "DefaultConnectSecurityDescriptor": "D:P(A;;FA;;;SY)(A;;FA;;;<operator SID>)"
        }
      }
    }
  }
}
```

The values 4096 and 2 are placeholders for the profile values.

Rules for the document:

- Schema version: the lowest version that has every field the helper sets. That is 2.3, as WSL uses.
- The schema reference puts `AlwaysAllowSparseFiles` in 2.6 (Windows 11 SDK 21H2). The data disk sets it only if spike T-014 (e) and T-110 keep it.
- T-101 tests whether HCS accepts the field in a 2.3 document, as WSL sends it. Otherwise the helper sets 2.6.
- READ for schema facts: https://learn.microsoft.com/en-us/virtualization/api/hcs/schemareference (read 2026-10-14).
- The document never has `NetworkAdapters`, `Plan9`, `VirtualSmb`, `ComPorts` or a `ServiceTable` entry.
- The types in crates/box-vmm/src/hcs/schema.rs cannot express those devices (T-101). A test proves that.
- The operator SID in the SDDL comes from the token of the authenticated pipe client, never from a request field.
- The compute system id is the instance id. T-101 confirms the format that HCS accepts.
- `Owner` is the fixed string `agent-box`. Inventory filters by it.
- `HcsCreateComputeSystem` takes a security descriptor argument that must be NULL. READ: https://learn.microsoft.com/en-us/virtualization/api/hcs/reference/hcscreatecomputesystem (read 2026-10-14).

### LinuxKernelDirect and the kernel

- `LinuxKernelDirect` is new in schema 2.2 and works on x64 only. WSL notes that ARM64 does not support it yet. READ: the WSL source above.
- agent-box supports Windows 11 x64 hosts only.
- HCS boots the x86_64 `bzImage` of the T-060 build. Firecracker boots the `vmlinux` of the same build ([guest.md](guest.md)).
- Firecracker v1.17.0 can also boot a `bzImage` (#6037), so one `bzImage` for both engines is a fallback, not the plan. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:CHANGELOG.md:29`.
- The HCS part of the command line adds `initcall_blacklist=virtio_vsock_init`, as hcsshim does, so the guest uses `hv_sock`.
- READ: https://github.com/microsoft/hcsshim/blob/main/internal/uvm/create_lcow.go (read 2026-10-14).
- The rest of the command line comes from [guest.md](guest.md). No secret goes on it.
- The design boots with no initrd and the root on SCSI LUN 0. WSL uses an initrd, so T-105 confirms a boot without one.
- Whether `virtio_vsock` and `hv_sock` can live in one kernel is open. T-061 measures it.

### VHDX disks

- The data disk is one dynamic VHDX of 30 GiB, created with `CreateVirtualDisk` from virtdisk (T-103).
- MEASURED on the operator's Windows 11 Pro build 26200 host, 2026-10-14: a new 30 GiB dynamic VHDX was a 4 MiB file.
- That probe ran with a filtered administrator token. A true standard user is not tested.
- The base image is a read-only VHDX copy of the Linux base image, shared by digest. hcsshim attaches one read-only root VHDX to many utility VMs.
- Whether two compute systems can attach one base VHDX read-only at the same time is not known. T-065 measures it.
- The kernel and the base VHDX are read-only for every VM. The helper calls `HcsGrantVmGroupAccess` on them once, at import (T-103).
- The data VHDX gets `HcsGrantVmAccess` for its own compute system at each start, and `HcsRevokeVmAccess` at each kill and delete.
- The grant adds an entry for the per-VM worker account, which HCS derives from the compute system id. READ: https://learn.microsoft.com/en-us/virtualization/api/hcs/reference/hcsgrantvmaccess (read 2026-10-14).
- A grant runs only on files inside the agent-box store (see "The helper builds every configuration").
- Guest `fstrim` on a dynamic VHDX returns host space only if spike T-014 (e) passes. T-110 measures it.

Store layout on Windows, design values. The configuration file of the daemon stays in `%ProgramData%\agent-box\`, outside the store, so the operator can read it.

```
%ProgramData%\agent-box\store\           SYSTEM and Administrators: full; traverse for the VM group; nobody else
  kernels\<sha256>                       read-only, group grant for VMs
  images\<sha256>.vhdx                   read-only, group grant for VMs
  instances\<id>\data.vhdx               granted to its own compute system only
  instances\<id>\record                  SYSTEM only
```

Known trap: third-party HCS callers saw an opaque 0x80070005 at `Construct`.
Reporters fixed it with ACLs for SYSTEM and `NT VIRTUAL MACHINE\Virtual Machines`, plus `HcsGrantVmAccess`.
The reports concern a third-party product (Claude Cowork) and no maintainer confirmed the fix.
READ: https://github.com/anthropics/claude-code/issues/39285 (read 2026-10-14).
T-103 measures the narrowest ACL that avoids the error: traverse only for the `Virtual Machines` group (S-1-5-83-0), file access by grant.

### Memory and processors

| field | value | reason |
| --- | --- | --- |
| `SizeInMB` | profile memory | |
| `AllowOvercommit` | `true` | pagefile-backed guest memory, as WSL2 |
| `EnableDeferredCommit` | `true` | commit on first touch, as WSL2 |
| `EnableColdDiscardHint` | `true` | the host can trim guest pages that the guest freed, as WSL2 |
| `Processor.Count` | profile cpus | |

READ: WSL sets all three memory flags (the WSL source above).
The guest kernel has `hv_balloon` with page reporting ([guest.md](guest.md)).
T-110 measures the reclaim on Windows.

### HvSocket

- Per-VM SDDL for bind and connect: `D:P(A;;FA;;;SY)(A;;FA;;;<operator SID>)`, as WSL uses (T-104).
- That grants SYSTEM and the operator only. Another user cannot bind or connect for that VM.
- No registry write: the per-VM `DefaultBindSecurityDescriptor` covers a bind to a service that has no entry, for that VM only.
- A wildcard bind needs a registry entry. agent-box never binds the wildcard VmId and never sets `AllowWildcardBinds`.
- The daemon binds one listener per VmId and port.
- The daemon listens on ports 5002 and 5003, and on port 5005 after T-257.
- Every host socket sets `HVSOCKET_CONNECTED_SUSPEND`. The default is to disconnect on pause.
- READ: Windows SDK 10.0.22621.0, header shared/hvsocket.h, lines 50 to 57: a non-zero value keeps the socket across a VM pause.
- The mapping from ports to service GUIDs is in [protocol.md](protocol.md), section "Transport on HCS".

### Pause, save and stop on Windows

| verb | HCS call | notes |
| --- | --- | --- |
| `pause` | `HcsPauseComputeSystem` with `{"SuspensionLevel": "Suspend"}` | sockets survive only with `HVSOCKET_CONNECTED_SUSPEND` |
| `resume` | `HcsResumeComputeSystem`, then host time over port 5000 | |
| `stop` | `POWEROFF` over port 5000, then box-guest resets the guest; `HcsTerminateComputeSystem` after the grace | `StopOnReset` ends the compute system on a guest reset (T-105 measures it) |
| `start` | a new compute system from the record, then a cold boot | |
| `hibernate` | `HcsSaveComputeSystem` with `SaveType` `ToFile` after a pause; `RestoreState` at create | later, only if spike T-014 (d) passes |

READ for the pause and save calls: https://learn.microsoft.com/en-us/virtualization/api/hcs/reference/hcssavecomputesystem (read 2026-10-14).
A save fails with `HCS_E_INVALID_STATE` while the system runs, so a pause comes first.
agent-box does not use `HcsShutDownComputeSystem`. The `POWEROFF` request and the guest reset are the soft stop on both engines.

### Owner, lifetime and re-adoption

- The helper sets `Owner` to `agent-box` and `ShouldTerminateOnLastHandleClosed` to false.
- A compute system then keeps running after the helper service stops. This is spike T-014 (b), not a READ fact.
- `inventory` calls `HcsEnumerateComputeSystems` with the owner filter, then `HcsOpenComputeSystem` and a property query for each id.
- HCS state `Running` maps to `running`, `Paused` to `paused`. No entry maps to `stopped`. A failed query maps to `unknown`.
- The runtime id from the property query is the `AF_HYPERV` VmId. It differs from the compute system id.
- An agent-box compute system that no daemon record claims is an orphan. The daemon terminates it and keeps its VHDX for the operator.

Delete on Windows terminates the compute system when it exists, then removes and verifies:

| item | verified by |
| --- | --- |
| the compute system | `HcsEnumerateComputeSystems` does not list the id |
| the VM access grant | the ACL of the data VHDX has no entry for that VM, before the file goes |
| the data VHDX and the record | the instance directory does not exist |
| a save file (later) | no `.vmrs` file for the id |

### Preflight facts

The probe (T-108) reports each fact with its evidence:

- `Win32_ComputerSystem.HypervisorPresent`.
- The Hyper-V-Hypervisor event log: the hypervisor started during this boot.
- The Virtual Machine Platform or Hyper-V feature state.
- The `vmcompute` service state.
- HCS access through the helper: one `HcsEnumerateComputeSystems` call.
- The edition (Home, Pro, Enterprise, Education) and the build number.

Why the event log:

- A third-party product saw 0x80370102 ("Hyper-V is not installed") on hosts where the Hyper-V management service answered queries.
- One reporter found the cause: NX was off in the BIOS, so the hypervisor never started.
- READ: https://github.com/anthropics/claude-code/issues/60371 (read 2026-10-14).

MEASURED on the operator's Windows 11 Pro build 26200 host, 2026-10-14, non-elevated token:

- `HypervisorPresent` was True.
- `hcsdiag list` exited 27: only administrators or Hyper-V Administrators may access virtual machines.

So HCS refuses a caller outside those groups, and the helper runs as LocalSystem.
Windows 11 Home has no evidence either way for a third-party HCS caller. T-109 runs the spike on Home.

### The M0 spike (T-014)

Pass or fail, on Windows 11 Pro, before M1 code depends on it. T-109 repeats it on Home.
The kernel is T-060, or a stock WSL kernel for the spike.

| item | pass condition |
| --- | --- |
| (a) | A VM with no NIC and a SCSI VHDX boots. The daemon binds per VmId under the SDDL. No registry key is written. |
| (b) | With `ShouldTerminateOnLastHandleClosed` false, the VM survives a helper restart, and `HcsOpenComputeSystem` re-adopts it. |
| (c) | `HcsPause` and `HcsResume` with `HVSOCKET_CONNECTED_SUSPEND` keep a bridge connection open. |
| (d) | `HcsSave` with `ToFile`, then `RestoreState`, resumes the guest. |
| (e) | `AlwaysAllowSparseFiles` plus guest `fstrim` makes the VHDX file smaller. |

The results go to [status.md](status.md) with evidence.
A failed (b) or (c) is a gate failure for M4: T-111 then decides between the fallbacks.
A failed (d) moves `hibernate` on Windows to the VMMS fallback or to guest S4.

### Not verified on Windows

- Every spike item above.
- Whether an accepted `AF_HYPERV` socket inherits `HVSOCKET_CONNECTED_SUSPEND` from its listener. The daemon sets it on each accepted socket.
- Whether HCS compute systems on Windows 11 run `vmwp.exe` under a per-VM virtual account. The source is Server 2012 documentation (INFERRED).
- The narrowest store ACL that avoids 0x80070005 (T-103).
- A boot with no initrd (T-105).
- The format that HCS accepts for a compute system id (T-101).
- Per-VM host memory of `vmwp.exe` and `vmmem` (T-181).

### Rust bindings

- The `windows` crate exposes the HCS functions in `Win32::System::HostComputeSystem`. READ: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/HostComputeSystem/index.html (read 2026-10-14).
- It has no typed schema structs. T-101 writes serde types for the subset above, modelled on WSL `hcs_schema.h` and hcsshim `schema2`.
- virtdisk creates the VHDX. Raw WinSock opens `AF_HYPERV` sockets (T-025).
- The research estimate is 1 to 2 thousand lines (INFERRED, not measured).

## The isolation difference

The isolation is not the same on the two operating systems.

On Linux with Firecracker:

- The guest reaches KVM in the host kernel.
- The block, vsock, balloon and entropy devices run in the VMM process, in user mode.
- That process is jailed: chroot, mount and PID namespaces, a per-instance uid, a cgroup leaf, per-thread seccomp.
- A device bug gives the attacker the jailed uid inside the chroot, with no capabilities.

On Windows with HCS:

- The guest reaches the hypervisor and the per-VM worker `vmwp.exe`.
- The guest also reaches kernel code in the root partition: the VMBus root side, storvsp, vhdmp and the hvsocket provider.
- That kernel code is not inside the worker sandbox. A bug there gives the attacker kernel execution on the host.
- The worker runs under its own virtual account, with ACG and CIG on and SeImpersonate removed.
- READ: https://www.microsoft.com/en-us/msrc/blog/2019/09/attacking-the-vm-worker-process (read 2026-10-14).
- Microsoft services the Hyper-V virtual machine boundary as a security boundary. READ: https://www.microsoft.com/en-us/msrc/windows-security-servicing-criteria (read 2026-10-14).

The same on both:

- A hardware virtualization boundary and a guest kernel that agent-box builds.
- No shared filesystem, and no NIC device in the VM configuration. The guest `tun0` device sends its packets over vsock to the daemon (D5).
- All guest traffic goes over vsock or hvsocket to the daemon, which runs as the operator.
- The daemon parses guest frames on both systems, so a daemon bug lands in the operator account on both. T-127 confines the packet parser.

[../SECURITY.md](../SECURITY.md) carries the summary. T-182 completes the threat models.

## The privileged helper

The helper is the second binary, `agent-box-helper` (crate box-helper, planned).
It is the only privileged part of agent-box (D3).
It runs a fixed set of operations, never parses guest data, and builds every engine configuration itself.
The bailey helper is the model: a fixed operation set, a versioned protocol and privilege that lasts one run.
READ: `QaidVoid/bailey@d3c73f7d8fd5a9da8289151eb06a35124037fbea:docs/security/privilege.md:26`.

### Shape on each operating system

| property | Linux | Windows |
| --- | --- | --- |
| identity | root | LocalSystem service |
| start | systemd socket activation, one process per connection (`Accept=yes`) | demand start; the service stops itself when idle |
| endpoint | `/run/agent-box/helper.sock`, `root:agent-box`, mode 0660 | named pipe `\\.\pipe\agent-box-helper` |
| endpoint access | the kernel checks group membership through the socket mode | the pipe DACL grants SYSTEM and the operator SID only |
| peer check | `SO_PEERCRED` uid equals the operator uid recorded at setup | client token user SID equals the operator SID recorded at setup |
| remote clients | not possible on a Unix socket | `PIPE_REJECT_REMOTE_CLIENTS` |
| pipe squatting | not applicable | the service creates the pipe with `FILE_FLAG_FIRST_PIPE_INSTANCE` |
| installed by | `agent-box setup` (T-174) | `agent-box setup` as administrator (T-174) |

Decision for T-081: `Accept=yes`.
No helper process exists between operations, so no privileged process sits idle.
The entry text allows a long-lived helper. If T-081 keeps one, `CapEff` must read 0 in `/proc/<pid>/status` between operations.

Why the peer check uses the uid, not the group:

- `SO_PEERCRED` returns the primary gid only. A check of the gid misses a supplementary group.
- The kernel already checks the group when the client connects, because of the socket mode.
- The helper adds the uid check. One operator per host is the v1 model; a second member of the group is refused.
- Hosts with more than one operator are an open question.

### Fixed operations

The request and reply fields are in [protocol.md](protocol.md), section "The helper protocol".

| operation | Linux steps | Windows steps |
| --- | --- | --- |
| `probe` | open `/dev/kvm`, `KVM_GET_API_VERSION`, `KVM_CREATE_VM`; a throwaway jail that runs `firecracker --version`, then teardown; the delegated root; digests | the preflight facts above; `HcsEnumerateComputeSystems` |
| `create` | allocate the uid; the instance directory; `ftruncate` the data disk; the record | the instance directory; `CreateVirtualDisk`; ACLs; the record |
| `start` | "The boot sequence" above | build the document; grant access; create and start the compute system; return the runtime id |
| `pause` | `PATCH /vm` `Paused` | `HcsPauseComputeSystem` |
| `resume` | `PATCH /vm` `Resumed` | `HcsResumeComputeSystem` |
| `stop` | `PUT /actions` `SendCtrlAltDel` on x86_64; refusal `unsupported` on aarch64 | refusal `unsupported` |
| `kill` | `SIGKILL` to the verified pid; teardown of the jail and the leaf; absences | `HcsTerminateComputeSystem`; absences |
| `delete` | "Delete with verified absence" above | "Owner, lifetime and re-adoption" above |
| `inventory` | "Re-adoption and orphans" above | enumeration by owner, then a query per id |
| `image_add` (proposed) | read the image from a passed descriptor, hash it, store it by digest | the same, into the Windows store |

`kill` is idempotent. The daemon also calls it after a clean guest poweroff, to remove the jail.

NOTE: `image_add` is not in the T-023 list.
The images must sit in the root-owned store, on the filesystem of the jail, and the helper must not read a caller path.
The proposed operation takes an open file descriptor (Linux `SCM_RIGHTS`) or a duplicated handle (Windows) and a digest.
The helper hashes the bytes as it copies them and refuses on a mismatch. T-023 decides.

### Input provenance

Each value that the helper uses, where it comes from, and the check before use.
This table extends the table in [research/microvm-tier.md](research/microvm-tier.md), section 5.6.

| value | provenance | check before any syscall |
| --- | --- | --- |
| instance id | daemon request, 16 bytes | the helper renders it as 32 lowercase hex digits; all-zero is refused |
| uid and gid | the helper's own allocator and record | inside the configured range; never a request field |
| cgroup leaf | configured delegated root plus the rendered id | never a request field |
| jail base | the helper configuration, root-owned | never a request field |
| store root | the helper configuration, root-owned | never a request field |
| kernel and base image | digest in the request, 32 bytes | a file with that digest exists in the store and has the right kind for the engine |
| cpus | request | Firecracker: 1 or an even number up to 32, and at most the host CPU count |
| memory | request, MiB | between the configured minimum and maximum; a multiple of 2 |
| data disk size | request at `create`, GiB | at most the configured maximum; free space checked first |
| instance count | the helper's records | at most the configured maximum per host |
| operator uid or SID | the authenticated peer | equals the value recorded at setup |
| HvSocket SDDL | built from the authenticated client SID | never a request field |
| kernel command line | built by the helper | never a request field |

No request field is a string. A caller-supplied path cannot be expressed in the request types (T-023).

### Validation before any syscall

The helper lists its store, then decodes the whole request, then checks every field, and only then acts (T-081).
The list uses no request value. The checks are pure functions over the bytes, the helper configuration and that list.
A unit test for each of these refusals runs with no privilege and no syscall.
The other refusals come from a step of the operation, for example a held lock or a hard link across filesystems.
Each refusal names the failed check. A syscall failure names the step and the errno or HRESULT.
[protocol.md](protocol.md), section "Refusals", says which refusal comes from which group.

### The helper builds every configuration

- Linux: the helper builds the jailer argv, the Firecracker API bodies and the kernel command line from its records and its pinned schema.
- Windows: the helper builds the whole HCS document. It accepts no schema fragment.
- A caller-supplied `Plan9`, `VirtualSmb` or `NetworkAdapters` entry would expose host files or the host network to the guest.
- A caller-chosen `HcsGrantVmAccess` path would let a VM read any file. So the helper grants access only to files inside the store.
- Before a grant, the helper opens the file with `FILE_FLAG_OPEN_REPARSE_POINT` and refuses a reparse point.
- It reads the final path from the handle and refuses a path outside the store.

### Capability lifecycle

Linux:

1. systemd starts one helper process per connection, as root. The unit sets `CapabilityBoundingSet` to the list below.
2. Before it reads the request, the helper clears its effective capability set. The peer check needs no capability.
3. For the operation, the helper raises only the capabilities that the operation needs.
4. It lowers them before it writes the reply, then exits.
5. The jailer child gets the bounding set. It drops to the instance uid before exec, so Firecracker runs with no capabilities.

Capabilities that the operations need. This list is design, not measured; T-081 measures the minimum:

| capability | used for |
| --- | --- |
| `CAP_SYS_ADMIN` | the jailer: `unshare`, `pivot_root`, `clone(CLONE_NEWPID)` |
| `CAP_SYS_CHROOT` | the jailer: `chroot` |
| `CAP_MKNOD` | the jailer: `/dev/kvm` and `/dev/net/tun` in the jail |
| `CAP_CHOWN` | the jailer and the seal: ownership of the jail entries |
| `CAP_FOWNER` | modes on files that the instance uid owns; hard links to them |
| `CAP_SETUID`, `CAP_SETGID` | the jailer: the drop to the instance uid and gid |
| `CAP_KILL` | `kill`: a signal to a VMM that runs as another uid |
| `CAP_SYS_PTRACE` | `stat` of `/proc/<pid>/exe` and `/proc/<pid>/root` of another uid |
| `CAP_DAC_OVERRIDE` | only if T-081 shows that a step needs it |

Research section 1.3 listed only `CAP_SYS_ADMIN`, `CAP_MKNOD` and `CAP_CHOWN`.
The jailer steps also need `chroot` and the uid drop, so that list is incomplete.

Windows:

- The service runs as LocalSystem, because HCS refuses callers outside Administrators and Hyper-V Administrators.
- T-100 measures which privileges the HCS calls need. The service then declares only those with `SERVICE_CONFIG_REQUIRED_PRIVILEGES_INFO`.
- The service stops itself after an idle period. Running compute systems stay up (spike T-014 (b)).

## Threat model of the helper

This section is the start of the helper threat model. T-080 completes it, and T-182 reviews it.
Every refusal below names a planned test. T-080 is done when each named test exists.

### Assets

- Root on Linux and LocalSystem on Windows.
- Host files outside the store, and the host network.
- The data disks of other instances.
- The guarantee that no VM configuration has a NIC device or a shared filesystem.

The helper never holds a credential. The credential store is in the daemon ([broker.md](broker.md)).

### Attackers and entry points

| attacker | entry point | what stops it |
| --- | --- | --- |
| a local user outside the `agent-box` group | the helper socket or pipe | socket mode or pipe DACL; then the peer check |
| a group member who is not the operator | the helper socket | the uid check against the recorded operator |
| a compromised daemon (it parses guest frames) | the fixed operations | no path, device, uid or schema can be expressed; limits on cpus, memory, disk and count |
| a compromised VMM at the instance uid | files in its jail | the seal; no-follow teardown; the root-owned pid record |
| a compromised `vmwp.exe` | files that its VM may open | grants only on files in the store, per VM |
| the guest | nothing | no channel reaches the helper; the helper never reads a guest byte |

A compromised daemon can still stop, start and delete the operator's own instances.
That is accepted: the daemon already runs as the account that owns them.

### Refusal conditions

| condition | where | planned test |
| --- | --- | --- |
| peer uid differs from the recorded operator uid | Linux, before reading a frame | `helper::refuses_foreign_uid` |
| client SID differs from the recorded operator SID | Windows, before reading a frame | `helper::refuses_foreign_sid` |
| protocol version differs | first frame | `helper::refuses_version_mismatch` |
| a length above the ceiling, or a truncated frame | framing | `helper::refuses_oversize_frame`, `helper::refuses_truncated_frame` |
| an unknown operation code | request decode | `helper::refuses_unknown_op` |
| trailing bytes after the last field | request decode | `helper::refuses_trailing_bytes` |
| the all-zero instance id | request check | `helper::refuses_zero_id` |
| an id with no record (except `create`) | request check | `helper::refuses_unknown_instance` |
| `create` for an id that exists | request check | `helper::refuses_existing_instance` |
| cpus, memory or disk outside the configured bounds | request check | `helper::refuses_out_of_bounds_resources` |
| an instance count above the maximum | request check | `helper::refuses_instance_limit` |
| a digest with no image of the right kind | request check | `helper::refuses_missing_image` |
| a recorded uid outside the configured range | record load | `helper::refuses_uid_outside_range` |
| the instance lock is held | operation start | `helper::refuses_busy_instance` |
| `start` while a verified VMM runs | Linux `start` | `helper::refuses_double_start` |
| the store and the jail base on different filesystems | Linux `start`, hard link | `helper::refuses_cross_device_link` |
| the VMM pid is outside the cgroup leaf | Linux `start` | `helper::kills_vmm_outside_leaf` |
| `GET /vm/config` differs from the built document | Linux `start` | `helper::kills_vmm_on_config_drift` |
| a missing `memory.max` or `pids.max` controller | Linux `start` | `helper::refuses_missing_memory_controller` |
| a symbolic link inside a jail during teardown | Linux `kill` and `delete` | `helper::teardown_never_follows_links` |
| a grant target outside the store, or a reparse point | Windows `start` | `helper::refuses_grant_outside_store` |
| `delete` while a verified VMM or compute system runs | `delete` | `helper::refuses_delete_while_running` |
| a signal to a pid that fails verification | `kill` | `helper::never_signals_unverified_pid` |

### Residual risks

- The jailer runs as root and trusts its inputs. A jailer bug is a root bug. The helper limits this by deriving every input.
- HCS is closed source. A bug in HCS or in a root-partition VSP is outside agent-box control.
- The helper parses its request frames as root. T-183 adds a fuzz target for the helper messages.
- A compromised daemon can create instances up to the configured limits and fill that much disk.

## Fallbacks and reopening conditions

Each fallback is a decision with its own entry, not a setting. No engine falls back at run time.

### Linux: Cloud Hypervisor

- Reopening condition: measured block I/O dominates the agent workload, or the vsock latency under disk load fails T-015.
- T-015 and T-093 produce the measurements.
- Cloud Hypervisor has multiqueue virtio-blk, io_uring with discard, optional `O_DIRECT` and disk locks. READ: research reader of 2026-10-14, summarized in [research/engines-2026-10.md](research/engines-2026-10.md).
- It has no jailer. agent-box would build the jail: namespaces, chroot, uid, cgroup and a seccomp policy.
- The Firecracker jailer cannot wrap it, because the jailer always appends Firecracker arguments. READ: `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:src/jailer/src/env.rs:561`.
- It speaks the same hybrid vsock protocol, so the transport stays.

### Linux on 16K-page aarch64: QEMU under libvirt

- Condition: a boot on a 16K-page host fails, or nobody supports it (T-094).
- QEMU under libvirt confines with an unprivileged user, cgroups, seccomp and sVirt (INFERRED).
- It has a large device model, which is why it is not the primary engine.

### Windows first fallback: Hyper-V VMMS through WMI

- Condition: HCS fails a gate (T-014 or T-109). T-111 decides.
- Podman machine (libhvee), Multipass, minikube and Vagrant use `Msvm_VirtualSystemManagementService`. READ: https://docs.podman.io/en/latest/markdown/podman-system-hyperv-prep.1.html (read 2026-10-14).
- VMs are persistent and registered. Save and suspend work with Linux guests. The worker is the same sandboxed `vmwp.exe`.
- Costs: Pro, Enterprise or Education only; Gen2 UEFI boot with no `LinuxKernelDirect`; one HKLM registry entry per hvsocket service GUID.
- It also gives `hibernate` if spike T-014 (d) fails.

### Windows second fallback: OpenVMM

- Condition: HCS fails on Home and Home support is wanted (T-109, T-111).
- It works on Home and needs no administrator at run time.
- It speaks the same hybrid vsock protocol as Firecracker. READ: `microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:support/hybrid_vsock/src/lib.rs:18`.
- It has no process sandbox: its `SandboxProfile` trait has no implementation. READ: `microsoft/openvmm@8daf1d537602c70c849b42402072c42c29e9e245:support/mesh/mesh_process/src/lib.rs:283`.
- WHP refuses an AppContainer caller and accepts a Low-IL caller. MEASURED on the operator's Windows 11 Pro build 26200 host, 2026-10-14.
- It has no memory reclaim, it ignores TRIM, and it has no save call in its API.
- agent-box would need a launcher: a dedicated account, a job object and mitigation policies.

### Single-engine options that were rejected

The operator asked first for one engine native on both systems.
Four exist, and each weakens isolation, so the split by operating system stands (D11).
Details: [research/engines-2026-10.md](research/engines-2026-10.md).

| option | reason for rejection |
| --- | --- |
| crosvm | Windows WHPX is not tested upstream; its Windows sandbox is an opaque 2022 library; the main Linux process is unconfined. |
| OpenVMM | No sandbox on either system, no memory reclaim, no TRIM, weak aarch64 Linux, no binaries. |
| libkrun | Windows WHP exists only in an unreleased branch and a fork; the VMM runs as the user. |
| QEMU | On Windows: no vsock, no save, no reclaim, no TRIM, no sandbox; heavier than Firecracker on Linux. |
| a custom VMM | The most expensive route, and it still cannot match the `vmwp.exe` sandbox on Windows. |

Structural reason: only the Hyper-V stack (HCS, or VMMS through WMI) has Microsoft run the VMM in a hardened per-VM worker.
Of the two, HCS boots the kernel directly and needs no registry entries.
With any WHP engine, the VMM is a process that agent-box launches and must confine itself.

Re-evaluate when one of these happens:

- libkrun 2.0 ships with WHP.
- crosvm upstreams a sandbox that others can build, and runs WHPX in CI.
- OpenVMM ships signed binaries, memory reclaim and a sandbox profile.

Cheap hedge: the guest kernel keeps both the VMBus and the virtio drivers ([guest.md](guest.md)).

## Open questions

| question | entry |
| --- | --- |
| Does the proposed service unit `agent-box-vmm.service` hold the delegated root on the target systemd versions? | T-082 |
| What is the minimal capability set of the helper and the jailer? | T-081 |
| Do the seal and the socket modes hold on a KVM host? | T-086 |
| Does the jailer get `--resource-limit fsize`? | T-086 |
| Does agent-box ship a custom seccomp filter without the io_uring syscalls? | T-084 |
| Does a `stat` of `/proc/<pid>/root` or a `readlink` identify the jail across mount namespaces? | T-090 |
| Is `free_page_reporting` generally available in v1.17.0? | T-181 |
| Does the helper get an `image_add` operation? | T-023 |
| Do hosts with more than one operator need support? | T-081 |
| Every item in "Not verified on Windows". | T-014, T-101, T-103, T-105, T-109, T-110 |
