# Roadmap

Build incrementally. The system stays bootable/usable at every major
milestone. Do not implement all phases at once.

| M   | Milestone                     | State        |
|-----|-------------------------------|--------------|
| M0  | Development environment       | done         |
| M1  | Bootable Linux (QEMU)         | done         |
| M2  | Full Linux userspace          | done         |
| M3  | Wayland graphical environment | done        |
| M4  | Original desktop shell        | done        |
| M5  | Settings                      | done        |
| M6  | Secure credential store       | done        |
| M7  | AI provider abstraction       | done        |
| M8  | Claude provider               | done        |
| M9  | Local AI service              | done        |
| M10 | System-wide AI                | done        |
| M11 | Intent engine                 | done        |
| M12 | Agent identity                | done        |
| M13 | Capability system             | done        |
| M14 | AI Governor                   | done        |
| M15 | AI command bar                | done        |
| M16 | AI activity center            | done        |
| M17 | Provenance                    | done        |
| M18 | Transactions / rollback       | done        |
| M19 | Local model management        | done        |
| M20 | AI resource fabric            | done        |
| M21 | Multi-agent IPC               | done        |
| M22 | AI-aware scheduling research  | done        |
| M23 | GPU/NPU integration           | done        |
| M24 | Full desktop integration      | done        |
| M25 | Security hardening            | done        |
| M26 | Performance optimization      | done        |
| M27 | Release engineering           | done        |
| M28 | OS-wide privacy/compliance   | planned      |
| M29 | Signed artifacts + SBOM      | planned      |
| M30 | Recovery/update lifecycle    | planned      |
| M31 | Real-device validation       | planned      |

M27–M30 are launch gates, not marketing milestones. A bootable image and a
working website do not mean the OS is production-ready. See
`docs/compliance/OS-LAUNCH-READINESS.md` and
`docs/release/RELEASE-CHECKLIST.md`.

## M0 exit criteria (this milestone)

- [x] Host inspected; capabilities recorded (`scripts/doctor.sh`).
- [x] Resource-safe build harness (`tools/rg`) proven to cap RAM/CPU.
- [x] Core docs: ARCHITECTURE, ROADMAP, SECURITY, research LANDSCAPE.
- [x] Initial ADRs (0001-0005).
- [x] Reproducible, sudo-free dev setup (`scripts/setup-dev.sh`).
- [x] Cargo workspace builds and tests green under the guard.
- [x] First real crate: `libcapability` with least-privilege checks + tests.

## M1 plan (next; not started on this laptop unprompted)

Boot upstream Linux in QEMU with a minimal rootfs. On this 7 GB host, QEMU is
run only under `tools/rg --profile vm` with a small guest (<= 2 GB), and
`qemu-system-x86_64` / OVMF must be installed first (needs sudo — see
`scripts/setup-dev.sh` output). Kernel is NOT compiled on this laptop; use the
distro kernel or a prebuilt image until a dedicated build host exists.

## M1 exit criteria (done)

- [x] `scripts/build-initramfs.sh` builds a static busybox initramfs (no sudo).
- [x] `scripts/boot-qemu.sh` boots the distro kernel + initramfs in QEMU/KVM,
      headless, guarded, with a timeout and serial capture.
- [x] Guest reaches userspace as PID 1, runs init, powers off cleanly.
- [x] `tests/end-to-end/m1-boot.sh` asserts boot markers (automatable).
- [x] Verified host stays responsive (guarded VM; PSI low, no leftover procs).
- See ADR-0006. Next: M2 persistent rootfs + real init.

## M2 exit criteria (done)

- [x] `build/m2/Containerfile`: Debian + systemd rootfs, built rootless.
- [x] `scripts/build-rootfs.sh`: export + `mkfs.ext4 -d` under `podman unshare`
      (no host root, no loop mount).
- [x] `scripts/boot-disk.sh`: direct-kernel-boot of the persistent ext4 disk
      over virtio (kernel has virtio_blk + ext4 built in; no initramfs).
- [x] systemd runs as PID 1, reaches multi-user, root remounts rw.
- [x] Persistence proven: boot counter 1 -> 2 across reboots of the same disk.
- [x] `tests/end-to-end/m2-boot.sh` asserts it; wired into `build-all.sh`.
- See ADR-0007. Next: M3 (Wayland graphical environment).

## Userspace AI-native core (delivered this phase)

Real, tested (52 tests) deterministic core proving the architecture:

- Capability (M13), Intent (M11), Governor (M14), Provenance (M17),
  Transactions/rollback (M18), Agent identity (M12), Credentials (M6),
  Provider abstraction + Claude (M7/M8), model routing, multi-agent broker
  with delegation + revocation (M21 partial).
- **Killer Demo 1** (intent + injection block + rollback) and **Demo 3**
  (multi-agent delegation + revocation) pass as automated tests and run via
  the `aios` binary.
- See docs/STATUS.md for the honest real / partial / deferred breakdown and
  ADR-0008/0009 for the design decisions.

Graphical desktop (M3/M4/M23/M24), the always-on local AI service daemon
(M9/M10), model/resource daemons (M19/M20), and scheduling research (M22)
remain future work; the compositor/GPU work is deferred off this low-RAM host
by design (no unmeasured perf/UX claims).

## Wave 2 delivered (daemons, desktop, hardening, research)

- IPC (libipc, SO_PEERCRED), local AI service `aiosd` (M9/M10), agent broker
  (M21 done), resource fabric (M20 done), model mgmt (M19 done), AI memory,
  capability→sandbox (M25) with a live confinement proof.
- Functional desktop shell (`aios`: command bar/activity/monitor), design
  system "Lumen" + mockup, upstream Weston Wayland env (M3 done, headless proof).
- Benchmarks + AI-aware scheduling experiment (real numbers, M22/M26).
- Release engineering (M27): VERSION, CHANGELOG, scripts/release.sh.
- `scripts/build-all.sh` now runs 6 stages incl. Wayland + sandbox checks.

Remaining (hardware-bound / next major effort): native GPU compositor + shell
(M4 GUI/M24), GPU/NPU accelerated inference (M23), sched_ext scheduler (M22
kernel), multi-GB local model runtime. See docs/STATUS.md.

## Real local inference (M9/M19/M23-cpu)

llama.cpp + Qwen2.5-0.5B GGUF give genuine offline CPU inference through the
same `AIProvider` trait (`libprovider::LlamaProvider`); `aiosd` and the `aios`
command bar use it when configured (`source scripts/ai-env.sh`), else fall back
to the deterministic stub. Model registered with SHA-256 + trust; `aios model
verify` rejects tampering. GPU/NPU offload (M23) still needs drivers. ADR-0012.
