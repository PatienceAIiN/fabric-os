# ADR-0011: Agent sandboxing and AI-aware scheduling

## Problem
Deterministic enforcement must sit below the AI (M25), and background agent
workloads should not starve foreground interactivity (M22) — both proven, not
asserted.

## Sandboxing
`libsandbox` maps an agent's capability set onto systemd user-service sandbox
properties: `ProtectSystem=strict`, `NoNewPrivileges`, `PrivateTmp`, syscall
filtering, `ReadWritePaths` only for granted `fs.write`, and
`PrivateNetwork`/`RestrictAddressFamilies` unless a `net.*` capability exists.
`scripts/verify-sandbox.sh` proves live confinement: an agent with no write
capability gets a read-only world and its writes fail (RESULT: PASS). This is
the kernel-enforced backstop for a compromised agent (threats: privilege
escalation, unauthorized fs, network abuse). PrivateNetwork needs unprivileged
user namespaces (present on this host).

## Scheduling
`scripts/sched-experiment.sh` compares foreground latency with background
"agent" hogs at default vs deprioritized (CPUWeight=10, Nice=19) priority,
producing real numbers (~1.09x foreground protection here; see
docs/research/benchmarks.md). This is a userspace cgroup policy; a `sched_ext`
BPF scheduler is the next step for finer, agent-aware control (host supports
SCHED_CLASS_EXT). No performance claim is made beyond the measurement.

## Tradeoffs
systemd sandboxing is coarser than a bespoke seccomp+Landlock profile but is
robust, dependency-free, and already enforced by the kernel. Landlock (present
in the host LSM list) is the path to per-path FS confinement inside the process
without a service boundary — future work.
