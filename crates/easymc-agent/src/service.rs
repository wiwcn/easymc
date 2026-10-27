//! High-level Agent service: rules first, optional LLM for chat / smarter diagnose.

use easymc_core::{load_settings, Paths, Result};

use crate::actions::{
    bump_memory, check_mods, env_status_summary, list_accounts, list_instances, open_latest_log,
    open_screenshots, select_account, select_instance, set_download_source, show_data_dir,
    show_instance_dir, suggest_memory_text, toggle_mod, write_diagnostic_bundle, ActionResult,
};
use crate::diagnose::{diagnose, DiagnoseReport, DiagnoseRequest};
use crate::intent::{route, Intent, IntentResult};
use crate::llm::{llm_status, system_prompt_zh, tool_call_to_utterance, LlmStatus, OpenAiCompatibleProvider};
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
    /// Normalized tool card for the UI (derived from action / diagnose).
    pub tool: Option<crate::tool::ToolCall>,
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


fn make_ask_response(
    mode: impl Into<String>,
    intent: Option<IntentResult>,
    reply_zh: String,
    report: Option<DiagnoseReport>,
    action: Option<ActionResult>,
    llm: LlmStatus,
) -> AskResponse {
    let tool = crate::tool::attach_tool_to_ask(action.as_ref(), report.as_ref());
    AskResponse {
        mode: mode.into(),
        intent,
        reply_zh,
        report,
        action,
        tool,
        llm,
    }
}

pub async fn ask(paths: &Paths, req: AskRequest) -> Result<AskResponse> {
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
                        return Ok(make_ask_response(
                            "rules+llm",
                            Some(intent),
                            reply,
                            report,
                            action,
                            status,
                        ));
                    }
                    Err(e) => {
                        reply.push_str(&format!("\n\n（LLM 调用失败，已回退规则结果：{e}）"));
                    }
                }
            }
        }
        return Ok(make_ask_response(
            "rules",
            Some(intent),
            reply,
            report,
            action,
            status,
        ));
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
        return Ok(make_ask_response(
            "rules",
            Some(intent),
            reply,
            report,
            action,
            status,
        ));
    }

    // Help / Unknown → LLM chat if configured (with tool schemas).
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
                    "

[本地诊断上下文] {}
日志摘录：
{}",
                    report.summary_zh,
                    truncate(&report.log_excerpt, 4000)
                ));
            }
            let msgs = build_messages(&req.history, &user);
            match provider.complete_raw(&msgs, true).await {
                Ok(completion) => {
                    if let Some(tc) = completion.tool_calls.first() {
                        let utterance = tool_call_to_utterance(&tc.name, &tc.arguments);
                        let routed = route(&utterance);
                        let (reply, report, action) = execute_intent(
                            paths,
                            &routed.intent,
                            req.process_logs.clone(),
                            req.instance_id.clone(),
                        )
                        .await?;
                        let mut reply_zh = reply;
                        if !completion.content.trim().is_empty()
                            && completion.content.trim() != "(空回复)"
                        {
                            reply_zh = format!(
                                "{}

{}",
                                completion.content.trim(),
                                reply_zh
                            );
                        }
                        return Ok(make_ask_response(
                            "llm+tool",
                            Some(routed),
                            reply_zh,
                            report,
                            action,
                            status,
                        ));
                    }
                    return Ok(make_ask_response(
                        "llm",
                        Some(intent),
                        completion.content,
                        None,
                        None,
                        status,
                    ));
                }
                Err(e) => {
                    let fallback = intent.reply_zh.clone();
                    return Ok(make_ask_response(
                        "rules_fallback",
                        Some(intent),
                        format!("LLM 不可用（{e}）。
{fallback}"),
                        None,
                        None,
                        status,
                    ));
                }
            }
        }
    }

    let (reply, report, action) =
        execute_intent(paths, &intent.intent, req.process_logs, req.instance_id).await?;
    Ok(make_ask_response(
        "rules",
        Some(intent),
        reply,
        report,
        action,
        status,
    ))
}

fn build_messages(history: &[ChatMessage], user: &str) -> Vec<ChatMessage> {
    let mut msgs = vec![ChatMessage {
        role: "system".into(),
        content: system_prompt_zh().into(),
    }];
    for h in history
        .iter()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        if h.role == "system" {
            continue;
        }
        msgs.push(h.clone());
    }
    msgs.push(ChatMessage {
        role: "user".into(),
        content: user.into(),
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
