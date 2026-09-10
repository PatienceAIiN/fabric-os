# ADR-0004: Language strategy and IPC

## Problem
Choose implementation languages and an inter-daemon protocol for security-
sensitive userspace with language-neutral, evolvable interfaces.

## Selected approach
- **Rust** for security-sensitive userspace daemons and libraries (memory
  safety, strong types for capability tokens). **C** only where kernel/low-level
  compatibility requires. **Python** for tooling, orchestration, experiments.
- **IPC transport**: Unix domain sockets with SO_PEERCRED peer identification;
  authenticated, capability-scoped. No ambient trust between daemons.
- **Serialization**: CBOR for compact typed daemon messages; JSON for human-
  facing config/settings and provenance export; protobuf reserved for stable,
  cross-language SDK surfaces if/when needed. Do not invent custom wire formats.

## Alternatives considered
- D-Bus everywhere: convenient on the desktop but heavier and less suited to
  high-rate typed daemon IPC; may still be used at the desktop/Settings edge.
- gRPC/protobuf everywhere now: premature; adds protoc/codegen deps (protoc is
  not even installed). Deferred until a stable SDK boundary exists.

## Tradeoffs
Mixing CBOR/JSON/protobuf by layer adds a small mapping burden but fits each
layer's needs. Rust learning/build cost accepted for safety.

## Security implications
Typed capability tokens in Rust prevent whole classes of confused-deputy/
forgery bugs; SO_PEERCRED gives kernel-verified caller identity, not a name.

## Performance implications
CBOR keeps hot-path messages small; Unix sockets avoid network stack overhead.

## Future migration path
Introduce protobuf + versioned schemas at the public SDK boundary at M-SDK.
