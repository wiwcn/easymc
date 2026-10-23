//! High-level Agent service: rules first, optional LLM for chat / smarter diagnose.

use easymc_core::{load_instances, load_settings, Paths, Result};

use crate::actions::{
    bump_memory, show_data_dir, suggest_memory_text, write_diagnostic_bundle, ActionResult,
};
use crate::diagnose::{diagnose, DiagnoseReport, DiagnoseRequest};
use crate::intent::{route, Intent, IntentResult};
use crate::llm::{llm_status, system_prompt_zh, LlmStatus, OpenAiCompatibleProvider};
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
    pub llm: LlmStatus,
}

fn resolve_instance_id(paths: &Paths, hint: Option<&str>) -> Option<String> {
    if let Some(id) = hint {
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    load_instances(paths).ok().and_then(|s| s.active_id)
}

pub fn run_diagnose(
    paths: &Paths,
    instance_id: Option<String>,
    process_logs: Option<Vec<String>>,
) -> DiagnoseReport {
    let id = resolve_instance_id(paths, instance_id.as_deref());
    diagnose(
        paths,
        DiagnoseRequest {
            process_logs,
            instance_id: id,
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
        Intent::Diagnose { instance_id: iid } => {
            let id = iid.clone().or(instance_id);
            let report = run_diagnose(paths, id, process_logs);
            let mut reply = report.summary_zh.clone();
            for f in &report.findings {
                reply.push_str(&format!("\n\n【{}】{}\n{}", f.title, f.summary, f.fixes.join("\n")));
                if let Some(ev) = &f.evidence {
                    reply.push_str(&format!("\n证据: {ev}"));
                }
            }
            Ok((reply, Some(report), None))
        }
        Intent::EnsureJava { major } => {
            // Caller (CLI/Tauri) should run ensure_java; we return a structured hint.
            Ok((
                format!("请执行 ensure-java {major}（CLI: easymc ensure-java {major}）。"),
                None,
                Some(ActionResult {
                    ok: true,
                    action: "ensure_java".into(),
                    message_zh: format!("建议确保 Java {major}"),
                    path: None,
                    value: Some(serde_json::json!({ "major": major })),
                }),
            ))
        }
        Intent::BumpMemory { mb } => {
            let ar = bump_memory(paths, *mb)?;
            Ok((ar.message_zh.clone(), None, Some(ar)))
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
        Intent::InstallMod { query } => Ok((
            format!(
                "模组「{query}」：请到「商店」搜索，或 CLI：\n  easymc search-mods modrinth {query}\n  easymc install-mod modrinth <project-id> --instance <id>"
            ),
            None,
            Some(ActionResult {
                ok: true,
                action: "install_mod_hint".into(),
                message_zh: format!("搜索模组 {query}"),
                path: None,
                value: Some(serde_json::json!({ "query": query })),
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
                        reply = format!("【规则摘要】{}\n\n【LLM 分析】\n{content}", report.as_ref().map(|r| r.summary_zh.as_str()).unwrap_or(""));
                        return Ok(AskResponse {
                            mode: "rules+llm".into(),
                            intent: Some(intent),
                            reply_zh: reply,
                            report,
                            action,
                            llm: status,
                        });
                    }
                    Err(e) => {
                        reply.push_str(&format!("\n\n（LLM 调用失败，已回退规则结果：{e}）"));
                    }
                }
            }
        }
        return Ok(AskResponse {
            mode: "rules".into(),
            intent: Some(intent),
            reply_zh: reply,
            report,
            action,
            llm: status,
        });
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
        return Ok(AskResponse {
            mode: "rules".into(),
            intent: Some(intent),
            reply_zh: reply,
            report,
            action,
            llm: status,
        });
    }

    // Help / Unknown → LLM chat if configured
    if use_llm {
        if let Some(provider) = OpenAiCompatibleProvider::try_from_env() {
            let mut user = req.text.clone();
            // Attach short diagnose context if logs present
            if req.process_logs.as_ref().map(|l| !l.is_empty()).unwrap_or(false)
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
            match provider.complete(&msgs).await {
                Ok(AgentReply { content, .. }) => {
                    return Ok(AskResponse {
                        mode: "llm".into(),
                        intent: Some(intent),
                        reply_zh: content,
                        report: None,
                        action: None,
                        llm: status,
                    });
                }
                Err(e) => {
                    let fallback = intent.reply_zh.clone();
                    return Ok(AskResponse {
                        mode: "rules_fallback".into(),
                        intent: Some(intent),
                        reply_zh: format!("LLM 不可用（{e}）。\n{fallback}"),
                        report: None,
                        action: None,
                        llm: status,
                    });
                }
            }
        }
    }

    let (reply, report, action) =
        execute_intent(paths, &intent.intent, req.process_logs, req.instance_id).await?;
    Ok(AskResponse {
        mode: "rules".into(),
        intent: Some(intent),
        reply_zh: reply,
        report,
        action,
        llm: status,
    })
}

fn build_messages(history: &[ChatMessage], user: &str) -> Vec<ChatMessage> {
    let mut msgs = vec![ChatMessage {
        role: "system".into(),
        content: system_prompt_zh().into(),
    }];
    for h in history.iter().rev().take(12).collect::<Vec<_>>().into_iter().rev() {
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
