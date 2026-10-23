//! AI agent for crash analysis, JVM tuning, pack building.
//! M0: trait + stub cloud provider only (no real API calls).

use async_trait::async_trait;
use easymc_core::{EasyMcError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentReply {
    pub content: String,
    pub proposed_changes: Option<serde_json::Value>,
}

/// Pluggable cloud LLM backend (OpenAI-compatible, Anthropic, etc.).
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn complete(&self, messages: &[ChatMessage]) -> Result<AgentReply>;
}

/// Stub that echoes a canned helpful reply — swap for real HTTP in M1+.
pub struct StubCloudProvider {
    pub display_name: String,
}

impl Default for StubCloudProvider {
    fn default() -> Self {
        Self {
            display_name: "stub-cloud".into(),
        }
    }
}

#[async_trait]
impl LlmProvider for StubCloudProvider {
    fn name(&self) -> &str {
        &self.display_name
    }

    async fn complete(&self, messages: &[ChatMessage]) -> Result<AgentReply> {
        let last = messages
            .last()
            .map(|m| m.content.as_str())
            .unwrap_or("");
        Ok(AgentReply {
            content: format!(
                "[stub] Received: {last}\n\
                 I can analyze crash logs, tune JVM flags, and build packs once a real LLM provider is configured."
            ),
            proposed_changes: None,
        })
    }
}

pub async fn run_tool(_name: &str) -> Result<()> {
    Err(EasyMcError::NotImplemented("agent tools"))
}
