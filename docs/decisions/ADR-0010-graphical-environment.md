# ADR-0010: Graphical environment (upstream compositor now, AI-native later)

## Problem
M3 needs a Wayland graphical environment on a 7 GB laptop without a from-
scratch compositor (huge effort/risk) and without a GPU-accelerated fullscreen
build that could hang the host.

## Selected approach
- **Upstream Weston** as the M3 compositor base (principle: prefer upstream,
  don't fork unnecessarily). Proven up headless with a client connecting via
  `scripts/boot-wayland.sh` (RESULT: PASS), the same evidence style as the QEMU
  boots. Headless backend needs no GPU, so it is safe here.
- **Original shell identity "Lumen"** (M4/M39): design tokens
  (`design/tokens.json`), `docs/design/design-system.md`, and a self-contained
  `docs/design/mockup.html` — original visuals, no proprietary assets. The
  functional shell (command bar / activity center / monitor) ships today in the
  `aios` terminal UI driving the real daemons.

## Deferred (with rationale)
- A bespoke **AI-native Smithay compositor** (Rust) and a **GPU-accelerated**
  desktop shell are the next major body of work, on hardware with more RAM and
  a usable GPU. Building/testing them here risks the desktop hang we must avoid,
  and the UI-framework choice is deliberately measurement-gated (ADR-0005).
- The compositor stays deterministic; AI only requests window actions through
  controlled, policy-checked APIs (spec §59-60). That API is specified; the
  graphical implementation follows the compositor work.

## Consequence
M3 (graphical environment) is satisfied by the upstream compositor; M4 is
partially delivered (identity + functional shell), with the GPU shell deferred.
