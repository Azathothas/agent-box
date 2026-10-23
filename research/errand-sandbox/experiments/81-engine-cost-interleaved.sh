#!/bin/sh
# Question: the same as 80. What does each engine cost an agent against
# plain runc, on the same image and the same work?
# Why 81 replaces 80: 80 measured one engine after another. This host's disk
# speed drifts over minutes: one runc pass of `files` on the bind-mounted
# workspace took 184 ms, and another, minutes later, took about 1500 ms, as
# did a host-root run with no container at all. So 80's disk-backed rows
# compared moments, not engines. Its 8x for Sysbox on files@workspace was
# that artefact: interleaved, the two are within 5% (results/README.md).
# Here every engine is up at once, and each round runs one workload on every
# engine in turn, so all engines share each moment.
# Work directories, the same for each: /workspace is a host directory
# bind-mounted in (what errand gives a session), and /scratch is a tmpfs.
# Run as root, with dockerd, the Sysbox daemons and incusd running, and the
# images bench:trixie (docker, see 80) and bench (incus, see 80) present.
# Exit: 0 measured, 1 a workload failed somewhere, 2 could not run.
set -u
here=$(cd "$(dirname "$0")" && pwd)
runs=${RUNS:-5}
engines=${ENGINES:-"runc sysbox-runc runsc incus"}
loads=${LOADS:-"spawn files git cpu compile"}
wl="$here/lib/workload.sh"
[ "$(id -u)" = 0 ] || { echo "run as root"; exit 2; }
docker image inspect bench:trixie >/dev/null 2>&1 || { echo "no bench:trixie; run 80 once"; exit 2; }
case " $engines " in *" incus "*) incus image info bench >/dev/null 2>&1 || { echo "no incus image bench; run 80 once"; exit 2; } ;; esac

root=$(mktemp -d /var/tmp/b81.XXXXXX)
for e in $engines; do
  n="b81-$e"; mkdir -p "$root/$e"
  case $e in
    incus)
      # The container's root is host uid 1000000 (its idmap). A directory it
      # owns needs no shift, which this host refuses for disk devices.
      chown 1000000:1000000 "$root/$e"
      incus delete -f "$n" >/dev/null 2>&1
      incus init -q bench "$n" >/dev/null &&
        incus config device add "$n" ws disk source="$root/$e" path=/workspace >/dev/null &&
        incus start "$n" && sleep 3 &&
        incus file push -q "$wl" "$n/run/workload.sh" &&
        incus exec "$n" -- sh -c 'mkdir -p /scratch && mount -t tmpfs -o exec tmpfs /scratch' || { echo "incus: could not start"; exit 2; } ;;
    *)
      docker rm -f "$n" >/dev/null 2>&1
      docker run -d --runtime="$e" --name "$n" -v "$wl:/run/workload.sh:ro" \
        -v "$root/$e:/workspace" --tmpfs /scratch:exec bench:trixie sleep infinity >/dev/null || { echo "$e: could not start"; exit 2; } ;;
  esac
done
sleep 2
run_one() { # engine dir load
  case $1 in
    incus) incus exec "b81-$1" --env WL_DIR="/$2" -- sh /run/workload.sh "$3" ;;
    *)     docker exec -e WL_DIR="/$2" "b81-$1" sh /run/workload.sh "$3" ;;
  esac 2>/dev/null | sed -n 's/^inner_ms=//p'
}
raw="$root/raw.txt"; : > "$raw"
for dir in workspace scratch; do
  for load in $loads; do
    for round in $(seq "$runs"); do
      for e in $engines; do
        v=$(run_one "$e" "$dir" "$load")
        echo "$e $load@$dir ${v:--}" >> "$raw"
      done
    done
  done
done
failed=0
grep -q ' -$' "$raw" && failed=1
median() { sort -n | awk '{a[NR]=$1} END {print (NR%2 ? a[(NR+1)/2] : int((a[NR/2]+a[NR/2+1])/2))}'; }
printf '%-18s' "workload@dir"; for e in $engines; do printf ' %16s' "$e"; done; echo "   (median ms of $runs interleaved rounds)"
for dir in workspace scratch; do
  for load in $loads; do
    printf '%-18s' "$load@$dir"
    base=""
    for e in $engines; do
      vals=$(awk -v e="$e" -v k="$load@$dir" '$1==e && $2==k {print $3}' "$raw")
      if echo "$vals" | grep -q -- -; then printf ' %16s' "-"; continue; fi
      m=$(echo "$vals" | median)
      [ -z "$base" ] && base=$m
      printf ' %9s (%sx)' "$m" "$(awk -v a="$m" -v b="$base" 'BEGIN {printf "%.1f", a/b}')"
    done
    echo
  done
done
echo "raw rounds: $raw"
for e in $engines; do case $e in incus) incus delete -f "b81-$e" >/dev/null ;; *) docker rm -f "b81-$e" >/dev/null ;; esac; done
echo "conditions: host=$(uname -srm) cpus=$(nproc) cgroup=v1 kvm=$([ -e /dev/kvm ] && echo yes || echo no) docker=$(docker version -f '{{.Server.Version}}') sysbox=0.7.1-patched runsc=$(runsc --version | head -1 | cut -d' ' -f3) incus=$(incus version | sed -n 's/Server version: //p') runs=$runs order=interleaved date=$(date -u +%FT%TZ)"
exit $failed
