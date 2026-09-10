//! Append-only, tamper-evident provenance.
//!
//! Every meaningful action is recorded as a [`Record`] linked into a hash
//! chain: each record's `hash` covers its content plus the previous record's
//! hash, so any edit to history is detectable via [`ProvenanceLog::verify`].
//! Secrets and API keys are never placed in provenance; [`redact`] scrubs
//! key-shaped material defensively.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyDecision {
    Allowed,
    Denied,
    Approved,
    Blocked,
}

/// The content of one provenance event (everything except its own hash).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordContent {
    pub seq: u64,
    pub timestamp: u64,
    pub actor: String,
    pub agent_id: String,
    pub intent_id: String,
    pub model: String,
    pub tool: String,
    pub action: String,
    pub resource: String,
    pub result: String,
    pub policy_decision: PolicyDecision,
    pub parent: Option<u64>,
    pub prev_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    #[serde(flatten)]
    pub content: RecordContent,
    pub hash: String,
}

/// Redact key-shaped material so secrets never persist in provenance/logs.
/// Conservative: masks long base64/hex-ish runs and known key prefixes.
pub fn redact(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for tok in s.split_inclusive(|c: char| c.is_whitespace()) {
        let (word, tail) = split_trailing_ws(tok);
        if looks_secret(word) {
            out.push_str("[REDACTED]");
        } else {
            out.push_str(word);
        }
        out.push_str(tail);
    }
    out
}

fn split_trailing_ws(tok: &str) -> (&str, &str) {
    let idx = tok
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_whitespace())
        .last()
        .map(|(i, _)| i)
        .unwrap_or(tok.len());
    tok.split_at(idx)
}

fn looks_secret(w: &str) -> bool {
    let w = w.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    if w.starts_with("sk-") && w.len() >= 12 {
        return true;
    }
    // long high-entropy-ish token
    w.len() >= 24
        && w.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && w.chars().any(|c| c.is_ascii_digit())
        && w.chars().any(|c| c.is_ascii_alphabetic())
}

fn hash_content(c: &RecordContent) -> String {
    let json = serde_json::to_vec(c).expect("record content serializes");
    libcrypto::sha256_hex(&json)
}

/// The genesis previous-hash for the first record.
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProvenanceLog {
    records: Vec<Record>,
}

#[allow(clippy::too_many_arguments)]
impl ProvenanceLog {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    fn last_hash(&self) -> String {
        self.records
            .last()
            .map(|r| r.hash.clone())
            .unwrap_or_else(|| GENESIS.to_string())
    }

    /// Append an event. Fields are redacted defensively; the record is
    /// hash-chained to the previous one. Returns the new record's seq.
    pub fn append(
        &mut self,
        timestamp: u64,
        actor: &str,
        agent_id: &str,
        intent_id: &str,
        model: &str,
        tool: &str,
        action: &str,
        resource: &str,
        result: &str,
        policy_decision: PolicyDecision,
        parent: Option<u64>,
    ) -> u64 {
        let seq = self.records.len() as u64;
        let content = RecordContent {
            seq,
            timestamp,
            actor: redact(actor),
            agent_id: agent_id.to_string(),
            intent_id: intent_id.to_string(),
            model: model.to_string(),
            tool: tool.to_string(),
            action: redact(action),
            resource: redact(resource),
            result: redact(result),
            policy_decision,
            parent,
            prev_hash: self.last_hash(),
        };
        let hash = hash_content(&content);
        self.records.push(Record { content, hash });
        seq
    }

    /// Verify the whole chain: seq order, prev_hash links, and content hashes.
    pub fn verify(&self) -> Result<(), String> {
        let mut prev = GENESIS.to_string();
        for (i, r) in self.records.iter().enumerate() {
            if r.content.seq != i as u64 {
                return Err(format!("seq mismatch at {i}"));
            }
            if r.content.prev_hash != prev {
                return Err(format!("broken chain at seq {i}"));
            }
            if hash_content(&r.content) != r.hash {
                return Err(format!("content tampered at seq {i}"));
            }
            prev = r.hash.clone();
        }
        Ok(())
    }

    pub fn by_intent(&self, intent_id: &str) -> Vec<&Record> {
        self.records
            .iter()
            .filter(|r| r.content.intent_id == intent_id)
            .collect()
    }
    pub fn by_agent(&self, agent_id: &str) -> Vec<&Record> {
        self.records
            .iter()
            .filter(|r| r.content.agent_id == agent_id)
            .collect()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("log serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log() -> ProvenanceLog {
        let mut l = ProvenanceLog::new();
        l.append(
            1,
            "user",
            "agentA",
            "intent1",
            "local",
            "fs",
            "read",
            "/a",
            "ok",
            PolicyDecision::Allowed,
            None,
        );
        l.append(
            2,
            "user",
            "agentA",
            "intent1",
            "local",
            "fs",
            "write",
            "/a/out",
            "ok",
            PolicyDecision::Allowed,
            Some(0),
        );
        l
    }

    #[test]
    fn chain_verifies() {
        assert!(log().verify().is_ok());
    }

    #[test]
    fn tamper_is_detected() {
        let mut l = log();
        l.records[0].content.resource = "/etc/passwd".into(); // tamper
        assert!(l.verify().is_err());
    }

    #[test]
    fn reorder_is_detected() {
        let mut l = log();
        l.records.swap(0, 1);
        assert!(l.verify().is_err());
    }

    #[test]
    fn queries_filter() {
        let l = log();
        assert_eq!(l.by_intent("intent1").len(), 2);
        assert_eq!(l.by_agent("agentA").len(), 2);
        assert_eq!(l.by_intent("nope").len(), 0);
    }

    #[test]
    fn secrets_are_redacted() {
        let mut l = ProvenanceLog::new();
        l.append(
            1,
            "user",
            "a",
            "i",
            "m",
            "net",
            "post",
            "Authorization: Bearer sk-ant-api03-ABCDEF1234567890abcdef",
            "sent",
            PolicyDecision::Allowed,
            None,
        );
        let j = l.to_json();
        assert!(!j.contains("sk-ant-api03"));
        assert!(j.contains("[REDACTED]"));
    }
}
