#!/bin/sh
# Question: on this host (no KVM), can Docker and rootless podman each run a
# Debian 13 image under runc and under gVisor, the second with the flags
# errand's podman backend uses (src/sandbox/podman.rs:62-98 at
# errand@4a8011cd3668)?
# Run as root. It changes the host, each change named:
#   - starts dockerd, if it is not running
#   - /etc/docker/daemon.json: adds runtime "runsc" (overwrites the file)
#   - installs runsc and its gvisor-bin sidecars into /usr/local/bin
#   - apt installs podman uidmap slirp4netns fuse-overlayfs
#   - chmod 666 /dev/net/tun (it was 0600; distributions ship 0666, and
#     rootless slirp4netns and pasta need it)
#   - /usr/local/bin/runsc-rootless: runsc with --ignore-cgroups, because
#     cgroup v1 here has no delegation for an unprivileged user
#   - ~USER/.config/containers/containers.conf: registers runtime "runsc"
# Exit: 0 all eight runs passed, 1 one failed, 2 could not run.
set -u
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
user=${RUN_AS:-svc}
image=docker.io/library/debian:trixie-slim
[ "$(id -u)" = 0 ] || { echo "run as root"; exit 2; }
id "$user" >/dev/null 2>&1 || { echo "no user $user"; exit 2; }
[ -x "$repo/.work/bin/gv/runsc" ] || { echo "run 10-fetch-runsc.sh first"; exit 2; }

install -m755 "$repo/.work/bin/gv/runsc" /usr/local/bin/runsc
cp -r "$repo/.work/bin/gv/gvisor-bin" /usr/local/bin/
cat > /usr/local/bin/runsc-rootless <<'EOF'
#!/bin/sh
# runsc for rootless podman on a host with no cgroup delegation.
exec /usr/local/bin/runsc --ignore-cgroups --platform=systrap "$@"
EOF
chmod 755 /usr/local/bin/runsc-rootless

mkdir -p /etc/docker
cat > /etc/docker/daemon.json <<'EOF'
{
  "runtimes": {
    "runsc": { "path": "/usr/local/bin/runsc", "runtimeArgs": ["--platform=systrap"] }
  }
}
EOF
if pgrep -x dockerd >/dev/null; then kill -HUP "$(pgrep -x dockerd)"; sleep 2
else nohup dockerd > /tmp/dockerd.log 2>&1 & fi
for _ in $(seq 30); do docker info >/dev/null 2>&1 && break; sleep 1; done
docker info >/dev/null 2>&1 || { echo "dockerd did not start: /tmp/dockerd.log"; exit 2; }

command -v podman >/dev/null || apt-get install -y -q podman uidmap slirp4netns fuse-overlayfs >/dev/null || exit 2
chmod 666 /dev/net/tun
su - "$user" -c 'mkdir -p ~/.config/containers && printf "[engine.runtimes]\nrunsc = [\"/usr/local/bin/runsc-rootless\"]\n" > ~/.config/containers/containers.conf'

# Pulled only when missing: Docker Hub rate-limits anonymous pulls (429).
docker image inspect "$image" >/dev/null 2>&1 || docker pull -q "$image" >/dev/null || exit 2
podman image exists "$image" || podman pull -q "$image" >/dev/null || exit 2
rm -f /tmp/debian-trixie.tar; podman save -q "$image" -o /tmp/debian-trixie.tar && chmod 644 /tmp/debian-trixie.tar
su - "$user" -c "podman load -q -i /tmp/debian-trixie.tar" >/dev/null || exit 2

fail=0
check() { # name expected command...
  name=$1 want=$2; shift 2
  got=$("$@" 2>&1 | tail -1)
  case "$got" in *"$want"*) echo "PASS $name: $got" ;; *) echo "FAIL $name: $got"; fail=1 ;; esac
}
errand_flags="--network=none --userns=keep-id --read-only --cap-drop=ALL --security-opt no-new-privileges"
check "docker runc"            13.     docker run --rm "$image" cat /etc/debian_version
check "docker runsc"           gvisor  docker run --rm --runtime=runsc "$image" uname -r
check "podman root runc"       13.     podman run --rm "$image" cat /etc/debian_version
check "podman root runsc"      gvisor  podman --runtime /usr/local/bin/runsc run --rm "$image" uname -r
check "podman rootless runc"   13.     su - "$user" -c "podman run --rm $image cat /etc/debian_version"
check "podman rootless pasta"  eth0    su - "$user" -c "podman run --rm --network=pasta $image ls -m /sys/class/net"
check "podman rootless runsc, errand flags" gvisor su - "$user" -c "podman --runtime runsc run --rm $errand_flags $image uname -r"
check "podman rootless runsc, network"      tap0   su - "$user" -c "podman --runtime runsc run --rm $image cat /proc/net/dev"
echo "conditions: docker=$(docker version -f '{{.Server.Version}}') podman=$(podman --version | cut -d' ' -f3) runsc=$(runsc --version | head -1) cgroup=v1 kvm=$([ -e /dev/kvm ] && echo yes || echo no) user=$user date=$(date -u +%FT%TZ)"
exit $fail
