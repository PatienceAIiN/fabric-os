//! Provider-independent AI interface.
//!
//! The rest of the OS talks to [`AIProvider`], never to a specific vendor.
//! [`LocalProvider`] is offline and deterministic (local-first default).
//! [`ClaudeProvider`] calls the Anthropic Messages API via `curl`, taking the
//! key from a [`CredentialStore`] at call time and never logging it. The API
//! key is never held by callers; they invoke this local abstraction.
#![forbid(unsafe_code)]

use libcredentials::CredentialStore;
use serde::{Deserialize, Serialize};

pub const DEFAULT_CLAUDE_MODEL: &str = "claude-sonnet-5";
pub const ANTHROPIC_VERSION: &str = "2023-06-01";
pub const ANTHROPIC_BASE: &str = "https://api.anthropic.com";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: String, // "user" | "assistant"
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatResponse {
    pub text: String,
    pub model: String,
    pub stop_reason: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// True if produced by a local model (for the cloud/local UI indicator).
    pub local: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProviderError {
    NotConfigured,
    Auth,
    Network,
    Timeout,
    RateLimit,
    Overloaded,
    ContextOverflow,
    Malformed(String),
    Other(String),
}
impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ProviderError::*;
        match self {
            NotConfigured => write!(f, "provider not configured"),
            Auth => write!(f, "authentication required"),
            Network => write!(f, "network failure"),
            Timeout => write!(f, "request timed out"),
            RateLimit => write!(f, "rate limited"),
            Overloaded => write!(f, "provider overloaded"),
            ContextOverflow => write!(f, "context overflow"),
            Malformed(e) => write!(f, "malformed response: {e}"),
            Other(e) => write!(f, "provider error: {e}"),
        }
    }
}
impl std::error::Error for ProviderError {}

/// The provider-independent interface.
pub trait AIProvider {
    fn name(&self) -> &str;
    fn is_local(&self) -> bool;
    fn health_check(&self) -> bool;
    fn model_list(&self) -> Vec<String>;
    fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, ProviderError>;
}

fn approx_tokens(s: &str) -> u64 {
    // Rough, provider-agnostic estimate (~4 chars/token). Never presented as
    // exact billing.
    (s.len() as u64).div_ceil(4)
}

/// Offline, deterministic local provider (local-first default + test double).
pub struct LocalProvider {
    model: String,
}
impl LocalProvider {
    pub fn new() -> Self {
        LocalProvider {
            model: "local/reasoning".into(),
        }
    }
}
impl Default for LocalProvider {
    fn default() -> Self {
        Self::new()
    }
}
impl AIProvider for LocalProvider {
    fn name(&self) -> &str {
        "local"
    }
    fn is_local(&self) -> bool {
        true
    }
    fn health_check(&self) -> bool {
        true
    }
    fn model_list(&self) -> Vec<String> {
        vec![self.model.clone()]
    }
    fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        let last_user = req
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .unwrap_or("");
        let text = format!(
            "[local model] I received {} message(s). Last user request: {}",
            req.messages.len(),
            last_user.chars().take(200).collect::<String>()
        );
        let out_tokens = approx_tokens(&text);
        let in_tokens = req.messages.iter().map(|m| approx_tokens(&m.content)).sum();
        Ok(ChatResponse {
            text,
            model: self.model.clone(),
            stop_reason: "end_turn".into(),
            input_tokens: in_tokens,
            output_tokens: out_tokens,
            local: true,
        })
    }
}

/// Anthropic Claude provider. Holds a reference to a credential store and the
/// credential *name* — never the key itself.
pub struct ClaudeProvider<'a> {
    store: &'a dyn CredentialStore,
    key_name: String,
    model: String,
    base: String,
    timeout_secs: u32,
}

impl std::fmt::Debug for ClaudeProvider<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never expose the key or even the credential contents.
        f.debug_struct("ClaudeProvider")
            .field("model", &self.model)
            .field("base", &self.base)
            .field("key_name", &self.key_name)
            .finish()
    }
}

impl<'a> ClaudeProvider<'a> {
    pub fn new(store: &'a dyn CredentialStore, key_name: &str, model: &str) -> Self {
        ClaudeProvider {
            store,
            key_name: key_name.to_string(),
            model: model.to_string(),
            base: ANTHROPIC_BASE.to_string(),
            timeout_secs: 60,
        }
    }
    pub fn with_base(mut self, base: &str) -> Self {
        self.base = base.to_string();
        self
    }

    /// Build the Messages API request body. Pure and testable; contains no key.
    pub fn build_body(&self, req: &ChatRequest) -> String {
        let mut body = serde_json::Map::new();
        body.insert(
            "model".into(),
            serde_json::Value::String(self.model.clone()),
        );
        body.insert("max_tokens".into(), serde_json::Value::from(req.max_tokens));
        if let Some(sys) = &req.system {
            body.insert("system".into(), serde_json::Value::String(sys.clone()));
        }
        let msgs: Vec<serde_json::Value> = req
            .messages
            .iter()
            .map(|m| serde_json::json!({ "role": m.role, "content": m.content }))
            .collect();
        body.insert("messages".into(), serde_json::Value::Array(msgs));
        serde_json::Value::Object(body).to_string()
    }

    fn map_status(code: u32, stderr: &str) -> ProviderError {
        match code {
            401 | 403 => ProviderError::Auth,
            429 => ProviderError::RateLimit,
            529 => ProviderError::Overloaded,
            400 if stderr.contains("context") || stderr.contains("max_tokens") => {
                ProviderError::ContextOverflow
            }
            0 => ProviderError::Network,
            c => ProviderError::Other(format!("http {c}")),
        }
    }
}

impl AIProvider for ClaudeProvider<'_> {
    fn name(&self) -> &str {
        "claude"
    }
    fn is_local(&self) -> bool {
        false
    }
    fn health_check(&self) -> bool {
        self.store.exists(&self.key_name)
    }
    fn model_list(&self) -> Vec<String> {
        vec![
            "claude-fable-5-1".into(),
            "claude-opus-5".into(),
            "claude-sonnet-5".into(),
        ]
    }
    fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        let key = self
            .store
            .retrieve(&self.key_name)
            .map_err(|_| ProviderError::NotConfigured)?;
        let body = self.build_body(req);
        // curl call; key passed only as a header value, never logged. Response
        // and HTTP status separated via -w.
        use std::io::Write;
        let mut child = std::process::Command::new("curl")
            .args([
                "-sS",
                "--max-time",
                &self.timeout_secs.to_string(),
                "-w",
                "\n%{http_code}",
                "-X",
                "POST",
                &format!("{}/v1/messages", self.base),
                "-H",
                "content-type: application/json",
                "-H",
                &format!("anthropic-version: {ANTHROPIC_VERSION}"),
                "-H",
                &format!("x-api-key: {}", key.expose()),
                "--data-binary",
                "@-",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| ProviderError::Other(e.to_string()))?;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(body.as_bytes())
            .map_err(|e| ProviderError::Other(e.to_string()))?;
        let out = child
            .wait_with_output()
            .map_err(|e| ProviderError::Other(e.to_string()))?;
        // curl exit 28 = timeout; 6/7 = network
        if !out.status.success() {
            return Err(match out.status.code() {
                Some(28) => ProviderError::Timeout,
                Some(6) | Some(7) => ProviderError::Network,
                _ => ProviderError::Network,
            });
        }
        let raw = String::from_utf8_lossy(&out.stdout);
        let (payload, code_str) = raw.rsplit_once('\n').unwrap_or((raw.as_ref(), "0"));
        let code: u32 = code_str.trim().parse().unwrap_or(0);
        if !(200..300).contains(&code) {
            return Err(Self::map_status(code, payload));
        }
        let v: serde_json::Value =
            serde_json::from_str(payload).map_err(|e| ProviderError::Malformed(e.to_string()))?;
        let text = v["content"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|b| b["text"].as_str())
            .ok_or_else(|| ProviderError::Malformed("no content[].text".into()))?
            .to_string();
        Ok(ChatResponse {
            text,
            model: v["model"].as_str().unwrap_or(&self.model).to_string(),
            stop_reason: v["stop_reason"].as_str().unwrap_or("").to_string(),
            input_tokens: v["usage"]["input_tokens"].as_u64().unwrap_or(0),
            output_tokens: v["usage"]["output_tokens"].as_u64().unwrap_or(0),
            local: false,
        })
    }
}

/// Local llama.cpp CPU inference provider (real local-first inference).
/// Shells out to a `llama-cli` binary with a GGUF model. `is_local() == true`.
pub struct LlamaProvider {
    cli: String,
    model_path: String,
    model_id: String,
    threads: u32,
    n_predict: u32,
    ctx: u32,
    n_gpu_layers: u32,
}

impl LlamaProvider {
    pub fn new(cli: &str, model_path: &str, model_id: &str) -> Self {
        LlamaProvider {
            cli: cli.to_string(),
            model_path: model_path.to_string(),
            model_id: model_id.to_string(),
            threads: 6,
            n_predict: 256,
            ctx: 2048,
            n_gpu_layers: 0,
        }
    }

    /// Offload N transformer layers to a GPU (0 = CPU only).
    pub fn with_gpu_layers(mut self, n: u32) -> Self {
        self.n_gpu_layers = n;
        self
    }

    /// Resolve from environment: AIOS_LLAMA_CLI + AIOS_LLAMA_MODEL. Returns None
    /// if either the binary or model file is missing (caller falls back).
    pub fn from_env() -> Option<Self> {
        let cli = std::env::var("AIOS_LLAMA_CLI").ok()?;
        let model = std::env::var("AIOS_LLAMA_MODEL").ok()?;
        if std::path::Path::new(&cli).exists() && std::path::Path::new(&model).exists() {
            Some(LlamaProvider::new(
                &cli,
                &model,
                "local/qwen2.5-0.5b-instruct",
            ))
        } else {
            None
        }
    }

    pub fn available(&self) -> bool {
        std::path::Path::new(&self.cli).exists() && std::path::Path::new(&self.model_path).exists()
    }

    /// Extract the completion from llama-cli output: the text between the echoed
    /// prompt line ("> ...") and the perf marker ("[ Prompt:"), banner removed.
    pub fn parse_completion(stdout: &str) -> String {
        // cut off the trailing perf/exit section
        let body = match stdout.find("\n[ Prompt:") {
            Some(i) => &stdout[..i],
            None => stdout,
        };
        // answer begins after the last echoed "> " prompt line
        if let Some(pos) = body.rfind("\n> ") {
            let after_prompt = &body[pos + 1..];
            if let Some(nl) = after_prompt.find('\n') {
                return after_prompt[nl + 1..].trim().to_string();
            }
        }
        // fallback: last non-empty line block
        body.lines()
            .rfind(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .to_string()
    }
}

impl AIProvider for LlamaProvider {
    fn name(&self) -> &str {
        "llama"
    }
    fn is_local(&self) -> bool {
        true
    }
    fn health_check(&self) -> bool {
        self.available()
    }
    fn model_list(&self) -> Vec<String> {
        vec![self.model_id.clone()]
    }
    fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        if !self.available() {
            return Err(ProviderError::NotConfigured);
        }
        let prompt = req
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.clone())
            .unwrap_or_default();
        let out = std::process::Command::new(&self.cli)
            .args([
                "-m",
                &self.model_path,
                "-st",
                "--no-warmup",
                "--no-display-prompt",
                "--simple-io",
                "-t",
                &self.threads.to_string(),
                "-n",
                &req.max_tokens.min(self.n_predict).to_string(),
                "-c",
                &self.ctx.to_string(),
                "-p",
                &prompt,
            ])
            .output()
            .map_err(|e| ProviderError::Other(e.to_string()))?;
        if !out.status.success() {
            return Err(ProviderError::Other(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ));
        }
        let text = Self::parse_completion(&String::from_utf8_lossy(&out.stdout));
        if text.is_empty() {
            return Err(ProviderError::Malformed("empty completion".into()));
        }
        let out_tokens = approx_tokens(&text);
        let in_tokens = req.messages.iter().map(|m| approx_tokens(&m.content)).sum();
        Ok(ChatResponse {
            text,
            model: self.model_id.clone(),
            stop_reason: "end_turn".into(),
            input_tokens: in_tokens,
            output_tokens: out_tokens,
            local: true,
        })
    }
}

/// Minimal privacy-aware model routing (M13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Local,
    Cloud,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteDecision {
    pub route: Route,
    pub reason: String,
}

/// Choose a route. Privacy and offline force local; otherwise honor preference.
pub fn route(
    local_only: bool,
    contains_personal_data: bool,
    cloud_available: bool,
    prefer_cloud_for_complex: bool,
) -> RouteDecision {
    if local_only {
        return RouteDecision {
            route: Route::Local,
            reason: "local-only mode".into(),
        };
    }
    if contains_personal_data {
        return RouteDecision {
            route: Route::Local,
            reason: "personal data: never sent to cloud".into(),
        };
    }
    if !cloud_available {
        return RouteDecision {
            route: Route::Local,
            reason: "cloud unavailable".into(),
        };
    }
    if prefer_cloud_for_complex {
        return RouteDecision {
            route: Route::Cloud,
            reason: "complex reasoning: cloud".into(),
        };
    }
    RouteDecision {
        route: Route::Local,
        reason: "default local-first".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libcredentials::{MemoryStore, Secret};

    fn req() -> ChatRequest {
        ChatRequest {
            model: DEFAULT_CLAUDE_MODEL.into(),
            system: Some("be terse".into()),
            messages: vec![Message {
                role: "user".into(),
                content: "hello".into(),
            }],
            max_tokens: 256,
        }
    }

    #[test]
    fn local_provider_is_deterministic_and_offline() {
        let p = LocalProvider::new();
        assert!(p.is_local());
        let r1 = p.chat(&req()).unwrap();
        let r2 = p.chat(&req()).unwrap();
        assert_eq!(r1, r2);
        assert!(r1.text.contains("hello"));
        assert!(r1.local);
    }

    #[test]
    fn claude_body_has_expected_shape_and_no_key() {
        let store = MemoryStore::default();
        let p = ClaudeProvider::new(&store, "claude-key", DEFAULT_CLAUDE_MODEL);
        let body = p.build_body(&req());
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["model"], DEFAULT_CLAUDE_MODEL);
        assert_eq!(v["max_tokens"], 256);
        assert_eq!(v["system"], "be terse");
        assert_eq!(v["messages"][0]["role"], "user");
        // body must never contain a key
        assert!(!body.contains("x-api-key"));
        assert!(!body.contains("sk-"));
    }

    #[test]
    fn claude_without_key_is_not_configured() {
        let store = MemoryStore::default();
        let p = ClaudeProvider::new(&store, "missing", DEFAULT_CLAUDE_MODEL);
        assert!(!p.health_check());
        // No key stored => NotConfigured, and no network call is attempted.
        assert_eq!(p.chat(&req()).unwrap_err(), ProviderError::NotConfigured);
    }

    #[test]
    fn claude_debug_never_leaks_key_material() {
        let mut store = MemoryStore::default();
        store
            .store("claude-key", &Secret::new("sk-ant-SECRET"))
            .unwrap();
        let p = ClaudeProvider::new(&store, "claude-key", DEFAULT_CLAUDE_MODEL);
        let dbg = format!("{p:?}");
        assert!(!dbg.contains("sk-ant-SECRET"));
    }

    #[test]
    fn status_mapping() {
        assert_eq!(ClaudeProvider::map_status(401, ""), ProviderError::Auth);
        assert_eq!(
            ClaudeProvider::map_status(429, ""),
            ProviderError::RateLimit
        );
        assert_eq!(
            ClaudeProvider::map_status(529, ""),
            ProviderError::Overloaded
        );
    }

    #[test]
    fn routing_respects_privacy_and_offline() {
        assert_eq!(route(true, false, true, true).route, Route::Local);
        assert_eq!(route(false, true, true, true).route, Route::Local);
        assert_eq!(route(false, false, false, true).route, Route::Local);
        assert_eq!(route(false, false, true, true).route, Route::Cloud);
        assert_eq!(route(false, false, true, false).route, Route::Local);
    }

    #[test]
    fn llama_parser_extracts_answer() {
        let sample = "banner junk\nbuild : x\n\n> what is 2+2?\nThe answer is 4.\n\n[ Prompt: 100 t/s | Generation: 40 t/s ]\nExiting...";
        assert_eq!(LlamaProvider::parse_completion(sample), "The answer is 4.");
    }

    #[test]
    fn llama_provider_real_inference_if_present() {
        // Gated: only runs when a real cli+model are provided via env.
        let p = match LlamaProvider::from_env() {
            Some(p) => p,
            None => {
                eprintln!("SKIP: set AIOS_LLAMA_CLI + AIOS_LLAMA_MODEL for live test");
                return;
            }
        };
        let r = p
            .chat(&ChatRequest {
                model: "local".into(),
                system: None,
                messages: vec![Message {
                    role: "user".into(),
                    content: "Reply with the single word: ok".into(),
                }],
                max_tokens: 16,
            })
            .expect("inference");
        assert!(!r.text.is_empty());
        assert!(r.local);
    }
}
