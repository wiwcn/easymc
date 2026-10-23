//! Optional OpenAI-compatible chat client.
//! Enabled only when EASYMC_LLM_BASE_URL + EASYMC_LLM_API_KEY are set.
//! Never hard-code secrets. Works with api.tomail.gq / DeepSeek / etc.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use easymc_core::{EasyMcError, Result};

use crate::{AgentReply, ChatMessage, LlmProvider};

pub const ENV_BASE_URL: &str = "EASYMC_LLM_BASE_URL";
pub const ENV_API_KEY: &str = "EASYMC_LLM_API_KEY";
pub const ENV_MODEL: &str = "EASYMC_LLM_MODEL";

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl LlmConfig {
    /// Load from environment. Returns None if base URL or API key missing.
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var(ENV_BASE_URL).ok()?.trim().to_string();
        let api_key = std::env::var(ENV_API_KEY).ok()?.trim().to_string();
        if base_url.is_empty() || api_key.is_empty() {
            return None;
        }
        let model = std::env::var(ENV_MODEL)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "deepseek-chat".into());
        // normalize: strip trailing /v1 or / so we can append /chat/completions
        let base_url = base_url.trim_end_matches('/').to_string();
        Some(Self {
            base_url,
            api_key,
            model,
        })
    }

    pub fn chat_url(&self) -> String {
        if self.base_url.ends_with("/v1") {
            format!("{}/chat/completions", self.base_url)
        } else {
            format!("{}/v1/chat/completions", self.base_url)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStatus {
    pub configured: bool,
    pub model: Option<String>,
    pub base_url_host: Option<String>,
    pub mode: String,
}

pub fn llm_status() -> LlmStatus {
    match LlmConfig::from_env() {
        Some(c) => {
            let host = url_host(&c.base_url);
            LlmStatus {
                configured: true,
                model: Some(c.model),
                base_url_host: host,
                mode: "openai_compatible".into(),
            }
        }
        None => LlmStatus {
            configured: false,
            model: None,
            base_url_host: None,
            mode: "rules_only".into(),
        },
    }
}

fn url_host(u: &str) -> Option<String> {
    u.strip_prefix("https://")
        .or_else(|| u.strip_prefix("http://"))
        .map(|rest| rest.split('/').next().unwrap_or(rest).to_string())
}

pub struct OpenAiCompatibleProvider {
    pub config: LlmConfig,
    client: reqwest::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new(config: LlmConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(90))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    pub fn try_from_env() -> Option<Self> {
        LlmConfig::from_env().map(Self::new)
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ApiMessage<'a>],
    temperature: f32,
}

#[derive(Serialize)]
struct ApiMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Option<Vec<Choice>>,
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct Choice {
    message: Option<ChoiceMessage>,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ApiError {
    message: Option<String>,
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    fn name(&self) -> &str {
        "openai-compatible"
    }

    async fn complete(&self, messages: &[ChatMessage]) -> Result<AgentReply> {
        let api_msgs: Vec<ApiMessage<'_>> = messages
            .iter()
            .map(|m| ApiMessage {
                role: m.role.as_str(),
                content: m.content.as_str(),
            })
            .collect();
        let body = ChatRequest {
            model: &self.config.model,
            messages: &api_msgs,
            temperature: 0.3,
        };
        let resp = self
            .client
            .post(self.config.chat_url())
            .bearer_auth(&self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| EasyMcError::msg(format!("LLM 请求失败: {e}")))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| EasyMcError::msg(format!("LLM 读响应失败: {e}")))?;
        if !status.is_success() {
            return Err(EasyMcError::msg(format!(
                "LLM HTTP {status}: {}",
                truncate(&text, 400)
            )));
        }
        let parsed: ChatResponse = serde_json::from_str(&text)
            .map_err(|e| EasyMcError::msg(format!("LLM JSON 解析失败: {e}; {}", truncate(&text, 200))))?;
        if let Some(err) = parsed.error {
            return Err(EasyMcError::msg(format!(
                "LLM 错误: {}",
                err.message.unwrap_or_else(|| "unknown".into())
            )));
        }
        let content = parsed
            .choices
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.message)
            .and_then(|m| m.content)
            .unwrap_or_else(|| "(空回复)".into());
        Ok(AgentReply {
            content: sanitize_reply(&content),
            proposed_changes: None,
        })
    }
}

fn sanitize_reply(s: &str) -> String {
    s.chars()
        .filter(|c| match *c {
            // strip common model trailer / zero-width junk
            '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{feff}' => false,
            c if ('\u{2062}'..='\u{2064}').contains(&c) => false,
            c if ('\u{e0000}'..='\u{e007f}').contains(&c) => false, // tags
            _ => true,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

/// System prompt for MC crash / launcher assistant (zh-CN).
pub fn system_prompt_zh() -> &'static str {
    "你是 EasyMC 本地启动器的 Minecraft 助手。用简体中文回答。\
重点帮助：崩溃日志分析、Java/内存/加载器/Mixin/OpenGL 问题、模组安装建议。\
不要编造不存在的 EasyMC 命令；可引用 ensure-java、install、search-mods、agent diagnose。\
若信息不足，先说明需要哪些日志。"
}


/// Best-effort load of `~/.config/easymc/llm.env` (KEY=VALUE lines) when env vars unset.
/// Does not override existing process env. Never logs secret values.
pub fn try_load_dotenv_llm() {
    if std::env::var(ENV_API_KEY).is_ok() && std::env::var(ENV_BASE_URL).is_ok() {
        return;
    }
    let candidates: Vec<PathBuf> = if let Some(p) = std::env::var_os("EASYMC_LLM_ENV_FILE") {
        // Explicit override: only this file (missing => do not fall back).
        vec![PathBuf::from(p)]
    } else {
        dirs_config_llm().into_iter().collect()
    };
    for path in candidates {
        if !path.is_file() {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else { continue };
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            let k = k.trim();
            let v = v.trim().trim_matches('"').trim_matches('\'');
            if matches!(k, ENV_BASE_URL | ENV_API_KEY | ENV_MODEL) {
                if std::env::var_os(k).is_none() {
                    std::env::set_var(k, v);
                }
            }
        }
        break;
    }
}

fn dirs_config_llm() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(std::path::PathBuf::from(home).join(".config/easymc/llm.env"))
}
