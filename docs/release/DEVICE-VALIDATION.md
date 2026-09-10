# Real-device validation

This is the required evidence path before Fabric OS is described as ready for physical hardware. The current build is an early research image; it is not a universal installer.

## Device record

For each exact device, record manufacturer, model, CPU, RAM, graphics, storage, Wi-Fi, Bluetooth, audio, display, firmware/UEFI version, Secure Boot state, disk layout, release hash, and tester/date.

## Safe installation

1. Use a spare device or a separately replaceable test disk.
2. Back up and verify anything on the target disk.
3. Confirm the target disk twice before writing.
4. Preserve a recovery image and documented return path.
5. Verify the release signature and SHA-256 manifest before booting.
6. Record each command and result without recording secrets or personal data.

## Functional checks

- Boot, shutdown, restart, display, keyboard, touchpad, storage, USB, network, audio, clock, sleep, wake, and thermal behavior.
- Local-only mode with no network and no cloud credential.
- Provider enable/disable, masked credentials, privacy indicator, and network permission.
- Least-privilege action, denied action, approval-required action, provenance view, rollback, and deletion with synthetic test data.
- Update, interrupted update, recovery, downgrade/rollback, and power-loss behavior.
- Resource pressure, network loss, malformed input, prompt injection, malicious document, and untrusted model tests.

## Exit rule

No device is supported until every applicable check has evidence, a named owner, a known limitation, and release approval. A QEMU pass can support regression confidence but cannot substitute for this device record.
