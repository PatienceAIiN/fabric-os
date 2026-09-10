//! End-to-end killer demonstrations as automated tests (spec sections 76, 78).
use aios::{run_demo1, run_demo3};

#[test]
fn demo1_intent_security_rollback() {
    let sandbox = std::env::temp_dir().join(format!("aios-demo1-test-{}", std::process::id()));
    let r = run_demo1(&sandbox).expect("demo1 runs");

    // It did real work inside the intent's scope.
    assert!(r.files_seen >= 4);
    assert_eq!(r.files_moved, 2);
    assert_eq!(r.files_deleted, 1);

    // The prompt-injected exfiltration attempt was blocked by policy, not by
    // filtering, and nothing was ever sent.
    assert!(r.injection_blocked, "injection must be blocked");
    assert!(r.block_reason.contains("not authorized by intent"));
    assert_eq!(r.network_requests, 0);

    // Rollback restored the filesystem, and provenance is intact.
    assert!(r.rolled_back_ok, "rollback must restore files");
    assert!(r.provenance_ok, "provenance chain must verify");
    assert!(r.provenance_events >= 4);
}

#[test]
fn demo3_multi_agent_delegation_and_revocation() {
    let r = run_demo3();
    // Delegated read works; the parent's network capability was NOT delegated.
    assert!(r.data_agent_read_before, "delegated read should work");
    assert!(
        r.data_agent_net_denied,
        "non-delegated network must be denied"
    );
    // Revoking the parent invalidates the delegated capability.
    assert!(
        !r.data_agent_read_after_revoke,
        "revoked parent must stop delegated capability"
    );
}
