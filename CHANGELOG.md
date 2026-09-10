# Changelog

## 0.2.0-fabric — 2026-09-10

### Fabric OS desktop
- Renamed to **Fabric OS** (user-facing: os-release, boot, UI, README).
- New animated web desktop (`fabric-desktop`): splash -> login -> Ubuntu-style
  desktop with top bar, dock, windows, scrollable views. Apps: AI Assistant
  ("ask AI to do", drafts intents), Activity, Agents (CRUD), Intents (CRUD +
  authorize/cancel), Governor (evaluate -> allow/approve/block), Monitor,
  Settings. Backed by the real deterministic core over a JSON API with login.
- `scripts/fabric-os.sh` runs the desktop fullscreen on your machine.
- The booted OS **serves its own desktop**: fabric-desktop baked into the image
  as a service; virtio_net module + network bring-up added; `launch-os.sh`
  boots the OS and opens the desktop (hostfwd 8788->8787). Verified end to end.


All notable changes. Versioning is experimental (0.x); nothing is stable yet.

## 0.1.0-experimental — 2026-09-10

### System
- M0 Dev environment: resource-guarded build harness (`tools/rg`), reproducible
  sudo-free setup, host doctor.
- M1 Bootable Linux in QEMU (distro kernel + busybox initramfs).
- M2 Persistent systemd userspace booting from a virtio ext4 disk.
- M3 Wayland graphical environment via upstream Weston (headless-proven).

### AI-native core (tested Rust)
- Capabilities (M13), Intent engine (M11), deterministic AI Governor (M14),
  hash-chained Provenance (M17), reversible Transactions (M18), Agent identity
  (M12), Secure credentials via systemd-creds (M6), Provider abstraction +
  Claude (M7/M8), model routing, multi-agent broker with delegation +
  revocation (M21), resource fabric (M20), model management + supply-chain
  (M19), OS-managed memory, capability→sandbox mapping (M25).

### Desktop
- Functional shell (`aios`): AI command bar, activity center, system monitor,
  settings. Local AI service daemon `aiosd` with system-wide toggle (M9/M10).
- Design system "Lumen": tokens, spec, self-contained mockup (M39).

### Demonstrations (automated tests)
- Demo 1: intent + prompt-injection BLOCK + rollback.
- Demo 3: multi-agent delegation + revocation.

### Research / evidence
- Microbenchmarks (governor, provenance, crypto) and an AI-aware scheduling
  experiment — measured, in docs/research/benchmarks.md.
- Live sandbox confinement proof (scripts/verify-sandbox.sh).

### Known limitations
- Native GPU compositor/desktop shell, GPU/NPU accelerated inference, and
  multi-GB local models are deferred (hardware-bound); see docs/STATUS.md.
- Crypto uses a verified pure-Rust SHA-256/HMAC prototype; ed25519 is the
  production identity path (ADR-0008).
