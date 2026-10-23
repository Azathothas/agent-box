#!/bin/sh
# Question: can the shim keep a credential errand does NOT broker (GH_TOKEN)
# out of the sandbox, and still have it arrive at the origin?
# Abandon if: the real value is readable anywhere inside, or the origin does
# not receive it.
# Observed from outside: a TLS origin standing in for api.github.com logs the
# Authorization header it received; a CONNECT proxy stands in for errand's
# broker and logs every tunnel.
# Exit: 0 held, 1 failed, 2 could not run.
set -u
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
. "$here/lib/policy.sh"
shim="${SHIM:-$repo/research/errand-sandbox/poc/bailey-gvisor/bailey}"
PATH="$repo/.work/bin/gv:$PATH"; export PATH
command -v runsc >/dev/null || { echo "runsc missing: run 10-fetch-runsc.sh"; exit 2; }

work=$(mktemp -d "${TMPDIR:-/tmp}/b30.XXXXXX")
cd "$work" || exit 2
# A CA standing in for the public one api.github.com is signed by.
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout up-ca.key \
  -out up-ca.crt -days 1 -subj "/CN=test upstream CA" \
  -addext "basicConstraints=critical,CA:TRUE" -addext "keyUsage=critical,keyCertSign,cRLSign" 2>/dev/null || exit 2
openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout origin.key -out origin.csr \
  -subj "/CN=api.github.com" 2>/dev/null
printf 'subjectAltName=DNS:api.github.com\n' > origin.ext
openssl x509 -req -in origin.csr -CA up-ca.crt -CAkey up-ca.key -CAcreateserial -out origin.crt \
  -days 1 -extfile origin.ext 2>/dev/null || exit 2

oport=$(free_port) bport=$(free_port)
python3 -I "$here/lib/fakes.py" origin $oport origin.crt origin.key > origin.log 2>&1 &
p1=$!
python3 -I "$here/lib/fakes.py" broker $bport $oport > broker.log 2>&1 &
p2=$!
trap 'kill $p1 $p2 2>/dev/null' EXIT
sleep 1

mkdir -p ohome/.config/bailey-gvisor
cat > ohome/.config/bailey-gvisor/credentials.toml <<EOF
upstream_ca = "$work/up-ca.crt"
[[inject]]
variable = "GH_TOKEN"
hosts = ["github.com", "api.github.com"]
EOF

# A test value shaped like a token, made for this run.
real="ghp_TEST$(od -An -N12 -tx1 /dev/urandom | tr -d ' \n')"
write_policy "$work" '"GH_TOKEN"' "http://169.254.169.1:$bport"
BAILEY_GVISOR_HOME="$work/ohome" "$shim" trust "$work/policy.toml" || exit 2
cd "$work/proj" || exit 2
GH_TOKEN="$real" BAILEY_GVISOR_HOME="$work/ohome" XDG_CONFIG_HOME=/state/kage XDG_DATA_HOME=/state/kage "$shim" run --isolate \
  --egress-proxy 169.254.169.1:$bport --config "$work/policy.toml" -- /bin/sh -c '
echo "inside GH_TOKEN=$GH_TOKEN"
curl -sS -H "Authorization: token $GH_TOKEN" https://api.github.com/user
curl -sS -u "x-access-token:$GH_TOKEN" https://api.github.com/repos
# Everything a process inside can read about itself and its neighbours, plus
# the granted trees, copied out for the host to search. The probe carries no
# part of the secret.
for f in /proc/[0-9]*/environ /proc/[0-9]*/cmdline; do cat "$f"; echo; done > /workspace/dump 2>/dev/null
cat /run/bailey/egress/* /etc/ssl/certs/* >> /workspace/dump 2>/dev/null
grep -r -h -a -s . /state /workspace/written >> /workspace/dump 2>/dev/null
true' > "$work/inside.log" 2>&1
code=$?
cat "$work/inside.log"
cat "$work/origin.log"
fail=0
grep -q "inside GH_TOKEN=bailey-placeholder-" "$work/inside.log" || { echo "FAIL inside saw something other than a placeholder"; fail=1; }
grep -q "$real" "$work/inside.log" && { echo "FAIL real token visible inside"; fail=1; }
grep -a -q "$real" "$work/proj/dump" && { echo "FAIL real token found in what the sandbox can read"; fail=1; }
[ -s "$work/proj/dump" ] && echo "PASS leak scan read $(wc -c < "$work/proj/dump") bytes from inside (positive control: placeholder found $(grep -a -c bailey-placeholder- "$work/proj/dump") times)"
grep -q "ORIGIN-RECEIVED Authorization: token $real" "$work/origin.log" || { echo "FAIL origin did not get the token header"; fail=1; }
b64=$(printf 'x-access-token:%s' "$real" | base64 -w0)
grep -q "ORIGIN-RECEIVED Authorization: Basic $b64" "$work/origin.log" || { echo "FAIL origin did not get the basic header"; fail=1; }
grep -q "CONNECT api.github.com:443" "$work/broker.log" && echo "PASS the tunnel went through the broker" || { echo "FAIL broker saw no tunnel"; fail=1; }
[ $fail -eq 0 ] && echo "PASS real token reached the origin and was never inside"
echo "run exit=$code user=$(id -un) date=$(date -u +%FT%TZ) evidence=$work"
[ $code -eq 0 ] || exit 1
exit $fail
