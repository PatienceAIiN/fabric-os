# ADR-0012: Local model runtime (real CPU inference via llama.cpp)

## Problem
"Local-first, works offline without cloud AI" must be real, not a stub, on a
7 GB CPU-only laptop — without a heavy Rust ML dependency tree.

## Selected approach
- **llama.cpp** (upstream, C/C++), built here as the `llama-cli` target only
  (guarded, CPU backend, no CUDA). Small, self-contained, CPU-efficient.
- **Model**: Qwen2.5-0.5B-Instruct Q4_K_M GGUF (~469 MB), which fits in RAM and
  runs at ~40-50 tok/s generation on this CPU (measured).
- **`libprovider::LlamaProvider`** implements the same `AIProvider` trait as the
  cloud/stub providers, shelling out to `llama-cli` and parsing the completion
  (parser unit-tested; live inference gated on `AIOS_LLAMA_CLI`/`AIOS_LLAMA_MODEL`).
- **`aiosd`** prefers `LlamaProvider` for local routes when a model is
  configured, else falls back to the deterministic stub — so the desktop never
  blocks and offline still works.
- **Supply chain (M19/M70)**: the model is registered via a manifest
  (`models/manifests/…json`) with a real SHA-256 and trust status; `aios model
  verify` refuses a tampered file (demonstrated). libmodel gates load on
  verification + memory fit.

## Why shell-out (not a Rust binding) now
Keeps the Rust workspace dependency-light and the build fast on this host. A
persistent `llama-server` (OpenAI-compatible) or an in-process binding is a
later optimization for latency (avoids per-call model load).

## Tradeoffs / future
Per-call model load adds ~1 s latency; fine for a prototype, not for
high-throughput. GPU/NPU offload (M23) needs drivers (ROCm/Vulkan) not wired
here; the CPU path is real today. Larger models are a config change + more RAM.

## Security
The model file is treated as untrusted until checksum+trust verified. Inference
runs as a subprocess and can be wrapped by `libsandbox` (ProtectSystem, no net)
like any other tool.
