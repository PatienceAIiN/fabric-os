//! Agent identity as an OS principal.
//!
//! Every agent has a cryptographic identity, not a name. The `agent_id` is a
//! content-addressed digest of the agent's public metadata plus a random
//! nonce. A per-agent secret key authenticates the agent's messages via
//! HMAC-SHA256 (prototype; ed25519 is the production path, ADR-0008). Names
//! are never used as security identities.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustLevel {
    Untrusted,
    Limited,
    Standard,
    Elevated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub cpu_percent: u32, // e.g. 200 = 2 cores
    pub ram_bytes: u64,
    pub gpu_percent: u32,
    pub net_bps: u64,
    pub duration_secs: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        // Conservative least-privilege defaults.
        ResourceLimits {
            cpu_percent: 100,
            ram_bytes: 512 * 1024 * 1024,
            gpu_percent: 0,
            net_bps: 0,
            duration_secs: 300,
        }
    }
}

/// Public, shareable agent identity (no secret material).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub agent_id: String, // sha256 hex, content-addressed
    pub owner: String,
    pub purpose: String,
    pub model_identity: String,
    pub creation_time: u64,
    pub expiration: u64,
    pub trust_level: TrustLevel,
    pub resource_limits: ResourceLimits,
}

/// An agent with its private authentication key. Never serialize the secret.
#[derive(Clone)]
pub struct Agent {
    pub identity: AgentIdentity,
    secret: [u8; 32],
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn random32() -> [u8; 32] {
    let mut buf = [0u8; 32];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        if f.read_exact(&mut buf).is_ok() {
            return buf;
        }
    }
    // Fallback (documented weakness): time + address entropy. Prototype only.
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seed = libcrypto::sha256(&t.to_le_bytes());
    buf.copy_from_slice(&seed);
    buf
}

impl Agent {
    /// Mint a new agent. `ttl_secs` sets expiration from now.
    pub fn create(
        owner: &str,
        purpose: &str,
        model_identity: &str,
        trust_level: TrustLevel,
        resource_limits: ResourceLimits,
        ttl_secs: u64,
    ) -> Agent {
        let secret = random32();
        let nonce = random32();
        let created = now();
        let mut pre = Vec::new();
        pre.extend_from_slice(owner.as_bytes());
        pre.push(0);
        pre.extend_from_slice(purpose.as_bytes());
        pre.push(0);
        pre.extend_from_slice(model_identity.as_bytes());
        pre.push(0);
        pre.extend_from_slice(&created.to_le_bytes());
        pre.extend_from_slice(&nonce);
        let agent_id = libcrypto::sha256_hex(&pre);
        Agent {
            identity: AgentIdentity {
                agent_id,
                owner: owner.to_string(),
                purpose: purpose.to_string(),
                model_identity: model_identity.to_string(),
                creation_time: created,
                expiration: created.saturating_add(ttl_secs),
                trust_level,
                resource_limits,
            },
            secret,
        }
    }

    pub fn id(&self) -> &str {
        &self.identity.agent_id
    }

    pub fn is_expired(&self, at: u64) -> bool {
        at >= self.identity.expiration
    }

    /// Authenticate a message. Tag binds to the agent_id to prevent reuse
    /// across identities.
    pub fn sign(&self, msg: &[u8]) -> [u8; 32] {
        let mut m = self.identity.agent_id.as_bytes().to_vec();
        m.push(0);
        m.extend_from_slice(msg);
        libcrypto::hmac_sha256(&self.secret, &m)
    }

    /// Verify a tag this agent produced (constant time).
    pub fn verify(&self, msg: &[u8], tag: &[u8]) -> bool {
        libcrypto::ct_eq(&self.sign(msg), tag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk() -> Agent {
        Agent::create(
            "user:harsh",
            "organize-downloads",
            "local/reasoning",
            TrustLevel::Limited,
            ResourceLimits::default(),
            300,
        )
    }

    #[test]
    fn ids_are_unique_and_hex() {
        let a = mk();
        let b = mk();
        assert_ne!(a.id(), b.id()); // random nonce => distinct
        assert_eq!(a.id().len(), 64);
        assert!(a.id().bytes().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn sign_verify_roundtrip_and_reject() {
        let a = mk();
        let tag = a.sign(b"proposal:move /a -> /b");
        assert!(a.verify(b"proposal:move /a -> /b", &tag));
        assert!(!a.verify(b"proposal:move /a -> /c", &tag)); // tampered msg
    }

    #[test]
    fn a_cannot_forge_bs_tag() {
        let a = mk();
        let b = mk();
        let tag = a.sign(b"x");
        assert!(!b.verify(b"x", &tag)); // different secret => reject
    }

    #[test]
    fn expiry() {
        let a = mk();
        let exp = a.identity.expiration;
        assert!(!a.is_expired(exp - 1));
        assert!(a.is_expired(exp));
    }

    #[test]
    fn identity_serializes_without_secret() {
        let a = mk();
        let j = serde_json::to_string(&a.identity).unwrap();
        assert!(j.contains("agent_id"));
        assert!(!j.contains("secret")); // secret is not part of AgentIdentity
    }
}
