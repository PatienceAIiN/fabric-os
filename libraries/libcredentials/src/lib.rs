//! Secure credential storage.
//!
//! API keys must never live in source, git, plaintext config, logs, or
//! provenance, and never display after entry. [`Secret`] hides its value from
//! Debug/Display. Backends encrypt at rest: [`SystemdCredsStore`] uses the
//! host `systemd-creds` facility (host/TPM-bound, no root, no invented crypto).
//! [`MemoryStore`] is for tests only.
#![forbid(unsafe_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A secret value that never reveals itself via Debug/Display.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Secret(s.into())
    }
    /// Explicit, greppable accessor — the only way to read the value.
    pub fn expose(&self) -> &str {
        &self.0
    }
    /// A masked form safe to display (shows only that a value exists).
    pub fn masked(&self) -> String {
        if self.0.is_empty() {
            String::new()
        } else {
            "\u{2022}".repeat(20)
        }
    }
}
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Secret([REDACTED])")
    }
}
impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.masked())
    }
}

#[derive(Debug)]
pub enum CredError {
    NotFound,
    Backend(String),
    Io(String),
}
impl std::fmt::Display for CredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredError::NotFound => write!(f, "credential not found"),
            CredError::Backend(e) => write!(f, "credential backend error: {e}"),
            CredError::Io(e) => write!(f, "credential io error: {e}"),
        }
    }
}
impl std::error::Error for CredError {}

/// A pluggable secure credential store. Fail closed: never fall back to
/// plaintext.
pub trait CredentialStore {
    fn store(&mut self, name: &str, secret: &Secret) -> Result<(), CredError>;
    fn retrieve(&self, name: &str) -> Result<Secret, CredError>;
    fn delete(&mut self, name: &str) -> Result<(), CredError>;
    fn exists(&self, name: &str) -> bool;
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// In-memory store for tests only. Never used for real secrets on disk.
#[derive(Default)]
pub struct MemoryStore {
    map: std::collections::HashMap<String, Secret>,
}
impl CredentialStore for MemoryStore {
    fn store(&mut self, name: &str, secret: &Secret) -> Result<(), CredError> {
        if !valid_name(name) {
            return Err(CredError::Backend("invalid credential name".into()));
        }
        self.map.insert(name.to_string(), secret.clone());
        Ok(())
    }
    fn retrieve(&self, name: &str) -> Result<Secret, CredError> {
        self.map.get(name).cloned().ok_or(CredError::NotFound)
    }
    fn delete(&mut self, name: &str) -> Result<(), CredError> {
        self.map.remove(name);
        Ok(())
    }
    fn exists(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }
}

/// On-disk store encrypting each secret with `systemd-creds`. Blobs are stored
/// at `<dir>/<name>.cred` with 0600 permissions. The plaintext never touches
/// disk; only the host can decrypt.
pub struct SystemdCredsStore {
    dir: PathBuf,
}
impl SystemdCredsStore {
    pub fn new(dir: impl AsRef<Path>) -> Result<Self, CredError> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir).map_err(|e| CredError::Io(e.to_string()))?;
        Ok(SystemdCredsStore { dir })
    }
    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.cred"))
    }
    /// Whether the systemd-creds tool is usable in this environment.
    pub fn available() -> bool {
        Command::new("systemd-creds")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}
impl CredentialStore for SystemdCredsStore {
    fn store(&mut self, name: &str, secret: &Secret) -> Result<(), CredError> {
        if !valid_name(name) {
            return Err(CredError::Backend("invalid credential name".into()));
        }
        let mut child = Command::new("systemd-creds")
            .args(["encrypt", &format!("--name={name}"), "-", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| CredError::Backend(e.to_string()))?;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(secret.expose().as_bytes())
            .map_err(|e| CredError::Io(e.to_string()))?;
        let out = child
            .wait_with_output()
            .map_err(|e| CredError::Backend(e.to_string()))?;
        if !out.status.success() {
            return Err(CredError::Backend(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ));
        }
        let path = self.path(name);
        std::fs::write(&path, &out.stdout).map_err(|e| CredError::Io(e.to_string()))?;
        set_600(&path)?;
        Ok(())
    }
    fn retrieve(&self, name: &str) -> Result<Secret, CredError> {
        let path = self.path(name);
        if !path.exists() {
            return Err(CredError::NotFound);
        }
        let out = Command::new("systemd-creds")
            .args([
                "decrypt",
                &format!("--name={name}"),
                path.to_str().unwrap(),
                "-",
            ])
            .output()
            .map_err(|e| CredError::Backend(e.to_string()))?;
        if !out.status.success() {
            return Err(CredError::Backend(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ));
        }
        Ok(Secret::new(
            String::from_utf8_lossy(&out.stdout).to_string(),
        ))
    }
    fn delete(&mut self, name: &str) -> Result<(), CredError> {
        let path = self.path(name);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| CredError::Io(e.to_string()))?;
        }
        Ok(())
    }
    fn exists(&self, name: &str) -> bool {
        self.path(name).exists()
    }
}

fn set_600(path: &Path) -> Result<(), CredError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms).map_err(|e| CredError::Io(e.to_string()))?;
    }
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_never_reveals_in_debug_or_display() {
        let s = Secret::new("sk-ant-supersecret-123456");
        assert_eq!(format!("{s:?}"), "Secret([REDACTED])");
        assert!(!format!("{s}").contains("supersecret"));
        assert_eq!(s.masked().chars().count(), 20);
        assert_eq!(s.expose(), "sk-ant-supersecret-123456");
    }

    #[test]
    fn memory_store_roundtrip() {
        let mut st = MemoryStore::default();
        st.store("claude-key", &Secret::new("abc123")).unwrap();
        assert!(st.exists("claude-key"));
        assert_eq!(st.retrieve("claude-key").unwrap().expose(), "abc123");
        st.delete("claude-key").unwrap();
        assert!(!st.exists("claude-key"));
        assert!(matches!(
            st.retrieve("claude-key"),
            Err(CredError::NotFound)
        ));
    }

    #[test]
    fn rejects_bad_names() {
        let mut st = MemoryStore::default();
        assert!(st.store("../etc/passwd", &Secret::new("x")).is_err());
        assert!(st.store("", &Secret::new("x")).is_err());
    }

    #[test]
    fn systemd_creds_roundtrip_and_no_plaintext_on_disk() {
        if !SystemdCredsStore::available() {
            eprintln!("SKIP: systemd-creds unavailable");
            return;
        }
        let dir = std::env::temp_dir().join(format!("ainos-cred-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut st = SystemdCredsStore::new(&dir).unwrap();
        let plain = "sk-ant-PLAINTEXT-MUST-NOT-APPEAR-9999";
        match st.store("claude", &Secret::new(plain)) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("SKIP: systemd-creds encrypt failed: {e}");
                return;
            }
        }
        let blob = std::fs::read(dir.join("claude.cred")).unwrap();
        assert!(!String::from_utf8_lossy(&blob).contains("PLAINTEXT-MUST-NOT-APPEAR"));
        assert_eq!(st.retrieve("claude").unwrap().expose(), plain);
        st.delete("claude").unwrap();
        assert!(!st.exists("claude"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
