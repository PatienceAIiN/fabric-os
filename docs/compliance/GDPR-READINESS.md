# GDPR readiness notes

The GDPR applies based on the processing and people affected, not only where the company is incorporated. Treat this document as a control map, not legal advice.

## Required operating records

- Controller/processor role analysis for the website, downloads, support, telemetry, and any AI-provider flow.
- Record of processing activities with purpose, categories, lawful basis, recipients, transfers, retention, and security measures.
- Vendor/subprocessor register and Article 28 processing agreements where applicable.
- Data retention and deletion schedule; do not keep download or infrastructure logs indefinitely.
- Data-subject request intake, identity verification, triage, response, and evidence procedure.
- Personal-data breach register and escalation process; assess notification to the supervisory authority within 72 hours where required.
- Transfer assessment and appropriate safeguards for processing outside the EEA.
- DPIA when processing is likely to result in a high risk to people.

## Product defaults

- Minimise collection: the download endpoint accepts only a bounded platform string.
- Do not put API keys, prompts, files, or model outputs into download analytics or provenance records.
- Keep non-essential analytics, advertising, and third-party cookies disabled unless a compliant consent mechanism is added.
- Use clear, plain-language notices at the point of collection.
- Make support and rights requests possible at `info@patienceai.in` until a dedicated privacy contact is approved.

## Launch decisions still required

1. Legal entity and establishment/representative details.
2. Lawful basis and retention period for each processing operation.
3. Hosting, email, CDN, monitoring, database, and AI vendors.
4. Whether children or special-category data can enter any product flow.
5. Applicable supervisory authority and complaint route.
