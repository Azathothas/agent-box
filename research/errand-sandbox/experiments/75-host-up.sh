#!/bin/sh
# Question: after this host restarts, can one idempotent command bring back
# everything the experiments need? It must, because the host has no init.
# Measured on 2026-10-23: the machine restarted mid-session. Disk state
# survived (packages, binaries, images, /etc/docker/daemon.json). Everything
# held in memory did not: device modes, sysctls, and every daemon.
# Safe to run any number of times. Each step checks before it acts.
# Run as root. Exit: 0 all up, 1 something did not come up, 2 could not run.
set -u
[ "$(id -u)" = 0 ] || { echo "run as root"; exit 2; }
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
sb="$repo/.work/sysbox"
fail=0
say() { printf '%-28s %s\n' "$1" "$2"; }
up() { # name check-command start-command
  if sh -c "$2" >/dev/null 2>&1; then say "$1" "already up"; return; fi
  sh -c "$3"
  for _ in $(seq 30); do sh -c "$2" >/dev/null 2>&1 && { say "$1" "started"; return; }; sleep 1; done
  say "$1" "FAILED"; fail=1
}

# Device modes the sandboxes need. Distributions ship both at 0666; this
# platform boots them at 0600.
for d in /dev/net/tun /dev/fuse; do
  [ "$(stat -c %a "$d")" = 666 ] && say "$d" "0666 already" || { chmod 666 "$d" && say "$d" "set to 0666"; }
done

# Sysbox's kernel limits (engines-shortlist.md section 1.1).
sysctl -q -w fs.inotify.max_queued_events=1048576 fs.inotify.max_user_watches=1048576 \
  fs.inotify.max_user_instances=1048576 kernel.keys.maxkeys=20000 kernel.keys.maxbytes=1400000 \
  kernel.pid_max=4194304 && say "sysctls" "applied"

up dockerd "docker info" "nohup dockerd > /tmp/dockerd.log 2>&1 &"
if [ -x "$sb/sysbox-mgr-patched" ]; then
  up sysbox-mgr "test -S /run/sysbox/sysmgr.sock && pgrep -f '^$sb/sysbox-mgr-patched'" \
     "rm -f /run/sysbox/sysmgr.sock; nohup $sb/sysbox-mgr-patched > /tmp/sysbox-mgr.log 2>&1 &"
  up sysbox-fs "test -S /run/sysbox/sysfs.sock && pgrep -f '^$sb/sysbox-fs-patched'" \
     "rm -f /run/sysbox/sysfs.sock; nohup $sb/sysbox-fs-patched > /tmp/sysbox-fs.log 2>&1 &"
fi
# lxcfs gives guests a /proc/meminfo, cpuinfo and uptime that match their
# limits. Without it a guest sees the host's 16 GB and every CPU. It is a
# root FUSE daemon that serves guests, so it widens the trusted surface.
if command -v lxcfs >/dev/null; then
  up lxcfs "grep -q ' /var/lib/lxcfs fuse.lxcfs ' /proc/mounts" "mkdir -p /var/lib/lxcfs; nohup lxcfs /var/lib/lxcfs > /tmp/lxcfs.log 2>&1 &"
fi
if [ -x /usr/libexec/incus/incusd ]; then
  up incusd "incus info" "mkdir -p /var/log/incus; nohup /usr/libexec/incus/incusd --group root > /tmp/incusd.log 2>&1 &"
fi

# The platform's proxy listens on loopback only; relay it onto each bridge.
# Guests always dial the same relay port, so nothing inside them changes. The
# upstream is read from HTTPS_PROXY on every run: on 2026-10-23 the platform's
# proxy moved from port 33337 to 42151 across a restart, and a relay with the
# old upstream answered nothing while it still looked "up".
relay_port=${RELAY_PORT:-33337}
upstream=${PROXY_UPSTREAM:-$(printf '%s' "${HTTPS_PROXY:-}" | sed -n 's#^[a-z]*://\([^/@]*@\)\{0,1\}\([^/]*\).*#\2#p')}
relay() { # bridge-address
  [ -n "$1" ] || return
  [ -n "$upstream" ] || { say "relay $1" "skipped: no HTTPS_PROXY or PROXY_UPSTREAM"; return; }
  want="socat TCP-LISTEN:$relay_port,bind=$1,fork,reuseaddr TCP:$upstream"
  for pid in $(pgrep -f "^socat TCP-LISTEN:$relay_port,bind=$1,"); do
    [ "$(tr '\0' ' ' < /proc/"$pid"/cmdline | sed 's/ $//')" = "$want" ] && { say "relay $1" "already up, to $upstream"; return; }
    kill "$pid"   # a relay to a stale upstream, by pid
  done
  sleep 1
  nohup $want > /tmp/relay-"$1".log 2>&1 &
  sleep 1; pgrep -f "^$want\$" >/dev/null && say "relay $1" "started, to $upstream" || { say "relay $1" "FAILED"; fail=1; }
}
relay "$(docker network inspect bridge -f '{{(index .IPAM.Config 0).Gateway}}' 2>/dev/null)"
command -v incus >/dev/null && relay "$(incus network get incusbr0 ipv4.address 2>/dev/null | cut -d/ -f1)"

# Thin pools give space back to the host only when trimmed (section 11).
for pool in /var/lib/incus/storage-pools/*; do
  grep -q " $pool btrfs " /proc/mounts && fstrim "$pool" 2>/dev/null && say "fstrim $(basename "$pool")" "done"
done

# Proof, not assertion: one container per engine.
docker info 2>/dev/null | grep -q sysbox-runc &&
  { docker run --rm --runtime=sysbox-runc debian:trixie-slim true >/dev/null 2>&1 && say "sysbox container" "runs" || { say "sysbox container" "FAILED"; fail=1; }; }
command -v incus >/dev/null &&
  { incus launch -q bench upcheck >/dev/null 2>&1 && incus exec upcheck -- true && incus delete -f upcheck >/dev/null &&
    say "incus container" "runs" || { say "incus container" "FAILED"; incus delete -f upcheck >/dev/null 2>&1; fail=1; }; }
echo "uptime: $(cut -d' ' -f1 /proc/uptime) s, date: $(date -u +%FT%TZ)"
exit $fail
