# Fabric OS: OS-wide launch readiness

Status: **engineering controls implemented; not launch-approved**.

This checklist covers the bootable Linux-based operating system as well as the public website. It is an engineering and operations control map, not legal advice or a certification.

## Product scope

The release surface includes the upstream Linux kernel and boot chain, root filesystem, system services, AI userspace, Rust libraries, SDKs, model manifests and weights, build scripts, QEMU images, update/recovery path, documentation, and website.

## Release blockers

- [ ] Confirm Patience AI’s legal entity, registered address, jurisdiction, support/privacy contact, and launch countries.
- [ ] Approve the OS EULA, website Terms, Privacy Notice, OSS notices, acceptable-use rules, and export/sanctions position.
- [ ] Decide controller/processor roles for website events, support, OS diagnostics, AI-provider traffic, telemetry, crash reports, and updates.
- [ ] Complete ROPA, data map, retention/deletion schedule, vendor/subprocessor register, DPAs, transfer assessments, and rights-request workflow.
- [ ] Decide whether a DPO or representative is required in each target market; publish the correct contact details.
- [ ] Run security and privacy risk assessments; complete a DPIA where processing is likely to be high risk.
- [ ] Define a personal-data incident process and test assessment, containment, notification, and recovery paths.
- [ ] Review child safety, accessibility, consumer protection, tax, product safety, and sector-specific requirements for every market.
- [ ] Inventory every package, kernel component, firmware, font, icon, model, dataset, and binary; record license and provenance.
- [ ] Produce an SBOM, dependency/vulnerability report, signed artifact manifest, checksums, and reproducible-build evidence.
- [ ] Establish secure update, rollback, recovery, key rotation, revocation, and end-of-life procedures.
- [ ] Verify that credentials never enter source, images, logs, provenance, crash reports, or ordinary settings.
- [ ] Run real-device boot, install, upgrade, rollback, recovery, sandbox, injection, data-loss, resource-exhaustion, and network-boundary tests on every named supported device.
- [ ] Keep QEMU as a repeatable regression check, but do not use it as evidence that physical hardware is ready.
- [ ] Obtain independent security review and legal sign-off before describing the OS as production-ready.

## Public boundary

The website may describe goals, supported environments, public principles, and release status. It must not disclose private threat-response details, unpublished business rules, secrets, raw model prompts, credentials, internal endpoints, or exploitable implementation detail.

## Exit evidence

Each checked item must have an owner, evidence link, date, and approver. A green website page is not evidence that the OS itself is compliant or safe for production.

## Verified in this repository

- [x] Website production build and public legal routes.
- [x] Rust workspace tests through the resource-safe test harness.
- [x] Website dependency audit and SBOM input generation.
- [x] Release SHA-256 manifest generation for artifacts that exist.
- [x] Consent endpoint with policy-versioned category choices and no stored IP/user-agent.
- [x] Baseline security headers, API body limit, and API rate limit.

These checks verify repository behavior only. They do not replace production deployment controls, legal approval, a security audit, or signed release-key custody.
