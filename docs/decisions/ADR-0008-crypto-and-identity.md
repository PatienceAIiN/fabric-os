# ADR-0008: Cryptography and agent identity (prototype -> production)

## Problem
Agents need cryptographic identities and message authentication; provenance
needs tamper-evidence; credentials need encryption at rest. We must avoid both
inventing crypto and dragging in a heavy dependency tree on a low-RAM host.

## Selected approach (prototype)
- **Hashing / MAC**: a zero-dependency `libcrypto` implements SHA-256 (FIPS
  180-4) and HMAC-SHA256 (RFC 2104), verified against published NIST/RFC 4231
  test vectors in-tree. These are standard, fully specified algorithms, not
  invented crypto. Zero deps keeps builds tiny and fast on this machine.
- **Agent identity**: `agent_id` is content-addressed (SHA-256 of public
  metadata + a `/dev/urandom` nonce). Message authentication uses HMAC-SHA256
  with a per-agent secret, bound to the agent_id to prevent cross-identity
  reuse. Names are never identities.
- **Provenance**: hash-chained records (each hash covers content + prev hash);
  any edit/reorder is detected by `verify()`.
- **Credentials at rest**: `systemd-creds` (host/TPM-bound) via `libcredentials`
  — a real facility, no invented crypto, no root, no plaintext on disk.

## Tradeoffs / production path
HMAC is symmetric: it authenticates within the local trust domain but is not
public-key. Production agent identity should migrate to **ed25519** (a vetted
library) so identities are verifiable without sharing secrets, enabling
cross-host/agent attestation. The `Agent::sign/verify` API is shaped to allow
swapping the primitive without changing callers. Hand-rolled SHA-256 should be
replaced by a vetted crate before any security claim beyond "prototype".

## Security implications
Verified vectors give confidence in correctness now; the ed25519 migration is
required before trusting identities across trust boundaries. Constant-time
compare (`ct_eq`) is used for MAC verification to avoid timing leaks.
