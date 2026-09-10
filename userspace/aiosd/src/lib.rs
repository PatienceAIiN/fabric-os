//! Local AI service (M9) + system-wide AI toggle (M10).
//!
//! Applications never hold the API key; they send requests here and this
//! service resolves providers, routes local vs cloud by privacy/availability,
//! and returns responses with a clear local/cloud indicator. When system-wide
//! AI is OFF, AI requests are refused and normal OS operation continues.
#![forbid(unsafe_code)]

use libcredentials::CredentialStore;
use libprovider::{
    route, AIProvider, ChatRequest, ClaudeProvider, LlamaProvider, LocalProvider, Message, Route,
    DEFAULT_CLAUDE_MODEL,
};
use serde::{Deserialize, Serialize};

pub const CLAUDE_KEY_NAME: &str = "claude-api-key";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Status,
    SetSystemWide {
        on: bool,
    },
    ModelList,
    Chat {
        prompt: String,
        #[serde(default)]
        local_only: bool,
        #[serde(default)]
        personal_data: bool,
        #[serde(default)]
        prefer_cloud: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Status {
        system_wide: bool,
        local: bool,
        cloud_configured: bool,
    },
    Ok,
    Models {
        models: Vec<String>,
    },
    Chat {
        text: String,
        indicator: String,
        model: String,
        input_tokens: u64,
        output_tokens: u64,
    },
    Error {
        message: String,
    },
}

/// Daemon state. `store` supplies the Claude key only to the cloud provider.
pub struct State<S: CredentialStore> {
    pub system_wide: bool,
    pub store: S,
    pub cloud_base: Option<String>, // override for tests
}

impl<S: CredentialStore> State<S> {
    pub fn new(store: S) -> Self {
        State {
            system_wide: true,
            store,
            cloud_base: None,
        }
    }

    fn cloud_configured(&self) -> bool {
        self.store.exists(CLAUDE_KEY_NAME)
    }

    /// Dispatch one request. Pure with respect to state; the socket layer wraps
    /// this. Never returns the API key or provider error internals.
    pub fn dispatch(&mut self, req: Request) -> Response {
        match req {
            Request::Status => Response::Status {
                system_wide: self.system_wide,
                local: true,
                cloud_configured: self.cloud_configured(),
            },
            Request::SetSystemWide { on } => {
                self.system_wide = on;
                Response::Ok
            }
            Request::ModelList => {
                let mut models = LocalProvider::new().model_list();
                if self.cloud_configured() {
                    let p = ClaudeProvider::new(&self.store, CLAUDE_KEY_NAME, DEFAULT_CLAUDE_MODEL);
                    models.extend(p.model_list());
                }
                Response::Models { models }
            }
            Request::Chat {
                prompt,
                local_only,
                personal_data,
                prefer_cloud,
            } => {
                if !self.system_wide {
                    return Response::Error {
                        message: "system-wide AI is OFF".into(),
                    };
                }
                let cloud_ok = self.cloud_configured();
                let decision = route(local_only, personal_data, cloud_ok, prefer_cloud);
                let chat = ChatRequest {
                    model: DEFAULT_CLAUDE_MODEL.into(),
                    system: None,
                    messages: vec![Message {
                        role: "user".into(),
                        content: prompt,
                    }],
                    max_tokens: 512,
                };
                match decision.route {
                    Route::Local => {
                        // Prefer REAL local inference (llama.cpp) when a model is
                        // configured; else fall back to the deterministic stub.
                        if let Some(llama) = LlamaProvider::from_env() {
                            if let Ok(r) = llama.chat(&chat) {
                                return Response::Chat {
                                    text: r.text,
                                    indicator: format!(
                                        "● Local AI · llama.cpp ({})",
                                        decision.reason
                                    ),
                                    model: r.model,
                                    input_tokens: r.input_tokens,
                                    output_tokens: r.output_tokens,
                                };
                            }
                        }
                        let p = LocalProvider::new();
                        match p.chat(&chat) {
                            Ok(r) => Response::Chat {
                                text: r.text,
                                indicator: format!("● Local AI ({})", decision.reason),
                                model: r.model,
                                input_tokens: r.input_tokens,
                                output_tokens: r.output_tokens,
                            },
                            Err(e) => Response::Error {
                                message: e.to_string(),
                            },
                        }
                    }
                    Route::Cloud => {
                        let mut p =
                            ClaudeProvider::new(&self.store, CLAUDE_KEY_NAME, DEFAULT_CLAUDE_MODEL);
                        if let Some(base) = &self.cloud_base {
                            p = p.with_base(base);
                        }
                        match p.chat(&chat) {
                            Ok(r) => Response::Chat {
                                text: r.text,
                                indicator: "☁ Claude".into(),
                                model: r.model,
                                input_tokens: r.input_tokens,
                                output_tokens: r.output_tokens,
                            },
                            Err(e) => Response::Error {
                                message: e.to_string(),
                            },
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libcredentials::{MemoryStore, Secret};

    #[test]
    fn status_reflects_config() {
        let mut st = State::new(MemoryStore::default());
        assert_eq!(
            st.dispatch(Request::Status),
            Response::Status {
                system_wide: true,
                local: true,
                cloud_configured: false
            }
        );
        st.store
            .store(CLAUDE_KEY_NAME, &Secret::new("sk-x"))
            .unwrap();
        match st.dispatch(Request::Status) {
            Response::Status {
                cloud_configured, ..
            } => assert!(cloud_configured),
            _ => panic!(),
        }
    }

    #[test]
    fn system_wide_off_refuses_chat() {
        let mut st = State::new(MemoryStore::default());
        st.dispatch(Request::SetSystemWide { on: false });
        match st.dispatch(Request::Chat {
            prompt: "hi".into(),
            local_only: false,
            personal_data: false,
            prefer_cloud: true,
        }) {
            Response::Error { message } => assert!(message.contains("OFF")),
            _ => panic!("must refuse"),
        }
    }

    #[test]
    fn personal_data_forces_local_indicator() {
        let mut st = State::new(MemoryStore::default());
        st.store
            .store(CLAUDE_KEY_NAME, &Secret::new("sk-x"))
            .unwrap();
        match st.dispatch(Request::Chat {
            prompt: "my ssn is secret".into(),
            local_only: false,
            personal_data: true,
            prefer_cloud: true,
        }) {
            Response::Chat { indicator, .. } => {
                assert!(indicator.contains("Local"));
                assert!(indicator.contains("personal data"));
            }
            _ => panic!("expected local chat"),
        }
    }

    #[test]
    fn model_list_includes_cloud_when_configured() {
        let mut st = State::new(MemoryStore::default());
        st.store
            .store(CLAUDE_KEY_NAME, &Secret::new("sk-x"))
            .unwrap();
        match st.dispatch(Request::ModelList) {
            Response::Models { models } => {
                assert!(models.iter().any(|m| m.starts_with("local/")));
                assert!(models.iter().any(|m| m.contains("claude")));
            }
            _ => panic!(),
        }
    }
}
