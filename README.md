# Fabric OS

_(engine: ai-native-os)_

## Quick start — Fabric OS desktop

```
cd ~/ai-native-os
source scripts/ai-env.sh          # enable local AI (GPU if available)
./scripts/fabric-os.sh            # desktop on your machine (admin / fabric)
# or boot the whole OS in a VM and open its desktop:
./scripts/launch-os.sh
```

See TESTING.md for details.

A research-grade Linux system whose core abstraction is the **AI agent
operating under explicit human intent, capabilities, policies, resource
constraints, provenance, and reversible execution**.

> AI proposes. Policy verifies. Kernel enforces. Provenance records.
> Transactions allow rollback.

This is a real operating system first and an AI operating system second. The
underlying Linux system remains fully usable without any AI running.

## Status

Milestone **M0 — development environment** (see [ROADMAP.md](ROADMAP.md)).
Nothing here is stable. APIs are experimental.

## Non-negotiable constraints on this hardware

This repo is being developed on a memory-constrained laptop (7 GB RAM). Every
heavy command **must** run under the resource guard so the machine never hangs:

```
tools/rg -- cargo build            # capped: ~3 GB RAM, 6 cores, low priority
tools/rg --profile test -- cargo test
```

Never run an unguarded `cargo build -j$(nproc)`, kernel build, or QEMU launch
on the development laptop. See [docs/decisions/ADR-0002-resource-safe-build-harness.md](docs/decisions/ADR-0002-resource-safe-build-harness.md).

## Layout

See the tree in [ARCHITECTURE.md](ARCHITECTURE.md). Userspace daemons live in
`userspace/`, shared crates in `libraries/`, docs and decision records in
`docs/`.

## Release and compliance

This repository covers both the Linux-based OS and its public website. The OS
is a research release, not a production-certified distribution. Before any
public launch, use:

- [OS launch readiness](docs/compliance/OS-LAUNCH-READINESS.md)
- [OS data governance](docs/compliance/OS-DATA-GOVERNANCE.md)
- [Release checklist](docs/release/RELEASE-CHECKLIST.md)
- [Model and asset license register](docs/compliance/MODEL-AND-ASSET-LICENSES.md)

These controls cover the kernel and boot path, rootfs, userspace, AI
providers, credentials, models, SDKs, build artifacts, updates, recovery,
website, and support operations. They do not replace jurisdiction-specific
legal review.

## Getting started

```
scripts/doctor.sh        # check host capabilities (read-only)
scripts/setup-dev.sh     # install user-local Rust toolchain, guarded
tools/rg -- cargo test   # build and test the workspace, guarded
```

`scripts/setup-dev.sh` never uses sudo. Packages that need root (QEMU,
kernel-devel) are printed for you to install manually.
