#!/bin/sh
# Inside one sandbox: can a session mount FUSE filesystems, including one
# backed by network storage, and run podman? Prints PASS or FAIL per step.
#   PROXY_HOST  where the relayed proxy listens, as seen from this sandbox
#   PROXY_PORT  its port (default: the research host's, 33337)
#   IMAGE_TAR   a `docker save` of debian:trixie-slim, for podman to load
# Needs fuse3, rclone and podman in the image (bench-fuse:trixie).
host=${PROXY_HOST:-172.17.0.1}
ok() { echo "PASS $1"; }
no() { echo "FAIL $1: $2"; }
echo "dev/fuse: $(ls -l /dev/fuse 2>&1 | cut -c1-60)"

# 1. The FUSE mechanism, with no network: rclone serving a local directory.
mkdir -p /srv/local /mnt/loc && echo hello > /srv/local/f
if rclone mount /srv/local /mnt/loc --daemon --daemon-timeout 20s 2>/tmp/r1.err &&
   [ "$(cat /mnt/loc/f 2>/dev/null)" = hello ]; then
  ok "fuse mount (rclone, local dir): $(grep ' /mnt/loc ' /proc/mounts | cut -d' ' -f3)"
  fusermount3 -u /mnt/loc
else no "fuse mount (rclone, local dir)" "$(head -c 200 /tmp/r1.err)"; fi

# 2. Network storage over FUSE: the Debian mirror over HTTPS, read through
#    the mount. The proxy's CA is at /run/proxy-ca.crt.
mkdir -p /mnt/net
if HTTPS_PROXY="http://$host:${PROXY_PORT:-33337}" rclone mount :http: /mnt/net --http-url https://deb.debian.org/debian/ \
     --ca-cert /run/proxy-ca.crt --read-only --daemon --daemon-timeout 30s 2>/tmp/r2.err &&
   head -c 200 /mnt/net/dists/trixie/Release 2>/tmp/r2.read | grep -q '^Origin: Debian'; then
  ok "network storage over fuse (rclone :http:, $(du -b /mnt/net/dists/trixie/Release 2>/dev/null | cut -f1) bytes of Release read)"
  fusermount3 -u /mnt/net
else no "network storage over fuse" "$(head -c 200 /tmp/r2.err) $(head -c 200 /tmp/r2.read 2>/dev/null)"; fi

# 3. podman inside: load an image from a tarball, then run it.
if [ -f "${IMAGE_TAR:-/run/debian.tar}" ] && podman load -q -i "${IMAGE_TAR:-/run/debian.tar}" >/dev/null 2>/tmp/p.err; then
  out=$(podman run --rm --network=none debian:trixie-slim cat /etc/debian_version 2>>/tmp/p.err)
  if [ -n "$out" ]; then ok "podman run (driver $(podman info --format '{{.Store.GraphDriverName}}' 2>/dev/null), runtime $(podman info --format '{{.Host.OCIRuntime.Name}}' 2>/dev/null)): debian $out"
  else no "podman run" "$(tail -c 300 /tmp/p.err)"; fi
else no "podman load" "$(tail -c 300 /tmp/p.err)"; fi
