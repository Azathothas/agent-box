# errand sandbox research (in progress, paused)

Question: can errand (QaidVoid/errand@4a8011cd3668) run kage under a different
sandbox by replacing only `bailey` (QaidVoid/bailey@98d9b455adc6), with
credentials held outside it? kage was read at QaidVoid/kage@cce8b9a6dca4.

## State on 2026-10-23

Paused by the operator. This tree is evidence, not a product.

- `poc/bailey-gvisor/bailey`: a drop-in `bailey` (doctor, trust, untrust, run)
  over gVisor. **gVisor was a stand-in only**, because this host had no
  /dev/kvm. It is not the recommendation.
- `experiments/`: numbered scripts. 20 (boundary), 30 (credential injection),
  40 (kage over ACP) and 50 (unmodified errand probe) passed as root and as an
  unprivileged user.
- 60 (cost): the last run is not valid. Its bailey rows failed on the fixture,
  not on bailey.

## Known wrong or weak, read before anything else

1. The shim's comment says Landlock resolves `/usr/bin/python3 ->
   /etc/alternatives/...` under bailey. That is false. Real bailey's
   `--isolate` dangles the same link, so errand's read list is what lacks it,
   and the shim grants more than bailey. The fix was not applied.
2. The shim binds host /usr and the host's kage into the sandbox, because it
   translates errand's policy as written. For a microVM this is the wrong
   design: the guest needs its own image.
3. No KVM on this host: settled 2026-10-23, with three routes examined.
   - Nested VMX or SVM: the CPU shows no vmx or svm flag, and the kernel has
     KVM_INTEL and KVM_AMD off. A node made with
     `mknod kvm c 10 232` opens with ENODEV.
   - PVM (KVM without hardware virtualization): built into this kernel, but
     the kernel refuses to register it. dmesg says: "not registering
     kvm_pvm, so there is no /dev/kvm: the kernel command line does not
     carry nested_pvm (add nested_pvm=1 to enable)". Every parameter under
     /sys/module/kvm_pvm/parameters is read-only, and `force` is N.
   - Boot again with that flag: there is no kexec (no
     /sys/kernel/kexec_loaded) and no module loading (`nomodule`).
   Reopens if: the platform boots this VM with `nested_pvm=1`, or the work
   moves to a host that has /dev/kvm (GitHub-hosted Linux runners have it).
4. gVisor cost (svc user, systrap, median of 7, measured once): 300 spawns took
   0.42 s bare and 3.3 s under gVisor; a git commit of 1000 files took about
   1.0 s bare and 3.7 s under gVisor. That makes gVisor a poor fit for build
   work. The figures are for this host only.

## Findings that hold whatever the engine

- errand brokers only model-provider keys. GH_TOKEN crosses into the sandbox
  as the real token (src/session/session.rs:2378-2382).
- With no known base URL, errand gives the session the real provider key and
  logs only a warning (src/serve.rs:727-736).
- session_environment lets the launch env's XDG_*=/state/kage override the
  operator's dirs for bailey (src/sandbox/bailey.rs:97-127). This is not
  exploitable: an explicit --config needs no trust record
  (bailey crates/bailey/src/config.rs:404-406,466-473).
- An injection proxy is defeated by an origin that echoes the credential back
  (results/30-run1-reflection-inside.log).

## Re-fetch the references

```sh
git clone https://github.com/QaidVoid/errand && git -C errand checkout 4a8011cd3668
git clone https://github.com/QaidVoid/bailey && git -C bailey checkout 98d9b455adc6
git clone https://github.com/QaidVoid/kage && git -C kage checkout cce8b9a6dca4
```

The trackers of all three were not read, because the forge proxy was rate
limited. That is a gap.
