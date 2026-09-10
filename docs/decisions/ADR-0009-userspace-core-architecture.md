# ADR-0009: Userspace core architecture (library-first, deterministic gate)

## Problem
Build the AI-native layer so that probabilistic AI can never bypass the
security boundary, while keeping it testable and light on this host.

## Selected approach
- **Library-first.** The core is a set of small, `forbid(unsafe_code)`,
  mostly zero-dependency crates (`libcapability`, `libcrypto`, `libagent`,
  `libintent`, `libgovernor`, `libprovenance`, `libtransaction`,
  `libcredentials`, `libprovider`). Daemons wrap libraries; the `aios` binary
  is the current functional front-end. This makes every rule unit-testable and
  avoids a running-daemon dependency for CI.
- **Deterministic gate.** `libgovernor::evaluate` is the single authorization
  chokepoint: agent validity -> intent state -> intent expiry -> capability
  permitted-by-intent -> deterministic risk -> approval policy. A model may
  supply a risk *hint* that can only escalate, never lower. Every decision
  emits provenance. Fail closed throughout.
- **Authority from capabilities, not content.** A prompt-injected request for a
  capability the intent did not grant is denied regardless of wording — proven
  by the Demo 1 test.
- **Reversible execution.** `libtransaction` performs confined, rollback-able
  filesystem operations; deletes are move-to-backup (compensatable), never
  true unlinks.

## Why not a monolithic daemon now
A daemon-first design would couple tests to IPC and a running service. The
library-first split gives the same guarantees with fast, hermetic tests, and
the daemons/IPC (Unix sockets + CBOR, ADR-0004) layer on unchanged.

## Tradeoffs
No live inter-daemon IPC yet; the `aios` CLI substitutes for the command bar
until the graphical shell exists. Documented in ROADMAP and STATUS.
