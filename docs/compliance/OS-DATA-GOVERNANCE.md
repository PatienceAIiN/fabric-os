# OS data governance

Fabric OS is local-first. That is a product design goal, not a substitute for a completed data-protection assessment.

## Data classes to control

| Class | Examples | Default handling |
| --- | --- | --- |
| Credentials | Provider keys, tokens, recovery material | Secure credential facility only; never ordinary settings or logs |
| Intent and plans | User requests, proposed actions, approvals | Local, access-controlled, minimised, and retention-limited |
| Provenance | Meaningful action records and hashes | No secrets or raw sensitive content; user-visible retention controls |
| AI memory | User-created memory and derived context | Explicit ownership, access control, export, deletion, and retention |
| Model data | Manifests, checksums, weights, prompts, outputs | Separate public metadata from private content; record license/provenance |
| Diagnostics | Crash, performance, security, and update events | Off by default where practical; consent/notice, minimisation, redaction |
| Website/support | Download events, emails, support content | Separate from OS data; documented controller, processor, retention, and access |

## Required controls before product launch

- Privacy by design and default for every new data flow.
- A lawful purpose and retention decision for each class.
- Export and deletion behavior tested for user-controlled data.
- Redaction tests for keys, tokens, prompts, files, and personal data.
- Clear local/cloud indicator and network permission for every cloud provider.
- User-visible provider status, usage, privacy, and network controls.
- No raw provider key exposure to ordinary applications or agents.
- Documented subprocessors, regions, transfer safeguards, and incident contacts.

## Consent implementation

- The website shows a clear privacy banner before optional analytics is enabled.
- Necessary preference storage is limited to the `fabric_consent` cookie.
- Optional analytics is off by default and can be accepted, declined, or withdrawn.
- Consent events record only policy version, category choices, and timestamp when PostgreSQL is configured; no IP address, user-agent, email, prompt, or OS data is stored by this endpoint.
- The OS must use the same principles in its Settings/privacy surface: clear notice, purpose-specific choices, easy withdrawal, and an auditable policy version.
- Before launch, counsel must confirm the lawful basis, retention period, data-fiduciary/controller role, and whether any additional consent categories are needed.
