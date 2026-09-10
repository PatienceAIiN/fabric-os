# ADR-0005: UI technology direction (provisional)

## Problem
The desktop must be fast (<16 ms input, 60 FPS), GPU-accelerated, Wayland-
native, accessible, and original. Framework choice must be evidence-based.

## Status
PROVISIONAL — no desktop code is written until M3/M4. This ADR records the
evaluation criteria and current leaning so the decision is not made by default.

## Criteria (per spec section 58)
Linux-native fit, GPU rendering, startup performance, memory usage,
accessibility, Wayland compatibility, multi-monitor, animation performance,
maintainability. Popularity alone is not a reason.

## Candidates to evaluate at M3
- **Rust + wgpu/GPU toolkit** (e.g. an immediate/retained GPU UI): best control
  over frame pacing and memory, aligns with Rust-first, but accessibility and
  maturity need verification.
- **GTK4/libadwaita**: excellent Wayland/a11y/native integration, mature, but
  a distinct visual identity must be built and animation control is coarser.
- **Qt/QML**: strong tooling and animation, licensing and footprint to weigh.
- **Compositor**: a wlroots-based (Smithay in Rust) compositor is the likely
  base so the compositor stays deterministic and AI touches it only via APIs.

## Decision rule
Prototype the command bar + one Settings pane in the top two candidates at M3,
measure the section-61 budgets on this reference hardware, then commit in a
superseding ADR. Do not choose before measuring.

## Constraints already fixed
Wayland preferred. Compositor is deterministic; AI never drives compositor
internals, only requests window actions through controlled, policy-checked APIs.
