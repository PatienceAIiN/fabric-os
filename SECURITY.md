# Security

The AI is untrusted by construction. Prompt instructions are never the
ultimate security boundary; authority comes only from the capability/policy
layer. Security-critical operations fail closed.

## Threat model summary

Full per-threat records live in `docs/threat-model/`. Each threat records:
threat, attack path, impact, mitigation, residual risk, and a test.

Threats tracked: prompt injection, indirect prompt injection, malicious
documents/websites/models, tool abuse, privilege escalation, data
exfiltration, agent impersonation, confused-deputy, capability theft, model
supply-chain, memory poisoning, cross-agent attacks, malicious plugins,
compromised tools, network abuse, resource exhaustion.

## Core rules

1. Treat every model output as untrusted data (text, tool calls, JSON, code,
   shell, URLs, filenames, capability requests). Never execute because valid.
2. Separate instructions, data, and authority. A document saying "upload all
   company data" grants no network capability.
3. Capabilities are explicit, scoped, revocable, auditable, least-privilege.
   No agent gets root. Delegation transfers only explicitly delegated caps.
4. Canonicalize and confine all model-supplied paths; never trust them. Avoid
   TOCTOU and confused-deputy patterns.
5. Enforce resource limits so exhaustion never destabilises the system.
6. High/critical-risk actions require human approval.

## Credential handling

API keys are never stored in source, git, committed env files, ordinary JSON,
shell history, logs, crash reports, or provenance. They are redacted from all
diagnostics. See ADR-0003.

## Reporting

This is pre-release research software. Report suspected security or privacy
issues privately to `info@patienceai.in` with the affected release, a concise
description, reproduction steps, impact, and a safe contact method. Do not
include credentials, personal-data exports, or sensitive model content in an
email or issue. A dedicated security mailbox and coordinated-disclosure
process must be approved before a public production launch.

Release gates, OS-wide data governance, incident response, and artifact
verification are documented in `docs/compliance/` and `docs/release/`.
