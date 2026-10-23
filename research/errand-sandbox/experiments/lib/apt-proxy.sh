#!/bin/sh
# Inside a container: point apt at HTTPS mirrors through the host's relayed
# egress proxy, trusting the proxy's CA bundle mounted at /run/proxy-ca.crt.
# Needed on hosts whose proxy allows CONNECT only (plain-http apt gets 405).
#   PROXY_HOST  the relay as this container sees it (default: Docker bridge)
#   PROXY_PORT  the relay port (default: this research host's proxy port)
host=${PROXY_HOST:-172.17.0.1}
port=${PROXY_PORT:-33337}
for f in /etc/apt/sources.list /etc/apt/sources.list.d/*.sources /etc/apt/sources.list.d/*.list; do
  [ -f "$f" ] && sed -i 's#http://deb.debian.org#https://deb.debian.org#g' "$f"
done
cat > /etc/apt/apt.conf.d/99proxy <<C
Acquire::https::Proxy "http://$host:$port";
Acquire::https::CaInfo "/run/proxy-ca.crt";
C
