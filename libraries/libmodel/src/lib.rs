//! Local model management (M19) + supply-chain checks (M70).
//!
//! Models are managed resources, not harmless data. A model may not be loaded
//! until its file checksum matches its manifest and its trust status permits
//! it. Never download and execute arbitrary models blindly.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustStatus {
    Untrusted,
    Verified,
    Signed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelManifest {
    pub model_id: String,
    pub version: String,
    pub architecture: String,
    pub quantization: String,
    pub size_bytes: u64,
    pub context_length: u32,
    pub capabilities: Vec<String>,
    pub required_memory: u64,
    pub supported_accelerators: Vec<String>,
    pub license: String,
    pub checksum_sha256: String,
    pub source: String,
    pub trust_status: TrustStatus,
}

impl ModelManifest {
    pub fn from_json(s: &str) -> Result<Self, String> {
        serde_json::from_str(s).map_err(|e| e.to_string())
    }
    /// Verify a model file against this manifest: checksum must match and the
    /// model must not be Untrusted. Fail closed.
    pub fn verify_file(&self, path: &Path) -> Result<(), String> {
        if self.trust_status == TrustStatus::Untrusted {
            return Err("model is untrusted; refusing".into());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let got = libcrypto::sha256_hex(&bytes);
        if got != self.checksum_sha256 {
            return Err(format!(
                "checksum mismatch: manifest {} != file {}",
                self.checksum_sha256, got
            ));
        }
        Ok(())
    }
    pub fn fits_in(&self, available_ram: u64) -> bool {
        self.required_memory <= available_ram
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelState {
    Registered,
    Verified,
    Loaded,
    Unloaded,
}

#[derive(Default)]
pub struct Registry {
    models: HashMap<String, (ModelManifest, ModelState)>,
}

impl Registry {
    pub fn new() -> Self {
        Registry::default()
    }
    pub fn register(&mut self, m: ModelManifest) {
        self.models
            .insert(m.model_id.clone(), (m, ModelState::Registered));
    }
    pub fn get(&self, id: &str) -> Option<&(ModelManifest, ModelState)> {
        self.models.get(id)
    }
    pub fn list(&self) -> Vec<&ModelManifest> {
        self.models.values().map(|(m, _)| m).collect()
    }
    /// Verify then mark Verified. Loading is only allowed after this.
    pub fn verify(&mut self, id: &str, file: &Path) -> Result<(), String> {
        let (m, st) = self.models.get_mut(id).ok_or("no such model")?;
        m.verify_file(file)?;
        *st = ModelState::Verified;
        Ok(())
    }
    /// Load: refused unless the model was Verified and fits in `available_ram`.
    pub fn load(&mut self, id: &str, available_ram: u64) -> Result<(), String> {
        let (m, st) = self.models.get_mut(id).ok_or("no such model")?;
        if *st != ModelState::Verified && *st != ModelState::Unloaded {
            return Err("model not verified; refusing to load".into());
        }
        if !m.fits_in(available_ram) {
            return Err(format!(
                "insufficient memory: needs {}, have {}",
                m.required_memory, available_ram
            ));
        }
        *st = ModelState::Loaded;
        Ok(())
    }
    pub fn unload(&mut self, id: &str) -> Result<(), String> {
        let (_, st) = self.models.get_mut(id).ok_or("no such model")?;
        *st = ModelState::Unloaded;
        Ok(())
    }
    pub fn state(&self, id: &str) -> Option<ModelState> {
        self.models.get(id).map(|(_, s)| *s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(checksum: &str, trust: TrustStatus, mem: u64) -> ModelManifest {
        ModelManifest {
            model_id: "local/reasoning".into(),
            version: "1.0".into(),
            architecture: "llama".into(),
            quantization: "q4".into(),
            size_bytes: 10,
            context_length: 8192,
            capabilities: vec!["chat".into()],
            required_memory: mem,
            supported_accelerators: vec!["cpu".into()],
            license: "apache-2.0".into(),
            checksum_sha256: checksum.into(),
            source: "local".into(),
            trust_status: trust,
        }
    }

    fn tmpfile(content: &[u8]) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "model-{}-{}.bin",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn manifest_roundtrips_json() {
        let m = manifest("abc", TrustStatus::Verified, 1);
        let j = serde_json::to_string(&m).unwrap();
        assert_eq!(ModelManifest::from_json(&j).unwrap(), m);
    }

    #[test]
    fn wrong_checksum_fails_verification() {
        let f = tmpfile(b"model-weights");
        let good = libcrypto::sha256_hex(b"model-weights");
        let m_ok = manifest(&good, TrustStatus::Verified, 1);
        assert!(m_ok.verify_file(&f).is_ok());
        let m_bad = manifest("deadbeef", TrustStatus::Verified, 1);
        assert!(m_bad.verify_file(&f).is_err());
        std::fs::remove_file(&f).ok();
    }

    #[test]
    fn untrusted_model_refused() {
        let f = tmpfile(b"x");
        let good = libcrypto::sha256_hex(b"x");
        let m = manifest(&good, TrustStatus::Untrusted, 1);
        assert!(m.verify_file(&f).is_err());
        std::fs::remove_file(&f).ok();
    }

    #[test]
    fn load_requires_verify_and_fit() {
        let f = tmpfile(b"weights");
        let good = libcrypto::sha256_hex(b"weights");
        let mut reg = Registry::new();
        reg.register(manifest(&good, TrustStatus::Verified, 1000));
        // cannot load before verify
        assert!(reg.load("local/reasoning", 5000).is_err());
        reg.verify("local/reasoning", &f).unwrap();
        // fits
        assert!(reg.load("local/reasoning", 5000).is_ok());
        assert_eq!(reg.state("local/reasoning"), Some(ModelState::Loaded));
        reg.unload("local/reasoning").unwrap();
        // does not fit
        assert!(reg.load("local/reasoning", 500).is_err());
        std::fs::remove_file(&f).ok();
    }
}
