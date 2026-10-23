#!/bin/sh
# Question: what does a session pay for the gVisor shim, against the same
# work run bare and under real bailey, on this host?
# Abandon gVisor if: the work an agent does most (spawning processes, many
# small files, git) costs so much more that a session is visibly slower than
# under bailey.
# Measured from outside: wall time read on the host around each run; the
# workload also times itself inside, so start-up and work are separated.
# Memory: resident set of the sandbox's own processes while the target idles.
# Exit: 0 measured, 1 a mode failed to run its workload, 2 could not run.
set -u
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
. "$here/lib/policy.sh"
shim="${SHIM:-$repo/research/errand-sandbox/poc/bailey-gvisor/bailey}"
PATH="$repo/.work/bin/gv:$PATH"; export PATH
command -v runsc >/dev/null || { echo "runsc missing: run 10-fetch-runsc.sh"; exit 2; }
bailey=${BAILEY:-}
runs=${RUNS:-5}
py=$(readlink -f /usr/bin/python3)
# bailey refuses a policy its group can write; a login umask of 002 makes one.
umask 022

work=$(mktemp -d "${TMPDIR:-/tmp}/b60.XXXXXX")
write_policy "$work" '' "http://169.254.169.1:9"
# Real bailey needs its own trust store; the shim gets its test home.
BAILEY_GVISOR_HOME="$work/ohome"; export BAILEY_GVISOR_HOME
mkdir -p "$BAILEY_GVISOR_HOME"
"$shim" trust "$work/policy.toml" > /dev/null || exit 2
[ -n "$bailey" ] && { HOME="$work/ohome" "$bailey" trust "$work/policy.toml" > /dev/null || exit 2; }

cat > "$work/proj/w.sh" <<'EOF'
#!/bin/sh
# One workload, named by $1, in a fresh directory under the cwd; prints its
# own elapsed milliseconds.
# The interpreter by its real path: /usr/bin/python3 is a link through
# /etc/alternatives, which errand's policy does not grant (see the shim).
PY=$2
d="./run.$$"; rm -rf "$d"; mkdir -p "$d"; cd "$d" || exit 1
t0=$(date +%s%N)
( set -e
case "$1" in
  noop) : ;;
  spawn) i=0; while [ $i -lt 300 ]; do /bin/true; i=$((i+1)); done ;;
  files) "$PY" -I -c "
import os
for i in range(3000):
    open(f'f{i}', 'w').write('x' * 512)
n = sum(len(open(f'f{i}').read()) for i in range(3000))
for i in range(3000):
    os.stat(f'f{i}')
" ;;
  git) git init -q .
       i=0; while [ $i -lt 1000 ]; do echo $i > f$i; i=$((i+1)); done
       git -c user.name=t -c user.email=t@t add -A
       git -c user.name=t -c user.email=t@t commit -qm x
       test "$(git rev-list --count HEAD)" = 1 ;;
  cpu) "$PY" -I -c "
s = 0
for i in range(3_000_000):
    s += i * i
" ;;
esac
) > ../w.err."$$" 2>&1
code=$?
t1=$(date +%s%N)
cd .. && rm -rf "$d"
# Only a workload that succeeded reports a time.
[ $code -eq 0 ] && echo "inner_ms=$(( (t1 - t0) / 1000000 ))" || { echo "workload failed: $(head -3 w.err."$$")" >&2; }
rm -f w.err."$$"
EOF
chmod +x "$work/proj/w.sh"

median() { sort -n | awk '{a[NR]=$1} END {print (NR%2 ? a[(NR+1)/2] : int((a[NR/2]+a[NR/2+1])/2))}'; }

measure() { # mode workload
  mode=$1 load=$2 outer="" inner=""
  for _ in $(seq "$runs"); do
    t0=$(date +%s%N)
    case $mode in
      bare)   out=$(cd "$work/proj" && ./w.sh "$load" "$py") ;;
      gvisor) out=$(cd "$work/proj" && "$shim" run --isolate --config "$work/policy.toml" --quiet -- /bin/sh /workspace/w.sh "$load" "$py" 2>/dev/null) ;;
      bailey) out=$(cd "$work/proj" && HOME="$work/ohome" "$bailey" run --isolate --config "$work/policy.toml" --profile ai-agent --quiet -- /bin/sh /workspace/w.sh "$load" "$py" 2>/dev/null) ;;
    esac
    t1=$(date +%s%N)
    outer="$outer $(( (t1 - t0) / 1000000 ))"
    inner="$inner $(echo "$out" | sed -n 's/^inner_ms=//p')"
  done
  # A run that printed no inner time did not run the workload; its wall time
  # is not a measurement of anything, so it is reported as a dash.
  if [ "$(echo $inner | wc -w)" -ne "$runs" ]; then
    printf '%-7s %-6s outer_ms=-      inner_ms=-      (a run failed; not measured)\n' "$mode" "$load"
    failed=1
    return
  fi
  printf '%-7s %-6s outer_ms=%-6s inner_ms=%-6s (runs:%s)\n' "$mode" "$load" \
    "$(echo $outer | tr ' ' '\n' | median)" "$(echo $inner | tr ' ' '\n' | median)" "$outer"
}
failed=0

modes="bare gvisor"
[ -n "$bailey" ] && modes="$modes bailey"
for load in noop spawn files git cpu; do
  for mode in $modes; do measure "$mode" "$load"; done
done

# Resident memory of the sandbox while its target sleeps. gVisor: every
# process runsc started for this run (sandbox, gofer, and the shim itself).
(cd "$work/proj" && "$shim" run --isolate --config "$work/policy.toml" --quiet -- /bin/sleep 6 2>/dev/null) &
sleep 4
rss=$(ps -eo rss=,args= | awk '/runsc|bailey-gvisor\/bailey/ && !/awk/ {s+=$1} END {print s}')
echo "gvisor idle resident set: ${rss} KiB (sum over runsc, its sandbox and gofer, and the shim)"
wait
if [ -n "$bailey" ]; then
  (cd "$work/proj" && HOME="$work/ohome" "$bailey" run --isolate --config "$work/policy.toml" --quiet -- /bin/sleep 6 2>/dev/null) &
  sleep 4
  rss=$(ps -eo rss=,args= | awk '/bailey run/ && !/awk/ {s+=$1} END {print s}')
  echo "bailey idle resident set: ${rss} KiB (sum over bailey processes)"
  wait
fi
echo "conditions: host=$(uname -srm) cpus=$(nproc) mem=$(awk '/MemTotal/ {print $2}' /proc/meminfo)KiB kvm=$([ -e /dev/kvm ] && echo yes || echo no) platform=systrap runsc=$(runsc --version | head -1) user=$(id -un) runs=$runs date=$(date -u +%FT%TZ)"
echo "evidence: $work"
[ $failed -eq 0 ] || exit 1
