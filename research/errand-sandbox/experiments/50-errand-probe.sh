#!/bin/sh
# Question: does an unmodified errand accept the gVisor shim as its `bailey`
# (doctor, the policy-shape probe, trust and the confined check), with the
# egress broker on, up to the point where it reaches for the chat service?
# Abandon if: errand refuses the backend, or reports a gap the shim cannot
# answer without lying.
# Observed from outside: errand's own startup log, and a wrapper that records
# every argv errand gives `bailey`.
# Exit: 0 errand passed its sandbox probe, 1 it refused, 2 could not run.
set -u
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
errand=${ERRAND:-}
[ -n "$errand" ] && [ -x "$errand" ] || { echo "set ERRAND to a built errand binary"; exit 2; }
command -v kage >/dev/null || { echo "kage missing from PATH"; exit 2; }

work=$(mktemp -d "${TMPDIR:-/tmp}/b50.XXXXXX")
# Without a base URL errand cannot route the provider through its broker and
# hands the session the real key (src/serve.rs:727-736). NO_BASEURL=1 shows it.
BASEURL=', "baseUrl": "https://api.openai.com/v1"'
[ "${NO_BASEURL:-0}" = 1 ] && BASEURL=''
mkdir -p "$work/bin" "$work/projects" "$work/state" "$work/ohome"
# Every call errand makes is logged, then handed to the shim unchanged.
cat > "$work/bin/bailey" <<EOF
#!/bin/sh
printf '%s\n' "\$*" >> "$work/calls.log"
BAILEY_GVISOR_HOME="$work/ohome" exec "$repo/research/errand-sandbox/poc/bailey-gvisor/bailey" "\$@"
EOF
chmod +x "$work/bin/bailey"
cat > "$work/config.json" <<EOF
{
  "chat": { "token": "not-a-token", "channelId": "1", "allowedUserIds": ["1"] },
  "agent": { "provider": "openai",
             "providers": { "openai": { "credentialName": "OPENAI_API_KEY", "credential": "test-not-a-key"$BASEURL } } },
  "projectRoot": "$work/projects",
  "stateDir": "$work/state",
  "sandbox": { "backend": "bailey", "network": "restricted", "requireFullEnforcement": false,
               "egress": { "mode": "proxy", "allow": ["api.openai.com"] } }
}
EOF
PATH="$work/bin:$repo/.work/bin/gv:$(dirname "$(command -v kage)"):/usr/bin:/bin" \
ERRAND_CONFIG="$work/config.json" HOME="$work/ohome" timeout 60 "$errand" run > "$work/errand.log" 2>&1
code=$?
cat "$work/errand.log" | cut -c1-400
echo "--- argv errand gave bailey:"
cat "$work/calls.log" | cut -c1-200
echo "errand exit=$code user=$(id -un) date=$(date -u +%FT%TZ) evidence=$work"
grep -q "unavailable" "$work/errand.log" && exit 1
grep -q -i "chat service rejected\|did not answer\|failed to start" "$work/errand.log" || exit 1
grep -q "^run --config .*/probe/shape.toml --quiet -- true" "$work/calls.log" || exit 1
exit 0
