//! The AI Governor — deterministic authorization.
//!
//! An agent *proposes* an action (a requested capability). The governor
//! verifies it against the intent's capability set, the agent's validity, the
//! intent's lifecycle state, a deterministic risk classification, and the
//! approval policy. An LLM may *suggest* a risk score, but the deterministic
//! rules are authoritative. Every decision emits a provenance event. Fail
//! closed: anything not explicitly allowed is denied.
#![forbid(unsafe_code)]

use libagent::Agent;
use libcapability::{Capability, Scope};
use libintent::{Intent, IntentState, RiskLevel};
use libprovenance::{PolicyDecision, ProvenanceLog};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    Allow,
    Deny { reason: String },
    NeedApproval { risk: RiskLevel },
}

/// Deterministic risk classification for a requested capability.
/// A model may propose a risk hint, but this function is authoritative and is
/// never lowered by model input (hint can only raise the floor).
pub fn classify_risk(cap: &Capability, model_hint: Option<RiskLevel>) -> RiskLevel {
    let base = match (cap.resource.as_str(), &cap.scope) {
        // credential / security operations are always critical
        ("cred", _) | ("security", _) => RiskLevel::Critical,
        // external network egress is high
        ("net", _) => RiskLevel::High,
        // writing/deleting user data is medium (delete handled by caller scale)
        ("fs", Scope::Path(_)) if cap.action == libcapability::Action::Write => RiskLevel::Medium,
        ("fs", Scope::Path(_)) if cap.action == libcapability::Action::Read => RiskLevel::Low,
        // invoking cloud model is high; local is low (encoded in scope id)
        ("model", Scope::Id(id)) if !id.starts_with("local/") => RiskLevel::High,
        ("model", _) => RiskLevel::Low,
        ("gpu", _) | ("camera", _) | ("microphone", _) => RiskLevel::Medium,
        _ => RiskLevel::Medium,
    };
    match model_hint {
        Some(h) if h > base => h, // hint can only escalate, never de-escalate
        _ => base,
    }
}

fn decision_to_prov(d: &Decision) -> PolicyDecision {
    match d {
        Decision::Allow => PolicyDecision::Allowed,
        Decision::Deny { .. } => PolicyDecision::Blocked,
        Decision::NeedApproval { .. } => PolicyDecision::Denied, // pending until approved
    }
}

/// Like [`evaluate`] but reuses a prebuilt [`CapabilitySet`] instead of parsing
/// the intent's capability strings on every call. For hot paths that issue many
/// actions under one intent, build the set once and call this (M26 optimization).
#[allow(clippy::too_many_arguments)]
pub fn evaluate_with_caps(
    agent: &Agent,
    intent: &Intent,
    set: &libcapability::CapabilitySet,
    requested: &str,
    model_hint: Option<RiskLevel>,
    now: u64,
    prov: &mut ProvenanceLog,
) -> Decision {
    let record = |prov: &mut ProvenanceLog, d: &Decision, result: &str| {
        prov.append(
            now,
            &intent.principal,
            agent.id(),
            &intent.intent_id,
            &agent.identity.model_identity,
            "governor",
            "evaluate",
            requested,
            result,
            decision_to_prov(d),
            None,
        );
    };
    if agent.is_expired(now) {
        let d = Decision::Deny {
            reason: "agent identity expired".into(),
        };
        record(prov, &d, "expired");
        return d;
    }
    if !matches!(
        intent.state,
        IntentState::Authorized | IntentState::Executing | IntentState::Planned
    ) {
        let d = Decision::Deny {
            reason: format!("intent not authorized (state {:?})", intent.state),
        };
        record(prov, &d, "unauthorized-state");
        return d;
    }
    if intent.is_expired(now) {
        let d = Decision::Deny {
            reason: "intent expired".into(),
        };
        record(prov, &d, "intent-expired");
        return d;
    }
    let cap = match Capability::parse(requested) {
        Ok(c) => c,
        Err(e) => {
            let d = Decision::Deny {
                reason: format!("unparseable request: {e}"),
            };
            record(prov, &d, "unparseable");
            return d;
        }
    };
    if !set.permits(&cap) {
        let d = Decision::Deny {
            reason: "requested capability not authorized by intent".into(),
        };
        record(prov, &d, "capability-not-authorized");
        return d;
    }
    let risk = classify_risk(&cap, model_hint);
    if intent.approval_policy.requires_approval(risk) {
        let d = Decision::NeedApproval { risk };
        record(prov, &d, "approval-required");
        return d;
    }
    let d = Decision::Allow;
    record(prov, &d, "allow");
    d
}

/// Evaluate a proposed action. Records a provenance event and returns the
/// decision. `now` is the current unix time (for expiry checks).
#[allow(clippy::too_many_arguments)]
pub fn evaluate(
    agent: &Agent,
    intent: &Intent,
    requested: &str,
    model_hint: Option<RiskLevel>,
    now: u64,
    prov: &mut ProvenanceLog,
) -> Decision {
    let record = |prov: &mut ProvenanceLog, d: &Decision, result: &str| {
        prov.append(
            now,
            &intent.principal,
            agent.id(),
            &intent.intent_id,
            &agent.identity.model_identity,
            "governor",
            "evaluate",
            requested,
            result,
            decision_to_prov(d),
            None,
        );
    };

    // 1. Agent validity.
    if agent.is_expired(now) {
        let d = Decision::Deny {
            reason: "agent identity expired".into(),
        };
        record(prov, &d, "expired");
        return d;
    }
    // 2. Intent must be authorized/executing.
    if !matches!(
        intent.state,
        IntentState::Authorized | IntentState::Executing | IntentState::Planned
    ) {
        let d = Decision::Deny {
            reason: format!("intent not authorized (state {:?})", intent.state),
        };
        record(prov, &d, "unauthorized-state");
        return d;
    }
    // 3. Intent expiry.
    if intent.is_expired(now) {
        let d = Decision::Deny {
            reason: "intent expired".into(),
        };
        record(prov, &d, "intent-expired");
        return d;
    }
    // 4. Parse the requested capability (unparseable => deny).
    let cap = match Capability::parse(requested) {
        Ok(c) => c,
        Err(e) => {
            let d = Decision::Deny {
                reason: format!("unparseable request: {e}"),
            };
            record(prov, &d, "unparseable");
            return d;
        }
    };
    // 5. THE CORE CHECK: capability must be permitted by the intent's set.
    //    This is where a prompt-injected request for unauthorized capability
    //    is blocked, regardless of what any document/model said.
    let set = match intent.capability_set() {
        Ok(s) => s,
        Err(e) => {
            let d = Decision::Deny {
                reason: format!("intent caps invalid: {e}"),
            };
            record(prov, &d, "intent-caps-invalid");
            return d;
        }
    };
    if !set.permits(&cap) {
        let d = Decision::Deny {
            reason: "requested capability not authorized by intent".into(),
        };
        record(prov, &d, "capability-not-authorized");
        return d;
    }
    // 6. Risk classification + approval policy.
    let risk = classify_risk(&cap, model_hint);
    if intent.approval_policy.requires_approval(risk) {
        let d = Decision::NeedApproval { risk };
        record(prov, &d, "approval-required");
        return d;
    }
    // 7. Allowed.
    let d = Decision::Allow;
    record(prov, &d, "allow");
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use libagent::{ResourceLimits, TrustLevel};
    use libintent::{ApprovalPolicy, Intent};

    fn agent() -> Agent {
        Agent::create(
            "user:harsh",
            "organize",
            "local/reasoning",
            TrustLevel::Limited,
            ResourceLimits::default(),
            10_000,
        )
    }

    fn authorized_intent(caps: Vec<String>, policy: ApprovalPolicy) -> Intent {
        let mut i = Intent::create(
            "user:harsh",
            "Organize Downloads",
            "downloads",
            caps,
            RiskLevel::Medium,
            policy,
            ResourceLimits::default(),
            1000,
            10_000,
        );
        i.validate().unwrap();
        i.authorize().unwrap();
        i
    }

    #[test]
    fn allows_in_scope_read() {
        let mut p = ProvenanceLog::new();
        let a = agent();
        let i = authorized_intent(
            vec!["fs.read:/home/harsh/Downloads".into()],
            ApprovalPolicy::AtOrAbove(RiskLevel::High),
        );
        let d = evaluate(
            &a,
            &i,
            "fs.read:/home/harsh/Downloads/a.txt",
            None,
            1001,
            &mut p,
        );
        assert_eq!(d, Decision::Allow);
        assert!(p.verify().is_ok());
    }

    #[test]
    fn blocks_capability_outside_intent() {
        // Demo 1 core: a prompt-injected "upload everything" request.
        let mut p = ProvenanceLog::new();
        let a = agent();
        let i = authorized_intent(
            vec!["fs.read:/home/harsh/Downloads".into()],
            ApprovalPolicy::AtOrAbove(RiskLevel::High),
        );
        let d = evaluate(&a, &i, "net.connect:evil.com:443", None, 1001, &mut p);
        match d {
            Decision::Deny { reason } => assert!(reason.contains("not authorized by intent")),
            _ => panic!("must be denied"),
        }
        // provenance recorded the block
        assert!(p.records().last().unwrap().content.result == "capability-not-authorized");
    }

    #[test]
    fn high_risk_needs_approval() {
        let mut p = ProvenanceLog::new();
        let a = agent();
        // intent DOES authorize the net capability, but policy needs approval.
        let i = authorized_intent(
            vec!["net.connect:api.example.com:443".into()],
            ApprovalPolicy::AtOrAbove(RiskLevel::High),
        );
        let d = evaluate(
            &a,
            &i,
            "net.connect:api.example.com:443",
            None,
            1001,
            &mut p,
        );
        assert_eq!(
            d,
            Decision::NeedApproval {
                risk: RiskLevel::High
            }
        );
    }

    #[test]
    fn expired_agent_denied() {
        let mut p = ProvenanceLog::new();
        let a = agent();
        let i = authorized_intent(vec!["fs.read:/x".into()], ApprovalPolicy::Never);
        let exp = a.identity.expiration;
        let d = evaluate(&a, &i, "fs.read:/x/y", None, exp + 1, &mut p);
        assert!(matches!(d, Decision::Deny { .. }));
    }

    #[test]
    fn model_hint_can_only_escalate_risk() {
        let cap = Capability::parse("fs.read:/a").unwrap();
        assert_eq!(classify_risk(&cap, None), RiskLevel::Low);
        assert_eq!(
            classify_risk(&cap, Some(RiskLevel::Critical)),
            RiskLevel::Critical
        );
        // hint cannot lower a high-risk net op
        let net = Capability::parse("net.connect:x.com:443").unwrap();
        assert_eq!(classify_risk(&net, Some(RiskLevel::Low)), RiskLevel::High);
    }
}
