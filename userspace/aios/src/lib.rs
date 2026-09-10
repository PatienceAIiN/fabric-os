//! ai-native-os control library: the killer demos and settings helpers, built
//! on the deterministic capability/intent/governor/provenance/transaction core.
#![forbid(unsafe_code)]

pub mod shell;

use libagent::{Agent, ResourceLimits, TrustLevel};
use libcapability::CapabilitySet;
use libcredentials::{CredentialStore, Secret};
use libgovernor::{evaluate, Decision};
use libintent::{ApprovalPolicy, Intent, RiskLevel};
use libprovenance::ProvenanceLog;
use libprovider::{AIProvider, ClaudeProvider};
use libtransaction::{Op, Transaction};
use std::path::Path;

pub const CLAUDE_KEY_NAME: &str = "claude-api-key";

#[derive(Debug)]
pub struct Demo1Result {
    pub files_seen: usize,
    pub files_moved: usize,
    pub files_deleted: usize,
    pub network_requests: usize,
    pub injection_blocked: bool,
    pub block_reason: String,
    pub rolled_back_ok: bool,
    pub provenance_ok: bool,
    pub provenance_events: usize,
}

/// Demo 1 — Intent + Security + Rollback. Organizes a sandbox "Downloads",
/// blocks a prompt-injected exfiltration attempt via the governor, then rolls
/// the transaction back. Returns measured results.
pub fn run_demo1(sandbox: &Path) -> std::io::Result<Demo1Result> {
    use std::fs;
    let root = sandbox.join("Downloads");
    let backup = sandbox.join(".aios-trash");
    let _ = fs::remove_dir_all(sandbox);
    fs::create_dir_all(root.join("images"))?;
    // seed files
    for n in ["a.jpg", "b.jpg", "notes.txt", "junk.tmp"] {
        fs::write(root.join(n), b"x")?;
    }
    let files_seen = fs::read_dir(&root)?.count();

    let now = 1000u64;
    let agent = Agent::create(
        "user:harsh",
        "organize-downloads",
        "local/reasoning",
        TrustLevel::Limited,
        ResourceLimits::default(),
        10_000,
    );
    // Intent authorizes ONLY read+write inside Downloads. No network.
    let root_s = root.to_string_lossy().to_string();
    let mut intent = Intent::create(
        "user:harsh",
        "Organize my Downloads folder",
        "Downloads only",
        vec![format!("fs.read:{root_s}"), format!("fs.write:{root_s}")],
        RiskLevel::Medium,
        ApprovalPolicy::AtOrAbove(RiskLevel::High),
        ResourceLimits::default(),
        now,
        10_000,
    );
    intent.validate().unwrap();
    intent.authorize().unwrap();
    intent.begin_execution().unwrap();

    let mut prov = ProvenanceLog::new();

    // Legit plan: move the two jpgs into images/, delete junk.tmp.
    let mut tx = Transaction::begin(&root, &backup);
    let mut files_moved = 0;
    let mut files_deleted = 0;
    for img in ["a.jpg", "b.jpg"] {
        let req = format!("fs.write:{}", root.join("images").join(img).display());
        if evaluate(&agent, &intent, &req, None, now, &mut prov) == Decision::Allow {
            tx.add(Op::Move {
                from: img.into(),
                to: format!("images/{img}"),
            });
            files_moved += 1;
        }
    }
    let del_req = format!("fs.write:{}", root.join("junk.tmp").display());
    if evaluate(&agent, &intent, &del_req, None, now, &mut prov) == Decision::Allow {
        tx.add(Op::Delete {
            path: "junk.tmp".into(),
        });
        files_deleted += 1;
    }
    tx.commit().expect("transaction commits");

    // Prompt-injection: a malicious document tells the agent to upload files.
    // The agent proposes an out-of-intent network capability. Governor blocks.
    let inject_req = "net.connect:evil.com:443";
    let decision = evaluate(&agent, &intent, inject_req, None, now, &mut prov);
    let (injection_blocked, block_reason) = match decision {
        Decision::Deny { reason } => (true, reason),
        _ => (false, String::new()),
    };
    let network_requests = 0; // nothing was ever sent

    // Rollback the transaction: restore moved + deleted files.
    let errs = tx.rollback();
    let rolled_back_ok = errs.is_empty()
        && root.join("a.jpg").exists()
        && root.join("junk.tmp").exists()
        && !root.join("images/a.jpg").exists();
    intent.rolled_back().ok();

    let provenance_ok = prov.verify().is_ok();
    let provenance_events = prov.len();
    let _ = fs::remove_dir_all(sandbox);

    Ok(Demo1Result {
        files_seen,
        files_moved,
        files_deleted,
        network_requests,
        injection_blocked,
        block_reason,
        rolled_back_ok,
        provenance_ok,
        provenance_events,
    })
}

#[derive(Debug)]
pub struct Demo3Result {
    pub data_agent_read_before: bool,
    pub data_agent_net_denied: bool,
    pub data_agent_read_after_revoke: bool,
}

/// Demo 3 — Multi-agent OS. ResearchAgent delegates a read capability to
/// DataAgent (never the network one). Revoking ResearchAgent invalidates the
/// delegated capability.
pub fn run_demo3() -> Demo3Result {
    use libbroker::Broker;
    let research = Agent::create(
        "user:harsh",
        "research",
        "local/reasoning",
        TrustLevel::Standard,
        ResourceLimits::default(),
        10_000,
    );
    let data = Agent::create(
        "agent:research",
        "data-fetch",
        "local/reasoning",
        TrustLevel::Limited,
        ResourceLimits::default(),
        10_000,
    );

    let mut broker = Broker::new();
    let research_caps =
        CapabilitySet::parse_all(["fs.read:/research", "net.connect:api.example.com:443"]).unwrap();
    broker.register_root(research.id(), research_caps);
    // Delegate ONLY a narrowed read capability to DataAgent.
    broker
        .delegate(
            research.id(),
            data.id(),
            &["fs.read:/research/dataset".to_string()],
        )
        .expect("delegation of a subset succeeds");

    let data_agent_read_before = broker.permits(data.id(), "fs.read:/research/dataset/x.csv");
    let data_agent_net_denied = !broker.permits(data.id(), "net.connect:api.example.com:443");

    // Revoke the parent; the delegated capability must stop working.
    broker.revoke(research.id());
    let data_agent_read_after_revoke = broker.permits(data.id(), "fs.read:/research/dataset/x.csv");

    Demo3Result {
        data_agent_read_before,
        data_agent_net_denied,
        data_agent_read_after_revoke,
    }
}

/// Settings helper (M8/M9): store the Claude key in the secure store.
pub fn set_claude_key(store: &mut dyn CredentialStore, key: &str) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("empty key".into());
    }
    store
        .store(CLAUDE_KEY_NAME, &Secret::new(key.trim()))
        .map_err(|e| e.to_string())
}

/// UI status string for the AI & Models settings pane. Never reveals the key.
pub fn claude_status(store: &dyn CredentialStore) -> String {
    let p = ClaudeProvider::new(store, CLAUDE_KEY_NAME, libprovider::DEFAULT_CLAUDE_MODEL);
    if p.health_check() {
        "Claude: key configured (••••••••••••••••••••). Test connection to verify.".into()
    } else {
        "Claude: ! Authentication required (no key configured)".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libcredentials::MemoryStore;

    #[test]
    fn settings_status_masks_key() {
        let mut store = MemoryStore::default();
        assert!(claude_status(&store).contains("Authentication required"));
        set_claude_key(&mut store, "sk-ant-REALKEY-123456").unwrap();
        let s = claude_status(&store);
        assert!(s.contains("configured"));
        assert!(!s.contains("REALKEY"));
    }
}
