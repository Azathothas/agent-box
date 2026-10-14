# Release

This area covers milestone M9: the release workflow, the install guide with
its end-to-end check on fresh hosts, and the third-party licences. The exit
criteria are in [docs/roadmap.md](../docs/roadmap.md#m9-release).
[docs/development.md](../docs/development.md#releases) gives the release
rules of the tooling. [docs/guest.md](../docs/guest.md#the-image-supply-chain)
gives the verification tiers that the binaries and the images share.

A release ships two host binaries, `agent-box` and `agent-box-helper`. The
operator installs the helper as root or LocalSystem (D3), so a release is a
privileged install. The guest images publish from their own workflow (T-068).
A binary release names the images that it boots by digest. The product crates
do not exist yet, and T-033 makes them. A crate path in this file names a
planned file.

# T-200: Release workflow

**Source:** brief sections 8 and 12; [docs/roadmap.md](../docs/roadmap.md#m9-release); the planning session of 2026-10-14 (aarch64 release binaries serve 4K-page hosts only); the kage release workflow (READ, below).
**Category:** release
**Milestone:** M9
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

The operator runs `agent-box setup --system` as root or as an administrator.
A binary of unknown origin with that privilege can take the host. An operator must be
able to check the commit and the workflow that built each binary. Each binary
must also boot exactly the kernel and images that were tested with it. A
release that is public before its attestations exist gives a window in which
nobody can verify a download.

## Premise

- Design (brief section 8): the host binaries are `agent-box` (box-cli) and
  `agent-box-helper` (box-helper). box-guest is not a host artifact. It goes
  into the image (T-062).
- Design (T-066): a build script of box-image embeds the pins of
  images/pins.toml in the binary as `box_image::EMBEDDED`.
- Design (T-174): `agent-box setup --system` checks the sha256 of the helper
  against a digest that the release build embeds.
- Design (T-068): the images publish under tags `images-v*`, from
  .github/workflows/images.yml, with SHA256SUMS and attestations.
- Design ([docs/protocol.md](../docs/protocol.md)): a protocol version
  mismatch refuses the session, and the instance gets no egress. So a binary
  and the images that it names must carry the same `PROTOCOL_VERSION`.
- READ: kage builds its static musl binaries for x86_64 and aarch64 with
  `cross`, on an x86_64 runner
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:.github/workflows/release.yml:39-40`).
- READ: kage pins `cross` to version 0.2.5, and sets
  `-C target-feature=+crt-static` for those builds (same file, lines 54-65).
- READ: kage lists the files of each archive and fails on a missing file
  before the upload (same file, lines 90-114).
- READ: kage attests each archive with actions/attest-build-provenance (same
  file, lines 124-130).
- READ: the kage workflow runs when a release is already published, and it
  attests after the upload (same file, lines 7-9 and 116-130). A download in
  that window has no attestation to check.
- READ: Firecracker tests aarch64 only on AWS Graviton 2 to 5, and lists only
  host kernels with 4K pages
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:README.md:133-153`,
  `firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:docs/kernel-policy.md:26-30`).
- Design (the planning session of 2026-10-14): aarch64 Linux release binaries
  are for hosts with 4K pages only. A 16K-page host stays a backlog
  measurement (T-094).
- READ: three verification tiers answer three questions: transport, the
  reviewed bytes, and provenance
  (`Azathothas/TEMPLATE@ea26c7d91a087a7132f56034fb77f8d40d7766cb:docs/containers.md:94-115`).
- Not measured: the run time of the release workflow on hosted runners.

## Approach

1. Files: .github/workflows/release.yml and xtask/src/release.rs (planned).
   The xtask module adds `cargo xtask release check|notes|verify`.
   `.github/workflows/ci.yml` gets a `workflow_call` trigger.
2. Triggers: a push of a tag `v*`, and `workflow_dispatch` for a dry run. A
   dry run builds and checks. It creates no release and no attestation.
3. Pin each action by a full commit, with its tag in a comment, as `.github/workflows/ci.yml` does.
4. Set the default permissions to `contents: read`. Give `contents: write`,
   `id-token: write` and `attestations: write` to the publish job only.
5. Job `check` runs `cargo xtask release check --tag TAG`. It fails the run
   on any of these conditions:
   - the tag `vX.Y.Z` differs from the workspace version in `Cargo.toml`;
   - the tagged commit is not on `main`;
   - images/pins.toml does not parse, or an artifact row has no URL;
   - an image artifact fails its size, sha256 or attestation check;
   - the images release was built from a commit with another
     `PROTOCOL_VERSION`.
6. The check reads `PROTOCOL_VERSION` at the images commit with `git show`.
   The images release records that commit (T-068).
7. Job `gate` calls the workflow of `.github/workflows/ci.yml` at the tag
   (`workflow_call`). A release never ships a commit that the strict gate did
   not pass on Linux and Windows.
8. Job `build` is a matrix of three targets:

| Target | Runner | Tool |
| --- | --- | --- |
| x86_64-unknown-linux-musl | ubuntu-24.04 | `cross` at a pinned version |
| aarch64-unknown-linux-musl | ubuntu-24.04 | `cross` at a pinned version |
| x86_64-pc-windows-msvc | windows-2025 | cargo |

9. Build with `--locked --release` and `RUSTFLAGS=-C target-feature=+crt-static`.
   The Linux binaries are static musl. The Windows binaries then need no
   Visual C++ runtime on the host.
10. In each matrix job, build `agent-box-helper` first and compute its
    sha256. Then build `agent-box` with `AGENT_BOX_HELPER_SHA256` set.
11. The build script of box-cli embeds that digest. A release build without
    the variable fails. A debug build skips the embedded check (T-174).
12. `agent-box --version` prints the version, the source commit and the
    target. The startup report (T-175) adds the embedded image digests.
13. Package each target as `agent-box-VERSION-TARGET.tar.xz` on Linux and
    `agent-box-VERSION-TARGET.zip` on Windows.
14. Each archive holds the two binaries, `LICENSE`, `README.md`, and the
    licence file of T-202. A step lists each archive and fails on a missing
    file.
15. Job `publish` creates a draft release for the tag and uploads every
    archive. It writes SHA256SUMS over every asset and uploads it.
16. Then it attests each asset with actions/attest-build-provenance. It
    publishes the draft last.
17. A failure in any step leaves a draft. A published release always has
    every asset and every attestation.
18. `cargo xtask release notes --tag TAG` writes the release notes:
    - the images tag, and the digest of each kernel and image;
    - the supported hosts, with the limits below;
    - a link to the isolation section of SECURITY.md;
    - the verify commands of T-201.
19. The notes and the README in each aarch64 archive state the limit:
    aarch64 Linux hosts with 4K pages only.
20. On a host with another page size, the daemon reports the gap
    `page-size-unsupported` (T-092). No release claims 16K-page support
    before T-094 has a measured boot.
21. The notes name Windows 11 Pro, Enterprise and Education with Hyper-V.
    Windows 11 Home stays unverified until T-109 (question Q6 of
    `TODO/PROGRESS.md`).
22. `cargo xtask release verify --tag TAG --dir DIR` checks a downloaded
    release on any host. It checks SHA256SUMS, each attestation and the file
    list of each archive.
23. For the archive of the host target, `verify` also runs `agent-box
    --version`. It compares the version, the commit and the embedded image
    digests with the tag.
24. The operator bumps the version and pushes the tag. The workflow never
    creates a tag and never edits a file.
25. The first release adds CHANGELOG.md (brief section 10). Until then no
    changelog exists.

## Decision

"Signed" in the M9 exit criterion means attested. Each asset carries a GitHub
build-provenance attestation, which `gh attestation verify` checks against
this repository and the release workflow. Authenticode signing of the Windows
binaries is not part of v1. T-201 records whether SmartScreen or Defender
blocks an attested, unsigned binary on a fresh host. A block becomes a
question for the operator.

## Prove

```sh
cargo test -p xtask release::
cargo xtask release check --tag v0.1.0
gh release download v0.1.0 --dir .work/rel
gh attestation verify .work/rel/agent-box-0.1.0-x86_64-unknown-linux-musl.tar.xz --repo Azathothas/agent-box --signer-workflow Azathothas/agent-box/.github/workflows/release.yml
cargo xtask release verify --tag v0.1.0 --dir .work/rel
```

Expected: the unit tests refuse a tag that differs from the version, a
missing archive file, and an image built at another protocol version. The
release check exits 0 at the tag. The operator pushes the tag, and the run is
green; its URL goes into [docs/status.md](../docs/status.md).

Each asset passes `gh attestation verify`. `release verify` exits 0, and the
version, commit and image digests that the binary prints match the tag.

Host: any host with `gh` and network. The tag push is the operator's step.

## Start condition

T-007 (CI), T-066 (the embedded pins), T-068 (the images release), T-174
(the helper digest check) and T-180. The roadmap puts M9 after M8: no release
before the battery passes.

# T-201: Install guide and end-to-end verification

**Source:** brief sections 8 and 10; [docs/conventions.md](../docs/conventions.md); [docs/roadmap.md](../docs/roadmap.md#m9-release); the planning session of 2026-10-14 (verify aarch64 on a 4K-page host).
**Category:** release
**Milestone:** M9
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

A release that verifies in CI can still fail on a real host. A runtime
library can be missing, or a Windows feature can be off. A group membership
can need a new login, or SmartScreen can block the binary. The install guide is
the first thing an operator runs. Each step must run on a fresh host of each
supported kind, and the result must be on record.

## Premise

- Design (brief section 10): README.md holds the quick start. Today it shows
  the planned CLI only.
- Design ([docs/conventions.md](../docs/conventions.md)): the README quick
  start works in a fresh clone, and this entry tests it on both OSes.
- The roadmap lists a known gap: no entry boots an instance on a 4K-page
  aarch64 host, but T-200 releases aarch64 binaries. This entry closes it.
- Design (T-174): the live test of setup records the system state before
  setup and after uninstall, and the two records are equal.
- Design (T-200): the release has archives, SHA256SUMS and an attestation for
  each asset.
- READ: Firecracker tests aarch64 only on AWS Graviton 2 to 5
  (`firecracker-microvm/firecracker@95f868c8e345b1cc8faccd1a3c910b4989dc3f58:README.md:133-153`).
- Not measured: whether SmartScreen or Defender blocks an attested binary
  that has no Authenticode signature.
- Design (question Q6 of `TODO/PROGRESS.md`): Windows 11 Home is not a
  release target until T-109 passes.

## Approach

1. Replace the section "The planned CLI" of README.md with a section
   "Install" when T-200 has a release. Each step is one command and its
   expected result.
2. Keep the developer quick start of README.md: clone, `cargo xtask setup`,
   `cargo xtask doctor`, `cargo xtask gate --strict`.
3. Linux install steps:
   - download the archive and SHA256SUMS;
   - check the sum with `sha256sum -c`, and the provenance with `gh
     attestation verify`;
   - extract the archive, and run `sudo ./agent-box setup --system` in it;
   - copy `agent-box` into a directory on the PATH;
   - log in again, so that the group membership of setup applies;
   - run `agent-box setup` as the operator, without sudo.
4. The system part copies the helper from beside the running binary (T-174).
   So the guide runs `setup --system` from the extracted directory.
   [docs/cli.md](../docs/cli.md#setup-and-uninstall) gives the two parts.
5. Windows install steps:
   - download the zip and SHA256SUMS;
   - check the sum with `certutil -hashfile FILE SHA256`, and the provenance
     with `gh attestation verify`;
   - extract the zip, and run `agent-box setup --system` in an elevated
     terminal;
   - add the directory to the PATH of the user;
   - run `agent-box setup` in a normal terminal.
6. The guide installs `gh` for the provenance check. Without `gh`, the
   guide checks the sum only, and says that the sum does not prove the
   origin.
7. The check list after the install, on each host:
   - `agent-box doctor --json` reports no gap;
   - `agent-box image pull` and `agent-box image verify` exit 0;
   - `agent-box secret set NAME` takes the provider key from stdin;
   - create, start, run, attach, forward, pause, resume, stop, start, stop,
     delete and uninstall, with the expected result of each step.
8. `run` sends a small prompt, for example "print the kernel release". The
   result has exit 0 and a broker request count above zero (T-171).
9. `attach --shell` runs `uname -m`, `getconf PAGESIZE` and a file write.
   After stop and start, the file is still there.
10. `forward` uses the `python` variant (T-064): `python3 -m http.server 3000`
    in the guest, and `curl` from the host to the forwarded port.
11. `delete` prints a Removal with a verified absence for each item.
    `uninstall` prints its removal report.
12. Record the listings that T-174 names before setup and after uninstall.
    The two listings are equal.
13. The hosts, each a fresh install:
    - Linux x86_64 with KVM, not nested;
    - Linux aarch64 with KVM and 4K pages (`getconf PAGESIZE` prints 4096),
      for example an AWS Graviton host;
    - Windows 11 Pro x64 with Hyper-V, with no Visual C++ runtime and no
      developer tool except `gh`.
14. The Windows step records whether SmartScreen or Defender blocks setup.
    A block goes to `TODO/PROGRESS.md` as a question (T-200, Decision).
15. Record each result in [docs/status.md](../docs/status.md). Give the
    release tag, the date, the host kind, the OS build and the kernel. Give
    the page size, the CPU, and pass or fail per step.
16. A 16K-page aarch64 host is not in this check list. The status table
    names it as not tested (T-094).
17. A step that fails gets a defect entry from `cargo todo next`. When a fix
    changes a step, the guide changes in the same commit.

## Prove

```sh
sha256sum -c SHA256SUMS --ignore-missing
gh attestation verify agent-box-0.1.0-x86_64-unknown-linux-musl.tar.xz --repo Azathothas/agent-box
sudo ./agent-box setup --system
# log in again, so that the group membership applies
agent-box setup
agent-box doctor --json
agent-box image pull --json
agent-box secret set anthropic
agent-box create t201 --profile python
agent-box start t201
agent-box run t201 --prompt "print the kernel release" --json
agent-box stop t201
agent-box delete t201 --json
agent-box uninstall --purge --json
sudo agent-box uninstall --system --json
```

Expected: each command exits 0. `doctor` names no gap. The `run` result
carries a broker request count above zero. `delete` and `uninstall` list
each removed item with its verified absence. The check list of the Approach
passes, and docs/status.md has one row per host.

Host: three fresh hosts. A Linux x86_64 KVM host, a Linux aarch64 KVM host
with 4K pages, and Windows 11 Pro with Hyper-V as admin. The Windows run uses
the same verbs in an elevated terminal.

## Start condition

T-200, T-174 and T-175.

# T-202: Third-party licences

**Source:** brief section 10 (licence 0BSD); the licence policy of `deny.toml:34-44`; [docs/roadmap.md](../docs/roadmap.md#m9-release); the kage licence (READ, below).
**Category:** release
**Milestone:** M9
**Priority:** P2
**Effort:** S
**Status:** open

## Problem

The binaries link crates under MIT, Apache-2.0 and other licences. Most of
these licences require their notice in each copy that somebody distributes.
The images also carry code under other licences: the kernel, the Debian
packages and kage. The 0BSD licence covers only the code of this repository.

## Premise

- `deny.toml:34-44` lists the licences that cargo-deny accepts. The gate
  refuses any other licence in the dependency graph (T-007).
- READ: kage is under the MIT licence
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:Cargo.toml:28`).
  Its release archive holds its LICENSE file
  (`QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:.github/workflows/release.yml:78`).
- Design ([docs/guest.md](../docs/guest.md)): the image drops
  /usr/share/doc, but keeps the copyright file of each Debian package.
- Design (T-083): setup downloads Firecracker and the jailer from their own
  release. agent-box does not redistribute them.
- Design inference, not legal advice: a binary built from GPL code needs an
  offer of its corresponding source. The kernel and some Debian packages are
  such code.

## Approach

1. Add the planned module xtask/src/release/licences.rs with `cargo xtask
   release licences --target TARGET --out DIR`.
2. The command runs cargo-about over the dependency graph of box-cli and
   box-helper for that target. It writes THIRD_PARTY_LICENSES.txt.
3. `deny.toml` stays the one home of the accepted licences. The command
   writes the cargo-about configuration from it into `target/`, at each run.
4. A crate under a licence outside that list fails the command.
5. CI installs cargo-about at a pinned version, as `.github/workflows/ci.yml`
   installs cargo-deny.
6. T-200 puts THIRD_PARTY_LICENSES.txt into each archive and uploads it as an
   asset. The file-list check of T-200 requires it.
7. For the images, T-068 adds one licence manifest per image to the images
   release. The manifest holds each Debian copyright file, the kage
   LICENSE, and the kernel COPYING file.
8. The manifest also names the source of each GPL component. These are the
   kernel tarball URL and sha256, the kernel config of images/kernel/, and
   the Debian snapshot timestamp.
9. Question Q9 of [PROGRESS](PROGRESS.md) asks the operator whether that
   pointer is enough. Its recommendation adds the kernel tarball and its
   config as assets of the images release.

## Prove

```sh
cargo xtask release licences --target x86_64-unknown-linux-musl --out .work/lic
tar -tJf .work/rel/agent-box-0.1.0-x86_64-unknown-linux-musl.tar.xz
```

Expected: the first command exits 0 and writes
.work/lic/THIRD_PARTY_LICENSES.txt with an entry for each crate of the graph.
A planted dependency under a licence outside `deny.toml` makes it exit 1.
The archive listing contains THIRD_PARTY_LICENSES.txt. The images release has
one licence manifest per image.

Host: any host with cargo-about and network.

## Start condition

T-200 and T-068.
