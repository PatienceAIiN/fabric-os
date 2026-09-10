# Experimental AI-aware scheduler (M22) — sched_ext

`ainos_sched.bpf.c` is a minimal `sched_ext` (SCHED_CLASS_EXT) scheduler that
gives background **agent** tasks a shorter time slice so they yield to
interactive/foreground work — the kernel-side version of the deprioritization
measured in `scripts/sched-experiment.sh`.

## Status: COMPILE-VERIFIED, NOT LOADED
- Builds to a BPF object against this kernel's BTF (`vmlinux.h` generated from
  `/sys/kernel/btf/vmlinux`); the object has `.struct_ops`/`.maps` sections.
- It is **deliberately not attached** on this machine. Loading an scx scheduler
  replaces the system scheduler for all tasks; a bug can freeze a daily-driver
  desktop. Per spec §30/§64: build an experimental scheduler first, measure in
  isolation, never patch the core scheduler casually.

## How to build
```
bpftool btf dump file /sys/kernel/btf/vmlinux format c > kernel/sched_ext/vmlinux.h
clang -target bpf -D__TARGET_ARCH_x86 -mcpu=v3 -O2 -I kernel/sched_ext \
  -c kernel/sched_ext/ainos_sched.bpf.c -o kernel/sched_ext/ainos_sched.bpf.o
```

## How to test SAFELY (in a QEMU VM only)
A full loader (libbpf skeleton + `bpf_map__attach_struct_ops`) plus a userspace
component that marks agent PIDs into the `agent_pids` map is future work, to be
run inside the M1/M2 QEMU guest — never on the host. Compare against the default
scheduler with the same methodology as the userspace experiment.
