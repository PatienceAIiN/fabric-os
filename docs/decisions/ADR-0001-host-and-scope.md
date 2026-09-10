# ADR-0001: Development host, target, and M0 scope

## Problem
We must build a research OS on a specific developer laptop without destabilising
it, and decide how much to attempt first.

## Host facts (measured 2026-09-10)
- AMD Ryzen 5 5500U, 12 threads. SVM (AMD-V) present; /dev/kvm accessible (user
  in kvm group). KVM acceleration available.
- 7.1 GB RAM, ~2.7 GB available with normal desktop load. 7.1 GB zram swap,
  swappiness 180. RAM is the binding constraint.
- /home has 364 GB free. Disk is not a constraint.
- Fedora 44, kernel 7.1.3. cgroup v2 with cpu/io/memory/pids delegated to the
  user slice. LSMs: selinux,bpf,landlock,ipe,ima,evm. Kernel has
  SCHED_CLASS_EXT, BPF_LSM, LANDLOCK, DAMON.
- Toolchain present: gcc 16, clang 22, make, ninja, meson, cmake, python 3.14,
  git, podman (rootless, cgroup v2), docker. MISSING: rust, qemu, OVMF,
  kernel-devel, bpftool, flex, bison, protoc.
- sudo requires a password (no non-interactive root).

## Existing approaches
Typical distro/OS builds assume a beefy build host and root. That does not fit.

## Alternatives
1. Full local build incl. kernel + QEMU images now. Rejected: would thrash a
   7 GB laptop and needs root packages.
2. Do everything in a rootless container. Partial: good for rootfs assembly
   later, unnecessary overhead for pure-userspace Rust work now.
3. Userspace-first, resource-guarded, kernel/QEMU deferred. **Selected.**

## Selected approach
M0 = docs + decision records + resource-safe harness + a buildable, tested
Rust workspace, all userspace, all guarded. Kernel is never compiled here;
QEMU is deferred to M1 and only ever run guarded with a small guest.

## Tradeoffs
Slower path to a booting image, but the dev machine stays responsive and the
architecture's hard core (capabilities/policy/provenance) is built first.

## Security implications
Building the deterministic enforcement core before any AI wiring means the
untrusted-AI boundary exists before there is an AI to distrust.

## Performance implications
Guard caps mean builds are slower than an unbounded `-j12`, accepted to keep
the desktop at <16 ms input latency.

## Future migration path
When a dedicated build host or more RAM is available, raise guard profiles and
enable kernel/QEMU milestones.
