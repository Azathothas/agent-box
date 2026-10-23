#!/bin/sh
# usage: srch.sh N "query" [rank]
n=$1; q=$2; rk=${3:-reranked}
for i in 1 2 3 4; do
 if [ $((n%2)) = 0 ]; then
  curl -sS -o s/$n.json -w "%{http_code}" -X POST -H 'content-type: application/json' --data "$(python3 -I -c 'import json,sys;print(json.dumps({"q":sys.argv[1],"count":8,"rank":sys.argv[2]}))' "$q" "$rk")" https://search.web.pkgforge.dev/search > s/$n.code
 else
  curl -sS -o s/$n.json -w "%{http_code}" -G https://search.web.pkgforge.dev/search --data-urlencode "q=$q" --data-urlencode count=8 --data-urlencode rank=$rk > s/$n.code
 fi
 c=$(cat s/$n.code); [ "$c" = 200 ] && break; sleep 5
done
echo "## [$n] $q ($rk, http $c)"
python3 -I -c "
import json,sys
d=json.load(open('s/$n.json'))
for r in d.get('results',[]): print(' ',r.get('url'),'|',(r.get('title') or '')[:60],'|',(r.get('snippet') or '')[:140].replace('\n',' '))
if not d.get('results'): print(d)"
