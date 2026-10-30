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
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<&'a str>,
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
    #[serde(default)]
    tool_calls: Vec<ApiToolCall>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiToolCall {
    #[serde(default)]
    #[allow(dead_code)]
    id: String,
    #[serde(default)]
    #[serde(rename = "type")]
    #[allow(dead_code)]
    kind: String,
    function: Option<ApiToolFn>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiToolFn {
    name: String,
    #[serde(default)]
    arguments: String,
}

/// One tool the cloud model asked to run (host maps to local Intent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct LlmCompletion {
    pub content: String,
    pub tool_calls: Vec<LlmToolCall>,
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
        let c = self.complete_raw(messages, false).await?;
        Ok(AgentReply {
            content: c.content,
            proposed_changes: None,
        })
    }
}

impl OpenAiCompatibleProvider {
    /// Chat completion; when `with_tools` is true, advertise EasyMC tool schemas.
    pub async fn complete_raw(
        &self,
        messages: &[ChatMessage],
        with_tools: bool,
    ) -> Result<LlmCompletion> {
        let api_msgs: Vec<ApiMessage<'_>> = messages
            .iter()
            .map(|m| ApiMessage {
                role: m.role.as_str(),
                content: m.content.as_str(),
            })
            .collect();
        let tools = cloud_tool_definitions();
        let body = ChatRequest {
            model: &self.config.model,
            messages: &api_msgs,
            temperature: 0.3,
            tools: if with_tools { Some(&tools) } else { None },
            tool_choice: if with_tools { Some("auto") } else { None },
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
        let parsed: ChatResponse = serde_json::from_str(&text).map_err(|e| {
            EasyMcError::msg(format!(
                "LLM JSON 解析失败: {e}; {}",
                truncate(&text, 200)
            ))
        })?;
        if let Some(err) = parsed.error {
            return Err(EasyMcError::msg(format!(
                "LLM 错误: {}",
                err.message.unwrap_or_else(|| "unknown".into())
            )));
        }
        let message = parsed
            .choices
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.message);
        let mut tool_calls = Vec::new();
        let mut content = String::new();
        if let Some(m) = message {
            content = m.content.unwrap_or_default();
            for tc in m.tool_calls {
                if let Some(f) = tc.function {
                    let arguments: serde_json::Value = serde_json::from_str(&f.arguments)
                        .unwrap_or_else(|_| serde_json::json!({}));
                    tool_calls.push(LlmToolCall {
                        name: f.name,
                        arguments,
                    });
                }
            }
        }
        if content.trim().is_empty() && tool_calls.is_empty() {
            content = "(空回复)".into();
        }
        Ok(LlmCompletion {
            content: sanitize_reply(&content),
            tool_calls,
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
    "你是 EasyMC 本地启动器的 Minecraft 助手。用简体中文、玩家能听懂的话回答。\
重点：崩溃/启动日志、Java、内存、OpenAL、模组冲突、缺失 natives、OOM。\
EasyMC 已内置这些工具（玩家可直接说，或你引导他们说原话，系统会自动执行）：\
· 「查看崩溃」/ analyze_crash_log — 分析最近崩溃或启动日志；\
· 「搜模组 <关键词>」/ search_mods — Modrinth（有 Key 时含 CurseForge）搜索，不自动安装；\
· 「装 <模组名>」/ install_mod — 搜索并装进当前版本；\
· 「已装模组」/ list_installed_mods — 列出当前版本 mods；\
· 「推荐模组 科技|魔法|生存|性能」/ suggest_mods — 按主题推荐互补模组；\
· 「制作整合包」/ create_modpack — 从当前版本导出 Modrinth `.mrpack` 整合包；\
· 「导入整合包 <路径>」/ import_mrpack — 把本地 `.mrpack` 装成新版本；\
· 「禁用/启用 <模组>」/ toggle_mod — 启用或禁用模组；\
· 「版本列表」/ list_instances — 列出已装版本；\
· 「切换版本 <名>」/ select_instance — 切换当前版本；\
· 「重命名版本 <新名>」/ rename_instance — 改当前版本显示名；\
· 「复制版本」/ duplicate_instance — 复制当前版本（模组等，不含存档）；\
· 「删除版本 <名>」/ delete_instance — 永久删除版本及其游戏文件夹；\
· 「删除模组 <关键词>」/ delete_mod — 从当前版本永久删除模组文件；\
· 「打开日志」/ open_latest_log — 定位 latest.log 或崩溃报告；\
· 「打开模组文件夹」/「打开资源包」/「打开光影」/「打开存档」/「打开配置文件夹」/「打开日志文件夹」/ open_game_folder — 定位常见游戏文件夹；\
· 「账户列表」/ list_accounts — 列出已保存账户（不含令牌）；\
· 「一键环境」「优化配置」「修好 Java」「内存 4G」「切换 BMCLAPI 加速」。\
不要编造不存在的启动器命令；不要提 MCBBS。信息不足时先问需要哪份 latest.log / crash-report。"
}

/// OpenAI-compatible tool definitions the cloud model may call (host still executes).
pub fn cloud_tool_definitions() -> serde_json::Value {
    serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "analyze_crash_log",
                "description": "分析当前版本最新崩溃/启动日志，或玩家粘贴的日志路径。用玩家语言说明根因与修法。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "log_path": { "type": "string", "description": "可选：日志或 crash-report 文件路径" }
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "search_mods",
                "description": "按关键词搜索模组（Modrinth，有 CurseForge Key 时一并搜），返回结构化列表；不自动安装。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" },
                        "game_version": { "type": "string" },
                        "loader": { "type": "string", "description": "fabric/forge/neoforge/quilt" }
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "install_mod",
                "description": "搜索并安装模组到当前选中版本。",
                "parameters": {
                    "type": "object",
                    "properties": { "query": { "type": "string" } },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_installed_mods",
                "description": "列出当前版本已安装的模组 jar。",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "suggest_mods",
                "description": "按主题（survival/tech/magic/performance/qos）推荐互补热门模组。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "theme": { "type": "string" }
                    },
                    "required": ["theme"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "create_modpack",
                "description": "从当前版本导出 Modrinth .mrpack 整合包（可被 Prism / Modrinth App 打开）。",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "toggle_mod",
                "description": "按文件名关键词启用或禁用当前版本的模组（.jar ↔ .jar.disabled）。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "模组文件名关键词" },
                        "enable": { "type": "boolean", "description": "true=启用，false=禁用；省略则切换" }
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_instances",
                "description": "列出已安装的游戏版本。",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "import_mrpack",
                "description": "导入本地 Modrinth .mrpack（或 EasyMC 分享包）为新版本。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": ".mrpack / .zip / .json 路径" }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "select_instance",
                "description": "按名称、版本号或加载器关键词切换当前游戏版本。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "版本名 / MC 版本号 / 加载器关键词" }
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "rename_instance",
                "description": "修改版本的显示名称（默认当前版本；可带 query 指定）。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "new_name": { "type": "string", "description": "新的显示名" },
                        "query": { "type": "string", "description": "可选：要改名的版本关键词" }
                    },
                    "required": ["new_name"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "duplicate_instance",
                "description": "复制版本（模组/资源包/光影/配置/数据包；不含存档）。默认当前版本。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "new_name": { "type": "string", "description": "可选：副本显示名" },
                        "query": { "type": "string", "description": "可选：要复制的版本关键词" }
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "delete_instance",
                "description": "永久删除版本及其游戏文件夹（不可恢复）。须指定 query（版本名关键词，或「当前」）。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "版本关键词，或「当前」表示当前版本" }
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "delete_mod",
                "description": "从当前版本永久删除模组 jar（不可恢复；想先留着请用 toggle_mod 禁用）。关键词匹配文件名。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "模组文件名关键词，如 sodium" }
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "open_latest_log",
                "description": "定位当前版本的 latest.log 或最新崩溃报告文件路径，便于玩家打开或继续诊断。",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_accounts",
                "description": "列出已保存的 Minecraft 账户（正版/离线），不含访问令牌。",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "select_account",
                "description": "按用户名或账户 id 切换当前 Minecraft 账户（不含令牌）。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "用户名 / 账户 id / 正版|离线" }
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "open_screenshots",
                "description": "定位（必要时创建）当前版本的 screenshots 截图文件夹。",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "open_game_folder",
                "description": "定位（必要时创建）当前版本的 mods / resourcepacks / shaderpacks / saves / config / logs / crash-reports 文件夹。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "kind": {
                            "type": "string",
                            "description": "mods | resourcepacks | shaderpacks | saves | config | logs | crash-reports"
                        }
                    },
                    "required": ["kind"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "ensure_java",
                "description": "检查并准备合适的 Java。",
                "parameters": {
                    "type": "object",
                    "properties": { "major": { "type": "integer" } }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "diagnose",
                "description": "同 analyze_crash_log：本地规则诊断崩溃。",
                "parameters": {
                    "type": "object",
                    "properties": { "log_path": { "type": "string" } }
                }
            }
        }
    ])
}

/// Map a cloud tool name + JSON args into a player utterance the local router understands.
pub fn tool_call_to_utterance(name: &str, args: &serde_json::Value) -> String {
    match name {
        "analyze_crash_log" | "diagnose" | "analyze_launch_log" => {
            if let Some(p) = args
                .get("log_path")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
            {
                format!("查看崩溃 {p}")
            } else {
                "查看崩溃".into()
            }
        }
        "search_mods" => {
            let q = args
                .get("query")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim();
            if q.is_empty() {
                "搜模组".into()
            } else {
                format!("搜模组 {q}")
            }
        }
        "install_mod" => {
            let q = args
                .get("query")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim();
            if q.is_empty() {
                "装模组".into()
            } else {
                format!("装 {q}")
            }
        }
        "list_installed_mods" | "check_mods" => "已装模组".into(),
        "suggest_mods" => {
            let th = args
                .get("theme")
                .and_then(|x| x.as_str())
                .unwrap_or("survival");
            let zh = match th {
                "tech" => "科技",
                "magic" => "魔法",
                "performance" => "性能",
                "qos" => "生活质量",
                "adventure" => "冒险",
                "building" => "建筑",
                "shader" => "光影",
                _ => "生存",
            };
            format!("推荐模组 {zh}")
        }
        "create_modpack" | "build_pack" | "export_modpack" => "制作整合包".into(),
        "import_mrpack" | "import_modpack" => {
            let path = args
                .get("path")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim();
            if path.is_empty() {
                "导入整合包".into()
            } else {
                format!("导入整合包 {path}")
            }
        }
        "toggle_mod" | "enable_mod" | "disable_mod" => {
            let q = args
                .get("query")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim();
            let enable = args.get("enable").and_then(|x| x.as_bool());
            let verb = match enable {
                Some(true) => "启用",
                Some(false) => "禁用",
                None => "切换模组",
            };
            if q.is_empty() {
                format!("{verb}")
            } else if enable.is_none() {
                format!("切换模组 {q}")
            } else {
                format!("{verb} {q}")
            }
        }
        "list_instances" | "list_versions" => "版本列表".into(),
        "select_instance" | "switch_instance" | "switch_version" => {
            let q = args
                .get("query")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim();
            if q.is_empty() {
                "版本列表".into()
            } else {
                format!("切换版本 {q}")
            }
        }
        "rename_instance" | "rename_version" => {
            let name = args
                .get("new_name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim();
            if name.is_empty() {
                "重命名版本".into()
            } else {
                format!("重命名版本 {name}")
            }
        }
        "duplicate_instance" | "duplicate_version" | "copy_instance" => {
            let name = args
                .get("new_name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim();
            if name.is_empty() {
                "复制版本".into()
            } else {
                format!("复制版本 {name}")
            }
        }
        "delete_instance" | "delete_version" | "remove_instance" => {
            let q = args
                .get("query")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim();
            if q.is_empty() {
                "删除版本".into()
            } else {
                format!("删除版本 {q}")
            }
        }
        "delete_mod" | "remove_mod" | "uninstall_mod" => {
            let q = args
                .get("query")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim();
            if q.is_empty() {
                "删除模组".into()
            } else {
                format!("删除模组 {q}")
            }
        }
        "open_latest_log" | "open_log" => "打开日志".into(),
        "list_accounts" => "账户列表".into(),
        "select_account" | "switch_account" => {
            let q = args
                .get("query")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim();
            if q.is_empty() {
                "账户列表".into()
            } else {
                format!("切换账户 {q}")
            }
        }
        "open_screenshots" | "screenshots" => "打开截图".into(),
        "open_game_folder" | "open_mods" | "open_resourcepacks" | "open_shaderpacks"
        | "open_saves" | "open_config" | "open_logs" | "open_crash-reports" | "open_crash_reports" => {
            let kind = args
                .get("kind")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            match kind.as_str() {
                "mods" | "mod" | "模组" => "打开模组文件夹".into(),
                "resourcepacks" | "resourcepack" | "资源包" | "材质" | "材质包" => {
                    "打开资源包".into()
                }
                "shaderpacks" | "shaderpack" | "shader" | "光影" | "光影包" => "打开光影".into(),
                "saves" | "save" | "worlds" | "存档" | "世界" => "打开存档".into(),
                "config" | "配置" => "打开配置文件夹".into(),
                "logs" | "log" | "日志" => "打开日志文件夹".into(),
                "crash-reports" | "crash_reports" | "crash" | "崩溃报告" => "打开崩溃文件夹".into(),
                _ if name == "open_mods" => "打开模组文件夹".into(),
                _ if name == "open_resourcepacks" => "打开资源包".into(),
                _ if name == "open_shaderpacks" => "打开光影".into(),
                _ if name == "open_saves" => "打开存档".into(),
                _ if name == "open_config" => "打开配置文件夹".into(),
                _ if name == "open_logs" => "打开日志文件夹".into(),
                _ => "打开模组文件夹".into(),
            }
        }
        "ensure_java" => {
            let major = args.get("major").and_then(|x| x.as_u64()).unwrap_or(21);
            format!("确保 Java {major}")
        }
        "setup_env" => "一键环境".into(),
        "propose_changes" | "tune_config" => "优化配置".into(),
        other => other.to_string(),
    }
}

/// Best-effort load of `~/.config/easymc/llm.env` (KEY=VALUE lines) when env vars unset.
/// Does not override existing process env unless `force` is true. Never logs secret values.
pub fn try_load_dotenv_llm() {
    load_dotenv_llm(false);
}

fn load_dotenv_llm(force: bool) {
    if !force && std::env::var(ENV_API_KEY).is_ok() && std::env::var(ENV_BASE_URL).is_ok() {
        return;
    }
    let candidates: Vec<PathBuf> = if let Some(p) = std::env::var_os("EASYMC_LLM_ENV_FILE") {
        // Explicit override: only this file (missing => do not fall back).
        vec![PathBuf::from(p)]
    } else {
        llm_env_path().into_iter().collect()
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
                if force || std::env::var_os(k).is_none() {
                    std::env::set_var(k, v);
                }
            }
        }
        break;
    }
}

/// Path to the on-disk LLM config (`~/.config/easymc/llm.env` or `%USERPROFILE%\.config\easymc\llm.env`).
pub fn llm_env_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("EASYMC_LLM_ENV_FILE") {
        return Some(PathBuf::from(p));
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".config").join("easymc").join("llm.env"))
}

/// Safe view of LLM settings for the UI (never returns the raw API key).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfigView {
    pub path: Option<String>,
    pub base_url: String,
    pub model: String,
    pub api_key_set: bool,
    pub api_key_hint: String,
    pub configured: bool,
}

/// Read current LLM config for the form (file + process env).
pub fn read_llm_config() -> LlmConfigView {
    load_dotenv_llm(false);
    let path = llm_env_path().map(|p| p.display().to_string());
    let base_url = std::env::var(ENV_BASE_URL).unwrap_or_default();
    let model = std::env::var(ENV_MODEL).unwrap_or_else(|_| "deepseek-chat".into());
    let key = std::env::var(ENV_API_KEY).unwrap_or_default();
    let api_key_set = !key.trim().is_empty();
    let api_key_hint = if api_key_set {
        mask_secret(&key)
    } else {
        String::new()
    };
    let configured = LlmConfig::from_env().is_some();
    LlmConfigView {
        path,
        base_url,
        model,
        api_key_set,
        api_key_hint,
        configured,
    }
}

fn mask_secret(s: &str) -> String {
    let t = s.trim();
    if t.len() <= 8 {
        "••••••••".into()
    } else {
        format!("{}…{}", &t[..4], &t[t.len().saturating_sub(4)..])
    }
}

fn escape_env_value(v: &str) -> String {
    if v.chars().any(|c| c.is_whitespace() || matches!(c, '"' | '\'' | '#')) {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        v.to_string()
    }
}

/// Persist Base URL / API Key / Model to `llm.env` and apply to this process.
/// Empty `api_key` keeps the existing key when one is already set.
pub fn save_llm_config(base_url: &str, api_key: &str, model: &str) -> Result<LlmStatus> {
    let path = llm_env_path().ok_or_else(|| EasyMcError::msg("无法定位用户配置目录"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| EasyMcError::msg(format!("创建配置目录失败: {e}")))?;
    }
    let base = base_url.trim().trim_end_matches('/');
    let mut key = api_key.trim().to_string();
    if key.is_empty() {
        key = std::env::var(ENV_API_KEY).unwrap_or_default().trim().to_string();
        if key.is_empty() {
            if let Ok(raw) = std::fs::read_to_string(&path) {
                for line in raw.lines() {
                    let line = line.trim();
                    if let Some((k, v)) = line.split_once('=') {
                        if k.trim() == ENV_API_KEY {
                            key = v.trim().trim_matches('"').trim_matches('\'').to_string();
                        }
                    }
                }
            }
        }
    }
    let model = model.trim();
    if base.is_empty() || key.is_empty() {
        return Err(EasyMcError::msg("请填写 Base URL 和 API Key"));
    }
    let model_out = if model.is_empty() { "deepseek-chat" } else { model };
    let body = format!(
        "# EasyMC Agent LLM (OpenAI-compatible)\n# Do not commit this file.\n{}={}\n{}={}\n{}={}\n",
        ENV_BASE_URL,
        escape_env_value(base),
        ENV_API_KEY,
        escape_env_value(&key),
        ENV_MODEL,
        escape_env_value(model_out),
    );
    std::fs::write(&path, body).map_err(|e| EasyMcError::msg(format!("写入 llm.env 失败: {e}")))?;
    std::env::set_var(ENV_BASE_URL, base);
    std::env::set_var(ENV_API_KEY, &key);
    std::env::set_var(ENV_MODEL, model_out);
    Ok(llm_status())
}

/// Clear LLM file + process env (falls back to rules-only assistant).
pub fn clear_llm_config() -> Result<LlmStatus> {
    if let Some(path) = llm_env_path() {
        if path.is_file() {
            std::fs::remove_file(&path)
                .map_err(|e| EasyMcError::msg(format!("删除 llm.env 失败: {e}")))?;
        }
    }
    std::env::remove_var(ENV_BASE_URL);
    std::env::remove_var(ENV_API_KEY);
    std::env::remove_var(ENV_MODEL);
    Ok(llm_status())
}
