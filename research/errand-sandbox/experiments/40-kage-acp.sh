#!/bin/sh
# Question: does kage itself run under the gVisor shim, launched the way
# errand launches it (`bailey run ... -- kage rpc --yolo -m P/M`, ACP on
# stdio), through one model turn and one shell tool call?
# Abandon if: kage cannot start, cannot reach the model through the broker
# address, or its shell tool cannot run under gVisor.
# Observed from outside: the fake model logs every request and the
# Authorization header it carried; the file the tool writes is read on the
# host.
# Exit: 0 held, 1 failed, 2 could not run.
set -u
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
. "$here/lib/policy.sh"
shim="${SHIM:-$repo/research/errand-sandbox/poc/bailey-gvisor/bailey}"
PATH="$repo/.work/bin/gv:$PATH"; export PATH
command -v runsc >/dev/null || { echo "runsc missing: run 10-fetch-runsc.sh"; exit 2; }
kage=$(command -v kage) || { echo "kage missing from PATH"; exit 2; }
kdir=$(dirname "$kage")

work=$(mktemp -d "${TMPDIR:-/tmp}/b40.XXXXXX")
mport=$(free_port)
python3 -I "$here/lib/fake_model.py" "$mport" > "$work/model.log" 2>&1 &
mpid=$!
trap 'kill $mpid 2>/dev/null' EXIT
sleep 1

# errand's runtime grant for the agent: its directory, read and execute, and
# on PATH (src/sandbox/runtime.rs:86-108, policy.rs:180-226).
write_policy "$work" '"FAKE_API_KEY", "XDG_CACHE_HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME"' \
  "http://169.254.169.1:$mport"
sed -i "s#^read = \[\"/usr\",#read = [\"$kdir\", \"/usr\",#; s#^execute = \[\"/usr\",#execute = [\"$kdir\", \"/usr\",#" "$work/policy.toml"
# The configuration errand writes for a brokered custom provider
# (src/sandbox/agent_config.rs:310-326).
mkdir -p "$work/state/kage/kage"
cat > "$work/state/kage/kage/config.toml" <<EOF
[providers.custom.fake]
kind = "openai"
base_url = "http://169.254.169.1:$mport/provider/fake"
display_name = "fake"
api_key_env = "FAKE_API_KEY"

[[providers.custom.fake.models]]
id = "fake-1"
name = "fake-1"
context = 32000
max_output = 4096
EOF
BAILEY_GVISOR_HOME="$work/ohome"; export BAILEY_GVISOR_HOME
mkdir -p "$BAILEY_GVISOR_HOME"
"$shim" trust "$work/policy.toml" || exit 2
cd "$work/proj" || exit 2
start=$(date +%s%N)
FAKE_API_KEY=nonce-for-fake XDG_CACHE_HOME=/state/kage XDG_CONFIG_HOME=/state/kage \
XDG_DATA_HOME=/state/kage XDG_STATE_HOME=/state/kage \
  python3 -I "$here/lib/acp_client.py" "run the shell tool once" -- \
  "$shim" run --isolate --egress-proxy "169.254.169.1:$mport" --config "$work/policy.toml" \
  --profile ai-agent -- kage rpc --yolo -m fake/fake-1 > "$work/acp.log" 2> "$work/stderr.log"
code=$?
ms=$(( ($(date +%s%N) - start) / 1000000 ))
grep -E "^(TIMING|STOP|FAIL)" "$work/acp.log"
cat "$work/model.log"
fail=0
[ $code -eq 0 ] || { echo "FAIL acp client exit $code"; tail -5 "$work/stderr.log"; fail=1; }
grep -q "auth=Bearer nonce-for-fake" "$work/model.log" && echo "PASS model saw the nonce, through the broker address" || { echo "FAIL model saw no request with the nonce"; fail=1; }
grep -q "^CALLING " "$work/model.log" && echo "PASS kage offered tools: $(sed -n 's/.*tools=//p' "$work/model.log" | head -1)" || { echo "FAIL no tool call"; fail=1; }
grep -q "TOOL-RESULT.*gVisor\|TOOL-RESULT.*4.19.0" "$work/model.log" && echo "PASS the tool ran under the gVisor sentry (uname)" || { echo "FAIL tool result does not show gVisor"; fail=1; }
[ "$(cat "$work/proj/from-kage.txt" 2>/dev/null)" = "written-by-kage" ] && echo "PASS the tool's write landed in the project on the host" || { echo "FAIL no file from the tool"; fail=1; }
echo "wall_ms=$ms user=$(id -un) kage=$(kage --version) runsc=$(runsc --version | head -1) date=$(date -u +%FT%TZ) evidence=$work"
exit $fail
