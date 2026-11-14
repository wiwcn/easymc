//! Multi-step LLM + tool agent loop for EasyMC.
//!
//! Design adapted from **earendil-works/pi** (MIT):
//! <https://github.com/earendil-works/pi> — `packages/agent/src/agent-loop.ts`
//!
//! `max_turns` / stop-reason budget also follows **OpenAI Agents SDK** (MIT)
//! Runner-style run limits: <https://github.com/openai/openai-agents-python>
//!
//! Dangerous-tool propose/confirm gates + 1–3 skill inject follow dsh-style
//! guard/skill patterns (see `skills.rs`, `NOTICE`, `docs/AGENT-LOOP.md`).
//!
//! MVP: complete_raw → execute ALL tool_calls → append role=tool results → call again,
//! until no tools / final text / max_turns / error. Dangerous tools are gated
//! (propose/confirm); readonly tools auto-run. Does not vendor pi / Agents runtimes.

use easymc_core::{Paths, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::diagnose::DiagnoseReport;
use async_trait::async_trait;

use crate::llm::{tool_call_to_utterance, LlmCompletion, LlmToolCall, LlmUsage, OpenAiCompatibleProvider};
use crate::propose::{build_tune_proposal, proposal_to_action};
use crate::tool::{tool_call_from_action, ToolCall, ToolStatus, ToolUiAction};
use crate::tool_runtime::{self, ToolContext, ToolExecResult};
use crate::{ActionResult, ChatMessage};

/// Default max model turns (each complete_raw counts as one).
///
/// Kept deliberately high so dependency / diagnose multi-step work is not cut
/// off by a demo-sized budget. Override with `EASYMC_AGENT_MAX_TURNS` (>0).
pub const DEFAULT_MAX_TURNS: u32 = 64;

/// Env override for [`DEFAULT_MAX_TURNS`] (positive integer).
pub const ENV_MAX_TURNS: &str = "EASYMC_AGENT_MAX_TURNS";

/// Resolve max turns: `EASYMC_AGENT_MAX_TURNS` if set and >0, else default.
pub fn resolve_max_turns() -> u32 {
    std::env::var(ENV_MAX_TURNS)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_MAX_TURNS)
}

/// One progressive step the Agent chat UI shows while the loop runs.
///
/// Emitted live via Tauri `agent-step` (and also returned on [`AgentLoopResult::steps`] /
/// [`crate::AskResponse::steps`]) so players see look → do → verify, not only a final「好了。」.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentStepEvent {
    /// 1-based index in the current ask.
    pub index: u32,
    /// `thinking` | `tool_start` | `tool_done` | `reply`
    pub phase: String,
    /// One short Chinese line, e.g. 「正在检查模组依赖…」.
    pub summary_zh: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_status: Option<String>,
}

impl AgentStepEvent {
    pub fn thinking(index: u32) -> Self {
        Self {
            index,
            phase: "thinking".into(),
            summary_zh: "正在思考下一步…".into(),
            tool_name: None,
            tool_status: None,
        }
    }

    pub fn tool_start(index: u32, tool_name: &str) -> Self {
        Self {
            index,
            phase: "tool_start".into(),
            summary_zh: tool_start_summary_zh(tool_name),
            tool_name: Some(tool_name.into()),
            tool_status: Some("running".into()),
        }
    }

    pub fn tool_done(index: u32, tool_name: &str, card: &ToolCall) -> Self {
        let status = match card.status {
            ToolStatus::Ok => "ok",
            ToolStatus::Error => "error",
            ToolStatus::Pending => "pending",
            ToolStatus::Running => "running",
        };
        Self {
            index,
            phase: "tool_done".into(),
            summary_zh: tool_done_summary_zh(tool_name, card),
            tool_name: Some(tool_name.into()),
            tool_status: Some(status.into()),
        }
    }

    pub fn reply(index: u32, reply_zh: &str) -> Self {
        let line = first_line_trimmed(reply_zh, 48);
        let summary_zh = if line.is_empty() || line == "(空回复)" {
            "整理好了答复。".into()
        } else {
            line
        };
        Self {
            index,
            phase: "reply".into(),
            summary_zh,
            tool_name: None,
            tool_status: None,
        }
    }
}

fn first_line_trimmed(s: &str, max_chars: usize) -> String {
    let line = s
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string();
    truncate(&line, max_chars)
}

/// Player-facing one-liner when a tool begins.
pub fn tool_start_summary_zh(name: &str) -> String {
    match name {
        "check_mod_deps" | "mod_dep_graph" | "analyze_mod_deps" => "正在检查模组依赖…".into(),
        "fetch_missing_mod_deps" | "install_missing_deps" => "正在补全缺失依赖…".into(),
        "check_mod_updates" => "正在检查模组更新…".into(),
        "update_mods" | "batch_update_mods" => "正在更新模组…".into(),
        "list_installed_mods" | "check_mods" => "正在查看已装模组…".into(),
        "install_mod" => "正在安装模组…".into(),
        "search_mods" => "正在搜索模组…".into(),
        "suggest_mods" => "正在推荐模组…".into(),
        "list_instances" | "list_versions" => "正在列出版本…".into(),
        "diagnose" | "analyze_crash_log" | "analyze_launch_log" => "正在查看崩溃日志…".into(),
        "ensure_java" => "正在准备 Java…".into(),
        "setup_env" => "正在一键准备环境…".into(),
        "propose_changes" | "bump_memory" | "suggest_memory" => "正在生成配置建议…".into(),
        "build_pack" | "create_modpack" | "export_modpack" => "正在制作整合包…".into(),
        "import_mrpack" | "import_modpack" => "正在导入整合包…".into(),
        "diagnostics" => "正在导出问题报告…".into(),
        "delete_instance" | "delete_version" | "remove_instance" => "准备删除版本（需确认）…".into(),
        "delete_mod" | "remove_mod" | "uninstall_mod" => "准备删除模组（需确认）…".into(),
        "toggle_mod" | "enable_mod" | "disable_mod" => "准备启用/禁用模组（需确认）…".into(),
        other => {
            let (title, _) = crate::tool::tool_meta(other);
            format!("正在{title}…")
        }
    }
}

/// Player-facing one-liner when a tool finishes (ok / pending / error).
pub fn tool_done_summary_zh(name: &str, card: &ToolCall) -> String {
    let result = card.result_zh.as_deref().unwrap_or("").trim();
    let first = first_line_trimmed(result, 56);

    match card.status {
        ToolStatus::Error => {
            if first.is_empty() {
                let (title, _) = crate::tool::tool_meta(name);
                format!("{title}没成功。")
            } else {
                first
            }
        }
        ToolStatus::Pending => {
            if first.contains("需确认") || !first.is_empty() {
                if first.is_empty() {
                    "需要你确认后才会改动。".into()
                } else {
                    truncate(&first, 56)
                }
            } else {
                let (title, _) = crate::tool::tool_meta(name);
                format!("{title}需你确认。")
            }
        }
        ToolStatus::Ok | ToolStatus::Running => {
            // Prefer structured payload for deps / install counts.
            if let Some(summary) = deps_done_summary(name, card) {
                return summary;
            }
            if !first.is_empty()
                && !first.starts_with("（")
                && first != "(空回复)"
            {
                return first;
            }
            let (title, _) = crate::tool::tool_meta(name);
            format!("已完成：{title}")
        }
    }
}

fn deps_done_summary(name: &str, card: &ToolCall) -> Option<String> {
    let payload = card.payload.as_ref()?;
    match name {
        "check_mod_deps" | "mod_dep_graph" | "analyze_mod_deps" => {
            if let Some(n) = payload
                .get("missing_ids")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
            {
                return Some(if n == 0 {
                    "依赖齐全，没有缺失。".into()
                } else {
                    format!("发现缺失 {n} 个，准备补全…")
                });
            }
            if let Some(s) = payload.get("summary_zh").and_then(|v| v.as_str()) {
                return Some(truncate(s.trim(), 56));
            }
            None
        }
        "fetch_missing_mod_deps" | "install_missing_deps" | "install_mod" => {
            if let Some(installed) = payload
                .get("installed")
                .and_then(|v| v.as_array())
                .or_else(|| payload.get("installed_ids").and_then(|v| v.as_array()))
            {
                if let Some(first) = installed.first().and_then(|x| x.as_str()) {
                    if installed.len() == 1 {
                        return Some(format!("已安装 {first}…"));
                    }
                    return Some(format!("已安装 {first} 等 {} 个…", installed.len()));
                }
            }
            if let Some(n) = payload.get("installed_count").and_then(|v| v.as_u64()) {
                return Some(format!("已补全 {n} 个依赖…"));
            }
            if let Some(s) = payload.get("summary_zh").and_then(|v| v.as_str()) {
                return Some(truncate(s.trim(), 56));
            }
            None
        }
        _ => None,
    }
}


/// Build a plain-Chinese「原因 / 方案」reply from tool cards.
///
/// Prefer structured payloads (missing deps, fill ok/fail, conflicts) over a bare「好了。」.
/// If `existing_reply` already contains both labels, it is kept as-is.
pub fn compose_reason_plan_zh(tools: &[ToolCall], existing_reply: &str) -> String {
    let existing = existing_reply.trim();
    if existing.contains("原因") && existing.contains("方案") {
        return existing.to_string();
    }

    let (reason, plan) = derive_reason_plan(tools);
    let thin = is_thin_reply(existing);

    if reason.is_none() && plan.is_none() {
        if !thin {
            return existing.to_string();
        }
        if tools.iter().any(|t| t.status == ToolStatus::Error) {
            return "原因：有步骤没做成。\n方案：展开上方工具卡看详情，或换个说法再试。".into();
        }
        if !tools.is_empty() {
            return "原因：已按你的请求跑完相关步骤。\n方案：需要细节可点开上方步骤摘要或工具卡。".into();
        }
        return existing.to_string();
    }

    let mut out = String::new();
    if let Some(r) = &reason {
        out.push_str("原因：");
        out.push_str(r);
    }
    if let Some(p) = &plan {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("方案：");
        out.push_str(p);
    }

    // Keep a substantial prose reply as an intro when it does not already explain 原因.
    if !thin && !existing.contains("原因") && existing.chars().count() > 40 {
        return format!("{existing}\n\n{out}");
    }
    out
}

fn is_thin_reply(s: &str) -> bool {
    let t = s.trim();
    t.is_empty()
        || t == "(空回复)"
        || t == "好了。"
        || t == "好了"
        || t == "这次没做成。"
        || t == "这次没做成"
        || t.starts_with("好了，步骤都跑完了")
        || t == "整理好了答复。"
}

fn derive_reason_plan(tools: &[ToolCall]) -> (Option<String>, Option<String>) {
    let mut reason_bits: Vec<String> = Vec::new();
    let mut plan_bits: Vec<String> = Vec::new();

    for card in tools {
        let name = card.name.as_str();
        let payload = card.payload.as_ref();

        match name {
            "check_mod_deps" | "mod_dep_graph" | "analyze_mod_deps" => {
                let missing = payload
                    .and_then(|p| p.get("missing_ids"))
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str())
                            .take(8)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let conflicts = payload
                    .and_then(|p| p.get("conflict_messages"))
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str())
                            .take(4)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if card.status == ToolStatus::Error {
                    reason_bits.push(
                        card.result_zh
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .unwrap_or("依赖检查没成功。")
                            .to_string(),
                    );
                    plan_bits.push("确认已选版本后再说「查依赖」。".into());
                } else if missing.is_empty() && conflicts.is_empty() {
                    reason_bits.push("模组必需依赖齐全，也没看出明显冲突。".into());
                    plan_bits.push("若游戏仍异常，把崩溃日志发我或说「诊断」。".into());
                } else {
                    if !missing.is_empty() {
                        let more = if missing.len() >= 8 { " 等" } else { "" };
                        reason_bits.push(format!(
                            "缺 {} 个必需依赖：{}{}",
                            payload
                                .and_then(|p| p.get("missing_ids"))
                                .and_then(|v| v.as_array())
                                .map(|a| a.len())
                                .unwrap_or(missing.len()),
                            missing.join("、"),
                            more
                        ));
                        plan_bits.push("已可自动从 Modrinth 补全能匹配到的项；也可以说「补全缺失依赖」。".into());
                    }
                    if !conflicts.is_empty() {
                        reason_bits.push(format!("存在冲突/版本不匹配：{}", conflicts.join("；")));
                        plan_bits.push("冲突项需手动调整模组版本或禁用冲突模组，无法一键下载解决。".into());
                    }
                }
            }
            "fetch_missing_mod_deps" | "install_missing_deps" => {
                let installed = payload
                    .and_then(|p| {
                        p.get("installed")
                            .or_else(|| p.get("installed_ids"))
                    })
                    .and_then(|v| v.as_array());
                let fails = payload
                    .and_then(|p| p.get("failed").or_else(|| p.get("failures")))
                    .and_then(|v| v.as_array());
                let skipped = payload
                    .and_then(|p| p.get("skipped"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let deferred = payload
                    .and_then(|p| p.get("deferred"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if card.status == ToolStatus::Error {
                    reason_bits.push(
                        card.result_zh
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .unwrap_or("补全依赖没成功。")
                            .to_string(),
                    );
                    plan_bits.push("检查网络/下载源后重试，或到下载页手动搜模组。".into());
                } else if skipped {
                    reason_bits.push("当前没有可自动补全的缺失依赖。".into());
                    plan_bits.push("若仍缺 jar，到下载页按模组名搜索安装。".into());
                } else if deferred {
                    let targets = payload
                        .and_then(|p| p.get("targets"))
                        .and_then(|v| v.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    reason_bits.push(format!("检测到约 {targets} 个可尝试补全的缺失依赖。"));
                    plan_bits.push("正在准备从 Modrinth 下载；完成后可再说「再查依赖」核对。".into());
                } else if let Some(arr) = installed {
                    let n = arr.len();
                    let names: Vec<_> = arr
                        .iter()
                        .filter_map(|x| {
                            x.as_str()
                                .or_else(|| x.get("mod_id").and_then(|v| v.as_str()))
                                .or_else(|| x.get("id").and_then(|v| v.as_str()))
                        })
                        .take(6)
                        .collect();
                    if n > 0 {
                        reason_bits.push(format!("此前缺依赖，已尝试补全 {n} 个。"));
                        if !names.is_empty() {
                            plan_bits.push(format!(
                                "已装上 {}{}；建议再说「再查依赖」确认是否还缺。",
                                names.join("、"),
                                if n > names.len() { " 等" } else { "" }
                            ));
                        } else {
                            plan_bits.push("建议再说「再查依赖」确认是否还缺。".into());
                        }
                    }
                    if let Some(f) = fails {
                        if !f.is_empty() {
                            reason_bits.push(format!("另有 {} 个未能自动下载。", f.len()));
                            plan_bits.push("失败项多半是别名、嵌套 jar，或 Modrinth 上没有对应工程，需要手动处理。".into());
                        }
                    }
                } else if let Some(n) = payload.and_then(|p| p.get("installed_count")).and_then(|v| v.as_u64()) {
                    reason_bits.push(format!("此前缺依赖，已补全 {n} 个。"));
                    plan_bits.push("建议再说「再查依赖」确认是否还缺。".into());
                } else if let Some(r) = card.result_zh.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    plan_bits.push(first_line_trimmed(r, 72));
                }
            }
            "diagnose" | "analyze_crash_log" | "analyze_launch_log" | "diagnostics" => {
                if let Some(s) = payload
                    .and_then(|p| p.get("summary_zh"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    reason_bits.push(truncate(s, 80));
                } else if let Some(r) = card.result_zh.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    reason_bits.push(first_line_trimmed(r, 72));
                }
                if card.status == ToolStatus::Ok {
                    plan_bits.push("按诊断卡里的修复建议操作；需要调内存可以说「内存调到 4G」。".into());
                }
            }
            _ => {
                match card.status {
                    ToolStatus::Error => {
                        let r = card
                            .result_zh
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(|s| first_line_trimmed(s, 72))
                            .unwrap_or_else(|| {
                                let (title, _) = crate::tool::tool_meta(name);
                                format!("{title}没成功。")
                            });
                        reason_bits.push(r);
                        plan_bits.push("展开工具卡看详情，或换个说法再试。".into());
                    }
                    ToolStatus::Pending => {
                        let (title, _) = crate::tool::tool_meta(name);
                        reason_bits.push(format!("{title}需要你确认后才会改动。"));
                        plan_bits.push("在工具卡上点确认/应用，或说「取消」。".into());
                    }
                    ToolStatus::Ok | ToolStatus::Running => {}
                }
            }
        }
    }

    // Dedup while preserving order
    fn dedup(bits: Vec<String>) -> Option<String> {
        let mut out: Vec<String> = Vec::new();
        for b in bits {
            let t = b.trim();
            if t.is_empty() {
                continue;
            }
            if out.iter().any(|x| x == t) {
                continue;
            }
            out.push(t.to_string());
        }
        if out.is_empty() {
            None
        } else {
            Some(out.join(" "))
        }
    }

    (dedup(reason_bits), dedup(plan_bits))
}

/// Abstraction over chat completions so the agent loop can be unit-tested
/// without a live OpenAI-compatible endpoint.
#[async_trait]
pub trait AgentCompleter: Send + Sync {
    async fn complete_raw(
        &self,
        messages: &[ChatMessage],
        with_tools: bool,
    ) -> Result<LlmCompletion>;
}

#[async_trait]
impl AgentCompleter for OpenAiCompatibleProvider {
    async fn complete_raw(
        &self,
        messages: &[ChatMessage],
        with_tools: bool,
    ) -> Result<LlmCompletion> {
        OpenAiCompatibleProvider::complete_raw(self, messages, with_tools).await
    }
}


/// Env override for the per-tool wall-clock limit (seconds).
pub const ENV_TOOL_TIMEOUT_SECS: &str = "EASYMC_AGENT_TOOL_TIMEOUT_SECS";

/// Default per-tool timeout. Downloads (mods / Java / packs) can legitimately
/// take minutes, but a hung network call must not freeze the whole chat.
pub const DEFAULT_TOOL_TIMEOUT_SECS: u64 = 300;

/// Env override for the model-context budget (approximate tokens).
pub const ENV_MAX_CONTEXT_TOKENS: &str = "EASYMC_AGENT_MAX_CONTEXT_TOKENS";

/// Keep the prompt under roughly 60k tokens (~180k chars) by default.
pub const DEFAULT_MAX_CONTEXT_TOKENS: usize = 60_000;

/// Resolve the tool timeout: `EASYMC_AGENT_TOOL_TIMEOUT_SECS` if set and >0.
pub fn resolve_tool_timeout() -> Duration {
    let secs = std::env::var(ENV_TOOL_TIMEOUT_SECS)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_TOOL_TIMEOUT_SECS);
    Duration::from_secs(secs)
}

/// Tools that must not auto-execute in the LLM loop (propose / confirm first).
pub fn is_dangerous_tool(name: &str) -> bool {
    matches!(
        name,
        "delete_instance"
            | "delete_version"
            | "remove_instance"
            | "delete_mod"
            | "remove_mod"
            | "uninstall_mod"
            | "toggle_mod"
            | "enable_mod"
            | "disable_mod"
            | "bump_memory"
            | "apply_changes"
            | "apply_proposal"
            | "apply_game_layer"
            | "discard_game_layer"
            | "modify_saves"
            | "write_saves"
    )
}

/// Read-only tools that may run in parallel within one model turn.
fn is_parallel_safe_tool(name: &str) -> bool {
    matches!(
        name,
        "search_mods"
            | "list_installed_mods"
            | "check_mod_deps"
            | "check_mod_updates"
            | "diagnose"
            | "list_instances"
            | "list_accounts"
            | "open_latest_log"
            | "open_screenshots"
            | "show_data_dir"
    )
}

#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    pub max_turns: u32,
    /// Cooperative cancellation from the UI (checked between turns / tools).
    pub cancel: Option<Arc<AtomicBool>>,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_turns: DEFAULT_MAX_TURNS,
            cancel: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLoopResult {
    pub reply_zh: String,
    pub tools: Vec<ToolCall>,
    pub report: Option<DiagnoseReport>,
    /// Last action for host side-effects (ensure_java / install_mod / …).
    pub action: Option<ActionResult>,
    /// Every action produced by tools this run, in execution order. Hosts use
    /// this to drive notifications; entries with `value.executed == true` have
    /// already been fully performed by the agent loop.
    #[serde(default)]
    pub actions: Vec<ActionResult>,
    pub turns_used: u32,
    /// `final_text` | `no_tools` | `max_turns` | `error` | `cancelled` | `no_progress`
    pub stop_reason: String,
    /// Ordered short Chinese step abstracts for the chat UI.
    #[serde(default)]
    pub steps: Vec<AgentStepEvent>,
    /// Wall-clock time spent in the loop.
    #[serde(default)]
    pub elapsed_ms: u64,
    /// Token usage summed over model turns (when the endpoint reports it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<LlmUsage>,
}

fn new_tool_call_id(turn: u32, idx: usize) -> String {
    format!("call_{turn}_{idx}")
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}

fn assistant_message_with_tools(content: &str, tool_calls: &[LlmToolCall]) -> ChatMessage {
    ChatMessage {
        role: "assistant".into(),
        content: content.to_string(),
        tool_call_id: None,
        name: None,
        tool_calls: tool_calls.to_vec(),
    }
}

fn tool_result_message(tool_call_id: &str, name: &str, content: &str) -> ChatMessage {
    ChatMessage {
        role: "tool".into(),
        content: content.to_string(),
        tool_call_id: Some(tool_call_id.to_string()),
        name: Some(name.to_string()),
        tool_calls: vec![],
    }
}

/// Gate a dangerous tool: return a pending card + text for the model (no side effect).
fn gate_dangerous(
    tc: &LlmToolCall,
    paths: &Paths,
    instance_id: Option<&str>,
) -> (String, ToolCall, Option<ActionResult>) {
    let name = tc.name.as_str();
    if matches!(name, "bump_memory" | "apply_changes" | "apply_proposal") {
        let mb = tc
            .arguments
            .get("mb")
            .or_else(|| tc.arguments.get("memory_mb"))
            .and_then(|x| x.as_u64())
            .map(|n| n as u32);
        let prop = build_tune_proposal(
            paths,
            instance_id,
            mb,
            "云端助手建议调整内存/启动参数。确认后点「应用更改」才会写入。",
        );
        let ar = proposal_to_action(
            prop,
            "已生成「建议更改」（需你确认后才会改内存/参数）。点卡片上的「应用更改」。",
        );
        let mut card = tool_call_from_action(&ar);
        card.status = ToolStatus::Pending;
        return (ar.message_zh.clone(), card, Some(ar));
    }

    let utterance = tool_call_to_utterance(name, &tc.arguments);
    let msg = if matches!(name, "apply_game_layer") {
        format!(
            "【需确认】「{utterance}」会把当前模组/配置等改动保存为可回退的干净状态（存档不动）。请在对话里明确说「确认应用隔离改动」，或到版本页点「应用更改」。"
        )
    } else if matches!(name, "discard_game_layer") {
        format!(
            "【需确认】「{utterance}」会丢弃未保存的隔离改动并恢复干净状态（存档不动）。请在对话里明确说「确认丢弃隔离改动」，或到版本页点「丢弃更改」。"
        )
    } else if matches!(name, "modify_saves" | "write_saves") {
        format!(
            "【需确认】「{utterance}」会改动存档。隔离环境下默认要先提出建议再写入。请明确说「确认修改存档 …」，或在设置关闭「存档修改需确认」。"
        )
    } else {
        format!(
            "【需确认】「{utterance}」会改动你的游戏文件，助手不会自动执行。点卡片上的「确认执行」，或在对话里明确说一次（例如「确认删除版本 xxx」）。"
        )
    };
    let (title, tag) = crate::tool::tool_meta(name);
    let can_confirm = matches!(name, "delete_instance" | "delete_mod" | "toggle_mod");
    let card = ToolCall {
        id: format!("gate-{}", tc.id),
        name: name.into(),
        title_zh: title.into(),
        status: ToolStatus::Pending,
        args: vec![crate::tool::ToolArg {
            key: "utterance".into(),
            label_zh: "待确认".into(),
            value: utterance,
        }],
        result_zh: Some(msg.clone()),
        path: None,
        payload: Some(tc.arguments.clone()),
        ui_actions: if can_confirm {
            vec![ToolUiAction {
                id: "confirm_tool".into(),
                label_zh: "确认执行".into(),
            }]
        } else {
            vec![]
        },
        tag_zh: tag.map(|s| s.into()),
    };
    (msg, card, None)
}

async fn run_one_tool(ctx: &ToolContext<'_>, tc: &LlmToolCall) -> ToolExecResult {
    if is_dangerous_tool(&tc.name) {
        let (msg, card, action) = gate_dangerous(tc, ctx.paths, ctx.instance_id.as_deref());
        return ToolExecResult {
            model_text: msg.clone(),
            message_zh: msg,
            card,
            action,
            report: None,
        };
    }

    let timeout = resolve_tool_timeout();
    match tokio::time::timeout(
        timeout,
        tool_runtime::execute(ctx, &tc.name, &tc.arguments),
    )
    .await
    {
        Ok(out) => out,
        Err(_) => tool_timeout_result(tc, timeout),
    }
}

/// Error card for a tool that exceeded the per-tool wall-clock budget.
fn tool_timeout_result(tc: &LlmToolCall, timeout: Duration) -> ToolExecResult {
    let secs = timeout.as_secs();
    let msg = format!(
        "「{}」超过 {secs} 秒还没完成，已中止。可能是网络或下载卡住了，稍后可以再试一次。",
        tc.name
    );
    let (title, tag) = crate::tool::tool_meta(&tc.name);
    ToolExecResult {
        model_text: msg.clone(),
        message_zh: msg.clone(),
        card: ToolCall {
            id: format!("tool-{}", tc.id),
            name: tc.name.clone(),
            title_zh: title.into(),
            status: ToolStatus::Error,
            args: vec![],
            result_zh: Some(msg),
            path: None,
            payload: Some(serde_json::json!({ "timeout_secs": secs })),
            ui_actions: vec![],
            tag_zh: tag.map(|s| s.into()),
        },
        action: None,
        report: None,
    }
}

/// Placeholder result when the model repeats the exact same tool call too often.
fn repeated_call_result(tc: &LlmToolCall, count: u32) -> ToolExecResult {
    let msg = format!(
        "同一工具（{}）用相同参数已调用 {count} 次，已跳过。请换一种方法，或直接给出结论。",
        tc.name
    );
    let (title, tag) = crate::tool::tool_meta(&tc.name);
    ToolExecResult {
        model_text: msg.clone(),
        message_zh: msg.clone(),
        card: ToolCall {
            id: format!("tool-{}", tc.id),
            name: tc.name.clone(),
            title_zh: title.into(),
            status: ToolStatus::Error,
            args: vec![],
            result_zh: Some(msg),
            path: None,
            payload: None,
            ui_actions: vec![],
            tag_zh: tag.map(|s| s.into()),
        },
        action: None,
        report: None,
    }
}

fn call_key(tc: &LlmToolCall) -> String {
    let name = tool_runtime::canonical_tool_name(&tc.name);
    format!("{name}:{}", tc.arguments)
}

/// Keep recent tool results intact; compact older ones so long sessions fit context.
fn compact_old_tool_results(messages: &mut [ChatMessage], keep_last: usize) {
    let tool_idx: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == "tool")
        .map(|(i, _)| i)
        .collect();
    if tool_idx.len() <= keep_last {
        return;
    }
    for i in tool_idx[..tool_idx.len() - keep_last].iter().copied() {
        let text = &mut messages[i].content;
        if text.chars().count() > 420 {
            let cut: String = text.chars().take(420).collect();
            *text = format!("{cut}…\n[较早的工具结果已压缩]");
        }
    }
}

/// Approximate token budget expressed in characters (CJK/JSON heavy mix).
fn context_budget_chars() -> usize {
    let tokens = std::env::var(ENV_MAX_CONTEXT_TOKENS)
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|&n| n >= 1_000)
        .unwrap_or(DEFAULT_MAX_CONTEXT_TOKENS);
    tokens.saturating_mul(3)
}

fn total_chars(messages: &[ChatMessage]) -> usize {
    messages.iter().map(|m| m.content.chars().count()).sum()
}

fn shrink(text: &mut String, max: usize, note: &str) {
    if text.chars().count() <= max {
        return;
    }
    let cut: String = text.chars().take(max).collect();
    *text = format!("{cut}…\n{note}");
}

/// Enforce a soft context budget between turns.
///
/// Tool/assistant bodies are compressed first; the system prompt and the last
/// user message are always preserved. We never delete messages because
/// `assistant(tool_calls)` / `tool` pairs must stay matched for OpenAI APIs.
fn enforce_context_budget(messages: &mut [ChatMessage], budget: usize) {
    if total_chars(messages) <= budget {
        return;
    }
    // Pass 1: shrink everything except the system prompt and final user turn.
    let last_user = messages.iter().rposition(|m| m.role == "user");
    for i in 0..messages.len() {
        if messages[i].role == "system" || Some(i) == last_user {
            continue;
        }
        {
            let m = &mut messages[i];
            match m.role.as_str() {
                "tool" => shrink(&mut m.content, 800, "[工具结果因上下文预算被压缩]"),
                "assistant" => shrink(&mut m.content, 1200, "[较早回复已压缩]"),
                "user" => shrink(&mut m.content, 800, "[较早消息已压缩]"),
                _ => {}
            }
        }
        if total_chars(messages) <= budget {
            return;
        }
    }
    // Pass 2: keep only the head of the most recent tool results.
    for i in (0..messages.len()).rev() {
        if messages[i].role == "tool" {
            shrink(
                &mut messages[i].content,
                300,
                "[工具结果因上下文预算被压缩]",
            );
        }
        if total_chars(messages) <= budget {
            return;
        }
    }
}

fn cancelled(config: &AgentLoopConfig) -> bool {
    config
        .cancel
        .as_ref()
        .map(|c| c.load(Ordering::Relaxed))
        .unwrap_or(false)
}

/// Retry transient model/network errors (not auth / bad-request).
async fn complete_with_retry(
    provider: &impl AgentCompleter,
    messages: &[ChatMessage],
    config: &AgentLoopConfig,
) -> Result<LlmCompletion> {
    let mut attempt = 0u32;
    loop {
        if cancelled(config) {
            return Err(easymc_core::EasyMcError::msg("已停止"));
        }
        match provider.complete_raw(messages, true).await {
            Ok(c) => return Ok(c),
            Err(e) => {
                let text = format!("{e}");
                let permanent = text.contains("HTTP 400")
                    || text.contains("HTTP 401")
                    || text.contains("HTTP 403")
                    || text.contains("HTTP 404");
                if permanent || attempt >= 2 {
                    return Err(e);
                }
                let wait = Duration::from_millis(300 * (1 << attempt));
                tokio::time::sleep(wait).await;
                attempt += 1;
            }
        }
    }
}

fn emit_step(
    steps: &mut Vec<AgentStepEvent>,
    on_step: Option<&(dyn Fn(AgentStepEvent) + Send + Sync)>,
    step: AgentStepEvent,
) {
    if let Some(cb) = on_step {
        cb(step.clone());
    }
    steps.push(step);
}

/// Run the multi-step agent loop (pi-style inner tool loop + max_turns).
pub async fn run_agent_loop(
    provider: &impl AgentCompleter,
    paths: &Paths,
    messages: Vec<ChatMessage>,
    process_logs: Option<Vec<String>>,
    instance_id: Option<String>,
    config: AgentLoopConfig,
) -> Result<AgentLoopResult> {
    run_agent_loop_with_progress(provider, paths, messages, process_logs, instance_id, config, None).await
}

/// Same as [`run_agent_loop`], with optional live step callbacks for the UI.
pub async fn run_agent_loop_with_progress(
    provider: &impl AgentCompleter,
    paths: &Paths,
    mut messages: Vec<ChatMessage>,
    process_logs: Option<Vec<String>>,
    instance_id: Option<String>,
    config: AgentLoopConfig,
    on_step: Option<&(dyn Fn(AgentStepEvent) + Send + Sync)>,
) -> Result<AgentLoopResult> {
    let mut tools_out: Vec<ToolCall> = Vec::new();
    let mut steps_out: Vec<AgentStepEvent> = Vec::new();
    let mut last_action: Option<ActionResult> = None;
    let mut actions_out: Vec<ActionResult> = Vec::new();
    let mut last_report: Option<DiagnoseReport> = None;
    let mut last_content = String::new();
    let mut turns_used = 0u32;
    let mut stop_reason = "final_text".to_string();
    let mut step_index = 0u32;
    let started = Instant::now();
    let mut usage: Option<LlmUsage> = None;
    let mut call_counts: HashMap<String, u32> = HashMap::new();
    let mut no_progress_turns = 0u32;
    let context_budget = context_budget_chars();

    let ctx = ToolContext {
        paths,
        instance_id: instance_id.clone(),
        process_logs: process_logs.clone(),
        confirmed: false,
        cancel: config.cancel.clone(),
    };

    for turn in 1..=config.max_turns {
        if cancelled(&config) {
            stop_reason = "cancelled".into();
            last_content = "已停止当前任务。".into();
            break;
        }
        turns_used = turn;
        step_index += 1;
        emit_step(&mut steps_out, on_step, AgentStepEvent::thinking(step_index));
        enforce_context_budget(&mut messages, context_budget);

        let completion: LlmCompletion = match complete_with_retry(provider, &messages, &config).await
        {
            Ok(c) => c,
            Err(e) => {
                if cancelled(&config) {
                    stop_reason = "cancelled".into();
                    last_content = "已停止当前任务。".into();
                    break;
                }
                if tools_out.is_empty() && last_content.is_empty() {
                    return Err(e);
                }
                stop_reason = "error".into();
                last_content = format!("（这一轮模型没响应：{e}。之前的工具结果都保留着。）");
                break;
            }
        };

        if let Some(u) = completion.usage.as_ref() {
            usage.get_or_insert_with(LlmUsage::default).add(u);
        }
        last_content = completion.content.clone();
        if completion.tool_calls.is_empty() {
            stop_reason = if turn == 1 { "no_tools" } else { "final_text" }.into();
            break;
        }

        // Ensure each tool call has a stable id for role=tool round-trip.
        let mut normalized: Vec<LlmToolCall> = Vec::with_capacity(completion.tool_calls.len());
        for (idx, mut tc) in completion.tool_calls.into_iter().enumerate() {
            if tc.id.trim().is_empty() {
                tc.id = new_tool_call_id(turn, idx);
            }
            normalized.push(tc);
        }

        messages.push(assistant_message_with_tools(
            &completion.content,
            &normalized,
        ));

        // Announce every tool start first so the UI shows the plan immediately.
        for tc in &normalized {
            step_index += 1;
            emit_step(
                &mut steps_out,
                on_step,
                AgentStepEvent::tool_start(step_index, &tc.name),
            );
        }

        let mut results: Vec<Option<ToolExecResult>> = (0..normalized.len()).map(|_| None).collect();
        let mut skipped: Vec<bool> = vec![false; normalized.len()];
        let mut to_run: Vec<usize> = Vec::new();
        for (i, tc) in normalized.iter().enumerate() {
            let key = call_key(tc);
            let seen = *call_counts.get(&key).unwrap_or(&0);
            if seen >= 2 {
                skipped[i] = true;
                results[i] = Some(repeated_call_result(tc, seen + 1));
            } else {
                call_counts.insert(key, seen + 1);
                to_run.push(i);
            }
        }

        // Read-only calls run concurrently; writes stay ordered.
        let (parallel_idx, sequential_idx): (Vec<usize>, Vec<usize>) = to_run
            .into_iter()
            .partition(|&i| {
                let n = tool_runtime::canonical_tool_name(&normalized[i].name);
                is_parallel_safe_tool(n) && !is_dangerous_tool(&normalized[i].name)
            });

        if !parallel_idx.is_empty() {
            let futures = parallel_idx
                .iter()
                .map(|&i| run_one_tool(&ctx, &normalized[i]));
            let outs = futures_util::future::join_all(futures).await;
            for (i, out) in parallel_idx.into_iter().zip(outs) {
                results[i] = Some(out);
            }
        }

        let mut cancelled_mid_turn = false;
        for i in sequential_idx {
            if cancelled(&config) {
                cancelled_mid_turn = true;
                break;
            }
            results[i] = Some(run_one_tool(&ctx, &normalized[i]).await);
        }

        let mut executed = 0usize;
        let mut repeated = 0usize;
        for (i, tc) in normalized.iter().enumerate() {
            let Some(out) = results[i].take() else {
                continue;
            };
            if skipped[i] {
                repeated += 1;
            } else {
                executed += 1;
            }
            if let Some(ar) = out.action.clone() {
                last_action = Some(ar.clone());
                actions_out.push(ar);
            }
            if out.report.is_some() {
                last_report = out.report;
            }
            step_index += 1;
            emit_step(
                &mut steps_out,
                on_step,
                AgentStepEvent::tool_done(step_index, &tc.name, &out.card),
            );
            tools_out.push(out.card);
            messages.push(tool_result_message(
                &tc.id,
                &tc.name,
                &truncate(&out.model_text, 8000),
            ));
        }
        compact_old_tool_results(&mut messages, 8);

        if cancelled_mid_turn || cancelled(&config) {
            stop_reason = "cancelled".into();
            if last_content.trim().is_empty() {
                last_content = "已停止当前任务。".into();
            }
            break;
        }

        if repeated > 0 && executed == 0 {
            no_progress_turns += 1;
        } else {
            no_progress_turns = 0;
        }
        if no_progress_turns >= 2 {
            stop_reason = "no_progress".into();
            break;
        }

        if turn == config.max_turns {
            stop_reason = "max_turns".into();
            break;
        }
    }

    if turns_used >= config.max_turns && stop_reason == "final_text" {
        stop_reason = "max_turns".into();
    }

    // If we stopped on max_turns after tools without a final assistant prose, ask once more
    // without requiring more tools — but only when under an extra soft cap and content empty.
    if stop_reason == "max_turns"
        && (last_content.trim().is_empty() || last_content.trim() == "(空回复)")
        && !tools_out.is_empty()
    {
        if let Ok(c) = provider.complete_raw(&messages, false).await {
            if let Some(u) = c.usage.as_ref() {
                usage.get_or_insert_with(LlmUsage::default).add(u);
            }
            if !c.content.trim().is_empty() && c.content.trim() != "(空回复)" {
                last_content = c.content;
            }
        }
    }

    if last_content.trim().is_empty() || last_content.trim() == "(空回复)" {
        if !tools_out.is_empty() {
            last_content = "好了，步骤都跑完了。详情见上方步骤摘要。".into();
        }
    }

    if stop_reason == "no_progress" {
        last_content.push_str("\n\n我连续几步都在重复同样的操作，先停在这里。你可以换个说法，或直接告诉我目标。");
    }

    // Prefer structured 原因/方案 over bare「好了。」 when tools ran.
    if !tools_out.is_empty() && stop_reason != "cancelled" {
        last_content = compose_reason_plan_zh(&tools_out, &last_content);
    }

    if !last_content.trim().is_empty() {
        step_index += 1;
        emit_step(
            &mut steps_out,
            on_step,
            AgentStepEvent::reply(step_index, &last_content),
        );
    }

    Ok(AgentLoopResult {
        reply_zh: last_content,
        tools: tools_out,
        report: last_report,
        action: last_action,
        actions: actions_out,
        turns_used,
        stop_reason,
        steps: steps_out,
        elapsed_ms: started.elapsed().as_millis() as u64,
        usage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use serde_json::json;

    struct ScriptedCompleter {
        /// Queue of completions returned for each `complete_raw` call (with or without tools).
        queue: Mutex<VecDeque<LlmCompletion>>,
        /// Snapshot of messages passed into each call (proves tool-result refeed).
        seen: Mutex<Vec<(bool, Vec<ChatMessage>)>>,
    }

    impl ScriptedCompleter {
        fn new(responses: Vec<LlmCompletion>) -> Self {
            Self {
                queue: Mutex::new(responses.into()),
                seen: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl AgentCompleter for ScriptedCompleter {
        async fn complete_raw(
            &self,
            messages: &[ChatMessage],
            with_tools: bool,
        ) -> Result<LlmCompletion> {
            self.seen
                .lock()
                .unwrap()
                .push((with_tools, messages.to_vec()));
            self.queue.lock().unwrap().pop_front().ok_or_else(|| {
                easymc_core::EasyMcError::msg("ScriptedCompleter: no more scripted completions")
            })
        }
    }

    fn tc(id: &str, name: &str, args: serde_json::Value) -> LlmToolCall {
        LlmToolCall {
            id: id.into(),
            name: name.into(),
            arguments: args,
        }
    }

    fn temp_paths() -> (tempfile_guard::TempRoot, Paths) {
        let root = tempfile_guard::TempRoot::new();
        let paths = Paths::from_root(root.path());
        paths.ensure().expect("ensure paths");
        (root, paths)
    }

    /// Minimal temp-dir guard without adding a crate dep.
    mod tempfile_guard {
        use std::path::{Path, PathBuf};
        pub struct TempRoot(PathBuf);
        impl TempRoot {
            pub fn new() -> Self {
                let p = std::env::temp_dir().join(format!(
                    "easymc-agent-loop-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
                std::fs::create_dir_all(&p).unwrap();
                Self(p)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }
        impl Drop for TempRoot {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn resolve_max_turns_defaults_high() {
        // Do not assert against a polluted process env; only the constant + parse path.
        assert!(DEFAULT_MAX_TURNS >= 32, "default must not be a tight demo cap");
        assert_eq!(ENV_MAX_TURNS, "EASYMC_AGENT_MAX_TURNS");
    }

    #[test]
    fn dangerous_set() {
        assert!(is_dangerous_tool("delete_instance"));
        assert!(is_dangerous_tool("toggle_mod"));
        assert!(is_dangerous_tool("bump_memory"));
        assert!(is_dangerous_tool("apply_game_layer"));
        assert!(is_dangerous_tool("discard_game_layer"));
        assert!(!is_dangerous_tool("analyze_crash_log"));
        assert!(!is_dangerous_tool("list_instances"));
    }

    #[tokio::test]
    async fn agent_loop_multi_turn_refeeds_tool_results_and_gates_dangerous() {
        let (_tmp, paths) = temp_paths();
        // Plant a fake instance folder that must NOT be deleted by the gated tool.
        let inst_dir = paths.instances_dir().join("keep-me").join(".minecraft");
        std::fs::create_dir_all(&inst_dir).unwrap();
        std::fs::write(inst_dir.join("marker.txt"), b"safe").unwrap();

        let script = ScriptedCompleter::new(vec![
            // Turn 1: safe list + dangerous delete in one batch
            LlmCompletion {
                content: "先看版本再删".into(),
                tool_calls: vec![
                    tc("c1", "list_instances", json!({})),
                    tc(
                        "c2",
                        "delete_instance",
                        json!({ "query": "keep-me", "name": "keep-me" }),
                    ),
                ],
                usage: None,
            },
            // Turn 2: model saw tool results; finishes with prose
            LlmCompletion {
                content: "版本列好了；删除需你确认，我不会自动删。".into(),
                tool_calls: vec![],
                usage: None,
            },
        ]);

        let result = run_agent_loop(
            &script,
            &paths,
            vec![ChatMessage {
                role: "user".into(),
                content: "看看版本然后删掉 keep-me".into(),
                tool_call_id: None,
                name: None,
                tool_calls: vec![],
            }],
            None,
            None,
            AgentLoopConfig {
                max_turns: 8,
                cancel: None,
            },
        )
        .await
        .expect("loop ok");

        assert_eq!(result.stop_reason, "final_text");
        assert_eq!(result.turns_used, 2);
        assert!(
            result.tools.len() >= 2,
            "expected list + gated delete cards, got {}",
            result.tools.len()
        );
        let delete_card = result
            .tools
            .iter()
            .find(|t| t.name == "delete_instance")
            .expect("delete card");
        assert_eq!(
            delete_card.status,
            ToolStatus::Pending,
            "dangerous tool must be pending, not executed"
        );
        assert!(
            delete_card
                .result_zh
                .as_deref()
                .unwrap_or("")
                .contains("需确认"),
            "gate message missing"
        );
        // Side effect must not have happened.
        assert!(
            inst_dir.join("marker.txt").is_file(),
            "gated delete_instance must not remove instance files"
        );

        let seen = script.seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "two model turns");
        assert!(seen[0].0, "turn1 with_tools");
        assert!(seen[1].0, "turn2 with_tools");
        // Prove tool results were refeed into the next completion.
        let turn2_msgs = &seen[1].1;
        let tool_msgs: Vec<_> = turn2_msgs.iter().filter(|m| m.role == "tool").collect();
        assert_eq!(
            tool_msgs.len(),
            2,
            "both tool results must be refeed as role=tool; msgs={:?}",
            turn2_msgs
                .iter()
                .map(|m| (&m.role, m.tool_call_id.as_deref(), m.name.as_deref()))
                .collect::<Vec<_>>()
        );
        assert!(tool_msgs.iter().any(|m| m.tool_call_id.as_deref() == Some("c1")));
        assert!(tool_msgs.iter().any(|m| m.tool_call_id.as_deref() == Some("c2")));
        assert!(
            tool_msgs
                .iter()
                .any(|m| m.name.as_deref() == Some("delete_instance")
                    && m.content.contains("需确认")),
            "dangerous gate text must be what the model sees on refeed"
        );
        // Assistant message with tool_calls must precede the tool results.
        let assistant_with_tools = turn2_msgs
            .iter()
            .find(|m| m.role == "assistant" && !m.tool_calls.is_empty())
            .expect("assistant+tool_calls in history");
        assert_eq!(assistant_with_tools.tool_calls.len(), 2);
    }

    #[tokio::test]
    async fn agent_loop_stops_at_max_turns() {
        let (_tmp, paths) = temp_paths();
        let script = ScriptedCompleter::new(vec![
            LlmCompletion {
                content: "t1".into(),
                tool_calls: vec![tc("a", "list_instances", json!({}))],
                usage: None,
            },
            LlmCompletion {
                content: "t2".into(),
                tool_calls: vec![tc("b", "list_instances", json!({}))],
                usage: None,
            },
            // Would be turn 3 if max_turns ignored — must not be consumed for tools path.
            // (max_turns finalizer may call once more with tools=false if content empty;
            // we keep non-empty content so this stays unused.)
            LlmCompletion {
                content: "should-not-run".into(),
                tool_calls: vec![],
                usage: None,
            },
        ]);

        let result = run_agent_loop(
            &script,
            &paths,
            vec![ChatMessage {
                role: "user".into(),
                content: "一直列版本".into(),
                tool_call_id: None,
                name: None,
                tool_calls: vec![],
            }],
            None,
            None,
            AgentLoopConfig {
                max_turns: 2,
                cancel: None,
            },
        )
        .await
        .expect("loop ok");

        assert_eq!(result.stop_reason, "max_turns");
        assert_eq!(result.turns_used, 2);
        assert_eq!(result.tools.len(), 2);
        let seen = script.seen.lock().unwrap();
        assert_eq!(
            seen.len(),
            2,
            "only max_turns completions; got {}",
            seen.len()
        );
        // Second turn's messages must include first turn's tool result (refeed under budget).
        let tool_on_t2 = seen[1]
            .1
            .iter()
            .filter(|m| m.role == "tool")
            .count();
        assert_eq!(tool_on_t2, 1);
    }

    #[test]
    fn tool_summaries_are_short_chinese() {
        let start = tool_start_summary_zh("check_mod_deps");
        assert!(start.contains("依赖"), "{start}");
        assert!(start.chars().count() <= 24, "{start}");

        let card = ToolCall {
            id: "t1".into(),
            name: "check_mod_deps".into(),
            title_zh: "模组依赖".into(),
            status: ToolStatus::Ok,
            args: vec![],
            result_zh: Some("缺 15 个".into()),
            path: None,
            payload: Some(json!({ "missing_ids": ["a","b","c"], "summary_zh": "缺 3 个" })),
            ui_actions: vec![],
            tag_zh: Some("依赖".into()),
        };
        let done = tool_done_summary_zh("check_mod_deps", &card);
        assert!(done.contains("3") || done.contains("缺失"), "{done}");
        assert!(done.chars().count() <= 40, "{done}");
    }

    #[tokio::test]
    async fn agent_loop_emits_progressive_steps() {
        let (_tmp, paths) = temp_paths();
        let script = ScriptedCompleter::new(vec![
            LlmCompletion {
                content: "".into(),
                tool_calls: vec![tc("c1", "list_instances", json!({}))],
                usage: None,
            },
            LlmCompletion {
                content: "版本列好了。".into(),
                tool_calls: vec![],
                usage: None,
            },
        ]);
        let recorded: std::sync::Mutex<Vec<AgentStepEvent>> = std::sync::Mutex::new(Vec::new());
        let result = run_agent_loop_with_progress(
            &script,
            &paths,
            vec![ChatMessage {
                role: "user".into(),
                content: "版本列表".into(),
                tool_call_id: None,
                name: None,
                tool_calls: vec![],
            }],
            None,
            None,
            AgentLoopConfig {
                max_turns: 8,
                cancel: None,
            },
            Some(&|ev| {
                recorded.lock().unwrap().push(ev);
            }),
        )
        .await
        .expect("loop ok");

        assert!(!result.steps.is_empty(), "steps on result");
        let live = recorded.lock().unwrap().clone();
        assert_eq!(live.len(), result.steps.len());
        assert!(live.iter().any(|s| s.phase == "thinking"));
        assert!(live.iter().any(|s| s.phase == "tool_start" && s.tool_name.as_deref() == Some("list_instances")));
        assert!(live.iter().any(|s| s.phase == "tool_done"));
        assert!(live.iter().any(|s| s.phase == "reply"));
        // Indices are 1-based and strictly increasing.
        let mut prev = 0u32;
        for s in &live {
            assert!(s.index > prev, "index order {:?}", live);
            prev = s.index;
            assert!(!s.summary_zh.is_empty());
            assert!(s.summary_zh.chars().count() <= 64, "{}", s.summary_zh);
        }
    }

    #[tokio::test]
    async fn cancellation_stops_before_any_model_call() {
        let (_tmp, paths) = temp_paths();
        let script = ScriptedCompleter::new(vec![]);
        let cancel = Arc::new(AtomicBool::new(true));
        let result = run_agent_loop(
            &script,
            &paths,
            vec![ChatMessage {
                role: "user".into(),
                content: "列版本".into(),
                tool_call_id: None,
                name: None,
                tool_calls: vec![],
            }],
            None,
            None,
            AgentLoopConfig {
                max_turns: 8,
                cancel: Some(cancel),
            },
        )
        .await
        .expect("loop ok");

        assert_eq!(result.stop_reason, "cancelled");
        assert_eq!(result.turns_used, 0);
        assert!(result.reply_zh.contains("停止"));
        assert!(script.seen.lock().unwrap().is_empty(), "model must not run");
    }

    #[tokio::test]
    async fn repeated_identical_calls_stop_with_no_progress() {
        let (_tmp, paths) = temp_paths();
        let call = || LlmCompletion {
            content: "再看一次".into(),
            tool_calls: vec![tc("same", "list_instances", json!({}))],
            usage: None,
        };
        let script = ScriptedCompleter::new(vec![
            call(),
            call(),
            call(),
            call(),
            LlmCompletion {
                content: "不该走到这里".into(),
                tool_calls: vec![],
                usage: None,
            },
        ]);
        let result = run_agent_loop(
            &script,
            &paths,
            vec![ChatMessage {
                role: "user".into(),
                content: "一直列版本".into(),
                tool_call_id: None,
                name: None,
                tool_calls: vec![],
            }],
            None,
            None,
            AgentLoopConfig {
                max_turns: 8,
                cancel: None,
            },
        )
        .await
        .expect("loop ok");

        assert_eq!(result.stop_reason, "no_progress");
        assert_eq!(result.turns_used, 4);
        assert_eq!(result.tools.len(), 4);
        assert_eq!(
            result
                .tools
                .iter()
                .filter(|t| t.status == ToolStatus::Error)
                .count(),
            2,
            "third and fourth identical calls must be skipped"
        );
        assert!(result.reply_zh.contains("重复"), "{}", result.reply_zh);
    }

    #[tokio::test]
    async fn usage_is_summed_across_turns() {
        let (_tmp, paths) = temp_paths();
        let mut first = LlmCompletion {
            content: "".into(),
            tool_calls: vec![tc("c1", "list_instances", json!({}))],
            usage: Some(LlmUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
        };
        let second = LlmCompletion {
            content: "好了。".into(),
            tool_calls: vec![],
            usage: Some(LlmUsage {
                prompt_tokens: 20,
                completion_tokens: 7,
                total_tokens: 27,
            }),
        };
        first.tool_calls.push(tc("c2", "list_accounts", json!({})));
        let script = ScriptedCompleter::new(vec![first, second]);
        let result = run_agent_loop(
            &script,
            &paths,
            vec![ChatMessage {
                role: "user".into(),
                content: "看看".into(),
                tool_call_id: None,
                name: None,
                tool_calls: vec![],
            }],
            None,
            None,
            AgentLoopConfig {
                max_turns: 4,
                cancel: None,
            },
        )
        .await
        .expect("loop ok");

        let usage = result.usage.expect("usage");
        assert_eq!(usage.prompt_tokens, 30);
        assert_eq!(usage.completion_tokens, 12);
        assert_eq!(usage.total_tokens, 42);
    }

    #[test]
    fn compact_keeps_recent_tool_results_intact() {
        let mut msgs: Vec<ChatMessage> = (0..10)
            .map(|i| ChatMessage {
                role: "tool".into(),
                content: format!("{}", "x".repeat(800 + i)),
                tool_call_id: Some(format!("c{i}")),
                name: Some("list_instances".into()),
                tool_calls: vec![],
            })
            .collect();
        compact_old_tool_results(&mut msgs, 3);
        for m in &msgs[..7] {
            assert!(m.content.chars().count() < 460, "old compacted");
            assert!(m.content.contains("已压缩"));
        }
        for m in &msgs[7..] {
            assert!(!m.content.contains("已压缩"), "recent stays intact");
        }
    }

    #[test]
    fn compose_reason_plan_from_deps_payload() {
        let check = ToolCall {
            id: "t1".into(),
            name: "check_mod_deps".into(),
            title_zh: "模组依赖".into(),
            status: ToolStatus::Ok,
            args: vec![],
            result_zh: Some("缺一堆".into()),
            path: None,
            payload: Some(json!({
                "missing_ids": ["fabric-api", "cloth-config"],
                "conflict_messages": [],
                "summary_zh": "缺 2 个"
            })),
            ui_actions: vec![],
            tag_zh: Some("依赖".into()),
        };
        let out = compose_reason_plan_zh(&[check], "好了。");
        assert!(out.contains("原因"), "{out}");
        assert!(out.contains("方案"), "{out}");
        assert!(out.contains("fabric-api") || out.contains("缺"), "{out}");
        assert!(!out.trim().eq("好了。"), "{out}");

        // Already labeled → keep
        let labeled = "原因：测试原因\n方案：测试方案";
        assert_eq!(compose_reason_plan_zh(&[], labeled), labeled);
    }

    #[test]
    fn compose_reason_plan_fetch_and_conflicts() {
        let fetch = ToolCall {
            id: "t2".into(),
            name: "fetch_missing_mod_deps".into(),
            title_zh: "补全依赖".into(),
            status: ToolStatus::Ok,
            args: vec![],
            result_zh: Some("装好了".into()),
            path: None,
            payload: Some(json!({
                "installed": ["fabric-api"],
                "failed": [{"id": "nope", "reason": "not_found"}]
            })),
            ui_actions: vec![],
            tag_zh: Some("依赖".into()),
        };
        let out = compose_reason_plan_zh(&[fetch], "");
        assert!(out.contains("原因"), "{out}");
        assert!(out.contains("方案"), "{out}");
        assert!(out.contains("1") || out.contains("补全") || out.contains("fabric"), "{out}");
    }

    #[test]
    fn context_budget_compacts_without_breaking_message_pairs() {
        let big = "字".repeat(20_000);
        let mut msgs = vec![
            ChatMessage {
                role: "system".into(),
                content: "系统提示".into(),
                tool_call_id: None,
                name: None,
                tool_calls: vec![],
            },
            ChatMessage {
                role: "user".into(),
                content: big.clone(),
                tool_call_id: None,
                name: None,
                tool_calls: vec![],
            },
            ChatMessage {
                role: "assistant".into(),
                content: big.clone(),
                tool_call_id: None,
                name: None,
                tool_calls: vec![],
            },
            ChatMessage {
                role: "tool".into(),
                content: big.clone(),
                tool_call_id: Some("c1".into()),
                name: Some("list_mods".into()),
                tool_calls: vec![],
            },
            ChatMessage {
                role: "user".into(),
                content: "最后一句".into(),
                tool_call_id: None,
                name: None,
                tool_calls: vec![],
            },
        ];
        let len_before = msgs.len();
        enforce_context_budget(&mut msgs, 5_000);
        assert_eq!(msgs.len(), len_before, "messages must not be dropped");
        assert!(total_chars(&msgs) < 60_000);
        assert_eq!(msgs[0].role, "system");
        assert_eq!(msgs.last().unwrap().content, "最后一句");
        assert!(msgs[3].content.contains("上下文预算"));
    }
}
