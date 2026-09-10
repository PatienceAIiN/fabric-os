# Fabric OS launch readiness

Status: **not launch-approved**. This checklist is an engineering and operations starting point, not a legal certification. A qualified lawyer and privacy professional must approve the final release for each market.

## Blocking items before a public launch

- [ ] Confirm the contracting legal entity, registered address, jurisdiction, support address, and privacy contact.
- [ ] Decide whether Patience AI is controller, processor, or both for each product flow.
- [ ] Approve the final Privacy Notice, Terms/EULA, OSS notices, cookie/consent position, and support process.
- [ ] Complete the record of processing activities (ROPA), vendor/subprocessor register, retention schedule, and deletion workflow.
- [ ] Decide whether a DPO or EU representative is required and publish the applicable contact details.
- [ ] Run a risk assessment and DPIA where processing is likely to create high risk.
- [ ] Put a data-subject request workflow in place and test access, deletion, correction, portability, objection, and restriction requests.
- [ ] Put a personal-data breach workflow in place, including a documented 72-hour assessment/notification path where applicable.
- [ ] Review international transfers and sign processor/data-processing agreements with relevant vendors.
- [ ] Complete export-control, sanctions, consumer-protection, accessibility, tax, and product-safety review for every target market.
- [ ] Generate an SBOM, scan dependencies, verify licenses, sign release artifacts, publish checksums, and preserve build provenance.
- [ ] Have counsel approve all public claims, warranties, disclaimers, pricing, support promises, and launch geography.

## Engineering controls in this repository

- Apache License 2.0 at the repository root.
- Security principles and threat model in `SECURITY.md` and `docs/threat-model/`.
- Website legal pages at `/privacy`, `/terms`, `/licenses`, and `/compliance`.
- PostgreSQL download-event schema intentionally stores only a platform label and timestamp.
- The website sets baseline security headers and does not ship advertising or analytics cookies.

## Release gates

No release is complete until the responsible owner records the decision, evidence, date, and approver for each blocking item. Keep an incident log and retain evidence according to the approved retention schedule.
