//! OS-managed AI memory (section 27).
//!
//! Memory is owned and access-controlled, not a shared vector blob. Each item
//! has a class, an owner, a retention policy, and provenance. Cross-owner
//! access is denied (threat T-014, cross-agent memory access).
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryClass {
    Working,
    Episodic,
    Semantic,
    Procedural,
    Preference,
    Task,
    Organizational,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: String,
    pub class: MemoryClass,
    pub owner: String,
    pub content: String,
    pub created: u64,
    pub retention_secs: u64, // 0 = keep until deleted
    pub provenance: Option<u64>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MemError {
    Denied,
    NotFound,
    Expired,
}

#[derive(Default)]
pub struct MemoryStore {
    items: HashMap<String, MemoryItem>,
    seq: u64,
}

impl MemoryStore {
    pub fn new() -> Self {
        MemoryStore::default()
    }

    pub fn put(
        &mut self,
        owner: &str,
        class: MemoryClass,
        content: &str,
        now: u64,
        retention_secs: u64,
        provenance: Option<u64>,
    ) -> String {
        self.seq += 1;
        let id = format!(
            "mem-{}",
            &libcrypto::sha256_hex(format!("{owner}:{now}:{}", self.seq).as_bytes())[..16]
        );
        self.items.insert(
            id.clone(),
            MemoryItem {
                id: id.clone(),
                class,
                owner: owner.to_string(),
                content: content.to_string(),
                created: now,
                retention_secs,
                provenance,
            },
        );
        id
    }

    fn live(item: &MemoryItem, now: u64) -> bool {
        item.retention_secs == 0 || now < item.created + item.retention_secs
    }

    /// Access enforces ownership AND retention. Requester must be the owner.
    pub fn get(&self, requester: &str, id: &str, now: u64) -> Result<&MemoryItem, MemError> {
        let item = self.items.get(id).ok_or(MemError::NotFound)?;
        if item.owner != requester {
            return Err(MemError::Denied); // cross-owner access denied
        }
        if !Self::live(item, now) {
            return Err(MemError::Expired);
        }
        Ok(item)
    }

    pub fn delete(&mut self, requester: &str, id: &str) -> Result<(), MemError> {
        let item = self.items.get(id).ok_or(MemError::NotFound)?;
        if item.owner != requester {
            return Err(MemError::Denied);
        }
        self.items.remove(id);
        Ok(())
    }

    /// Export all of an owner's live items (for data portability).
    pub fn export(&self, owner: &str, now: u64) -> Vec<&MemoryItem> {
        self.items
            .values()
            .filter(|i| i.owner == owner && Self::live(i, now))
            .collect()
    }

    /// Garbage-collect expired items; returns count removed.
    pub fn gc(&mut self, now: u64) -> usize {
        let before = self.items.len();
        self.items.retain(|_, i| Self::live(i, now));
        before - self.items.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_can_read_others_cannot() {
        let mut m = MemoryStore::new();
        let id = m.put("agentA", MemoryClass::Task, "secret plan", 100, 0, None);
        assert_eq!(m.get("agentA", &id, 101).unwrap().content, "secret plan");
        assert_eq!(m.get("agentB", &id, 101).unwrap_err(), MemError::Denied);
    }

    #[test]
    fn retention_expiry() {
        let mut m = MemoryStore::new();
        let id = m.put("agentA", MemoryClass::Working, "scratch", 100, 10, None);
        assert!(m.get("agentA", &id, 105).is_ok());
        assert_eq!(m.get("agentA", &id, 200).unwrap_err(), MemError::Expired);
        assert_eq!(m.gc(200), 1);
    }

    #[test]
    fn delete_requires_owner() {
        let mut m = MemoryStore::new();
        let id = m.put("agentA", MemoryClass::Semantic, "fact", 1, 0, None);
        assert_eq!(m.delete("agentB", &id).unwrap_err(), MemError::Denied);
        assert!(m.delete("agentA", &id).is_ok());
    }

    #[test]
    fn export_is_owner_scoped() {
        let mut m = MemoryStore::new();
        m.put("a", MemoryClass::Preference, "dark mode", 1, 0, None);
        m.put("a", MemoryClass::Task, "t", 1, 0, None);
        m.put("b", MemoryClass::Task, "other", 1, 0, None);
        assert_eq!(m.export("a", 2).len(), 2);
        assert_eq!(m.export("b", 2).len(), 1);
    }
}
