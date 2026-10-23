#!/bin/sh
G=https://api.gh.pkgforge.dev
for r in "$@"; do
 f=gh/$(echo $r|tr / _)
 [ -s $f.json ] || curl -sS --retry 3 $G/repos/$r -o $f.json
 [ -s $f.rel.json ] || curl -sS --retry 3 $G/repos/$r/releases/latest -o $f.rel.json
 python3 -I -c "
import json
d=json.load(open('$f.json')); r=json.load(open('$f.rel.json'))
print('$r|', d.get('stargazers_count'),'|push',(d.get('pushed_at') or '-')[:10],'|arch',d.get('archived'),'|',((d.get('license') or {}).get('spdx_id')),'|iss',d.get('open_issues_count'),'|rel',r.get('tag_name','-'),(r.get('published_at') or '-')[:10], d.get('message',''))"
done
