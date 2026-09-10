# Architecture

## Principle

The AI is probabilistic. The operating system must not be. The AI may make
mistakes, be compromised, or receive ambiguous instructions. The OS must
contain the consequences, preserve security boundaries, and either ask for
clarification or restrict execution.

```
HUMAN INTENT
   -> AGENTS            (probabilistic: propose plans)
   -> VERIFIED PLANS    (parsed, validated, typed)
   -> CAPABILITY CONTROL(deterministic: scoped, revocable authority)
   -> DETERMINISTIC EXECUTION (tool broker + transactions)
   -> LINUX             (namespaces, cgroups, seccomp, Landlock, LSM, eBPF)
   -> HARDWARE
```

Every model output — text, tool calls, JSON, code, shell, URLs, filenames,
capability requests — is untrusted data. Nothing executes because it is
syntactically valid. The pipeline is:

```
model output -> parse -> validate -> capability check -> policy check
             -> resource check -> risk check -> approval? -> execute -> provenance
```

## Components (target)

Userspace daemons (`userspace/`), all communicating over authenticated,
capability-scoped IPC — never by handing agents raw host access:

| Daemon         | Responsibility                                            |
|----------------|-----------------------------------------------------------|
| `intentd`      | Intent lifecycle: create/validate/authorize/plan/execute/rollback |
| `agentd`       | Agent identity, lifecycle, resource budgets               |
| `governord`    | Deterministic policy + capability + risk enforcement      |
| `policyd`      | Policy storage/evaluation (authoritative, deterministic)  |
| `provenanced`  | Append-only provenance graph of meaningful actions        |
| `transactiond` | begin/preview/checkpoint/commit/abort/rollback            |
| `modeld`       | Local model lifecycle, manifests, inference               |
| `resourced`    | CPU/GPU/NPU/RAM/VRAM/storage/network accounting           |
| `memoryd`      | Owned, access-controlled AI memory classes                |
| `agent-broker` | Tool broker + multi-agent IPC; the only path to host tools|

Shared crates (`libraries/`): `libintent`, `libagent`, `libcapability`,
`libprovenance`, `libtransaction`, `libaifabric`, `libcredentials`.

## Security posture

- Prompt instructions are never the ultimate security boundary. Authority
  comes only from the capability/policy layer.
- Fail closed for security-critical operations.
- Least privilege: no blanket permissions, no agent root.
- High-level capabilities map onto Linux mechanisms: namespaces, cgroups,
  Linux capabilities, seccomp, Landlock, LSM, eBPF, systemd sandboxing.
- API keys never touch source, git, logs, provenance, or diagnostics.

## Repository tree

```
ai-native-os/
  docs/{architecture,design,research,threat-model,specifications,decisions}/
  kernel/{patches,modules,configs}/
  userspace/{intentd,agentd,governord,policyd,provenanced,transactiond,
             modeld,resourced,memoryd,agent-broker}/
  libraries/{libintent,libagent,libcapability,libprovenance,libtransaction,
             libaifabric,libcredentials}/
  sdk/{rust,python,examples}/
  models/{registry,manifests}/
  security/{policies,profiles,test-attacks}/
  tests/{unit,integration,security,kernel,agents,end-to-end}/
  tools/  scripts/  build/
```

Deviation from the spec's tree: `libcredentials` is added under `libraries/`
(the spec names it in section 10 but omits it from the tree). Rationale in
[docs/decisions/ADR-0003-credential-store.md](docs/decisions/ADR-0003-credential-store.md).

## Implementation status

The deterministic core (capabilities, intent, governor, provenance,
transactions, agent identity, credentials, provider abstraction, multi-agent
broker) is implemented as tested Rust libraries under `libraries/` and exercised
by the `aios` binary under `userspace/aios`. See docs/STATUS.md.
