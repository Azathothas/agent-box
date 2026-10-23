#!/bin/sh
# Question: which gVisor do the other experiments run, exactly?
# Fetches the pinned release into .work/bin/gv and verifies its sha512.
# Exit: 0 fetched and verified, 1 digest mismatch, 2 could not fetch.
set -u
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
out="$repo/.work/bin"
release=20261019
# pin: gvisor release-20261019.0 x86_64 gvisor.tar.bz2
sha512=7cbc2aeeee809dcccfb40e612504a1b76f2c119790cee59e482f7b35428cccc4f71705e7e322442d07d1f7d29b5d9e72e31319fc3088d6dcbec9fc7c1f0d8881
url="https://storage.googleapis.com/gvisor/releases/release/$release/x86_64/gvisor.tar.bz2"

mkdir -p "$out" || exit 2
if [ ! -f "$out/gvisor.tar.bz2" ]; then
  curl -sSfo "$out/gvisor.tar.bz2" "$url" || exit 2
fi
got=$(sha512sum "$out/gvisor.tar.bz2" | cut -d' ' -f1)
if [ "$got" != "$sha512" ]; then
  echo "digest mismatch: $got"
  exit 1
fi
mkdir -p "$out/gv" && tar -xjf "$out/gvisor.tar.bz2" -C "$out/gv" || exit 2
"$out/gv/runsc" --version
echo "host: $(uname -srm); kvm: $( [ -e /dev/kvm ] && echo yes || echo no ); date: $(date -u +%F)"
