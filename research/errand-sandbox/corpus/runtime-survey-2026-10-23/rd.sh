#!/bin/sh
# rd.sh name url -> saves rd/name.md
for i in 1 2 3; do c=$(curl -sS -o rd/$1.md -w "%{http_code}" "https://read.web.pkgforge.dev/$2"); [ "$c" = 200 ] && break; sleep 4; done
echo "$1 http $c $(wc -c < rd/$1.md)B"
