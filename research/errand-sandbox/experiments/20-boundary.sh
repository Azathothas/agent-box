#!/bin/sh
# Question: under the gVisor shim, with --egress-proxy, is errand's broker the
# only thing a session can reach, and does nothing outside the policy cross?
# Abandon the approach if: any probe below reaches a non-broker address, or a
# variable not named in [env] pass appears inside.
# Observed from outside: the broker double logs what reached it.
# Exit: 0 every expectation held, 1 one failed, 2 could not run.
set -u
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
. "$here/lib/policy.sh"
shim="${SHIM:-$repo/research/errand-sandbox/poc/bailey-gvisor/bailey}"
PATH="$repo/.work/bin/gv:$PATH"; export PATH
command -v runsc >/dev/null || { echo "runsc missing: run 10-fetch-runsc.sh"; exit 2; }

work=$(mktemp -d "${TMPDIR:-/tmp}/b20.XXXXXX")
bport=$(free_port) other=$(free_port)
python3 -I "$here/lib/fakes.py" broker $bport 1 > "$work/broker.log" 2>&1 &
bpid=$!
python3 -m http.server $other --bind 127.0.0.1 > /dev/null 2>&1 &
opid=$!
trap 'kill $bpid $opid 2>/dev/null' EXIT
sleep 1

write_policy "$work" '"OPENAI_API_KEY"' "http://169.254.169.1:$bport"
BAILEY_GVISOR_HOME="$work/ohome"; export BAILEY_GVISOR_HOME
mkdir -p "$BAILEY_GVISOR_HOME"
"$shim" trust "$work/policy.toml" || exit 2
cd "$work/proj" || exit 2
start=$(date +%s%N)
OPENAI_API_KEY=nonce-test CHAT_TOKEN=must-not-cross "$shim" run --isolate \
  --egress-proxy 169.254.169.1:$bport --config "$work/policy.toml" --profile ai-agent -- \
  /usr/bin/python3 -I -c "
import os, socket, urllib.request
def ok(name, cond): print(('PASS ' if cond else 'FAIL ') + name)
try:
    r = urllib.request.urlopen('http://169.254.169.1:$bport/provider/x', timeout=5).read()
except OSError as e:
    r = repr(e)
ok('broker reachable', r == b'broker')
for host, port in [('1.1.1.1', 443), ('127.0.0.1', $other), ('169.254.169.1', $other), ('169.254.169.254', 80), ('10.0.0.1', 443)]:
    try:
        socket.create_connection((host, port), timeout=2); ok(f'{host}:{port} unreachable', False)
    except OSError:
        ok(f'{host}:{port} unreachable', True)
try:
    socket.getaddrinfo('example.com', 443); ok('dns blocked', False)
except OSError:
    ok('dns blocked', True)
ok('chat token absent', 'CHAT_TOKEN' not in os.environ)
ok('provider var is the nonce', os.environ.get('OPENAI_API_KEY') == 'nonce-test')
ok('cwd is /workspace', os.getcwd() == '/workspace')
ok('/etc/passwd absent', not os.path.exists('/etc/passwd'))
ok('host home absent', not os.path.exists('/root') and not os.path.exists('/home'))
open('/workspace/written', 'w').write('x')
try:
    open('/usr/x', 'w'); ok('/usr read-only', False)
except OSError:
    ok('/usr read-only', True)
caps = [l for l in open('/proc/self/status') if l.startswith('CapEff')][0].split()[1]
ok('no capabilities', int(caps, 16) == 0)
" > "$work/inside.log" 2>&1
code=$?
ms=$(( ($(date +%s%N) - start) / 1000000 ))
cat "$work/inside.log"
grep -q "GET /provider/x" "$work/broker.log" && echo "PASS broker logged the request (outside observer)" || echo "FAIL broker saw nothing"
[ -f "$work/proj/written" ] && echo "PASS workspace write landed on the host" || echo "FAIL workspace write missing"
echo "run exit=$code wall_ms=$ms user=$(id -un) runsc=$(runsc --version | head -1) kernel=$(uname -r) date=$(date -u +%FT%TZ)"
echo "evidence: $work"
if grep -q '^FAIL' "$work/inside.log" || [ $code -ne 0 ] || ! grep -q "GET /provider/x" "$work/broker.log"; then exit 1; fi
exit 0
