# Benchmarks (measured, host: AMD Ryzen 5 5500U, Fedora 44)

Numbers are measured by `tools/bench` (aios-bench) and
`scripts/sched-experiment.sh`. No claim is made beyond what is measured. Rerun:

```
tools/rg --profile build -- cargo build --release -p bench
tools/rg --profile tiny -- ./target/release/aios-bench 300000
./scripts/sched-experiment.sh
```

## Core operation latency (release, 300k iters)

| Operation | Latency | Throughput |
|-----------|--------:|-----------:|
| capability `permits()` | ~17 ns | ~58 M ops/s |
| SHA-256 (64 B) | ~467 ns | ~2.1 M ops/s |
| governor `evaluate()` (+provenance) | ~5.5 µs | ~180 K ops/s |
| provenance `append()` | ~4.0 µs | ~250 K ops/s |
| provenance `verify()` | ~3.3 µs/record | — |

The governor's cost is dominated by the provenance append (JSON serialize +
SHA-256 chain). Authorization without recording is ~1.5 µs. This is ample:
even at 180 K authorized actions/second the gate is not a bottleneck for an
interactive agent OS. SHA-256 is the pure-Rust reference implementation;
a vetted/asm crate would be faster (future work, ADR-0008).

## Scheduling: AI-aware deprioritization (M22)

Foreground task = `aios-bench` (CPU-bound). Background = 11 "agent" hogs.

| Condition | Foreground median | vs idle |
|-----------|------------------:|--------:|
| idle | ~1.81 s | 1.00x |
| default-priority agents | ~3.95 s | 2.19x |
| AI-aware (CPUWeight=10, Nice=19) | ~3.63 s | 2.01x |

**Foreground protection: ~1.09x** from deprioritizing background agent
workloads. The effect is modest here because the foreground task is
single-threaded on a 12-thread CPU, so it usually still gets a core; the
benefit grows as foreground parallelism approaches core count. This is a
userspace-policy result via cgroup CPUWeight/Nice; a sched_ext BPF scheduler
(the host supports SCHED_CLASS_EXT) is the next experiment for finer control.

## Local inference (llama.cpp, Qwen2.5-0.5B Q4_K_M, CPU)

| Metric | Value (this host) |
|--------|-------------------|
| prompt eval | ~120-130 tok/s |
| generation | ~40-51 tok/s |
| model load + first token | ~1-2 s |
| resident memory | < 1 GB |

Real, offline, CPU-only. Measured via `aios ask` with `scripts/ai-env.sh`
sourced. No GPU used (M23 offload is future work).

## GPU vs CPU inference (M23, AMD Radeon RADV RENOIR iGPU, Vulkan)

| Backend | Prompt eval | Note |
|---------|------------:|------|
| CPU (6 threads) | ~314 tok/s | ggml CPU |
| GPU (Vulkan, all layers) | ~446 tok/s | ~1.4x faster prompt eval |

Confirmed real GPU offload ("using device Vulkan0 (AMD Radeon Graphics (RADV
RENOIR))", all layers assigned to Vulkan0, GPU ~50% busy during `aios ask`).
Enable with `source scripts/ai-env.sh` (auto-selects the Vulkan build).
Speedup is modest on a 0.5B model on a shared-memory iGPU; larger models
benefit more.

## M26 optimization: reuse prebuilt capability set

`evaluate_with_caps()` skips re-parsing the intent's capability strings on each
call (build the set once per intent, reuse across its actions).

| Path | ns/op |
|------|------:|
| evaluate() (parse caps each call) | ~5040 |
| evaluate_with_caps() (prebuilt) | ~4860 |

~3.6% faster. Both are dominated by the provenance append (JSON+SHA-256 chain,
~4 µs); the remaining headroom is in provenance, tracked for future batching.
Honest result: small, because authorization itself is already ~1.5 µs.
