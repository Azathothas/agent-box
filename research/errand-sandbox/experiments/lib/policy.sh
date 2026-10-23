# Writes a policy shaped like errand's (QaidVoid/errand@4a8011cd3668,
# src/sandbox/policy.rs:239-327, policy_contents) for a scratch project and
# state directory. Experiment 40 uses the one errand itself writes instead.
#
#   write_policy DIR PASS_NAMES PROXY_URL
write_policy() {
  d=$1 pass=$2 proxy=$3
  mkdir -p "$d/proj" "$d/state/home"
  echo "nameserver 1.1.1.1" > "$d/resolv.conf"
  cat > "$d/policy.toml" <<EOF
# Generated per session by errand. Do not edit.
[filesystem]
reset = true
read = ["/usr", "/lib", "/lib64", "/bin", "/sbin", "/proc", "/etc/ld.so.cache", "/etc/ld.so.conf", "/etc/ld.so.conf.d", "/etc/ssl", "/etc/ca-certificates.conf", "/etc/ca-certificates", "/var/lib/ca-certificates", "/etc/pki", "/etc/nsswitch.conf", "/etc/services", "/etc/protocols", { path = "$d/resolv.conf", at = "/etc/resolv.conf" }, { path = "$d/proj", at = "/workspace" }, { path = "$d/state", at = "/state" }]
write = [{ path = "$d/proj", at = "/workspace" }, { path = "$d/state", at = "/state" }]
execute = ["/usr", "/lib", "/lib64", "/bin", "/sbin", "/state/home/bin", { path = "$d/proj", at = "/workspace" }]
[[device]]
path = "/dev/null"
access = "rw"
[[device]]
path = "/dev/zero"
access = "r"
[[device]]
path = "/dev/full"
access = "rw"
[[device]]
path = "/dev/urandom"
access = "r"
[[device]]
path = "/dev/random"
access = "r"
[[device]]
path = "/dev/tty"
access = "rw"

[env]
pass = [$pass]
set = { PATH = "/state/home/bin:/usr/local/bin:/usr/bin:/bin", HOME = "/state/home", HTTPS_PROXY = "$proxy", https_proxy = "$proxy", NO_PROXY = "169.254.169.1", no_proxy = "169.254.169.1" }

[resources]
file_max = "1g"
tmp_size = "512m"
shm_size = "256m"

[network]
egress_allow = [{ host = "*", port = 443 }]
EOF
}

# A port nothing on this host listens on now. Fixed ports let one run's
# leftover server answer for the next run, which once made a pass look like a
# fail.
free_port() {
  python3 -I -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])'
}
