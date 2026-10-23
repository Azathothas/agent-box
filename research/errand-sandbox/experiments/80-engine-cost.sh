#!/bin/sh
# SUPERSEDED by 81, 2026-10-23. Kept because results/80-* came from it. This
# measured one engine after another, and this host's disk speed drifts over
# minutes, so its disk-backed rows compare moments rather than engines (see
# 81's header). Its tmpfs, CPU, cold-start and memory rows are not affected
# that way, but the cross-engine comparison is 81's.
# Question: on this no-KVM host, what does each engine that passed the
# docker-inside test cost an agent, against plain runc, on the same image and
# the same work?
# Engines: docker+runc (the control), docker+sysbox-runc (patched, section 1),
# docker+runsc (gVisor systrap, no docker-inside flags), incus 6.0 container.
# Measured from outside: host wall time around each step. The workload also
# times itself inside (lib/workload.sh), so start-up and work are separated.
# Two numbers per engine and workload:
#   exec_ms   the workload's own time, inside a container that is already up
#   cold_ms   host wall time to start a fresh container, run `true`, remove it
# Memory: MemAvailable on the host before and after starting 5 idle
# containers of that engine, divided by 5.
# Run as root, with experiment 70 and the Sysbox setup of engines-shortlist.md
# done, dockerd and incusd running, and the proxy relays up.
# Exit: 0 measured, 1 a workload failed on an engine, 2 could not run.
set -u
here=$(cd "$(dirname "$0")" && pwd)
runs=${RUNS:-5}
image=bench:trixie
# The egress proxy CA bundle of the host. The default is the research host's.
proxy_ca=${PROXY_CA:-/root/.ccr/ca-bundle.crt}
engines=${ENGINES:-"runc sysbox-runc runsc incus"}
[ "$(id -u)" = 0 ] || { echo "run as root"; exit 2; }
docker info >/dev/null 2>&1 || { echo "dockerd is not running"; exit 2; }

# The bench image: Debian 13 with python3, git, gcc and make, built once
# through the relayed proxy (engines-shortlist.md section 1.4).
if ! docker image inspect "$image" >/dev/null 2>&1; then
  b=$(mktemp -d); cp "$proxy_ca" "$b/ca.crt"; cp "$here/lib/apt-proxy.sh" "$b/" || exit 2
  cat > "$b/Dockerfile" <<'EOF'
FROM debian:trixie-slim
COPY ca.crt /run/proxy-ca.crt
COPY apt-proxy.sh /run/apt-proxy.sh
RUN sh /run/apt-proxy.sh && apt-get -qq update && \
    DEBIAN_FRONTEND=noninteractive apt-get -qq install -y --no-install-recommends python3 git gcc libc6-dev make && \
    rm -rf /var/lib/apt/lists/* /etc/apt/apt.conf.d/99proxy /run/proxy-ca.crt
EOF
  DOCKER_BUILDKIT=0 docker build -q -t "$image" "$b" >/dev/null || exit 2
  rm -rf "$b"
fi
wl="$here/lib/workload.sh"

median() { sort -n | awk '{a[NR]=$1} END {print (NR%2 ? a[(NR+1)/2] : int((a[NR/2]+a[NR/2+1])/2))}'; }
now() { date +%s%N; }
failed=0

# Long-lived container per engine, for exec_ms. Each gets the same two work
# directories: /workspace, a host directory bind-mounted in (what errand gives
# a session), and /scratch, a tmpfs (syscall cost with no disk under it).
ws_root=$(mktemp -d /var/tmp/b80.XXXXXX)
up() { # engine name
  mkdir -p "$ws_root/$2"
  case $1 in
    incus) incus delete -f "$2" >/dev/null 2>&1
           incus image info bench >/dev/null 2>&1 || import_incus_image || return 1
           incus init -q bench "$2" >/dev/null || return 1
           incus config device add "$2" ws disk source="$ws_root/$2" path=/workspace shift=true >/dev/null || return 1
           incus start "$2" && sleep 2 && incus file push -q "$wl" "$2/run/workload.sh" &&
             incus exec "$2" -- mount -t tmpfs -o exec tmpfs /scratch --mkdir 2>/dev/null ||
             incus exec "$2" -- sh -c 'mkdir -p /scratch && mount -t tmpfs -o exec tmpfs /scratch' ;;
    *)     docker rm -f "$2" >/dev/null 2>&1
           docker run -d --runtime="$1" --name "$2" -v "$wl:/run/workload.sh:ro" \
             -v "$ws_root/$2:/workspace" --tmpfs /scratch:exec "$image" sleep infinity >/dev/null ;;
  esac
}
in_box() { # engine name args...
  e=$1 n=$2; shift 2
  case $e in incus) incus exec "$n" -- "$@" ;; *) docker exec "$n" "$@" ;; esac
}
down() { case $1 in incus) incus delete -f "$2" >/dev/null 2>&1 ;; *) docker rm -f "$2" >/dev/null 2>&1 ;; esac; }
cold() { # engine: start, run true, remove
  case $1 in
    incus) incus launch -q bench coldtmp >/dev/null && incus exec coldtmp -- true && incus delete -f coldtmp >/dev/null ;;
    *)     docker run --rm --runtime="$1" "$image" true ;;
  esac
}
# The same packages for Incus, on its own Debian 13 image: Incus boots an
# init, which the docker image does not have. Published once as "bench".
import_incus_image() {
  incus launch -q images:debian/13 benchbuild >/dev/null || return 1
  sleep 3
  incus file push -q "$proxy_ca" benchbuild/run/proxy-ca.crt
  incus file push -q "$here/lib/apt-proxy.sh" benchbuild/run/apt-proxy.sh
  incus exec benchbuild --env PROXY_HOST="$(incus network get incusbr0 ipv4.address | cut -d/ -f1)" -- sh -c \
    'sh /run/apt-proxy.sh && apt-get -qq update && DEBIAN_FRONTEND=noninteractive apt-get -qq install -y --no-install-recommends python3 git gcc libc6-dev make >/dev/null && rm -f /etc/apt/apt.conf.d/99proxy /run/proxy-ca.crt' || return 1
  incus stop benchbuild && incus publish -q benchbuild --alias bench >/dev/null; r=$?
  incus delete -f benchbuild >/dev/null; return $r
}

printf '%-12s %-18s %8s  %s\n' engine workload@dir median_ms runs
for e in $engines; do
  up "$e" "bench-$e" || { echo "$e: could not start"; failed=1; continue; }
  for where in workspace scratch; do
  for load in noop spawn files git cpu compile; do
    vals=""
    for _ in $(seq "$runs"); do
      v=$(in_box "$e" "bench-$e" env WL_DIR=/$where sh /run/workload.sh "$load" 2>/dev/null | sed -n 's/^inner_ms=//p')
      [ -n "$v" ] || { vals=""; break; }
      vals="$vals $v"
    done
    if [ -z "$vals" ]; then printf '%-12s %-18s %8s  %s\n' "$e" "$load@$where" - "failed; not measured"; failed=1
    else printf '%-12s %-18s %8s  %s\n' "$e" "$load@$where" "$(echo $vals | tr ' ' '\n' | median)" "$vals"; fi
  done
  done
  down "$e" "bench-$e"
  vals=""
  for _ in $(seq "$runs"); do t0=$(now); cold "$e" >/dev/null 2>&1 || { vals=""; break; }; vals="$vals $(( ($(now) - t0) / 1000000 ))"; done
  if [ -z "$vals" ]; then printf '%-12s %-18s %8s  %s\n' "$e" cold - "failed"; failed=1
  else printf '%-12s %-18s %8s  %s\n' "$e" cold "$(echo $vals | tr ' ' '\n' | median)" "$vals"; fi
  # Memory per idle container.
  sync; sleep 2; before=$(awk '/MemAvailable/ {print $2}' /proc/meminfo)
  for i in 1 2 3 4 5; do up "$e" "mem-$e-$i" >/dev/null 2>&1; done
  sleep 5; after=$(awk '/MemAvailable/ {print $2}' /proc/meminfo)
  for i in 1 2 3 4 5; do down "$e" "mem-$e-$i"; done
  printf '%-12s %-18s %8s  %s\n' "$e" mem_kib "$(( (before - after) / 5 ))" "per idle container, MemAvailable delta over 5"
done
echo "conditions: host=$(uname -srm) cpus=$(nproc) cgroup=v1 kvm=$([ -e /dev/kvm ] && echo yes || echo no) docker=$(docker version -f '{{.Server.Version}}') sysbox=0.7.1-patched runsc=$(runsc --version | head -1 | cut -d' ' -f3) incus=$(incus version | sed -n 's/Server version: //p') image=$image runs=$runs date=$(date -u +%FT%TZ)"
rm -rf "$ws_root"
exit $failed
