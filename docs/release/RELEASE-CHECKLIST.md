# Fabric OS release checklist

## Source and supply chain

- [ ] Clean, reviewed commit and release tag.
- [ ] `cargo test` and security tests pass through `tools/rg`.
- [ ] Website production build passes.
- [ ] Dependency lockfiles are committed and reviewed.
- [ ] SBOM and license report generated for Rust, Node, kernel, rootfs, firmware, and models.
- [ ] Vulnerability scan reviewed; exceptions have owners and expiry dates.
- [ ] Artifact hashes and provenance manifest generated.
- [ ] Release artifacts signed with protected release keys.

## OS behavior

- [ ] Boot verified on each named x86_64 UEFI test device with the exact release artifact.
- [ ] Install target, disk selection, recovery path, and rollback are tested on dedicated hardware; no daily-use machine is used for first install.
- [ ] Device matrix records CPU, memory, graphics, storage, Wi-Fi, audio, sleep, firmware, and known limitations.
- [ ] QEMU remains an automated regression fallback for repeatable CI checks; it is not evidence of physical-device readiness.
- [ ] Default-deny and fail-closed behavior tested.
- [ ] Capability scope, revocation, identity, approval, and rollback tested.
- [ ] Credential redaction tested across logs, diagnostics, provenance, and crash paths.
- [ ] Local-only mode works without cloud credentials or network access.
- [ ] Optional cloud provider path uses the credential store and provider abstraction.
- [ ] Resource limits and denial-of-service behavior measured.
- [ ] Update, interrupted-update, recovery, and rollback paths tested.
- [ ] Backup, restore, and data deletion behavior tested.

## Public release

- [ ] Supported hardware and known limitations match evidence.
- [ ] Release notes include checksums, signature instructions, risk, and recovery steps.
- [ ] Privacy, Terms, Licenses, Compliance, Security, and contact pages are current.
- [ ] No internal endpoints, credentials, unpublished rules, or sensitive implementation detail appear in public content.
- [ ] Legal, privacy, security, and release owners approve the launch record.
