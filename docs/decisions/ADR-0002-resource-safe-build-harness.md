# ADR-0002: Resource-safe build harness (tools/rg)

## Problem
The dev laptop has 7 GB RAM and runs a desktop + browser. Unbounded builds,
test suites, or VMs would exhaust RAM, hit zram/OOM, and make the UI lag or
hang. The user's explicit requirement: the PC must not hang and lag.

## Existing approaches
- `nice`/`ionice` alone: adjusts priority but does not cap RAM; a leaky build
  still exhausts memory and triggers global OOM.
- Global earlyoom/systemd-oomd: kills processes reactively, disruptive.
- Manual `-jN`: fragile, forgotten, does not bound peak RSS.

## Alternatives
1. Container with `--memory`. Works but adds image/overlay overhead for pure
   Rust builds and complicates editor/tooling paths.
2. systemd transient scope with cgroup v2 limits. **Selected** — cgroup
   memory/cpu/io controllers are delegated to the user slice (verified), so a
   scope can hard-cap RSS (MemoryMax => OOM-kill only the build, never the
   desktop), throttle earlier (MemoryHigh), bound CPU (CPUQuota), and lower
   priority/IO weight.

## Selected approach
`tools/rg [--profile ...] -- <cmd>` wraps the command in
`systemd-run --user --scope` with MemoryHigh, MemoryMax, MemorySwapMax,
CPUQuota, CPUWeight, IOWeight, plus `nice`/`ionice` on the payload. Profiles:
tiny/build/test/heavy/vm, sized for 7 GB RAM. Verified inside the scope:
memory.max=3 GB, memory.high=2.3 GB, cpu.max=600%.

## Tradeoffs
Builds cannot use all 12 cores/full RAM, so wall-clock is longer. Acceptable:
responsiveness beats raw build speed on the only available machine.

## Security implications
Bounding resource use is also the resource-exhaustion mitigation (threat model)
and the substrate for per-agent resource budgets (M20/M29).

## Performance implications
Nice=10 + CPUWeight=40 keep interactive tasks prioritised; MemoryHigh throttles
before the hard cap, avoiding thrash. Measured PSI stays low during guarded
builds (to be benchmarked per M0 exit).

## Future migration path
Same mechanism generalises to `resourced` agent budgets; profiles become
policy-driven reservations.
