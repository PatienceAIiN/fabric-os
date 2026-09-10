//! Unified settings (M5/§49). Typed, validated, JSON-persisted. Covers the
//! spec's categories relevant to a headless build; never stores secrets (the
//! API key lives in libcredentials, not here).
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiPrivacy {
    LocalOnly,
    AskBeforeCloud,
    NeverSendFiles,
    AllowCloud,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Appearance {
    pub theme: ThemeMode,
    pub accent: String,
    pub reduce_motion: bool,
    pub font_scale: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiSettings {
    pub system_wide: bool,
    pub default_provider: String,
    pub default_model: String,
    pub privacy: AiPrivacy,
    pub daily_budget_usd: f32,
    pub monthly_budget_usd: f32,
    pub allow_network: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub appearance: Appearance,
    pub ai: AiSettings,
}

impl Default for Settings {
    fn default() -> Self {
        // Privacy-preserving defaults (spec §14): local-first, no cloud, no budget.
        Settings {
            appearance: Appearance {
                theme: ThemeMode::Auto,
                accent: "#3B6EF5".into(),
                reduce_motion: false,
                font_scale: 1.0,
            },
            ai: AiSettings {
                system_wide: false,
                default_provider: "local".into(),
                default_model: "local/reasoning".into(),
                privacy: AiPrivacy::LocalOnly,
                daily_budget_usd: 0.0,
                monthly_budget_usd: 0.0,
                allow_network: false,
            },
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SetError(pub String);

impl Settings {
    pub fn load(path: &std::path::Path) -> Settings {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self).unwrap())
    }

    /// Get a value by dotted key (for CLI/settings UI).
    pub fn get(&self, key: &str) -> Option<String> {
        Some(match key {
            "appearance.theme" => format!("{:?}", self.appearance.theme).to_lowercase(),
            "appearance.accent" => self.appearance.accent.clone(),
            "appearance.reduce_motion" => self.appearance.reduce_motion.to_string(),
            "appearance.font_scale" => self.appearance.font_scale.to_string(),
            "ai.system_wide" => self.ai.system_wide.to_string(),
            "ai.default_provider" => self.ai.default_provider.clone(),
            "ai.default_model" => self.ai.default_model.clone(),
            "ai.privacy" => format!("{:?}", self.ai.privacy),
            "ai.daily_budget_usd" => self.ai.daily_budget_usd.to_string(),
            "ai.monthly_budget_usd" => self.ai.monthly_budget_usd.to_string(),
            "ai.allow_network" => self.ai.allow_network.to_string(),
            _ => return None,
        })
    }

    /// Set a value by dotted key with validation. Fail closed on bad input.
    pub fn set(&mut self, key: &str, val: &str) -> Result<(), SetError> {
        let boolv = || {
            val.parse::<bool>()
                .map_err(|_| SetError(format!("expected bool: {val}")))
        };
        let f32v = || {
            val.parse::<f32>()
                .map_err(|_| SetError(format!("expected number: {val}")))
        };
        match key {
            "appearance.theme" => {
                self.appearance.theme = match val {
                    "light" => ThemeMode::Light,
                    "dark" => ThemeMode::Dark,
                    "auto" => ThemeMode::Auto,
                    _ => return Err(SetError("theme: light|dark|auto".into())),
                }
            }
            "appearance.accent" => {
                if !(val.starts_with('#') && (val.len() == 7)) {
                    return Err(SetError("accent: #RRGGBB".into()));
                }
                self.appearance.accent = val.into();
            }
            "appearance.reduce_motion" => self.appearance.reduce_motion = boolv()?,
            "appearance.font_scale" => {
                let f = f32v()?;
                if !(0.5..=3.0).contains(&f) {
                    return Err(SetError("font_scale: 0.5..3.0".into()));
                }
                self.appearance.font_scale = f;
            }
            "ai.system_wide" => self.ai.system_wide = boolv()?,
            "ai.default_provider" => self.ai.default_provider = val.into(),
            "ai.default_model" => self.ai.default_model = val.into(),
            "ai.privacy" => {
                self.ai.privacy = match val {
                    "local_only" => AiPrivacy::LocalOnly,
                    "ask_before_cloud" => AiPrivacy::AskBeforeCloud,
                    "never_send_files" => AiPrivacy::NeverSendFiles,
                    "allow_cloud" => AiPrivacy::AllowCloud,
                    _ => {
                        return Err(SetError(
                            "privacy: local_only|ask_before_cloud|never_send_files|allow_cloud"
                                .into(),
                        ))
                    }
                }
            }
            "ai.daily_budget_usd" => self.ai.daily_budget_usd = f32v()?.max(0.0),
            "ai.monthly_budget_usd" => self.ai.monthly_budget_usd = f32v()?.max(0.0),
            "ai.allow_network" => self.ai.allow_network = boolv()?,
            _ => return Err(SetError(format!("unknown key: {key}"))),
        }
        Ok(())
    }

    pub fn keys() -> &'static [&'static str] {
        &[
            "appearance.theme",
            "appearance.accent",
            "appearance.reduce_motion",
            "appearance.font_scale",
            "ai.system_wide",
            "ai.default_provider",
            "ai.default_model",
            "ai.privacy",
            "ai.daily_budget_usd",
            "ai.monthly_budget_usd",
            "ai.allow_network",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_privacy_preserving() {
        let s = Settings::default();
        assert!(!s.ai.system_wide);
        assert_eq!(s.ai.privacy, AiPrivacy::LocalOnly);
        assert!(!s.ai.allow_network);
    }

    #[test]
    fn get_set_roundtrip_all_keys() {
        let mut s = Settings::default();
        for k in Settings::keys() {
            assert!(s.get(k).is_some(), "get {k}");
        }
        s.set("appearance.theme", "dark").unwrap();
        assert_eq!(s.get("appearance.theme").unwrap(), "dark");
        s.set("ai.system_wide", "true").unwrap();
        assert_eq!(s.get("ai.system_wide").unwrap(), "true");
    }

    #[test]
    fn validation_rejects_bad_values() {
        let mut s = Settings::default();
        assert!(s.set("appearance.theme", "neon").is_err());
        assert!(s.set("appearance.accent", "blue").is_err());
        assert!(s.set("appearance.font_scale", "9").is_err());
        assert!(s.set("ai.system_wide", "maybe").is_err());
        assert!(s.set("nope.key", "x").is_err());
    }

    #[test]
    fn persists_json_roundtrip() {
        let mut s = Settings::default();
        s.set("appearance.theme", "light").unwrap();
        s.set("ai.default_model", "local/qwen2.5-0.5b-instruct")
            .unwrap();
        let p = std::env::temp_dir().join(format!("ainos-settings-{}.json", std::process::id()));
        s.save(&p).unwrap();
        let loaded = Settings::load(&p);
        assert_eq!(loaded, s);
        std::fs::remove_file(&p).ok();
    }
}
