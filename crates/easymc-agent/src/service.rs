//! High-level Agent service: rules first, optional LLM for chat / smarter diagnose.

use easymc_core::{load_settings, Paths, Result};

use crate::actions::{
    bump_memory, check_mod_deps, check_mods, delete_instance, delete_mod, duplicate_version,
    env_status_summary, fetch_missing_mod_deps_action, list_accounts, list_instances,
    open_game_folder, open_latest_log, open_screenshots, rename_instance, select_account,
    select_instance, set_download_source, show_data_dir, show_instance_dir, suggest_memory_text,
    toggle_mod, write_diagnostic_bundle, ActionResult,
};
use crate::diagnose::{diagnose, DiagnoseReport, DiagnoseRequest};
use crate::intent::{route, Intent, IntentResult};
use crate::agent_loop::{compose_reason_plan_zh, resolve_max_turns, run_agent_loop_with_progress, AgentLoopConfig, AgentStepEvent};
use crate::llm::{llm_status, LlmStatus, OpenAiCompatibleProvider};
use crate::skills::skill_augmented_system_prompt;
use crate::propose::{
    build_tune_proposal, oom_suggested_mb, proposal_to_action, ChangeProposal,
};
use crate::{AgentReply, ChatMessage, LlmProvider};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AskRequest {
    pub text: String,
    pub instance_id: Option<String>,
    pub process_logs: Option<Vec<String>>,
    /// Force rules-only even if LLM configured.
    pub rules_only: bool,
    /// Prior chat turns (for LLM mode).
    pub history: Vec<ChatMessage>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AskResponse {
    pub mode: String,
    pub intent: Option<IntentResult>,
    pub reply_zh: String,
    pub report: Option<DiagnoseReport>,
    pub action: Option<ActionResult>,
    /// Normalized tool card for the UI (derived from action / diagnose) — last / primary.
    pub tool: Option<crate::tool::ToolCall>,
    /// All tool cards from a multi-step agent loop (Lit can render multiple).
    #[serde(default)]
    pub tools: Vec<crate::tool::ToolCall>,
    /// Progressive Chinese step abstracts (live via `agent-step` + final payload).
    #[serde(default)]
    pub steps: Vec<AgentStepEvent>,
    pub llm: LlmStatus,
}

fn resolve_instance_id(paths: &Paths, hint: Option<&str>) -> Option<String> {
    if let Some(id) = hint {
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    easymc_core::load_instances(paths)
        .ok()
        .and_then(|s| s.active_id)
}

fn maybe_oom_proposal(
    paths: &Paths,
    report: &DiagnoseReport,
    instance_id: Option<&str>,
) -> Option<ActionResult> {
    let oom = report.findings.iter().any(|f| {
        f.id == "oom"
            || f.action.as_deref() == Some("bump_memory")
            || f.title.contains("内存")
    });
    if !oom {
        return None;
    }
    let settings = load_settings(paths).unwrap_or_default();
    let current = {
        let id = resolve_instance_id(paths, instance_id);
        if let Some(id) = id {
            easymc_core::load_instances(paths)
                .ok()
                .and_then(|s| {
                    s.instances
                        .into_iter()
                        .find(|i| i.id == id)
                        .and_then(|i| i.memory_mb)
                })
                .unwrap_or(settings.memory_mb)
        } else {
            settings.memory_mb
        }
    };
    let suggested = oom_suggested_mb(current);
    let prop = build_tune_proposal(
        paths,
        instance_id,
        Some(suggested),
        "根据崩溃分析：疑似内存不足，建议提高内存并启用 G1GC（客户端启动参数）。",
    );
    Some(proposal_to_action(
        prop,
        "检测到可能的内存问题，已生成「建议更改」。确认后点「应用更改」。",
    ))
}

pub fn run_diagnose(
    paths: &Paths,
    instance_id: Option<String>,
    process_logs: Option<Vec<String>>,
) -> DiagnoseReport {
    run_diagnose_with_path(paths, instance_id, process_logs, None)
}

/// Diagnose using latest instance logs and/or an explicit log file path.
pub fn run_diagnose_with_path(
    paths: &Paths,
    instance_id: Option<String>,
    process_logs: Option<Vec<String>>,
    log_path: Option<String>,
) -> DiagnoseReport {
    let id = resolve_instance_id(paths, instance_id.as_deref());
    let mut extra_paths = Vec::new();
    if let Some(p) = log_path {
        let pb = std::path::PathBuf::from(p.trim());
        if pb.is_file() {
            extra_paths.push(pb);
        }
    }
    diagnose(
        paths,
        DiagnoseRequest {
            process_logs,
            instance_id: id,
            extra_paths,
            ..DiagnoseRequest::default()
        },
    )
}

/// Execute a routed intent with local side effects where safe.
pub async fn execute_intent(
    paths: &Paths,
    intent: &Intent,
    process_logs: Option<Vec<String>>,
    instance_id: Option<String>,
) -> Result<(String, Option<DiagnoseReport>, Option<ActionResult>)> {
    match intent {
        Intent::Diagnose {
            instance_id: iid,
            log_path,
        } => {
            let id = iid.clone().or(instance_id);
            let report = run_diagnose_with_path(
                paths,
                id.clone(),
                process_logs,
                log_path.clone(),
            );
            let mut reply = report.summary_zh.clone();
            for f in &report.findings {
                reply.push_str(&format!(
                    "\n\n【{}】{}\n{}",
                    f.title,
                    f.summary,
                    f.fixes.join("\n")
                ));
                if let Some(ev) = &f.evidence {
                    reply.push_str(&format!("\n证据: {ev}"));
                }
            }
            let action = maybe_oom_proposal(paths, &report, id.as_deref());
            if action.is_some() {
                reply.push_str("\n\n下方有「建议更改」卡片，确认后可一键应用。");
            }
            Ok((reply, Some(report), action))
        }
        Intent::EnsureJava { major } => {
            // Host (CLI/Tauri) runs ensure_java and rewrites reply with the real result.
            Ok((
                format!("正在检查并准备 Java {major}…"),
                None,
                Some(ActionResult {
                    ok: true,
                    action: "ensure_java".into(),
                    message_zh: format!("准备 Java {major}"),
                    path: None,
                    value: Some(serde_json::json!({ "major": major })),
                }),
            ))
        }
        Intent::BumpMemory { mb } => {
            let ar = bump_memory(paths, *mb)?;
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::TuneConfig { instance_id: iid } => {
            let id = iid.clone().or(instance_id);
            let prop = build_tune_proposal(
                paths,
                id.as_deref(),
                None,
                "根据当前内存与启动参数生成的客户端优化建议。确认后点「应用更改」。",
            );
            let ar = proposal_to_action(
                prop,
                "已生成优化建议（内存 + JVM）。请在卡片上点「应用更改」才会写入。",
            );
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::SetupEnv { instance_id: iid } => {
            let id = iid.clone().or(instance_id);
            let ar = env_status_summary(paths, id.as_deref());
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::BuildPack { instance_id: iid } => {
            let id = iid.as_ref().cloned().or(instance_id).or_else(|| {
                resolve_instance_id(paths, None)
            });
            Ok((
                "正在导出当前版本的 .mrpack 整合包…".into(),
                None,
                Some(ActionResult {
                    ok: true,
                    action: "build_pack".into(),
                    message_zh: "制作整合包".into(),
                    path: None,
                    value: Some(serde_json::json!({
                        "instance_id": id,
                        "pack": true,
                    })),
                }),
            ))
        }
        Intent::SetDownloadSource { source } => {
            let ar = set_download_source(paths, source)?;
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::OpenInstanceDir { instance_id: iid } => {
            let id = iid.clone().or(instance_id);
            let ar = show_instance_dir(paths, id.as_deref());
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::CheckMods { instance_id: iid } => {
            let id = iid.clone().or(instance_id);
            let ar = check_mods(paths, id.as_deref());
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::CheckModDeps { instance_id: iid } => {
            let id = iid.clone().or(instance_id);
            let ar = check_mod_deps(paths, id.as_deref());
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::FetchMissingModDeps {
            instance_id: iid,
            only_ids,
        } => {
            let id = iid.clone().or(instance_id);
            let ar = fetch_missing_mod_deps_action(paths, id.as_deref(), only_ids.clone());
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::ToggleMod {
            query,
            enable,
            instance_id: iid,
        } => {
            let id = iid.clone().or(instance_id);
            let ar = toggle_mod(paths, id.as_deref(), query, *enable);
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::ListInstances => {
            let ar = list_instances(paths);
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::SelectInstance { query } => {
            let ar = select_instance(paths, query);
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::RenameInstance { query, new_name } => {
            let ar = rename_instance(paths, query.as_deref(), new_name);
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::DuplicateInstance { query, new_name } => {
            let ar = duplicate_version(paths, query.as_deref(), new_name.as_deref());
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::DeleteInstance { query } => {
            let ar = delete_instance(paths, query);
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::DeleteMod { query, instance_id: iid } => {
            let id = iid.clone().or(instance_id);
            let ar = delete_mod(paths, id.as_deref(), query);
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::OpenLatestLog { instance_id: iid } => {
            let id = iid.clone().or(instance_id);
            let ar = open_latest_log(paths, id.as_deref());
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::ListAccounts => {
            let ar = list_accounts(paths);
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::SelectAccount { query } => {
            let ar = select_account(paths, query);
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::OpenScreenshots { instance_id: iid } => {
            let id = iid.clone().or(instance_id);
            let ar = open_screenshots(paths, id.as_deref());
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::OpenGameFolder {
            folder,
            instance_id: iid,
        } => {
            let id = iid.clone().or(instance_id);
            let ar = open_game_folder(paths, id.as_deref(), *folder);
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::ImportMrpack { path } => {
            let path = path.trim().to_string();
            if path.is_empty() {
                Ok((
                    "请带上整合包路径，例如：导入整合包 /path/to/pack.mrpack".into(),
                    None,
                    Some(ActionResult {
                        ok: false,
                        action: "import_mrpack".into(),
                        message_zh: "缺少 .mrpack 路径".into(),
                        path: None,
                        value: None,
                    }),
                ))
            } else {
                Ok((
                    format!("正在导入整合包：{path} …"),
                    None,
                    Some(ActionResult {
                        ok: true,
                        action: "import_mrpack".into(),
                        message_zh: "导入整合包".into(),
                        path: Some(path.clone()),
                        value: Some(serde_json::json!({ "path": path })),
                    }),
                ))
            }
        }
        Intent::ShowDataDir => {
            let ar = show_data_dir(paths);
            Ok((ar.message_zh.clone(), None, Some(ar)))
        }
        Intent::DiagnosticsBundle => {
            let report = run_diagnose(paths, instance_id, process_logs);
            let ar = write_diagnostic_bundle(paths, &report)?;
            Ok((ar.message_zh.clone(), Some(report), Some(ar)))
        }
        Intent::SearchMods { query } => Ok((
            format!("正在搜索模组「{query}」…"),
            None,
            Some(ActionResult {
                ok: true,
                action: "search_mods".into(),
                message_zh: format!("搜索「{query}」"),
                path: None,
                value: Some(serde_json::json!({
                    "query": query,
                    "tab": "mod",
                })),
            }),
        )),
        Intent::SuggestMods { theme } => Ok((
            format!("正在按「{theme}」主题推荐模组…"),
            None,
            Some(ActionResult {
                ok: true,
                action: "suggest_mods".into(),
                message_zh: format!("推荐「{theme}」主题模组"),
                path: None,
                value: Some(serde_json::json!({
                    "theme": theme,
                })),
            }),
        )),
        Intent::InstallMod { query } => Ok((
            format!("正在搜索模组「{query}」，找到后会装进当前选中的版本…"),
            None,
            Some(ActionResult {
                ok: true,
                action: "install_mod".into(),
                message_zh: format!("搜索并安装「{query}」"),
                path: None,
                value: Some(serde_json::json!({ "query": query, "tab": "mod" })),
            }),
        )),
        Intent::Help => Ok((crate::intent::help_text(), None, None)),
        Intent::Unknown { raw } => Ok((
            format!("未识别：「{raw}」。\n{}", crate::intent::help_text()),
            None,
            None,
        )),
    }
}



fn emit_response_steps(
    on_step: Option<&(dyn Fn(AgentStepEvent) + Send + Sync)>,
    resp: &AskResponse,
) {
    if let Some(cb) = on_step {
        for step in &resp.steps {
            cb(step.clone());
        }
    }
}

fn make_ask_response(
    mode: impl Into<String>,
    intent: Option<IntentResult>,
    reply_zh: String,
    report: Option<DiagnoseReport>,
    action: Option<ActionResult>,
    llm: LlmStatus,
) -> AskResponse {
    make_ask_response_with_tools(mode, intent, reply_zh, report, action, Vec::new(), Vec::new(), llm)
}

fn steps_from_tools(tools: &[crate::tool::ToolCall]) -> Vec<AgentStepEvent> {
    let mut steps = Vec::new();
    let mut idx = 0u32;
    for card in tools {
        idx += 1;
        steps.push(AgentStepEvent::tool_start(idx, &card.name));
        idx += 1;
        steps.push(AgentStepEvent::tool_done(idx, &card.name, card));
    }
    steps
}

fn make_ask_response_with_tools(
    mode: impl Into<String>,
    intent: Option<IntentResult>,
    reply_zh: String,
    report: Option<DiagnoseReport>,
    action: Option<ActionResult>,
    tools: Vec<crate::tool::ToolCall>,
    steps: Vec<AgentStepEvent>,
    llm: LlmStatus,
) -> AskResponse {
    let tool = tools.last().cloned().or_else(|| {
        crate::tool::attach_tool_to_ask(action.as_ref(), report.as_ref())
    });
    let tools = if tools.is_empty() {
        tool.clone().into_iter().collect()
    } else {
        tools
    };
    // Rule-path / thin LLM replies → structured 原因/方案 when tools are present.
    let reply_zh = if !tools.is_empty() {
        compose_reason_plan_zh(&tools, &reply_zh)
    } else {
        reply_zh
    };
    let steps = if steps.is_empty() && !tools.is_empty() {
        let mut s = steps_from_tools(&tools);
        if !reply_zh.trim().is_empty() {
            let next = s.last().map(|x| x.index).unwrap_or(0) + 1;
            s.push(AgentStepEvent::reply(next, &reply_zh));
        }
        s
    } else {
        // Refresh trailing reply abstract if we rewrote the bubble text.
        let mut s = steps;
        if let Some(last) = s.last_mut() {
            if last.phase == "reply" && !reply_zh.trim().is_empty() {
                *last = AgentStepEvent::reply(last.index, &reply_zh);
            }
        }
        s
    };
    AskResponse {
        mode: mode.into(),
        intent,
        reply_zh,
        report,
        action,
        tool,
        tools,
        steps,
        llm,
    }
}

pub async fn ask(paths: &Paths, req: AskRequest) -> Result<AskResponse> {
    ask_with_progress(paths, req, None).await
}

pub async fn ask_with_progress(
    paths: &Paths,
    req: AskRequest,
    on_step: Option<&(dyn Fn(AgentStepEvent) + Send + Sync)>,
) -> Result<AskResponse> {
    let status = llm_status();
    let intent = route(&req.text);

    // Strong local intents always run locally (even if LLM is up), except Unknown/Help with LLM.
    let use_llm = status.configured
        && !req.rules_only
        && matches!(
            intent.intent,
            Intent::Unknown { .. } | Intent::Help | Intent::Diagnose { .. }
        );

    // For Diagnose: always get rule report; optionally enrich with LLM.
    if matches!(intent.intent, Intent::Diagnose { .. }) {
        let (mut reply, report, action) = execute_intent(
            paths,
            &intent.intent,
            req.process_logs.clone(),
            req.instance_id.clone(),
        )
        .await?;
        if use_llm {
            if let Some(provider) = OpenAiCompatibleProvider::try_from_env() {
                let excerpt = report
                    .as_ref()
                    .map(|r| r.log_excerpt.clone())
                    .unwrap_or_default();
                let findings = report
                    .as_ref()
                    .map(|r| {
                        r.findings
                            .iter()
                            .map(|f| format!("{}: {}", f.title, f.summary))
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();
                let user = format!(
                    "用户请求：{}\n\n规则引擎已匹配：\n{findings}\n\n日志摘录：\n{}",
                    req.text,
                    truncate(&excerpt, 6000)
                );
                let msgs = build_messages(&req.history, &user);
                match provider.complete(&msgs).await {
                    Ok(AgentReply { content, .. }) => {
                        reply = format!(
                            "【规则摘要】{}\n\n【LLM 分析】\n{content}",
                            report
                                .as_ref()
                                .map(|r| r.summary_zh.as_str())
                                .unwrap_or("")
                        );
                        let resp = make_ask_response(
                            "rules+llm",
                            Some(intent),
                            reply,
                            report,
                            action,
                            status,
                        );
                        emit_response_steps(on_step, &resp);
                        return Ok(resp);
                    }
                    Err(e) => {
                        reply.push_str(&format!("\n\n（LLM 调用失败，已回退规则结果：{e}）"));
                    }
                }
            }
        }
        let resp = make_ask_response(
            "rules",
            Some(intent),
            reply,
            report,
            action,
            status,
        );
        emit_response_steps(on_step, &resp);
        return Ok(resp);
    }

    // Other concrete intents → local
    if !matches!(intent.intent, Intent::Unknown { .. } | Intent::Help) {
        let (reply, report, action) = execute_intent(
            paths,
            &intent.intent,
            req.process_logs,
            req.instance_id,
        )
        .await?;
        let resp = make_ask_response(
            "rules",
            Some(intent),
            reply,
            report,
            action,
            status,
        );
        emit_response_steps(on_step, &resp);
        return Ok(resp);
    }

    // Help / Unknown → multi-step LLM agent loop if configured (tool schemas + refeed).
    if use_llm {
        if let Some(provider) = OpenAiCompatibleProvider::try_from_env() {
            let mut user = req.text.clone();
            // Attach short diagnose context if logs present
            if req
                .process_logs
                .as_ref()
                .map(|l| !l.is_empty())
                .unwrap_or(false)
                || req.instance_id.is_some()
            {
                let report = run_diagnose(paths, req.instance_id.clone(), req.process_logs.clone());
                user.push_str(&format!(
                    "\n\n[本地诊断上下文] {}\n日志摘录：\n{}",
                    report.summary_zh,
                    truncate(&report.log_excerpt, 4000)
                ));
            }
            let msgs = build_messages(&req.history, &user);
            match run_agent_loop_with_progress(
                &provider,
                paths,
                msgs,
                req.process_logs.clone(),
                req.instance_id.clone(),
                AgentLoopConfig {
                    max_turns: resolve_max_turns(),
                },
                on_step,
            )
            .await
            {
                Ok(loop_result) => {
                    let mode = if loop_result.tools.is_empty() {
                        "llm"
                    } else {
                        "llm+agent"
                    };
                    return Ok(make_ask_response_with_tools(
                        mode,
                        Some(intent),
                        loop_result.reply_zh,
                        loop_result.report,
                        loop_result.action,
                        loop_result.tools,
                        loop_result.steps,
                        status,
                    ));
                }
                Err(e) => {
                    let fallback = intent.reply_zh.clone();
                    let resp = make_ask_response(
                        "rules_fallback",
                        Some(intent),
                        format!("LLM 不可用（{e}）。\n{fallback}"),
                        None,
                        None,
                        status,
                    );
                    emit_response_steps(on_step, &resp);
                    return Ok(resp);
                }
            }
        }
    }

    let (reply, report, action) =
        execute_intent(paths, &intent.intent, req.process_logs, req.instance_id).await?;
    let resp = make_ask_response(
        "rules",
        Some(intent),
        reply,
        report,
        action,
        status,
    );
    emit_response_steps(on_step, &resp);
    Ok(resp)
}

fn build_messages(history: &[ChatMessage], user: &str) -> Vec<ChatMessage> {
    let mut msgs = vec![ChatMessage {
        role: "system".into(),
        content: skill_augmented_system_prompt(user),
        tool_call_id: None,
        name: None,
        tool_calls: vec![],
    }];
    for h in history
        .iter()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        if h.role == "system" || h.role == "tool" {
            continue;
        }
        msgs.push(ChatMessage {
            role: h.role.clone(),
            content: h.content.clone(),
            tool_call_id: None,
            name: None,
            tool_calls: vec![],
        });
    }
    msgs.push(ChatMessage {
        role: "user".into(),
        content: user.into(),
        tool_call_id: None,
        name: None,
        tool_calls: vec![],
    });
    msgs
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}

pub fn memory_suggestion(paths: &Paths, suggested: u32) -> ActionResult {
    let cur = load_settings(paths).map(|s| s.memory_mb).unwrap_or(2048);
    suggest_memory_text(cur, suggested)
}

/// Build a tune proposal without going through ask() (UI shortcut / tests).
pub fn tune_proposal(paths: &Paths, instance_id: Option<&str>) -> ChangeProposal {
    build_tune_proposal(
        paths,
        instance_id,
        None,
        "根据当前内存与启动参数生成的客户端优化建议。",
    )
}

#[allow(dead_code)]
/// Stretch: local GGUF backend trait scaffold — disabled by default.
#[async_trait::async_trait]
pub trait LocalGgufBackend: Send + Sync {
    fn enabled(&self) -> bool {
        false
    }
    async fn complete(&self, _messages: &[ChatMessage]) -> Result<AgentReply> {
        Err(easymc_core::EasyMcError::NotImplemented(
            "local GGUF backend (disabled)",
        ))
    }
}

pub struct DisabledGguf;

#[async_trait::async_trait]
impl LocalGgufBackend for DisabledGguf {}
