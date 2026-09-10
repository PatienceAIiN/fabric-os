//! Capability -> Linux sandbox (M25/§21).
//!
//! Translates an agent's granted capability set into concrete `systemd-run
//! --user` sandbox properties, so the deterministic enforcement sits *below*
//! the AI: even if the agent is compromised, the kernel confines it. Network
//! is denied unless a `net.*` capability exists; the filesystem is read-only
//! except for granted `fs.write` paths; privilege escalation is blocked.
#![forbid(unsafe_code)]

use libcapability::{Action, CapabilitySet, Scope};

/// Build sandbox properties for a command run on behalf of an agent holding
/// `caps`. Returns args for a transient service: `systemd-run --user <args> --wait -- <cmd>` (service unit, not a scope; PrivateNetwork needs unprivileged user namespaces).
pub fn sandbox_args(caps: &CapabilitySet) -> Vec<String> {
    let mut v = vec![
        "--property=NoNewPrivileges=yes".to_string(),
        "--property=ProtectSystem=strict".to_string(),
        "--property=ProtectHome=read-only".to_string(),
        "--property=PrivateTmp=yes".to_string(),
        "--property=ProtectKernelTunables=yes".to_string(),
        "--property=ProtectControlGroups=yes".to_string(),
        "--property=RestrictSUIDSGID=yes".to_string(),
        "--property=LockPersonality=yes".to_string(),
        "--property=SystemCallFilter=@system-service".to_string(),
    ];

    // Filesystem: grant write only to explicitly authorized fs.write paths.
    let mut writable = Vec::new();
    let mut readable = Vec::new();
    for cap in caps.iter() {
        if cap.resource == "fs" {
            if let Scope::Path(p) = &cap.scope {
                match cap.action {
                    Action::Write => writable.push(p.clone()),
                    Action::Read => readable.push(p.clone()),
                    _ => {}
                }
            }
        }
    }
    for p in &readable {
        v.push(format!("--property=ReadOnlyPaths={p}"));
    }
    for p in &writable {
        v.push(format!("--property=ReadWritePaths={p}"));
    }

    // Network: deny entirely unless a net capability is present.
    let has_net = caps.iter().any(|c| c.resource == "net");
    if has_net {
        v.push("--property=RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6".to_string());
    } else {
        v.push("--property=PrivateNetwork=yes".to_string());
        v.push("--property=RestrictAddressFamilies=AF_UNIX".to_string());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_net_capability_means_private_network() {
        let caps = CapabilitySet::parse_all(["fs.read:/data"]).unwrap();
        let a = sandbox_args(&caps);
        assert!(a.iter().any(|x| x == "--property=PrivateNetwork=yes"));
        assert!(a.iter().any(|x| x.contains("ReadOnlyPaths=/data")));
        assert!(!a.iter().any(|x| x.contains("ReadWritePaths")));
    }

    #[test]
    fn net_capability_allows_inet_families() {
        let caps = CapabilitySet::parse_all(["net.connect:api.example.com:443"]).unwrap();
        let a = sandbox_args(&caps);
        assert!(a.iter().any(|x| x.contains("AF_INET")));
        assert!(!a.iter().any(|x| x == "--property=PrivateNetwork=yes"));
    }

    #[test]
    fn write_paths_only_for_fs_write() {
        let caps = CapabilitySet::parse_all(["fs.read:/a", "fs.write:/a/out"]).unwrap();
        let a = sandbox_args(&caps);
        assert!(a.iter().any(|x| x == "--property=ReadWritePaths=/a/out"));
        assert!(a.iter().any(|x| x == "--property=ReadOnlyPaths=/a"));
    }

    #[test]
    fn always_blocks_privilege_escalation() {
        let a = sandbox_args(&CapabilitySet::new());
        assert!(a.iter().any(|x| x == "--property=NoNewPrivileges=yes"));
        assert!(a.iter().any(|x| x == "--property=ProtectSystem=strict"));
    }
}
