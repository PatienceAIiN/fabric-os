//! Human intent as the top-level authority.
//!
//! An [`Intent`] carries the objective, the *explicit* capabilities it
//! authorizes, its data/network scope, a resource budget, a risk level and an
//! approval policy. Authority to act comes from the intent's capability set,
//! never from model output. The lifecycle is a small explicit state machine.
#![forbid(unsafe_code)]

use libagent::ResourceLimits;
use libcapability::CapabilitySet;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalPolicy {
    /// Never require human approval (only safe for LOW-risk intents).
    Never,
    /// Require approval at or above this risk level.
    AtOrAbove(RiskLevel),
    /// Always require approval.
    Always,
}

impl ApprovalPolicy {
    pub fn requires_approval(&self, risk: RiskLevel) -> bool {
        match self {
            ApprovalPolicy::Never => false,
            ApprovalPolicy::Always => true,
            ApprovalPolicy::AtOrAbove(th) => risk >= *th,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentState {
    Created,
    Validated,
    Authorized,
    Planned,
    Executing,
    Paused,
    Completed,
    Cancelled,
    RolledBack,
    Blocked,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Intent {
    pub intent_id: String,
    pub principal: String,
    pub objective: String,
    pub scope: String,
    /// Explicit capability strings this intent authorizes (least privilege).
    pub allowed_capabilities: Vec<String>,
    pub data_scope: Vec<String>,
    pub network_scope: Vec<String>,
    pub resource_budget: ResourceLimits,
    pub time_limit_secs: u64,
    pub risk_level: RiskLevel,
    pub approval_policy: ApprovalPolicy,
    pub expiration: u64,
    pub state: IntentState,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TransitionError(pub String);

impl Intent {
    /// Create an intent. The id is content-addressed over principal+objective
    /// plus the current time and allowed caps (stable, auditable).
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        principal: &str,
        objective: &str,
        scope: &str,
        allowed_capabilities: Vec<String>,
        risk_level: RiskLevel,
        approval_policy: ApprovalPolicy,
        resource_budget: ResourceLimits,
        now: u64,
        time_limit_secs: u64,
    ) -> Intent {
        let mut pre = Vec::new();
        pre.extend_from_slice(principal.as_bytes());
        pre.push(0);
        pre.extend_from_slice(objective.as_bytes());
        pre.push(0);
        pre.extend_from_slice(&now.to_le_bytes());
        for c in &allowed_capabilities {
            pre.push(0);
            pre.extend_from_slice(c.as_bytes());
        }
        let intent_id = format!("int-{}", &libcrypto::sha256_hex(&pre)[..24]);
        Intent {
            intent_id,
            principal: principal.to_string(),
            objective: objective.to_string(),
            scope: scope.to_string(),
            allowed_capabilities,
            data_scope: Vec::new(),
            network_scope: Vec::new(),
            resource_budget,
            time_limit_secs,
            risk_level,
            approval_policy,
            expiration: now.saturating_add(time_limit_secs),
            state: IntentState::Created,
        }
    }

    /// The capability set granted by this intent. Fails if any cap string is
    /// malformed (fail closed: an unparseable grant is rejected).
    pub fn capability_set(&self) -> Result<CapabilitySet, String> {
        CapabilitySet::parse_all(&self.allowed_capabilities).map_err(|e| e.to_string())
    }

    pub fn is_expired(&self, at: u64) -> bool {
        at >= self.expiration
    }

    fn expect(&self, want: &[IntentState]) -> Result<(), TransitionError> {
        if want.contains(&self.state) {
            Ok(())
        } else {
            Err(TransitionError(format!(
                "invalid transition from {:?}",
                self.state
            )))
        }
    }

    pub fn validate(&mut self) -> Result<(), TransitionError> {
        self.expect(&[IntentState::Created])?;
        // A validated intent must have a parseable, non-empty capability set.
        if self.capability_set().map(|c| c.is_empty()).unwrap_or(true) {
            self.state = IntentState::Blocked;
            return Err(TransitionError("no valid capabilities".into()));
        }
        self.state = IntentState::Validated;
        Ok(())
    }
    pub fn authorize(&mut self) -> Result<(), TransitionError> {
        self.expect(&[IntentState::Validated])?;
        self.state = IntentState::Authorized;
        Ok(())
    }
    pub fn plan(&mut self) -> Result<(), TransitionError> {
        self.expect(&[IntentState::Authorized])?;
        self.state = IntentState::Planned;
        Ok(())
    }
    pub fn begin_execution(&mut self) -> Result<(), TransitionError> {
        self.expect(&[IntentState::Authorized, IntentState::Planned])?;
        self.state = IntentState::Executing;
        Ok(())
    }
    pub fn pause(&mut self) -> Result<(), TransitionError> {
        self.expect(&[IntentState::Executing])?;
        self.state = IntentState::Paused;
        Ok(())
    }
    pub fn resume(&mut self) -> Result<(), TransitionError> {
        self.expect(&[IntentState::Paused])?;
        self.state = IntentState::Executing;
        Ok(())
    }
    pub fn complete(&mut self) -> Result<(), TransitionError> {
        self.expect(&[IntentState::Executing])?;
        self.state = IntentState::Completed;
        Ok(())
    }
    pub fn cancel(&mut self) -> Result<(), TransitionError> {
        self.expect(&[
            IntentState::Created,
            IntentState::Validated,
            IntentState::Authorized,
            IntentState::Planned,
            IntentState::Executing,
            IntentState::Paused,
        ])?;
        self.state = IntentState::Cancelled;
        Ok(())
    }
    pub fn rolled_back(&mut self) -> Result<(), TransitionError> {
        self.expect(&[IntentState::Executing, IntentState::Completed])?;
        self.state = IntentState::RolledBack;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(caps: Vec<String>) -> Intent {
        Intent::create(
            "user:harsh",
            "Organize files in /home/harsh/Downloads",
            "Downloads only",
            caps,
            RiskLevel::Medium,
            ApprovalPolicy::AtOrAbove(RiskLevel::High),
            ResourceLimits::default(),
            1000,
            600,
        )
    }

    #[test]
    fn lifecycle_happy_path() {
        let mut i = intent(vec!["fs.read:/home/harsh/Downloads".into()]);
        assert_eq!(i.state, IntentState::Created);
        i.validate().unwrap();
        i.authorize().unwrap();
        i.begin_execution().unwrap();
        i.complete().unwrap();
        assert_eq!(i.state, IntentState::Completed);
    }

    #[test]
    fn cannot_authorize_before_validate() {
        let mut i = intent(vec!["fs.read:/x".into()]);
        assert!(i.authorize().is_err());
    }

    #[test]
    fn empty_caps_block_validation() {
        let mut i = intent(vec![]);
        assert!(i.validate().is_err());
        assert_eq!(i.state, IntentState::Blocked);
    }

    #[test]
    fn capability_set_enforces_scope() {
        let i = intent(vec!["fs.read:/home/harsh/Downloads".into()]);
        let set = i.capability_set().unwrap();
        assert!(set.permits_str("fs.read:/home/harsh/Downloads/a.txt"));
        assert!(!set.permits_str("fs.read:/etc/passwd"));
        assert!(!set.permits_str("net.connect:evil.com:443"));
    }

    #[test]
    fn approval_policy_thresholds() {
        let p = ApprovalPolicy::AtOrAbove(RiskLevel::High);
        assert!(!p.requires_approval(RiskLevel::Medium));
        assert!(p.requires_approval(RiskLevel::High));
        assert!(p.requires_approval(RiskLevel::Critical));
        assert!(!ApprovalPolicy::Never.requires_approval(RiskLevel::Critical));
        assert!(ApprovalPolicy::Always.requires_approval(RiskLevel::Low));
    }
}
