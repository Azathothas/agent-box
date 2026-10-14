# Status

This page holds the measured state of agent-box: what was run, where, when,
and the result. A design is never a result on this page.

Every result below is MEASURED. Each section names its host, its date and
its conditions. "not run", "not run yet" and "not measured" mean that nobody
ran it. "not recorded" means that the condition was not written down.
[roadmap.md](roadmap.md) gives the exit criteria that these results serve.

## The host of 2026-10-14

The planning session measured the operator's Windows host on 2026-10-14.

| Fact | Value |
| --- | --- |
| Operating system | Windows 11 Pro, build 26200 |
| CPU | 12th Gen Intel Core i7-12700H, 20 logical CPUs |
| Memory | 64 GB |
| Rust | 1.99.0, installed |

## Windows virtualization

Host: the host of 2026-10-14. Date: 2026-10-14. Each check ran once.

The token was not elevated. The user is in Administrators, and the token
holds that group as deny-only: a filtered administrator token. A true
standard user is not tested. The Conditions column gives only what differs
from this.

| Check | Result | Conditions |
| --- | --- | --- |
| `Win32_ComputerSystem.HypervisorPresent` | True | as above |
| The services vmcompute, vmms and hns | running | as above |
| computecore.dll | present | as above |
| WinHvPlatform.dll | present | as above |
| `hcsdiag list` | refused, exit code 27 | as above |
| A WHP partition: get capability, create, set up with 1 processor, delete | each call returned S_OK | as above |
| The same WHP probe | each call returned S_OK | at Low integrity level (S-1-16-4096) |
| The same WHP probe | the capability call returned E_ACCESSDENIED (0x80070005) | in an AppContainer with no capabilities |
| A dynamic VHDX of 30 GiB through virtdisk | created; the file takes 4 MiB (4194304 bytes) on the host | as above |

The refusal of `hcsdiag list` holds this message:

> Only administrators or users that are members of the Hyper-V Administrators user group ...

## Linux KVM through wsl-toolkit

Host: the host of 2026-10-14. Date: 2026-10-14. The check ran once.

| Check | Result |
| --- | --- |
| /dev/kvm inside a wsl-toolkit 6.0.0 job | present |
| WSL2 kernel | 7.2.0-WSL2-STABLE |
| kvm_intel nested | Y |

NOTE: This is nested virtualization. It serves development tests only, never
a production claim.

## Measured elsewhere

The jailer of Firecracker v1.17.0 failed at `unshare(CLONE_NEWNS)` with
`CapEff` 0. That result is MEASURED by the operator's research document on
its own research host, not on the host of 2026-10-14
([research/microvm-tier.md](research/microvm-tier.md), section 1.3).

## Gate

The steps of `cargo xtask gate --strict` (T-007), in the order of
`xtask/src/gate.rs`. `cargo xtask gate --list` names them.

MEASURED 2026-10-14, one run per column:

- Windows host: the operator's Windows 11 Pro (build 26200), Rust 1.99.0,
  cargo-deny 0.20.2, a non-elevated shell.
- Linux runner: `cargo xtask linux`, wsl-toolkit 6.0.0, the image
  rust:1.99-trixie pinned by digest in `xtask/src/linux/args.rs`, WSL2
  kernel 7.2.0. The 266 unit tests of that tree passed; two self-tests skip
  when the copy has no .git/.
- Linux host: a fresh clone of the commit inside the same image, from a git
  bundle, so .git/ is present; `cargo xtask setup`, then the strict gate.
  The run took 27 minutes, most of it the source build of the pinned
  cargo-deny 0.20.2 by `cargo xtask setup`.
- Windows host, fresh clone: the same two commands in a new clone on the
  same host passed too.
- CI: not run before the first push. T-007 records the first run.

The Linux runner runs `cargo xtask gate fmt clippy test` only. Its copy of
the tree has no .git/ and no cargo-deny, so the strict gate runs natively.

| Step | What it runs | Windows host | Linux host | Linux runner | CI |
| --- | --- | --- | --- | --- | --- |
| fmt | `cargo fmt --all -- --check` | pass | pass | pass | not run yet |
| clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | pass | pass | pass | not run yet |
| test | `cargo test --workspace --locked` | pass (267 tests) | pass (267 tests) | pass (266 tests, one fewer: the tree before the last setup test) | not run yet |
| checks | `cargo xtask check all`: text, prose, docs, one-home, placeholders, secrets, attribution, layering and todo | pass (9 of 9) | pass (9 of 9) | does not run there | not run yet |
| deny | `cargo deny --locked check` | pass | pass | does not run there | not run yet |
| the whole gate | `cargo xtask gate --strict` | pass | pass | does not run there | not run yet |

## Spikes

| Entry | Item | Host it needs | Result |
| --- | --- | --- | --- |
| T-014 | (a) no NIC, a SCSI VHDX, a per-VmId hvsocket bind under SDDL, no registry writes | Windows 11 Pro with Hyper-V, as administrator | not run |
| T-014 | (b) ShouldTerminateOnLastHandleClosed false: the instance survives a helper restart, and HcsOpenComputeSystem re-adopts it | Windows 11 Pro with Hyper-V, as administrator | not run |
| T-014 | (c) HcsPause and HcsResume with HVSOCKET_CONNECTED_SUSPEND keep a bridge connected | Windows 11 Pro with Hyper-V, as administrator | not run |
| T-014 | (d) HcsSave ToFile, then RestoreState | Windows 11 Pro with Hyper-V, as administrator | not run |
| T-014 | (e) AlwaysAllowSparseFiles and guest fstrim shrink the VHDX | Windows 11 Pro with Hyper-V, as administrator | not run |
| T-015 | vsock round-trip latency during a cargo build on the data disk, with Sync, Writeback and discard | a Linux KVM host | not run |

## Measurements that the plan needs

| Entry | What | Result |
| --- | --- | --- |
| T-061 | One kernel with virtio_vsock and hv_sock, under both engines | not measured |
| T-065 | erofs against ext4 as the read-only lower layer, and the VHDX copy | not measured |
| T-093 | Block I/O of the data disk under cargo and npm | not measured |
| T-094 | A boot on a 16K-page aarch64 host | not measured |
| T-109 | The HCS spike on Windows 11 Home | not measured |
| T-110 | Memory reclaim and fstrim on Windows | not measured |
| T-120 | Throughput and CPU of the candidate userspace TCP/IP stacks | not measured |
| T-180 | The containment battery and its negative control | not measured |
| T-181 | Boot time, host RSS per idle instance, kage serve RSS, turn latency | not measured |
| T-185 | Containers inside the guest | not measured |
| T-186 | Instance-to-instance isolation | not measured |
