#!/bin/sh
# What can root inside this sandbox reach that touches the host? Each line is
# one probe: "open" means the probe got through, "closed" means it did not.
# A probe getting through is not an escape; it says what surface is exposed.
# Each probe gets 5 s: reading /dev/kmsg, for one, blocks when it is open.
# A probe that times out is reported as "blocked", which means it got as far
# as waiting on the host resource.
p() { name=$1; shift
  timeout 5 "$@" >/dev/null 2>&1; c=$?
  if [ $c -eq 0 ]; then echo "open    $name"; elif [ $c -eq 124 ]; then echo "blocked $name"; else echo "closed  $name"; fi; }
p "mknod the host disk (254:0) and read it" sh -c 'mknod /tmp/vd b 254 0 && dd if=/tmp/vd of=/dev/null bs=4k count=1'
p "read the kernel log (dmesg)"             sh -c 'dmesg | head -1 | grep -q .'
p "see more than 50 processes"              sh -c '[ "$(ls -d /proc/[0-9]* | wc -l)" -gt 50 ]'
p "mount a tmpfs"                           sh -c 'mkdir -p /tmp/m && mount -t tmpfs t /tmp/m'
p "mount a fresh procfs"                    sh -c 'mkdir -p /tmp/p && mount -t proc p /tmp/p'
p "unshare a new user namespace"            unshare -U -r true
p "write a host sysctl (vm.drop_caches)"    sh -c 'echo 1 > /proc/sys/vm/drop_caches'
p "read /proc/kcore"                        sh -c 'head -c 1 /proc/kcore | wc -c | grep -q 1'
p "load a kernel keyring key"               sh -c 'command -v keyctl && keyctl add user t v @u'
p "change the hostname"                     hostname probe-host
p "open /dev/kmsg for reading"             sh -c 'exec 3</dev/kmsg'
p "write under /sys"                        sh -c 'echo 1 > /sys/kernel/mm/ksm/run'
p "use ptrace (strace-like) on pid 1"       sh -c 'command -v gdb >/dev/null || exit 1; gdb -p 1 -batch'
echo "kernel seen: $(uname -r)"
echo "my uid inside: $(id -u); uid_map: $(head -1 /proc/self/uid_map | tr -s ' ')"
