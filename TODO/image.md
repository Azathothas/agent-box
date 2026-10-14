# Kernel and guest images

This area builds the guest kernel and the read-only guest images, pins each
output by its sha256, and publishes the outputs. The guest design is in
[docs/guest.md](../docs/guest.md). The supply-chain rules come from section 6
of the research document ([docs/research/microvm-tier.md](../docs/research/microvm-tier.md)).
The engines that boot these images are in [linux.md](linux.md) and
[windows.md](windows.md).

Every output of this area is an input that the host trusts. No step here reads
bytes that a guest wrote. The planned paths images/, xtask/src/image/ and
crates/box-image/ do not exist yet.

# T-060: Guest kernel: pinned source and one config for both engines

**Source:** The planning session of 2026-10-14 (kernel design in
[docs/guest.md](../docs/guest.md)); research document section 6; the engine
evaluation ([docs/research/engines-2026-10.md](../docs/research/engines-2026-10.md)).
**Category:** feature
**Milestone:** M2
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

Each instance boots a kernel that agent-box builds, so kernel maintenance moves
to this project (research document, section 12). Without a pinned source and a
committed config, two builds can differ, and a digest proves nothing. A kernel
per engine doubles the image matrix and the tests. One config must therefore
carry the virtio drivers for Firecracker and the VMBus drivers for Hyper-V.

## Premise

- READ: Firecracker v1.17.0 validates guest kernels 5.10, 6.1 and 6.18, all
  with 4K pages
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:34-38`).
  Other versions "might work" but are not tested (same file, lines 17-22).
- READ: Firecracker v1.17.0 boots a bzImage on x86_64 as well as a vmlinux,
  and it detects the format itself
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:CHANGELOG.md:29-32`).
- READ: Firecracker recommends vmlinux. The guest decompresses a bzImage at
  boot, which costs boot time and guest memory
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/rootfs-and-kernel-setup.md:13-16`).
- Design (main-session ruling on kernels): one config per architecture. On
  x86_64 it builds vmlinux for Firecracker and bzImage for HCS. One bzImage for
  both engines is a possibility, not the plan.
- READ: the Firecracker guest config items are CONFIG_VIRTIO_MMIO,
  CONFIG_VIRTIO_BLK, CONFIG_VIRTIO_VSOCKETS, CONFIG_HW_RANDOM_VIRTIO and
  CONFIG_VIRTIO_BALLOON
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:59-70`).
  On x86_64 it lists CONFIG_KVM_GUEST, CONFIG_PTP_1588_CLOCK_KVM,
  CONFIG_SERIO_I8042 and CONFIG_KEYBOARD_ATKBD (same file, lines 92-98).
- READ: an x86_64 boot from a block device needs CONFIG_VIRTIO_BLK,
  CONFIG_ACPI, CONFIG_PCI and CONFIG_KVM_GUEST (same file, lines 113-119).
  Firecracker suggests CONFIG_X86_MPPARSE=n and
  CONFIG_VIRTIO_MMIO_CMDLINE_DEVICES=n, because ACPI replaces the legacy
  device list (same file, lines 137-148).
- READ: a Linux guest under Hyper-V needs CONFIG_VSOCKETS and
  CONFIG_HYPERV_VSOCKETS for AF_VSOCK over hv_sock
  (https://learn.microsoft.com/en-us/virtualization/hyper-v-on-windows/user-guide/make-integration-service,
  read 2026-10-14).
- READ: HCS LinuxKernelDirect has the fields KernelFilePath, InitRdPath and
  KernelCmdLine, new in schema 2.2
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/schemareference,
  read 2026-10-14).
- READ: hcsshim boots a vmlinux or a compressed kernel through
  LinuxKernelDirect and appends `initcall_blacklist=virtio_vsock_init`
  (https://raw.githubusercontent.com/microsoft/hcsshim/main/internal/uvm/create_lcow.go,
  read 2026-10-14).
- READ: the kernel's reproducible-builds guide names the sources of
  difference (https://docs.kernel.org/kbuild/reproducible-builds.html, read
  2026-10-14). They are KBUILD_BUILD_TIMESTAMP, KBUILD_BUILD_USER and
  KBUILD_BUILD_HOST, absolute paths, and a new module signing key per build.
  The fix for absolute paths is `-fdebug-prefix-map` in KCFLAGS and KAFLAGS.
- READ: kage runs its shell tools with no sandbox, so the instance is the only
  boundary around agent code
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-tools/src/builtin/shell.rs:3-4`).
- READ: on Windows, `cargo xtask linux` copies the repository into the
  container, and changes do not come back (`xtask/src/linux.rs:43`).
- Not measured: no kernel was built in the planning session.

## Approach

1. Pin the source. Use the newest 6.18.y point release at the time of the
   work. 6.18 is the newest guest line that Firecracker validates.
2. Write version, URL and sha256 in images/pins.toml, table `[kernel]`. The
   URL has the form
   `https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-6.18.N.tar.xz`.
3. Check the sha256 once against the signed sha256sums.asc of kernel.org.
   Write the name of the reviewer in the same table.
4. Write the config as fragments: images/kernel/common.config,
   images/kernel/x86_64.config and images/kernel/aarch64.config.
5. Start from the arch defconfig. Merge the fragments with
   scripts/kconfig/merge_config.sh, then run `make olddefconfig`.
6. Commit the result as images/kernel/x86_64.full.config and
   images/kernel/aarch64.full.config. The build fails when the generated
   config differs from the committed one.
7. The build also fails when merge_config.sh reports a symbol it could not
   set. A renamed symbol (for example a split of CONFIG_HYPERV) then shows at
   once.
8. Use these fragment contents. Every driver is built in, because the command
   lines carry `nomodule` and the image has no module tree.

   ```text
   # images/kernel/common.config
   CONFIG_VIRTIO=y
   CONFIG_VIRTIO_MMIO=y
   CONFIG_VIRTIO_BLK=y
   CONFIG_VSOCKETS=y
   CONFIG_VIRTIO_VSOCKETS=y
   CONFIG_HW_RANDOM=y
   CONFIG_HW_RANDOM_VIRTIO=y
   CONFIG_MEMORY_BALLOON=y
   CONFIG_VIRTIO_BALLOON=y
   CONFIG_PAGE_REPORTING=y
   CONFIG_EXT4_FS=y
   CONFIG_EROFS_FS=y
   CONFIG_EROFS_FS_ZIP=y
   CONFIG_OVERLAY_FS=y
   CONFIG_TMPFS=y
   CONFIG_DEVTMPFS=y
   CONFIG_DEVTMPFS_MOUNT=y
   CONFIG_EFI_PARTITION=y
   CONFIG_CGROUPS=y
   CONFIG_MEMCG=y
   CONFIG_CGROUP_PIDS=y
   CONFIG_CGROUP_SCHED=y
   CONFIG_NAMESPACES=y
   CONFIG_USER_NS=y
   CONFIG_PID_NS=y
   CONFIG_NET_NS=y
   CONFIG_SECCOMP=y
   CONFIG_TUN=y
   CONFIG_INET=y
   CONFIG_IPV6=y
   CONFIG_UNIX=y
   CONFIG_PTP_1588_CLOCK=y
   CONFIG_PTP_1588_CLOCK_KVM=y
   # CONFIG_MODULES is not set
   # CONFIG_VIRTIO_NET is not set
   # CONFIG_NF_TABLES is not set
   # CONFIG_MODULE_SIG is not set
   # CONFIG_IKHEADERS is not set

   # images/kernel/x86_64.config
   CONFIG_HYPERVISOR_GUEST=y
   CONFIG_PARAVIRT=y
   CONFIG_KVM_GUEST=y
   CONFIG_ACPI=y
   CONFIG_PCI=y
   CONFIG_SERIO_I8042=y
   CONFIG_KEYBOARD_ATKBD=y
   CONFIG_SERIAL_8250=y
   CONFIG_SERIAL_8250_CONSOLE=y
   CONFIG_HYPERV=y
   CONFIG_HYPERV_UTILS=y
   CONFIG_HYPERV_BALLOON=y
   CONFIG_HYPERV_STORAGE=y
   CONFIG_HYPERV_VSOCKETS=y
   CONFIG_SCSI=y
   CONFIG_BLK_DEV_SD=y
   # CONFIG_X86_MPPARSE is not set
   # CONFIG_VIRTIO_MMIO_CMDLINE_DEVICES is not set
   # CONFIG_HYPERV_NET is not set
   # CONFIG_PCI_HYPERV is not set
   # CONFIG_DRM_HYPERV is not set
   # CONFIG_HYPERV_KEYBOARD is not set

   # images/kernel/aarch64.config
   CONFIG_ARM64_4K_PAGES=y
   CONFIG_ARM_AMBA=y
   CONFIG_RTC_DRV_PL031=y
   CONFIG_SERIAL_OF_PLATFORM=y
   ```

9. The aarch64 config has no Hyper-V drivers. HCS LinuxKernelDirect is x64
   only, and the Windows target is x64 only.
10. If Firecracker does not find the virtio devices through ACPI, turn
    CONFIG_VIRTIO_MMIO_CMDLINE_DEVICES back on. Firecracker documents that
    both mechanisms together work, with warnings
    (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:217-221`).
11. Add the command `cargo xtask image kernel --arch x86_64|aarch64` in
    xtask/src/image/kernel.rs. It runs the build in the Linux runner.
12. Pin the builder image by digest. Take its packages from the
    snapshot.debian.org timestamp of T-062. The packages are gcc, make, bc,
    bison, flex, libelf-dev, libssl-dev, cpio, xz-utils and
    gcc-aarch64-linux-gnu.
13. Set the build environment as follows. KBUILD_BUILD_TIMESTAMP comes from the
    SOURCE_DATE_EPOCH of the pins. KBUILD_BUILD_USER and KBUILD_BUILD_HOST are
    `agent-box`.
14. Build out of tree in the fixed path /build. Add the option
    -fdebug-prefix-map to KCFLAGS and KAFLAGS, and map /build to the current
    directory.
15. Write the outputs to .work/images/kernel/ARCH/: bzImage and vmlinux for
    x86_64, Image for aarch64, and the full config. Record each sha256 in an
    `[[artifact]]` row of images/pins.toml.
16. Firecracker boots vmlinux (T-084), and HCS boots bzImage (T-101). Both
    come from the same config and the same source.
17. Add `--out DIR` to `cargo xtask linux`. On Windows it maps to the
    wsl-toolkit option `--artifacts`, so that built files come back.
18. Do not put the command line in the kernel (no CONFIG_CMDLINE). Each engine
    builds it: T-084 for Firecracker, T-101 for HCS.

## Prove

```sh
cargo xtask image kernel --arch x86_64
cargo xtask image verify --kernel --arch x86_64
cargo xtask image kernel --arch x86_64 --fresh
cargo xtask image verify --kernel --arch x86_64
cargo xtask image kernel --arch aarch64
cargo xtask image verify --kernel --arch aarch64
```

Expected: each command exits 0. `--fresh` builds in a new runner container,
and its digests equal the first build and the rows of images/pins.toml.
`verify` exits 1 on a planted config change. Host: any host with the Linux
runner (Windows with wsl-toolkit, or Linux with podman or docker).

## Start condition

T-010 (the Linux runner) is done. `--out` for `cargo xtask linux` is part of
this entry.

# T-061: One kernel with virtio_vsock and hv_sock

**Source:** The planning session of 2026-10-14 (open question in
[docs/guest.md](../docs/guest.md)); the engine evaluation, open question on
the shared kernel config.
**Category:** measurement
**Milestone:** M2
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

hcsshim switches off the virtio vsock transport under Hyper-V on the kernel
command line. Nobody has shown that one kernel with both transports works
under both engines. If it does not, agent-box needs a kernel per engine, which
doubles T-060 and T-062. The root device also gets a different name under
each engine.

## Premise

- READ: hcsshim appends `initcall_blacklist=virtio_vsock_init` to force the
  hv_sock transport
  (https://raw.githubusercontent.com/microsoft/hcsshim/main/internal/uvm/create_lcow.go,
  read 2026-10-14).
- READ: the Linux hv_sock driver maps an AF_VSOCK port to the service GUID
  `<port>-facb-11e6-bd58-64006a7986d3` (Windows SDK 10.0.22621.0, header
  shared/hvsocket.h, lines 135-156, read 2026-10-14).
- READ: Firecracker exposes virtio-mmio devices only. It has no VMBus device
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/design.md:72-78`).
- Not read, not measured: why both transports conflict without the blacklist.
  The working assumption is that only one guest-to-host transport registers.
- Design: Firecracker names the drives /dev/vda and /dev/vdb in config order.
  Under HCS, SCSI disks get /dev/sdX names. Their order is not documented.

## Approach

1. Boot the T-060 vmlinux under Firecracker with the T-084 command line, with
   no blacklist. Record the port 5000 handshake result.
2. Read the guest kernel log lines that name vsock through an exec session on
   port 5001 (T-042). Record the device names of the base and the data disk.
3. Boot the bzImage of the same build under HCS with
   `initcall_blacklist=virtio_vsock_init` (T-105). Record the same facts.
4. Boot it under HCS without the blacklist. Record pass or fail. This answers
   the coexistence question.
5. Boot the bzImage under Firecracker too, and record its boot time and guest
   memory beside those of vmlinux. The plan stays vmlinux for Firecracker.
6. Boot 10 times under HCS and record the device name of each disk. If the
   names change, the root needs a stable name (step 7).
7. The candidate is a GPT in the base image with a fixed partition UUID, and
   `root=PARTUUID=<uuid>` on both engines. T-065 owns that change.
8. Write a table to docs/status.md: kernel digest, engine versions, host,
   date, and pass or fail per boot.
9. Close the entry with one outcome. Expected: one kernel, and a command line
   per engine. Fallback: a kernel per engine, as a pins change.

## Prove

```sh
cargo test -p box-daemon --test linux_boot -- --ignored vsock_transport
cargo test -p box-daemon --test windows_boot -- --ignored vsock_transport
```

Expected: both tests pass with kernels of one build, and docs/status.md holds
the table with the HCS run without the blacklist. Hosts: a Linux KVM host for
the first; Windows 11 Pro with Hyper-V, as admin, for the second.

## Start condition

T-060, T-088 and T-105.

# T-062: Debian 13 slim base image, reproducible

**Source:** The planning session of 2026-10-14 (ruling D6 in
[docs/decisions.md](../docs/decisions.md); image design in
[docs/guest.md](../docs/guest.md)); research document section 6.
**Category:** feature
**Milestone:** M2
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

Every instance boots the same read-only base image. A secret in it is a secret
handed to every instance (research document, section 6). An image that cannot
be rebuilt to the same digest cannot be reviewed, so a digest pin means
nothing. The image must also be small, because each variant repeats it.

## Premise

- READ: mmdebstrap 1.5.7-1+deb13u1 is the Debian 13 package. Its variants
  include `minbase` (the essential set plus Priority required) and `apt`
  (https://manpages.debian.org/trixie/mmdebstrap/mmdebstrap.1.en.html, read
  2026-10-14).
- READ: with SOURCE_DATE_EPOCH set, the mmdebstrap result is reproducible
  across runs with the same options and mirror content (same page).
- READ: mmdebstrap copies the host's /etc/resolv.conf and /etc/hostname. The
  page tells the user to overwrite both with a `--customize-hook` (same page).
- READ: the mmdebstrap cleanup writes an empty /etc/machine-id and removes the
  dpkg and apt logs (same page). `--aptopt='Acquire::Check-Valid-Until "false"'`
  allows old snapshot timestamps (same page).
- READ: mmdebstrap formats include `tar` and `ext4`. For ext4 it pipes a
  tarball to mke2fs, and SOURCE_DATE_EPOCH fixes the UUID and hash_seed (same
  page).
- READ: mkfs.erofs 1.8.6 has `-T` (a fixed timestamp for reproducible
  builds), `-U` (the UUID) and `--tar` (a tarball as the source)
  (https://manpages.debian.org/trixie/erofs-utils/mkfs.erofs.1.en.html, read
  2026-10-14).
- READ: pin anything fetched and run by a commit, a tag or a digest, never a
  branch. On a digest mismatch, delete the cached copy and run nothing
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:76-92`).
- READ: on Debian slim, /bin/sh is dash, not bash
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:155-174`).
- READ: a login inside the guest writes a real token into it, so logins happen
  on the host only
  (`earendil-works/pi@f9bcd351dc3cedf989bc5fc0f8aa012db5737df2:packages/coding-agent/docs/containerization.md:148`).
- Design: the broker uses explicit plain-HTTP endpoints (decision P4 in
  [docs/decisions.md](../docs/decisions.md)), so the image carries no broker
  CA.

## Approach

1. Write the recipe in images/base/recipe.toml. It holds the suite `trixie`,
   the components, the package list, the user and a fixed filesystem UUID.
2. Keep the snapshot timestamp in one place: table `[snapshot]` of
   images/pins.toml, in the form `YYYYMMDDTHHMMSSZ`.
3. Derive SOURCE_DATE_EPOCH from that timestamp. One input then fixes both the
   package set and every file time.
4. Add `cargo xtask image build --variant base --arch x86_64|aarch64` in
   xtask/src/image/base.rs. It runs mmdebstrap in the Linux runner.
5. Use this command. The real values come from the recipe and the pins.

   ```sh
   SOURCE_DATE_EPOCH="$EPOCH" mmdebstrap \
     --mode=unshare --variant=minbase --format=tar \
     --architectures=amd64 --components=main \
     --aptopt='Acquire::Check-Valid-Until "false"' \
     --aptopt='APT::Install-Recommends "false"' \
     --dpkgopt='path-exclude=/usr/share/man/*' \
     --dpkgopt='path-exclude=/usr/share/doc/*' \
     --dpkgopt='path-include=/usr/share/doc/*/copyright' \
     --dpkgopt='path-exclude=/usr/share/locale/*' \
     --include=ca-certificates,git,bash,coreutils,openssh-client,curl,e2fsprogs,util-linux \
     --customize-hook='copy-in /in/box-guest /usr/sbin' \
     --customize-hook='sh /work/images/base/customize.sh "$1"' \
     trixie rootfs.tar \
     "deb http://snapshot.debian.org/archive/debian/$STAMP/ trixie main" \
     "deb http://snapshot.debian.org/archive/debian-security/$STAMP/ trixie-security main"
   ```

6. If `--mode=unshare` fails in the runner container, use `--mode=root` there.
   The container is thrown away after the build.
7. Write images/base/customize.sh as POSIX sh, because /bin/sh is dash. It
   writes these files in the target root, with fixed contents:
   - /etc/hostname: `agent-box`.
   - /etc/resolv.conf: the gateway address that T-045 fixes.
   - /etc/hosts: localhost lines only.
   - /etc/passwd, /etc/group and /etc/shadow: one appended line each for user
     `agent`, uid 1000, gid 1000, shell /bin/bash, password locked.
   - /home/agent: owner 1000:1000, mode 0750.
   - /etc/gitconfig: the `url.insteadOf` rule that T-146 specifies.
8. Do not call useradd. It writes the current date into /etc/shadow.
9. Install no sudo. Clear the setuid and setgid bits of every file in the
   root, so that uid 1000 has no path to root.
   The main-session ruling on the trusted channel asks for both.
10. box-guest is the static musl binary of T-040. Copy it to
    /usr/sbin/box-guest, mode 0755. The command lines carry
    init=/usr/sbin/box-guest.
11. Make the filesystem image from rootfs.tar in the format that T-065 picks.
    The candidate is
    `mkfs.erofs -zlz4hc -T "$EPOCH" -U "$UUID" --tar=f base.erofs rootfs.tar`.
    The ext4 candidate is mmdebstrap `--format=ext4`.
12. Feed no secret to the build: no token, no key, no credential file. Scan
    rootfs.tar after the build with the rules of `xtask/src/check/secrets.rs`.
    A hit fails the build. This is the image check of T-256.
13. Record the sha256 of rootfs.tar and of the filesystem image in
    `[[artifact]]` rows of images/pins.toml.
14. Do not format the data disk here. box-guest formats it inside the guest on
    first boot (T-043). The image carries mkfs.ext4 for that from e2fsprogs.

## Prove

```sh
cargo xtask image build --variant base --arch x86_64
cargo xtask image build --variant base --arch x86_64 --fresh
cargo xtask image verify --variant base --arch x86_64
```

Expected: the two builds give the same sha256 for rootfs.tar and for the
filesystem image, and `verify` exits 0 against images/pins.toml. A planted
credential shape in the root makes the build fail. `verify` lists no setuid or
setgid file and no sudo. Host: any host with the Linux runner.

The boot check is `boot_base` in T-088 and T-105: the image reaches the port
5000 handshake.

## Start condition

T-040 (the box-guest binary).

# T-063: kage in the image

**Source:** The planning session of 2026-10-14 (ruling D2 in
[docs/decisions.md](../docs/decisions.md); kage in the guest in
[docs/guest.md](../docs/guest.md)); the kage findings in
[docs/references.md](../docs/references.md).
**Category:** feature
**Milestone:** M2
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

kage is the harness in every instance, and it runs the agent's commands. A
kage binary that nobody verified is code from an unknown origin inside the
boundary. kage must also get a config that points at the broker, because the
guest never holds a real credential.

## Premise

- READ: kage publishes static musl archives `kage-x86_64-linux.tar.xz` and
  `kage-aarch64-linux.tar.xz`
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:.github/workflows/release.yml:39-40`).
  Each archive holds the binary kage in a directory kage-NAME, with LICENSE,
  README.md and man/kage.1 (same file, lines 71-88).
- READ: the release attaches a GitHub build-provenance attestation to each
  archive, and publishes no checksum file (same file, lines 124-130).
- READ: kage at this pin is version 0.3.0
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:Cargo.toml:22`).
- READ: the built-in providers read ANTHROPIC_API_KEY, OPENAI_API_KEY and
  GEMINI_API_KEY
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/guide/providers.md:12-18`).
  `[providers.<id>]` can replace `base_url` and `api_key_env` (same file,
  lines 176-190).
- READ: kage reads its config from the directory kage under
  XDG_CONFIG_HOME
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-cli/src/paths.rs:44-48`).
  An environment variable `KAGE_<SECTION>__<KEY>` overrides a key
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:docs/guide/config.md:175`).
- READ: provider tables are read from the user config only, never from a
  project file
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:crates/kage-core/src/config.rs:255-258`).
- READ: a verifier reports four outcomes: verified, nothing published to
  verify, no verifier installed, failed
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:94-115`).

## Approach

1. Add table `[kage]` to images/pins.toml. It holds the release tag, and per
   arch the asset name, the URL and a sha256 that this project records.
2. Add the signer workflow to the same table. It is the file
   .github/workflows/release.yml of QaidVoid/kage.
3. Fetch on the host side of xtask, outside the runner container, in
   xtask/src/image/fetch.rs. Download to .work/images/fetch/.
4. Compare the sha256 with the pin. On a mismatch, delete the file and stop.
5. Then verify the attestation with this command:

   ```sh
   gh attestation verify FILE --repo QaidVoid/kage \
     --signer-workflow QaidVoid/kage/.github/workflows/release.yml
   ```

6. Report one of the four outcomes. Any outcome other than "verified" stops
   the build.
7. Pass the verified archive into the runner: with wsl-toolkit `--input` on
   Windows, and through .work/ on Linux.
8. Extract only the file kage from the archive. Install it as
   /usr/local/bin/kage, owner root, mode 0755, with a `--customize-hook` of
   T-062.
9. Install no kage config file in the image. box-guest renders it at each
   start of kage, on the root-owned tmpfs /run/agent-box/kage/config (T-160).
10. docs/guest.md, section "Configuration pins", holds the values: each
    provider `base_url` points at a broker endpoint. The openai endpoint ends
    in `/v1`.
11. Put no key value in the image. box-guest sets the key variables to the
    per-instance nonces at kage start (T-046).
12. box-guest also sets the KAGE_* pins of T-160 at kage start, for example
    KAGE_SHELL__PROGRAM and KAGE_SHELL__SCRUB_ENV. The image holds no value
    for them.
13. Run the installed binary once in the build: `kage --version` must print
    the pinned version. A mismatch fails the build.

## Prove

```sh
cargo xtask image build --variant base --arch x86_64
cargo xtask image verify --variant base --arch x86_64
cargo test -p xtask image::fetch::refuses_wrong_digest
```

Expected: the build exits 0, and `verify` shows the attestation outcome
"verified". The build runs `kage --version` and reads the pinned version. The
unit test plants a wrong sha256 and sees the file deleted and the build
refused. Host: any host with the Linux runner and `gh`.

The boot check is `kage_version` in T-088 and T-105: `kage --version` as uid
1000 in the guest prints the pin.

## Start condition

T-062.

# T-064: Toolchain variants and the variant rule

**Source:** The planning session of 2026-10-14 (variants in
[docs/guest.md](../docs/guest.md)); research document section 6, "Two images,
not one".
**Category:** feature
**Milestone:** M2
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

A coding agent needs a compiler toolchain, but not every instance needs every
toolchain. One image with all toolchains costs disk and host page cache for
every instance. A writable shared layer between variants would be a shared
writable cache between instances, which the design forbids.

## Premise

- READ (design source): a base image and a variant per toolchain, so that a
  toolchain is not resident in every instance. No image is ever made writable
  after build. The number of variants is bounded by a named approver
  (research document, section 6).
- READ: a moving reference is resolved once to an immutable one and recorded
  in a committed lock file. The tool never fetches the moving reference
  itself
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:119-143`).
- Not measured: the size of each variant, and its boot time.

## Approach

1. Build each variant as a complete image, never as a layer over base:
   `cargo xtask image build --variant rust|node|python --arch ARCH`.
2. A variant recipe is images/variants/NAME.toml. It names the base recipe
   and the extra packages.
3. Take toolchains from Debian packages of the same snapshot first. Then one
   supply chain and one verification path cover every variant.
4. Start with these package sets. Each set is a design default for review.
   - rust: build-essential, pkg-config, rustc, cargo.
   - node: build-essential, nodejs, npm.
   - python: build-essential, python3, python3-venv, python3-pip.
5. A toolchain from upstream (for example a newer Rust) is a pins change. Pin
   its tarball by sha256 and give its verification tier.
6. Add the field `approved_by` to each `[variant.NAME]` table of
   images/pins.toml. The build refuses a variant without it.
7. A new variant, or a change of its package set, is a pull request that
   changes images/pins.toml. The approver named in the row reviews it.
8. Each variant gets its own fixed filesystem UUID and its own `[[artifact]]`
   rows.
9. A profile chooses the variant (T-027). An instance never changes its
   variant after create.

## Prove

```sh
cargo xtask image build --variant rust --arch x86_64
cargo xtask image build --variant rust --arch x86_64 --fresh
cargo xtask image verify --variant rust --arch x86_64
cargo test -p xtask image::refuses_variant_without_approver
```

Expected: the two builds of each variant give the same digests, and `verify`
exits 0. The unit test removes `approved_by` and sees the build refused. Host:
any host with the Linux runner.

The boot check is `boot_variants` in T-088 and T-105. It runs
`cargo --version`, `node --version` and `python3 --version` in the matching
variants.

## Start condition

T-062 and T-063.

# T-065: Base image format: erofs or ext4, and the VHDX copy

**Source:** The planning session of 2026-10-14 (image format in
[docs/guest.md](../docs/guest.md)); the engine evaluation.
**Category:** measurement
**Milestone:** M2
**Priority:** P2
**Effort:** S
**Status:** open

## Problem

The base image is the read-only lower layer of every instance. Its format sets
its size, its boot cost and its page-cache cost on the host. HCS attaches only
virtual disks, so Windows needs a VHDX copy. A VHDX made with qemu-img is not
byte-reproducible, so its digest needs a rule.

## Premise

- READ: erofs and ext4 both build reproducibly from a tarball (mkfs.erofs `-T`
  and `-U`; mmdebstrap ext4 with SOURCE_DATE_EPOCH), see T-062.
- READ: the HCS attachment types are VirtualDisk, Iso and PassThru, and
  Attachment has the field ReadOnly
  (https://learn.microsoft.com/en-us/virtualization/api/hcs/schemareference,
  read 2026-10-14).
- READ: qemu-img writes a new random FileWriteGuid and DataWriteGuid into each
  VHDX it creates
  (`qemu/qemu@4fc49f46dc95d4a27de2509e7fceb2931e91faeb:block/vhdx.c:1535-1536`),
  and a random page 83 identifier (same file, line 1619).
- READ: Firecracker reads a drive through the host page cache, with no
  O_DIRECT, reported by the engine evaluation
  ([docs/research/engines-2026-10.md](../docs/research/engines-2026-10.md)).
- Not measured: boot time, image size and page-cache cost of either format.
- Not known: whether two running HCS compute systems can attach the same VHDX
  file with ReadOnly true at the same time. No reference was read on it.

## Approach

1. Build the base image in both formats from the same rootfs.tar (T-062).
2. Measure on a Linux KVM host with Firecracker. Record:
   - the file size on the host;
   - the time from InstanceStart to the port 5000 handshake, 10 boots each;
   - the host page cache of the base with 1 and with 4 instances (the
     Cached line of /proc/meminfo);
   - the guest memory in use after boot (/proc/meminfo in the guest).
3. Measure the same boot time under HCS on Windows 11 Pro.
4. For Windows, compare two ways to make the VHDX:
   - (a) CI converts with `qemu-img convert -f raw -O vhdx -o subformat=dynamic`.
     The VHDX digest then changes per build. Pin the published asset, and
     check that `qemu-img convert -O raw` gives back the reproducible raw
     digest.
   - (b) The Windows helper makes the VHDX at import with CreateVirtualDisk,
     then writes the verified raw bytes into it. The content then has one
     digest, but the helper needs a write path to an attached disk.
5. Attach the base VHDX read-only to two compute systems at once. If HCS
   refuses, each instance needs a differencing VHDX over the base. Record the
   result for T-103.
6. Hash the base VHDX before and after a boot. A read-only attach must not
   change the file.
7. Record the root device naming of T-061. If a stable name needs a partition
   table, put a GPT with a fixed partition UUID into both formats.
8. Write the numbers, the host, the versions, the sample count and the date to
   docs/status.md. Write the format decision as a row in docs/decisions.md.

## Prove

```sh
cargo xtask image build --variant base --arch x86_64 --format erofs
cargo xtask image build --variant base --arch x86_64 --format ext4
cargo test -p box-daemon --test linux_boot -- --ignored image_format_bench
cargo test -p box-daemon --test windows_boot -- --ignored vhdx_shared_readonly
```

Expected: docs/status.md has the table with conditions, and docs/decisions.md
has the format row and the VHDX rule. Hosts: a Linux KVM host for the third;
Windows 11 Pro with Hyper-V, as admin, for the fourth.

## Start condition

T-062, T-088 and T-105.

# T-066: Pins and verification: box-image

**Source:** The planning session of 2026-10-14 (crate graph in
[docs/architecture.md](../docs/architecture.md)); research document section 6,
verification tiers.
**Category:** feature
**Milestone:** M2
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The host hands the kernel and the base image to a VMM, and the VMM parses
guest data. A wrong file here is wrong code in every instance. Every image use
must therefore check the bytes against a pin, and a mismatch must never fall
back to an unverified copy.

## Premise

- READ: on a digest mismatch, delete the cached copy and run nothing. No
  network and no verified cache is an error, not a fall-back
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:76-92`).
- READ: three verification tiers answer three questions: a sums file proves
  transport, a reviewed digest proves the bytes, a signature proves provenance
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:94-115`).
- READ: a digest comes from the server of the file. A digest taken from a
  checked-out tree can differ, because a CRLF checkout changes the bytes (same
  file, lines 76-92).
- Design: box-image is layer 1 in the crate graph and holds no network code
  ([docs/architecture.md](../docs/architecture.md)). The daemon downloads.

## Approach

1. Define the pins file. images/pins.toml has `schema = 1` and these tables:
   `[snapshot]`, `[kernel]`, `[kage]`, `[firecracker]`, `[variant.NAME]` and
   `[[artifact]]`.
2. An `[[artifact]]` row has `name`, `arch`, `kind` (kernel, rootfs, vhdx),
   `sha256`, `size` and `url`.
3. Parse it in crates/box-image/src/pins.rs with serde `deny_unknown_fields`.
   An unknown key or an unknown schema number is an error.
4. Verify in crates/box-image/src/verify.rs. Stream the file through SHA-256
   (the `sha2` crate) and compare the size first, then the digest.
5. Define the cache in crates/box-image/src/cache.rs. The layout is
   STATE/images/SHA256/NAME, where STATE is the daemon's state directory.
6. Write a new file as a temporary file in the same directory. fsync it,
   verify it, then rename it into place.
7. Verify again before each use. On a mismatch, delete the file and return
   `ImageError::DigestMismatch`. Never use the file.
8. Return `ImageError::Unavailable` when the file is missing and no download
   is possible. There is no fall-back.
9. Embed the pins in the release binary. A build script of box-image reads
   images/pins.toml and emits a constant table, `box_image::EMBEDDED`.
10. Report the tier that ran for each artifact: transport, reviewed digest or
    attestation, with the four TEMPLATE outcomes.
11. The engine store copy is not in this crate. The helpers copy a verified
    image into the root-owned store at create (T-087, T-103).

## Prove

```sh
cargo test -p box-image
cargo test -p box-image verify::refuses_planted_digest
cargo test -p box-image cache::mismatch_deletes_file
```

Expected: all tests pass. A planted wrong digest is refused, and the cached
file is gone after the refusal. A pins file with an unknown key fails to
parse. Host: any host.

## Start condition

T-033 (the product crates).

# T-067: `agent-box image pull|list|verify|gc`

**Source:** The planning session of 2026-10-14 (CLI verbs in
[docs/cli.md](../docs/cli.md)).
**Category:** feature
**Milestone:** M2
**Priority:** P1
**Effort:** S
**Status:** open

## Problem

The operator needs a way to fetch the pinned images before the first create,
to check them, and to remove old ones. A garbage collector that removes an
image an instance still uses breaks that instance at its next start.

## Premise

- Design: the CLI verbs are `image pull|list|verify|gc`, and each takes
  `--json` ([docs/cli.md](../docs/cli.md)).
- Design: an instance record holds the digests of its kernel and base image
  (T-026).
- Design: the cache layout and the verification are in box-image (T-066).

## Approach

1. Add the verbs to crates/box-cli. Each verb calls one daemon method over
   the control API: `image.pull`, `image.list`, `image.verify` and
   `image.gc`.
2. `image pull [VARIANT] [--arch ARCH]` downloads each `[[artifact]]` row of
   the embedded pins in crates/box-daemon/src/image.rs. Use rustls and
   certificate verification.
3. Write each download through the box-image cache. A mismatch deletes the
   file and exits non-zero with the artifact name.
4. `image list` prints each cached artifact with name, arch, sha256, size,
   and the instances that use it.
5. `image verify [SHA256]` hashes each cached file again and prints the
   result per file. It exits 1 when any file fails.
6. `image gc [--dry-run]` removes cached artifacts that the embedded pins do
   not name and that no instance record names.
7. gc never removes a file that an instance record names, whatever its pins
   state. The helper store has its own rule (T-091, T-107).
8. Each verb prints JSON with `--json`. The exit codes follow
   [docs/cli.md](../docs/cli.md).

## Prove

```sh
cargo test -p box-cli image::
cargo test -p box-daemon image::gc_keeps_used_images
agent-box image pull base --json
agent-box image verify --json
```

Expected: the tests pass, and gc keeps a planted image that an instance record
names. After T-068, `pull` fetches the published assets and `verify` exits 0.
Hosts: any host for the tests; any host with network for the last two.

## Start condition

T-066. The live pull needs T-068.

# T-068: Image CI workflow and release assets with attestations

**Source:** The planning session of 2026-10-14 (repository layout in
[docs/architecture.md](../docs/architecture.md)); research document section 6,
verification tiers.
**Category:** release
**Milestone:** M2
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

Operators must not build kernels and images on their own hosts to use
agent-box. The published assets must be the same bytes that the pins name, and
an operator must be able to check where they came from.

## Premise

- READ: kage attaches build-provenance attestations with the action
  `actions/attest-build-provenance@4d101475d8b20a2381f78447822ac1eab6504dd8`
  (v4.2.2), pinned by its commit, with the permissions
  `attestations: write`, `contents: write` and `id-token: write`
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:.github/workflows/release.yml:21-24`,
  and lines 127-130).
- READ: kage builds aarch64 Linux binaries on ubuntu-latest by cross
  compilation (same file, line 40).
- Not measured: the run time of a kernel and image build on a hosted runner.

## Approach

1. Add .github/workflows/images.yml. Triggers: `workflow_dispatch`, and a
   push of a tag `images-v*`.
2. Pin every action by a full commit SHA, with the tag in a comment.
3. Set the permissions to `contents: write`, `id-token: write` and
   `attestations: write` on the release job only. Other jobs get
   `contents: read`.
4. Job `kernel`: a matrix of x86_64 and aarch64 on ubuntu runners. It runs
   `cargo xtask image kernel --arch ARCH`.
5. Job `images`: a matrix of base, rust, node and python, times x86_64 and
   aarch64. It runs `cargo xtask image build --variant V --arch A`.
6. Job `vhdx`: converts the x86_64 images to VHDX by the rule of T-065.
7. Job `reproduce`: builds base x86_64 a second time and compares its digests
   with the pins. A difference fails the run.
8. Job `release`: writes SHA256SUMS over every asset. It runs the action
   actions/attest-build-provenance for each asset, then uploads with
   `gh release upload`.
9. Fail the run when a built digest differs from images/pins.toml. The pins
   change in a reviewed commit before the tag, never in CI.
10. Record the source commit of the run in the release notes and in a file
    asset. The release check of T-200 compares `PROTOCOL_VERSION` at that
    commit.
11. Publish one licence manifest per image, with the content that T-202
    gives.

## Prove

```sh
gh workflow run images.yml --ref main
gh release download images-v0.1.0 --dir .work/rel
gh attestation verify .work/rel/base-x86_64.erofs --repo Azathothas/agent-box --signer-workflow Azathothas/agent-box/.github/workflows/images.yml
cargo xtask image verify --dir .work/rel
```

Expected: the run is green, and its URL goes into docs/status.md. The
attestation check passes for each asset, and `verify` matches each file
against the pins. Host: any host with `gh` and network.

## Start condition

T-060 and T-062. The `vhdx` job takes the rule that T-065 decides. Until
then, it uses candidate (a) of T-065.

# T-069: Image ownership and rebuild cadence

**Source:** Research document sections 1.4 and 6; errand Task G
(https://github.com/QaidVoid/errand/issues/5, revision 3, as summarized in the
research document).
**Category:** decision
**Milestone:** M2
**Priority:** P2
**Effort:** S
**Status:** open

## Problem

A microVM moves the kernel patch cadence from the distribution to this
project. If nobody owns the rebuilds, the kernel and the base image grow old
and keep known defects. errand Task G asks who owns the image supply chain and
leaves the answer open.

## Premise

- READ (design source): without a named owner, the answer to errand Task G is
  "no", and the tier is not scheduled (research document, section 6).
- READ (design source): the research document asks for a named owner of the
  rebuilds and patches, written beside the pin. It prefers a release tag to a
  commit (research document, section 6).
- READ: Firecracker supports each minor release for at least 6 months, as the
  engine evaluation reports
  ([docs/research/engines-2026-10.md](../docs/research/engines-2026-10.md)).

## Approach

1. Ask the operator to name the owner. The owner rebuilds and patches the
   kernel, the base image, the variants and the kage pin.
2. Add the field `owner` to table `[snapshot]` of images/pins.toml.
   `cargo xtask image verify` fails when it is empty.
3. Propose these rebuild triggers for the decision:
   - a new 6.18.y point release that fixes a security defect;
   - a Debian security advisory for a package in any image;
   - a new kage release that the operator accepts;
   - a new Firecracker release that the pins move to.
4. Propose a maximum image age. The operator sets the number; this entry does
   not invent one.
5. Write the decision as a row in docs/decisions.md, with the date and the
   owner.

## Prove

```sh
cargo xtask image verify --variant base --arch x86_64
cargo xtask check docs
```

Expected: `verify` exits 0 only with a non-empty `owner`. docs/decisions.md
has the row with the owner, the triggers and the maximum age. Host: any host.
