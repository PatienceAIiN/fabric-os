# ADR-0003: Secure credential storage (libcredentials)

## Problem
Users configure a Claude API key. It must never reach applications, agents,
source, git, logs, provenance, or diagnostics, and never display after entry.

## Existing approaches / alternatives
- Plaintext JSON/env: rejected outright by requirements.
- freedesktop **Secret Service** (gnome-keyring/KWallet): standard desktop
  keyring, D-Bus API, encrypts at rest, unlocks with login. Available on the
  target GNOME desktop.
- **systemd credentials** (`systemd-creds`, LoadCredentialEncrypted): TPM-
  sealed, ideal for delivering a secret to a *service* at start, less suited to
  interactive user-managed rotation.
- **TPM2 sealing** directly: strongest binding, more moving parts.

## Selected approach
`libcredentials` is an abstraction with pluggable backends. Default backend:
Secret Service for user-entered keys (interactive, rotatable). Service-delivery
backend: systemd encrypted credentials for daemons that need a secret at start.
TPM-backed sealing is a future backend. We do not invent cryptography.

The AI provider service is the only component that resolves the raw key;
applications/agents call the local AI API and never receive the key. A
compromised app cannot extract the key merely because system-wide AI is on.

## Tradeoffs
Two backends add surface, but interactive rotation and service delivery have
genuinely different needs.

## Security implications
Central redaction: a single `redact()` path scrubs key-shaped material from all
logs/diagnostics/provenance. Fail closed if no secure backend is available
(refuse to store rather than fall back to plaintext).

## Performance implications
Negligible; secrets are fetched rarely and cached in-process by the AI service
only.

## Future migration path
Add TPM2-sealed and hardware-token backends behind the same trait.
