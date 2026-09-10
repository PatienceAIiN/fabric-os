# Status — what is real vs. designed vs. deferred

Honest accounting. "Real" means implemented and covered by passing automated
tests on this host. Nothing below is claimed beyond what the tests show.

## Real and tested (52 tests passing, all guarded)

| Area | Milestone | Crate / artifact | Evidence |
|------|-----------|------------------|----------|
| Bootable Linux in QEMU | M1 | scripts/boot-qemu.sh | boots to userspace, PID 1 |
| Persistent systemd userspace | M2 | scripts/boot-disk.sh | boot counter 1->2 persists |
| Capability system | M13 | libcapability | least-privilege, traversal denied |
| Crypto primitives | — | libcrypto | SHA-256/HMAC vs NIST/RFC vectors |
| Agent identity | M12 | libagent | content-addressed id, HMAC auth |
| Intent engine | M11 | libintent | lifecycle state machine, scoped caps |
| AI Governor | M14 | libgovernor | deterministic allow/deny/approve |
| Provenance | M17 | libprovenance | hash-chained, tamper/reorder detected |
| Transactions / rollback | M18 | libtransaction | confined move/delete + rollback |
| Secure credentials | M6 | libcredentials | systemd-creds, no plaintext on disk |
| Provider abstraction | M7 | libprovider (AIProvider) | trait + Local + routing |
| Claude provider | M8 | libprovider (ClaudeProvider) | curl body/errors; key never logged |
| Model routing | M13 | libprovider::route | privacy/offline force local |
| Multi-agent broker | M21 | aios::broker | delegation subset + revocation |
| Killer Demo 1 | §76 | aios demo1 / tests | injection BLOCKED + rollback |
| Killer Demo 3 | §78 | aios demo3 / tests | delegate + revoke stops cap |

## Partial / functional-but-headless

- **Settings — AI & Models (M5/M8)**: functional via `aios settings` (status,
  set-key, remove-key) backed by the secure store; no graphical pane yet.
- **AI command bar (M15)**: the `aios` CLI is the functional stand-in.
- **Local AI service / system-wide AI (M9/M10)**: `LocalProvider` + the
  provider abstraction exist; the always-on local Unix-socket service daemon and
  the system-wide ON/OFF wiring are not yet built.

## Designed, not yet implemented (deferred with rationale)

- **M3 Wayland compositor, M4 desktop shell, M23 GPU/NPU, M24 full desktop**:
  a GPU-accelerated graphical stack cannot be built to completion on this 7 GB
  laptop in-session without risking the exact desktop hang we are required to
  avoid. UI framework choice is deliberately open (ADR-0005) pending
  measurement. These are the next major body of work, on suitable hardware.
- **M19 model management daemon, M20 resource fabric daemon**: data models and
  the `tools/rg` cgroup mechanism exist as the substrate; the daemons are not
  built.
- **M22 AI-aware scheduling**: sched_ext is present on the host; the
  experimental scheduler + benchmarks are future work (no perf claims made).
- **M25/M26/M27 hardening / perf / release**: ongoing; not started.

## Live-only (cannot be tested here without secrets/hardware)
- Real Claude API calls (need a user key; the code path and error mapping are
  tested offline, the network round-trip is not).


## Update — completing the remaining milestones

Now real and tested/demonstrated (beyond the earlier core):
- **M5 Settings**: libsettings (typed, validated, persisted) + `aios settings`.
- **M9/M10 Local AI + system-wide**: aiosd daemon + systemd user unit
  (enable/disable == ON/OFF); real llama.cpp inference wired in.
- **M15 command bar / M16 activity center / M55 monitor**: functional in `aios`
  AND a native GUI (`aios-gui`, FLTK) — screenshotted with real GPU inference.
- **M19/M70 model supply chain**: `aios model verify` (checksum+trust; rejects
  tampering).
- **M23 GPU inference**: llama.cpp Vulkan on the AMD iGPU (~1.4x prompt eval).
- **M22 scheduling**: userspace experiment (measured) + a compile-verified
  sched_ext BPF scheduler (NOT loaded — would replace the system scheduler).
- **M25 hardening**: capability→systemd sandbox (live confinement proof) + a
  10-case adversarial suite.
- **M26 perf**: benchmarks + a measured governor optimization.
- **M27 release**: VERSION, CHANGELOG, scripts/release.sh, git tag.

Still genuinely hardware/scope-bound (honest):
- **M24 full GPU-composited desktop**: a native GUI app exists and is
  screenshotted, but a full bespoke compositor + shell (replacing Weston) is a
  large future effort; M24 stays *partial*.
- **NPU** offload (no NPU on this host); multi-GB models (RAM); loading the
  sched_ext scheduler live (risk).
