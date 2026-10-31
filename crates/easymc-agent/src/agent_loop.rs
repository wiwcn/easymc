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

use crate::diagnose::DiagnoseReport;
use crate::intent::route;
use async_trait::async_trait;

use crate::llm::{tool_call_to_utterance, LlmCompletion, LlmToolCall, OpenAiCompatibleProvider};
use crate::propose::{build_tune_proposal, proposal_to_action};
use crate::service::execute_intent;
use crate::tool::{tool_call_from_action, tool_call_from_diagnose, ToolCall, ToolStatus};
use crate::{ActionResult, ChatMessage};

/// Default max model turns (each complete_raw counts as one).
pub const DEFAULT_MAX_TURNS: u32 = 8;

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

/// Tools whose real work is finished by the Tauri/CLI host after `ask` returns.
fn is_host_deferred_tool(name: &str) -> bool {
    matches!(
        name,
        "ensure_java"
            | "install_mod"
            | "search_mods"
            | "suggest_mods"
            | "setup_env"
            | "build_pack"
            | "create_modpack"
            | "export_modpack"
            | "import_mrpack"
            | "import_modpack"
    )
}

#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    pub max_turns: u32,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_turns: DEFAULT_MAX_TURNS,
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
    pub turns_used: u32,
    /// `final_text` | `no_tools` | `max_turns` | `error`
    pub stop_reason: String,
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
            "【需确认】「{utterance}」会改动你的游戏文件，助手不会自动执行。请你在对话里用明确的中文再说一次（例如「确认删除版本 xxx」或「禁用 sodium」），本地规则才会执行。"
        )
    };
    let (title, tag) = crate::tool::tool_meta(name);
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
        ui_actions: vec![],
        tag_zh: tag.map(|s| s.into()),
    };
    (msg, card, None)
}

async fn run_one_tool(
    paths: &Paths,
    tc: &LlmToolCall,
    process_logs: Option<Vec<String>>,
    instance_id: Option<String>,
) -> (String, ToolCall, Option<ActionResult>, Option<DiagnoseReport>) {
    if is_dangerous_tool(&tc.name) {
        let (msg, card, action) = gate_dangerous(tc, paths, instance_id.as_deref());
        return (msg, card, action, None);
    }

    let utterance = tool_call_to_utterance(&tc.name, &tc.arguments);
    let routed = route(&utterance);
    match execute_intent(
        paths,
        &routed.intent,
        process_logs,
        instance_id,
    )
    .await
    {
        Ok((reply, report, action)) => {
            let card = if let Some(ref ar) = action {
                tool_call_from_action(ar)
            } else if let Some(ref r) = report {
                tool_call_from_diagnose(r)
            } else {
                let (title, tag) = crate::tool::tool_meta(&tc.name);
                ToolCall {
                    id: format!("tool-{}", tc.id),
                    name: tc.name.clone(),
                    title_zh: title.into(),
                    status: ToolStatus::Ok,
                    args: vec![],
                    result_zh: Some(reply.clone()),
                    path: None,
                    payload: None,
                    ui_actions: vec![],
                    tag_zh: tag.map(|s| s.into()),
                }
            };
            let mut result_text = reply;
            if let Some(ref r) = report {
                if !r.summary_zh.is_empty() && !result_text.contains(&r.summary_zh) {
                    result_text.push_str(&format!("\n\n[诊断摘要] {}", r.summary_zh));
                }
                if !r.findings.is_empty() {
                    let lines: Vec<String> = r
                        .findings
                        .iter()
                        .take(6)
                        .map(|f| format!("- {}: {}", f.title, f.summary))
                        .collect();
                    result_text.push_str(&format!("\n{}", lines.join("\n")));
                }
            }
            if is_host_deferred_tool(&tc.name) {
                result_text.push_str(
                    "\n（此步骤由启动器宿主完成；若需安装/搜索结果，见工具卡或下载页。）",
                );
            }
            (truncate(&result_text, 8000), card, action, report)
        }
        Err(e) => {
            let msg = format!("工具执行失败：{e}");
            let (title, tag) = crate::tool::tool_meta(&tc.name);
            let card = ToolCall {
                id: format!("tool-{}", tc.id),
                name: tc.name.clone(),
                title_zh: title.into(),
                status: ToolStatus::Error,
                args: vec![],
                result_zh: Some(msg.clone()),
                path: None,
                payload: None,
                ui_actions: vec![],
                tag_zh: tag.map(|s| s.into()),
            };
            (msg, card, None, None)
        }
    }
}

/// Run the multi-step agent loop (pi-style inner tool loop + max_turns).
pub async fn run_agent_loop(
    provider: &impl AgentCompleter,
    paths: &Paths,
    mut messages: Vec<ChatMessage>,
    process_logs: Option<Vec<String>>,
    instance_id: Option<String>,
    config: AgentLoopConfig,
) -> Result<AgentLoopResult> {
    let mut tools_out: Vec<ToolCall> = Vec::new();
    let mut last_action: Option<ActionResult> = None;
    let mut last_report: Option<DiagnoseReport> = None;
    let mut last_content = String::new();
    let mut turns_used = 0u32;
    let mut stop_reason = "final_text".to_string();

    for turn in 1..=config.max_turns {
        turns_used = turn;
        let completion: LlmCompletion = match provider.complete_raw(&messages, true).await {
            Ok(c) => c,
            Err(e) => {
                if tools_out.is_empty() && last_content.is_empty() {
                    return Err(e);
                }
                stop_reason = "error".into();
                last_content = format!("（本轮模型调用失败：{e}。已保留此前工具结果。）");
                break;
            }
        };

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

        // Execute ALL tool_calls this turn (pi: batch then refeed).
        for tc in &normalized {
            let (result_text, card, action, report) = run_one_tool(
                paths,
                tc,
                process_logs.clone(),
                instance_id.clone(),
            )
            .await;
            if let Some(ar) = action {
                last_action = Some(ar);
            }
            if report.is_some() {
                last_report = report;
            }
            tools_out.push(card);
            messages.push(tool_result_message(&tc.id, &tc.name, &result_text));
        }

        if turn == config.max_turns {
            stop_reason = "max_turns".into();
            // One more optional: if we hit the cap after tools, keep last assistant text.
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
            if !c.content.trim().is_empty() && c.content.trim() != "(空回复)" {
                last_content = c.content;
            }
        }
    }

    if last_content.trim().is_empty() || last_content.trim() == "(空回复)" {
        if !tools_out.is_empty() {
            last_content = "好了，步骤都跑完了。详情见上方工具卡。".into();
        }
    }

    Ok(AgentLoopResult {
        reply_zh: last_content,
        tools: tools_out,
        report: last_report,
        action: last_action,
        turns_used,
        stop_reason,
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
            },
            // Turn 2: model saw tool results; finishes with prose
            LlmCompletion {
                content: "版本列好了；删除需你确认，我不会自动删。".into(),
                tool_calls: vec![],
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
            AgentLoopConfig { max_turns: 8 },
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
            },
            LlmCompletion {
                content: "t2".into(),
                tool_calls: vec![tc("b", "list_instances", json!({}))],
            },
            // Would be turn 3 if max_turns ignored — must not be consumed for tools path.
            // (max_turns finalizer may call once more with tools=false if content empty;
            // we keep non-empty content so this stays unused.)
            LlmCompletion {
                content: "should-not-run".into(),
                tool_calls: vec![],
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
            AgentLoopConfig { max_turns: 2 },
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
}
