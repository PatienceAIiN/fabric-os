//! Least-privilege capability model for ai-native-os.
//!
//! Authority in this OS never comes from model output or document content; it
//! comes from capabilities held in an intent's [`CapabilitySet`]. A model may
//! *request* an action, but it is granted only if an explicitly held
//! capability permits it. Default is deny (fail closed).
//!
//! Capabilities are parsed from stable strings, e.g.:
//!   `fs.read:/documents/project-a`
//!   `fs.write:/documents/project-a/output`
//!   `net.connect:api.example.com:443`
//!   `model.invoke:local/reasoning`
//!   `gpu.use`
//!
//! Matching is least-privilege: a request is permitted only if some held
//! capability *covers* it. Filesystem scopes confine to a path prefix using
//! lexical normalization that rejects `..` escapes (defense against
//! model-supplied traversal paths); no filesystem access or symlink follow is
//! performed here (that is the enforcing tool's job, TOCTOU-safe).
//!
//! This crate forbids `unsafe` and has no dependencies.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fmt;

/// A resource action. Kept small and explicit; extend deliberately.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    Read,
    Write,
    Connect,
    Invoke,
    Use,
}

impl Action {
    fn parse(kind: &str, verb: &str) -> Option<Action> {
        match (kind, verb) {
            ("fs", "read") => Some(Action::Read),
            ("fs", "write") => Some(Action::Write),
            ("net", "connect") => Some(Action::Connect),
            ("model", "invoke") => Some(Action::Invoke),
            ("gpu", "use") | ("camera", "read") | ("microphone", "read") => {
                Some(if verb == "use" {
                    Action::Use
                } else {
                    Action::Read
                })
            }
            _ => None,
        }
    }
}

/// The scope a capability applies to. Fail-closed: unknown scopes never match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Filesystem path prefix (already lexically normalized, absolute).
    Path(String),
    /// Network host + port. Host may be a leading-`*` wildcard, e.g. `*.ex.com`.
    HostPort { host: String, port: u16 },
    /// Opaque identifier, e.g. a model id `local/reasoning`.
    Id(String),
    /// No sub-scope (e.g. `gpu.use`).
    None,
}

/// A single held or requested capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capability {
    /// Resource class: `fs`, `net`, `model`, `gpu`, `camera`, `microphone`.
    pub resource: String,
    pub action: Action,
    pub scope: Scope,
}

/// Errors from parsing a capability string.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    MissingAction(String),
    UnknownActionForResource { resource: String, verb: String },
    MissingScope(String),
    BadPath(String),
    BadHostPort(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Empty => write!(f, "empty capability"),
            ParseError::MissingAction(s) => write!(f, "missing '.action' in {s:?}"),
            ParseError::UnknownActionForResource { resource, verb } => {
                write!(f, "unknown action {verb:?} for resource {resource:?}")
            }
            ParseError::MissingScope(s) => write!(f, "missing ':scope' in {s:?}"),
            ParseError::BadPath(p) => write!(f, "path must be absolute, non-empty: {p:?}"),
            ParseError::BadHostPort(s) => write!(f, "bad host:port {s:?}"),
        }
    }
}
impl std::error::Error for ParseError {}

/// Lexically normalize an absolute path, rejecting any escape above root.
/// Returns `None` if the path is not absolute or escapes root via `..`.
/// Does NOT touch the filesystem (no symlink resolution) — callers enforcing
/// the capability must open TOCTOU-safely (e.g. O_NOFOLLOW / *at()).
pub fn normalize_abs_path(p: &str) -> Option<String> {
    if !p.starts_with('/') {
        return None;
    }
    let mut out: Vec<&str> = Vec::new();
    for comp in p.split('/') {
        match comp {
            "" | "." => {}
            ".." => {
                // escaping above root is not allowed
                out.pop()?;
            }
            other => out.push(other),
        }
    }
    Some(format!("/{}", out.join("/")))
}

fn parse_host_port(s: &str) -> Option<Scope> {
    let (host, port) = s.rsplit_once(':')?;
    if host.is_empty() {
        return None;
    }
    let port: u16 = port.parse().ok()?;
    Some(Scope::HostPort {
        host: host.to_string(),
        port,
    })
}

impl Capability {
    /// Parse a capability string such as `fs.read:/documents/project-a`.
    pub fn parse(s: &str) -> Result<Capability, ParseError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(ParseError::Empty);
        }
        // Split resource.action[:scope]
        let (head, scope_str) = match s.split_once(':') {
            Some((h, sc)) => (h, Some(sc)),
            None => (s, None),
        };
        let (resource, verb) = head
            .split_once('.')
            .ok_or_else(|| ParseError::MissingAction(s.to_string()))?;
        let action =
            Action::parse(resource, verb).ok_or_else(|| ParseError::UnknownActionForResource {
                resource: resource.to_string(),
                verb: verb.to_string(),
            })?;

        let scope = match resource {
            "fs" => {
                let raw = scope_str.ok_or_else(|| ParseError::MissingScope(s.to_string()))?;
                let norm =
                    normalize_abs_path(raw).ok_or_else(|| ParseError::BadPath(raw.to_string()))?;
                Scope::Path(norm)
            }
            "net" => {
                let raw = scope_str.ok_or_else(|| ParseError::MissingScope(s.to_string()))?;
                parse_host_port(raw).ok_or_else(|| ParseError::BadHostPort(raw.to_string()))?
            }
            "model" => {
                let raw = scope_str.ok_or_else(|| ParseError::MissingScope(s.to_string()))?;
                Scope::Id(raw.to_string())
            }
            // gpu/camera/microphone take no scope
            _ => Scope::None,
        };

        Ok(Capability {
            resource: resource.to_string(),
            action,
            scope,
        })
    }

    /// Does this held capability COVER (permit) the `requested` action?
    /// Least-privilege: resource and action must match exactly, and the held
    /// scope must contain the requested scope.
    pub fn covers(&self, requested: &Capability) -> bool {
        if self.resource != requested.resource || self.action != requested.action {
            return false;
        }
        match (&self.scope, &requested.scope) {
            (Scope::Path(held), Scope::Path(req)) => path_contains(held, req),
            (Scope::HostPort { host: hh, port: hp }, Scope::HostPort { host: rh, port: rp }) => {
                hp == rp && host_matches(hh, rh)
            }
            (Scope::Id(h), Scope::Id(r)) => h == r,
            (Scope::None, Scope::None) => true,
            _ => false, // scope-kind mismatch => deny (fail closed)
        }
    }
}

/// True if `held` path-prefix contains `req` (both normalized absolute).
/// `/a` contains `/a` and `/a/b`, but not `/ab`.
fn path_contains(held: &str, req: &str) -> bool {
    if held == req {
        return true;
    }
    if held == "/" {
        return req.starts_with('/');
    }
    req.starts_with(held) && req.as_bytes().get(held.len()) == Some(&b'/')
}

/// Host wildcard match: `*.example.com` matches `api.example.com` but not
/// `example.com` itself and not `evil-example.com`.
fn host_matches(held: &str, req: &str) -> bool {
    if let Some(suffix) = held.strip_prefix("*.") {
        req.len() > suffix.len()
            && req.ends_with(suffix)
            && req.as_bytes()[req.len() - suffix.len() - 1] == b'.'
    } else {
        held == req
    }
}

/// An immutable set of granted capabilities. Deny-by-default.
#[derive(Clone, Debug, Default)]
pub struct CapabilitySet {
    caps: Vec<Capability>,
}

impl CapabilitySet {
    pub fn new() -> Self {
        Self { caps: Vec::new() }
    }

    /// Build from capability strings (e.g. an intent's `allowed` list).
    pub fn parse_all<I, S>(items: I) -> Result<Self, ParseError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut set = CapabilitySet::new();
        for it in items {
            set.caps.push(Capability::parse(it.as_ref())?);
        }
        Ok(set)
    }

    pub fn grant(&mut self, cap: Capability) {
        self.caps.push(cap);
    }

    pub fn len(&self) -> usize {
        self.caps.len()
    }
    pub fn is_empty(&self) -> bool {
        self.caps.is_empty()
    }

    /// Iterate over held capabilities (read-only) for sandbox/audit mapping.
    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.caps.iter()
    }

    /// Core check: is `request` permitted by any held capability?
    /// This is the deterministic authorization gate. Fail closed.
    pub fn permits(&self, request: &Capability) -> bool {
        self.caps.iter().any(|held| held.covers(request))
    }

    /// Convenience: parse a requested capability string and check it.
    pub fn permits_str(&self, request: &str) -> bool {
        match Capability::parse(request) {
            Ok(req) => self.permits(&req),
            Err(_) => false, // unparseable request => deny
        }
    }

    /// Delegate a SUBSET to a child. Delegation never widens authority:
    /// each requested capability must itself be permitted by this set.
    /// Returns Err listing the strings that were not permitted.
    pub fn delegate<I, S>(&self, requested: I) -> Result<CapabilitySet, Vec<String>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut out = CapabilitySet::new();
        let mut denied = Vec::new();
        for item in requested {
            let s = item.as_ref();
            match Capability::parse(s) {
                Ok(cap) if self.permits(&cap) => out.grant(cap),
                _ => denied.push(s.to_string()),
            }
        }
        if denied.is_empty() {
            Ok(out)
        } else {
            Err(denied)
        }
    }

    /// Revoke every capability whose resource matches `resource`. Returns count
    /// removed. Revocation is total for that resource (auditable, simple).
    pub fn revoke_resource(&mut self, resource: &str) -> usize {
        let before = self.caps.len();
        self.caps.retain(|c| c.resource != resource);
        before - self.caps.len()
    }

    /// Stable, deduplicated string view for provenance/audit (no secrets here).
    pub fn audit_strings(&self) -> BTreeSet<String> {
        self.caps.iter().map(cap_to_string).collect()
    }
}

fn cap_to_string(c: &Capability) -> String {
    let verb = match (c.resource.as_str(), c.action) {
        (_, Action::Read) => "read",
        (_, Action::Write) => "write",
        (_, Action::Connect) => "connect",
        (_, Action::Invoke) => "invoke",
        (_, Action::Use) => "use",
    };
    match &c.scope {
        Scope::Path(p) => format!("{}.{}:{}", c.resource, verb, p),
        Scope::HostPort { host, port } => format!("{}.{}:{}:{}", c.resource, verb, host, port),
        Scope::Id(id) => format!("{}.{}:{}", c.resource, verb, id),
        Scope::None => format!("{}.{}", c.resource, verb),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fs_read() {
        let c = Capability::parse("fs.read:/documents/project-a").unwrap();
        assert_eq!(c.resource, "fs");
        assert_eq!(c.action, Action::Read);
        assert_eq!(c.scope, Scope::Path("/documents/project-a".into()));
    }

    #[test]
    fn normalizes_and_confines_paths() {
        assert_eq!(normalize_abs_path("/a/./b/../c").as_deref(), Some("/a/c"));
        assert_eq!(normalize_abs_path("/a/b/"), Some("/a/b".to_string()));
        assert_eq!(normalize_abs_path("relative/path"), None);
        // escape above root is rejected
        assert_eq!(normalize_abs_path("/../etc/passwd"), None);
    }

    #[test]
    fn path_prefix_is_least_privilege() {
        let set = CapabilitySet::parse_all(["fs.read:/documents/project-a"]).unwrap();
        assert!(set.permits_str("fs.read:/documents/project-a"));
        assert!(set.permits_str("fs.read:/documents/project-a/notes.txt"));
        // sibling with shared prefix bytes must NOT match
        assert!(!set.permits_str("fs.read:/documents/project-ab"));
        // parent must NOT match
        assert!(!set.permits_str("fs.read:/documents"));
        // different action must NOT match
        assert!(!set.permits_str("fs.write:/documents/project-a/x"));
    }

    #[test]
    fn traversal_request_is_denied() {
        let set = CapabilitySet::parse_all(["fs.read:/documents/project-a"]).unwrap();
        // A model-supplied traversal path normalizes to /etc/passwd and is denied.
        assert!(!set.permits_str("fs.read:/documents/project-a/../../etc/passwd"));
    }

    #[test]
    fn net_host_and_port_exact_and_wildcard() {
        let set = CapabilitySet::parse_all([
            "net.connect:api.example.com:443",
            "net.connect:*.svc.internal:8080",
        ])
        .unwrap();
        assert!(set.permits_str("net.connect:api.example.com:443"));
        assert!(!set.permits_str("net.connect:api.example.com:80")); // wrong port
        assert!(!set.permits_str("net.connect:evil.com:443")); // wrong host
        assert!(set.permits_str("net.connect:a.svc.internal:8080")); // wildcard
        assert!(!set.permits_str("net.connect:svc.internal:8080")); // bare apex not covered
        assert!(!set.permits_str("net.connect:evilsvc.internal:8080")); // not a dot boundary
    }

    #[test]
    fn empty_set_denies_everything() {
        let set = CapabilitySet::new();
        assert!(!set.permits_str("fs.read:/anything"));
        assert!(!set.permits_str("gpu.use"));
    }

    #[test]
    fn unparseable_request_denied() {
        let set = CapabilitySet::parse_all(["gpu.use"]).unwrap();
        assert!(set.permits_str("gpu.use"));
        assert!(!set.permits_str("this is not a capability"));
        assert!(!set.permits_str("fs.read")); // missing scope
    }

    #[test]
    fn delegation_never_widens() {
        let parent =
            CapabilitySet::parse_all(["fs.read:/data", "net.connect:api.example.com:443"]).unwrap();
        // subset ok
        let child = parent.delegate(["fs.read:/data/sub"]).unwrap();
        assert!(child.permits_str("fs.read:/data/sub"));
        assert!(!child.permits_str("net.connect:api.example.com:443")); // not delegated
                                                                        // requesting beyond parent authority fails
        let denied = parent.delegate(["fs.read:/etc"]).unwrap_err();
        assert_eq!(denied, vec!["fs.read:/etc".to_string()]);
    }

    #[test]
    fn revocation_stops_access() {
        let mut set = CapabilitySet::parse_all(["fs.read:/data", "gpu.use"]).unwrap();
        assert!(set.permits_str("fs.read:/data"));
        let n = set.revoke_resource("fs");
        assert_eq!(n, 1);
        assert!(!set.permits_str("fs.read:/data")); // revoked
        assert!(set.permits_str("gpu.use")); // unrelated cap intact
    }

    #[test]
    fn audit_strings_roundtrip() {
        let set = CapabilitySet::parse_all([
            "fs.read:/a",
            "net.connect:api.example.com:443",
            "gpu.use",
            "model.invoke:local/reasoning",
        ])
        .unwrap();
        let a = set.audit_strings();
        assert!(a.contains("fs.read:/a"));
        assert!(a.contains("net.connect:api.example.com:443"));
        assert!(a.contains("gpu.use"));
        assert!(a.contains("model.invoke:local/reasoning"));
    }
}
