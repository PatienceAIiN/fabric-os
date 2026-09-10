# Contributing

## Golden rule on the dev laptop

Run every build, test, and VM under the resource guard:

```
tools/rg -- cargo build
tools/rg --profile test -- cargo test
```

Never run unguarded heavy jobs; this machine has 7 GB RAM and will thrash.

## Workflow per milestone

Research -> design -> document -> implement -> test -> QEMU boot/test ->
benchmark -> security analysis -> commit -> update docs -> update roadmap.

## Rules

- Prefer Rust for new security-sensitive userspace. C only where kernel/low-
  level compat requires it. Python for tooling/experiments.
- Validate all input. Document every `unsafe` block. Fail closed.
- No unsupported performance or novelty claims. Measure; cite prior art.
- No secrets in code, logs, or provenance.
