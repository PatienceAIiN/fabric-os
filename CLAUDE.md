# CLAUDE.md — working agreement for this repo

## What this is
A research OS where the core abstraction is an AI agent bound by intent,
capabilities, policy, resource limits, provenance, and reversible execution.
Read ARCHITECTURE.md and ROADMAP.md before touching code.

## Hard constraints on the current dev host (7 GB RAM laptop)
- ALWAYS wrap heavy commands: `tools/rg -- <cmd>` (see profiles in tools/rg).
  This caps RAM (hard OOM at ~3 GB), CPU (<= 6 cores), and lowers priority so
  the desktop never lags. Never run unguarded cargo/kernel builds or QEMU.
- No passwordless sudo. Do not attempt system package installs; print the
  needed packages for the user instead. Userspace-only (rustup, pip --user).
- Do NOT compile a Linux kernel on this laptop. Use distro/prebuilt kernels.
- Do NOT launch QEMU except under `tools/rg --profile vm` with a small guest.

## Engineering rules
- Every model output is untrusted. parse -> validate -> capability -> policy
  -> resource -> risk -> approval -> execute. Never execute because valid.
- Deterministic enforcement below probabilistic AI. Fail closed.
- Prefer Rust; C for kernel glue; Python for tooling.
- No secrets in code/logs/provenance. Redact keys from diagnostics.
- No performance/novelty/security claim without measurement / prior-art
  search / adversarial test. Record decisions in docs/decisions/.

## Before implementing anything
Inspect existing code first. Keep the system bootable/usable at each
milestone. Build incrementally; do not implement the whole roadmap at once.
