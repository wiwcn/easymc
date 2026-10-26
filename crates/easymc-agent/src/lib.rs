//! EasyMC local Agent: rule-based crash diagnose + intent router.
//! Optional OpenAI-compatible LLM when `EASYMC_LLM_BASE_URL` + `EASYMC_LLM_API_KEY` are set.
//! Never calls cloud providers unless those env vars are present.

mod actions;
mod diagnose;
mod intent;
pub mod llm;
mod patterns;
mod propose;
mod service;
mod tool;

pub use actions::{
    bump_memory, check_mods, env_status_summary, list_accounts, list_instances, open_latest_log,
    select_instance, set_download_source, show_data_dir, show_instance_dir, toggle_mod,
    write_diagnostic_bundle, ActionResult,
};
pub use diagnose::{diagnose, DiagnoseReport, DiagnoseRequest};
pub use intent::{route, Intent, IntentResult};
pub use llm::{
    clear_llm_config, cloud_tool_definitions, llm_env_path, llm_status, read_llm_config,
    save_llm_config, tool_call_to_utterance, try_load_dotenv_llm, LlmCompletion, LlmConfig,
    LlmConfigView, LlmStatus, LlmToolCall, OpenAiCompatibleProvider, ENV_API_KEY, ENV_BASE_URL,
    ENV_MODEL,
};
pub use patterns::{scan_log, Finding, FindingSeverity};
pub use propose::{
    apply_proposal, build_tune_proposal, java_major_for_mc, parse_proposal, proposal_to_action,
    suggested_client_jvm, undo_proposal, ApplyPayload, ChangeProposal, ProposeItem, UndoSnapshot,
};
pub use service::{
    ask, execute_intent, memory_suggestion, run_diagnose, run_diagnose_with_path, tune_proposal, AskRequest, AskResponse,
    DisabledGguf, LocalGgufBackend,
};
pub use tool::{
    attach_tool_to_ask, refresh_ask_tool, tool_call_from_action, tool_call_from_diagnose, tool_meta, tool_running,
    ToolArg, ToolCall, ToolStatus, ToolUiAction,
};

use async_trait::async_trait;
use easymc_core::Result;
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

/// Pluggable LLM backend (OpenAI-compatible HTTP, or future local GGUF).
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn complete(&self, messages: &[ChatMessage]) -> Result<AgentReply>;
}
