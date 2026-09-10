# Prior art landscape

Claims of novelty require prior-art search. This is the running survey; each
entry notes what exists so we can state differences honestly rather than
claim invention.

## Agent / AI-OS projects
- **AIOS (agent-as-OS research, "LLM as kernel")** — schedules agents, manages
  context/memory/tools as an OS-like layer, but runs as a userspace framework
  on top of Linux, not as an OS with kernel-enforced capability boundaries.
- **MemGPT / letta** — tiered agent memory (working vs archival). Informs
  `memoryd` memory classes; not a full OS memory-ownership/ACL model.
- **Generic agent frameworks** (LangChain, AutoGPT-style, CrewAI) — orchestrate
  tools/agents in-process with ambient authority. We explicitly reject ambient
  authority; authority is capability/policy-derived and kernel-backed.

## OS / security mechanisms we build on (not reinvent)
- **Capability security**: capsicum (FreeBSD), seL4 capabilities, Fuchsia
  handles, object-capability literature (Miller). We map high-level caps onto
  Linux primitives rather than inventing a new microkernel.
- **Linux enforcement**: namespaces, cgroup v2, Linux capabilities, seccomp-
  bpf, Landlock (path-scoped FS), LSM/BPF-LSM, eBPF, systemd sandboxing.
- **sched_ext (CONFIG_SCHED_CLASS_EXT)**: pluggable BPF schedulers — the
  vehicle for agent-aware scheduling research (M22) without forking core.
- **DAMON**: data-access-aware memory management, useful for model working sets.
- **Provenance**: PROV-DM, CamFlow (whole-system provenance via LSM). Informs
  `provenanced` graph design.

## Credential storage
- freedesktop Secret Service (gnome-keyring/KWallet), systemd credentials,
  TPM2 sealing. ADR-0003 selects an approach; we do not invent crypto.

## Honest positioning
Nothing above is assumed novel. The candidate contribution is the *integration*:
intent-scoped capabilities enforced deterministically by Linux primitives,
with provenance and transactional rollback, under an untrusted-AI threat model.
Each research feature must search literature and state differences before any
novelty claim (see docs/research/ per-feature notes, to be added).
