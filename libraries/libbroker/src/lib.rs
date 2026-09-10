//! Agent broker (M21/M33).
//!
//! Capability-scoped delegation between agents with revocation, plus the
//! multi-agent message envelope. Delegation never widens authority; revoking a
//! parent invalidates everything it delegated (no persistence after
//! revocation). Every message carries sender/receiver/intent/capability/
//! provenance so nothing is ambient.
#![forbid(unsafe_code)]

use libcapability::{Capability, CapabilitySet};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

struct Node {
    parent: Option<String>,
    caps: CapabilitySet,
}

#[derive(Default)]
pub struct Broker {
    nodes: HashMap<String, Node>,
    revoked: HashSet<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BrokerError {
    NoSuchAgent,
    NotPermittedByParent(Vec<String>),
    SenderRevoked,
    NotDelegated(String),
}

/// Kinds of multi-agent messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MsgKind {
    Request,
    Response,
    Delegation,
    Cancellation,
    Event,
}

/// A multi-agent message. Authority to act on it still flows through the
/// capability system; the envelope is metadata + provenance, not authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub sender: String,
    pub receiver: String,
    pub intent: String,
    pub purpose: String,
    pub capability: String,
    pub timestamp: u64,
    pub provenance: Option<u64>,
    pub kind: MsgKind,
    pub body: String,
}

impl Broker {
    pub fn new() -> Self {
        Broker::default()
    }

    pub fn register_root(&mut self, agent_id: &str, caps: CapabilitySet) {
        self.nodes
            .insert(agent_id.to_string(), Node { parent: None, caps });
    }

    pub fn delegate(
        &mut self,
        parent: &str,
        child: &str,
        requested: &[String],
    ) -> Result<(), BrokerError> {
        if self.ancestor_revoked(parent) {
            return Err(BrokerError::SenderRevoked);
        }
        let pcaps = self
            .nodes
            .get(parent)
            .ok_or(BrokerError::NoSuchAgent)?
            .caps
            .clone();
        let child_caps = pcaps
            .delegate(requested.iter())
            .map_err(BrokerError::NotPermittedByParent)?;
        self.nodes.insert(
            child.to_string(),
            Node {
                parent: Some(parent.to_string()),
                caps: child_caps,
            },
        );
        Ok(())
    }

    pub fn revoke(&mut self, agent_id: &str) {
        self.revoked.insert(agent_id.to_string());
    }

    pub fn is_revoked(&self, agent_id: &str) -> bool {
        self.ancestor_revoked(agent_id)
    }

    fn ancestor_revoked(&self, agent_id: &str) -> bool {
        let mut cur = Some(agent_id.to_string());
        let mut hops = 0;
        while let Some(id) = cur {
            if self.revoked.contains(&id) {
                return true;
            }
            cur = self.nodes.get(&id).and_then(|n| n.parent.clone());
            hops += 1;
            if hops > 64 {
                return true;
            }
        }
        false
    }

    pub fn permits(&self, agent_id: &str, requested: &str) -> bool {
        if self.ancestor_revoked(agent_id) {
            return false;
        }
        let node = match self.nodes.get(agent_id) {
            Some(n) => n,
            None => return false,
        };
        match Capability::parse(requested) {
            Ok(cap) => node.caps.permits(&cap),
            Err(_) => false,
        }
    }

    /// Route a message: the sender must currently hold the claimed capability,
    /// else the message is refused (fail closed). Returns the accepted message.
    pub fn route(&self, msg: Message) -> Result<Message, BrokerError> {
        if self.ancestor_revoked(&msg.sender) {
            return Err(BrokerError::SenderRevoked);
        }
        if !msg.capability.is_empty() && !self.permits(&msg.sender, &msg.capability) {
            return Err(BrokerError::NotDelegated(msg.capability.clone()));
        }
        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Broker, String, String) {
        let mut b = Broker::new();
        b.register_root(
            "research",
            CapabilitySet::parse_all(["fs.read:/research", "net.connect:api.example.com:443"])
                .unwrap(),
        );
        b.delegate("research", "data", &["fs.read:/research/ds".into()])
            .unwrap();
        (b, "research".into(), "data".into())
    }

    #[test]
    fn delegate_then_revoke() {
        let (mut b, research, data) = setup();
        assert!(b.permits(&data, "fs.read:/research/ds/x"));
        assert!(!b.permits(&data, "net.connect:api.example.com:443"));
        b.revoke(&research);
        assert!(!b.permits(&data, "fs.read:/research/ds/x"));
    }

    #[test]
    fn cannot_delegate_more_than_held() {
        let (mut b, research, _) = setup();
        let e = b
            .delegate(&research, "x", &["fs.read:/etc".into()])
            .unwrap_err();
        assert!(matches!(e, BrokerError::NotPermittedByParent(_)));
    }

    #[test]
    fn message_requires_capability() {
        let (b, _research, data) = setup();
        let ok = Message {
            sender: data.clone(),
            receiver: "analysis".into(),
            intent: "int-1".into(),
            purpose: "share dataset".into(),
            capability: "fs.read:/research/ds/x".into(),
            timestamp: 1,
            provenance: None,
            kind: MsgKind::Request,
            body: "here".into(),
        };
        assert!(b.route(ok).is_ok());
        let bad = Message {
            sender: data,
            receiver: "analysis".into(),
            intent: "int-1".into(),
            purpose: "exfiltrate".into(),
            capability: "net.connect:api.example.com:443".into(),
            timestamp: 1,
            provenance: None,
            kind: MsgKind::Request,
            body: "".into(),
        };
        assert!(matches!(b.route(bad), Err(BrokerError::NotDelegated(_))));
    }
}
