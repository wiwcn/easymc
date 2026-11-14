//! Direct tool execution for the agent loop.
//!
//! The old loop turned a tool call into a Chinese utterance and fed it back
//! through `intent::route`; arguments were lost and network tools were only
//! "deferred" to the host. This module executes by tool name with typed
//! arguments, returns real results (search/install/update/deps), and is also
//! used by the UI approval flow (`agent_confirm_tool`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use easymc_core::{load_instances, load_settings, EasyMcError, Instance, ModLoader, Paths};
use easymc_mods::{
    apply_mod_updates, check_mod_updates, fetch_missing_deps, search_and_install_best,
    search_mods, ModSource, ModUpdateApplyItem, ProjectType, SearchQuery,
};
use serde_json::{json, Value};

use crate::actions::{env_status_summary, ActionResult, GameFolderKind};
use crate::diagnose::DiagnoseReport;
use crate::intent::Intent;
use crate::service::execute_intent;
use crate::tool::{tool_call_from_action, tool_call_from_diagnose, ToolArg, ToolCall, ToolStatus, ToolUiAction};

/// Everything a tool needs from the host.
pub struct ToolContext<'a> {
    pub paths: &'a Paths,
    pub instance_id: Option<String>,
    pub process_logs: Option<Vec<String>>,
    /// True when the player approved a gated tool from the UI.
    pub confirmed: bool,
    pub cancel: Option<Arc<AtomicBool>>,
}

impl<'a> ToolContext<'a> {
    pub fn new(paths: &'a Paths) -> Self {
        Self {
            paths,
            instance_id: None,
            process_logs: None,
            confirmed: false,
            cancel: None,
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub struct ToolExecResult {
    /// Player-facing Chinese summary (card + reply).
    pub message_zh: String,
    /// Text fed back to the model (summary + structured data).
    pub model_text: String,
    pub card: ToolCall,
    pub action: Option<ActionResult>,
    pub report: Option<DiagnoseReport>,
}

/// Aliases the model / older prompts may still emit.
pub fn canonical_tool_name(name: &str) -> &str {
    match name {
        "list_versions" => "list_instances",
        "switch_instance" | "switch_version" => "select_instance",
        "rename_version" => "rename_instance",
        "duplicate_version" | "copy_instance" => "duplicate_instance",
        "delete_version" | "remove_instance" => "delete_instance",
        "remove_mod" | "uninstall_mod" => "delete_mod",
        "enable_mod" | "disable_mod" => "toggle_mod",
        "mod_dep_graph" | "analyze_mod_deps" | "dependency_tree" => "check_mod_deps",
        "install_missing_deps" | "resolve_deps" => "fetch_missing_mod_deps",
        "batch_update_mods" => "update_mods",
        "check_mods" => "list_installed_mods",
        "analyze_launch_log" | "analyze_crash_log" => "diagnose",
        "create_modpack" | "export_modpack" => "build_pack",
        "import_modpack" => "import_mrpack",
        "suggest_memory" => "propose_changes",
        "apply_changes" | "apply_proposal" => "propose_changes",
        "open_store" | "install_mod_hint" => "search_mods",
        other => other,
    }
}

fn s_arg(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn b_arg(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

fn u_arg(args: &Value, key: &str) -> Option<u32> {
    args.get(key)
        .and_then(|v| v.as_u64())
        .map(|n| n.clamp(512, 65536) as u32)
}

fn strings_arg(args: &Value, key: &str) -> Option<Vec<String>> {
    args.get(key).and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|x| x.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    })
}

fn resolve_instance(paths: &Paths, hint: Option<&str>) -> Option<Instance> {
    let store = load_instances(paths).ok()?;
    let id = hint
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| store.active_id.clone())?;
    store.instances.into_iter().find(|i| i.id == id)
}

fn need_instance(paths: &Paths, hint: Option<&str>) -> easymc_core::Result<Instance> {
    resolve_instance(paths, hint)
        .ok_or_else(|| EasyMcError::msg("还没有选版本。请先安装/选择一个版本，再让我操作模组。"))
}

/// Prefer a model-supplied `instance_id`, else the currently selected instance.
fn instance_hint(ctx: &ToolContext<'_>, args: &Value) -> Option<String> {
    s_arg(args, "instance_id").or_else(|| ctx.instance_id.clone())
}

fn card_args(pairs: &[(&str, &str, String)]) -> Vec<ToolArg> {
    pairs
        .iter()
        .map(|(k, label, v)| ToolArg {
            key: (*k).into(),
            label_zh: (*label).into(),
            value: v.clone(),
        })
        .collect()
}

fn ok_card(name: &str, message: impl Into<String>, payload: Value, args: Vec<ToolArg>) -> ToolCall {
    let (title, tag) = crate::tool::tool_meta(name);
    ToolCall {
        id: format!("tool-{}", chrono::Local::now().format("%Y%m%d%H%M%S%3f")),
        name: name.into(),
        title_zh: title.into(),
        status: ToolStatus::Ok,
        args,
        result_zh: Some(message.into()),
        path: None,
        payload: Some(payload),
        ui_actions: vec![],
        tag_zh: tag.map(str::to_string),
    }
}

fn err_card(name: &str, message: impl Into<String>) -> ToolCall {
    let (title, tag) = crate::tool::tool_meta(name);
    ToolCall {
        id: format!("tool-{}", chrono::Local::now().format("%Y%m%d%H%M%S%3f")),
        name: name.into(),
        title_zh: title.into(),
        status: ToolStatus::Error,
        args: vec![],
        result_zh: Some(message.into()),
        path: None,
        payload: None,
        ui_actions: vec![],
        tag_zh: tag.map(str::to_string),
    }
}

fn model_payload(message: &str, payload: &Value) -> String {
    let data = serde_json::to_string(payload).unwrap_or_else(|_| "{}".into());
    let data = if data.chars().count() > 5000 {
        data.chars().take(5000).collect::<String>() + "…"
    } else {
        data
    };
    format!("{message}\n[结构化结果] {data}")
}

fn from_action_result(name: &str, ar: ActionResult) -> ToolExecResult {
    let mut card = tool_call_from_action(&ar);
    card.name = name.into();
    ToolExecResult {
        message_zh: ar.message_zh.clone(),
        model_text: model_payload(&ar.message_zh, ar.value.as_ref().unwrap_or(&Value::Null)),
        card,
        action: Some(ar),
        report: None,
    }
}

fn from_error(name: &str, err: impl std::fmt::Display) -> ToolExecResult {
    let msg = format!("没做成：{err}");
    ToolExecResult {
        model_text: msg.clone(),
        message_zh: msg,
        card: err_card(name, "这次没做成，展开看原因。"),
        action: None,
        report: None,
    }
}

async fn via_intent(ctx: &ToolContext<'_>, requested: &str, intent: Intent) -> easymc_core::Result<ToolExecResult> {
    let (reply, report, action) = execute_intent(
        ctx.paths,
        &intent,
        ctx.process_logs.clone(),
        ctx.instance_id.clone(),
    )
    .await?;
    let card = if let Some(ref ar) = action {
        let mut c = tool_call_from_action(ar);
        c.name = requested.into();
        c
    } else if let Some(ref r) = report {
        let mut c = tool_call_from_diagnose(r);
        c.name = requested.into();
        c
    } else {
        ok_card(requested, reply.clone(), Value::Null, vec![])
    };
    let mut model_text = reply.clone();
    if let Some(ref r) = report {
        if !r.summary_zh.is_empty() {
            model_text.push_str(&format!("\n[诊断摘要] {}", r.summary_zh));
        }
        for f in r.findings.iter().take(6) {
            model_text.push_str(&format!("\n- {}: {}", f.title, f.summary));
        }
    }
    if let Some(v) = card.payload.as_ref() {
        model_text = model_payload(&model_text, v);
    }
    Ok(ToolExecResult {
        message_zh: reply,
        model_text,
        card,
        action,
        report,
    })
}

async fn search_mods_impl(ctx: &ToolContext<'_>, args: &Value) -> easymc_core::Result<ToolExecResult> {
    let query = s_arg(args, "query").unwrap_or_default();
    if query.is_empty() {
        return Err(EasyMcError::msg("请给一个模组关键词"));
    }
    let instance = resolve_instance(ctx.paths, instance_hint(ctx, args).as_deref());
    let game_version = s_arg(args, "game_version").or_else(|| instance.as_ref().map(|i| i.version.0.clone()));
    let loader = s_arg(args, "loader")
        .and_then(|s| ModLoader::parse_loose(&s))
        .or_else(|| instance.as_ref().map(|i| i.loader));
    let source = s_arg(args, "source")
        .and_then(|s| ModSource::parse(&s))
        .unwrap_or(ModSource::Modrinth);
    if let Ok(settings) = load_settings(ctx.paths) {
        easymc_mods::sync_api_key_from_settings(settings.curseforge_api_key.as_deref());
    }
    let sq = SearchQuery {
        project_type: ProjectType::Mod,
        text: query.clone(),
        game_version: game_version.clone(),
        loader,
        limit: 10,
        offset: 0,
    };
    let mut results = search_mods(source, &sq).await?;
    if results.is_empty() && source == ModSource::Modrinth {
        // Mirror the market page: retry without version/loader filters.
        let broad = SearchQuery {
            game_version: None,
            loader: None,
            ..sq
        };
        results = search_mods(ModSource::Modrinth, &broad).await.unwrap_or_default();
    }
    if results.is_empty() {
        let msg = format!("没搜到「{query}」相关模组。");
        let payload = json!({ "query": query, "results": [] });
        return Ok(ToolExecResult {
            model_text: model_payload(&msg, &payload),
            message_zh: msg,
            card: ok_card("search_mods", "没有匹配结果", payload, vec![]),
            action: None,
            report: None,
        });
    }

    let items: Vec<Value> = results
        .iter()
        .take(10)
        .map(|p| {
            json!({
                "name": p.name,
                "slug": p.slug,
                "source": p.source.as_str(),
                "downloads": p.downloads,
                "description": p.description.chars().take(160).collect::<String>(),
            })
        })
        .collect();
    let mut msg = format!("找到 {} 个模组：\n", results.len());
    for p in results.iter().take(5) {
        msg.push_str(&format!(
            "· {}（{}，下载 {}）\n",
            p.name,
            p.source.label_zh(),
            p.downloads
        ));
    }
    let payload = json!({
        "query": query,
        "game_version": game_version,
        "results": items,
    });
    let mut card = ok_card(
        "search_mods",
        msg.clone(),
        payload.clone(),
        card_args(&[("query", "关键词", query.to_string())]),
    );
    card.ui_actions = vec![ToolUiAction {
        id: "open_download".into(),
        label_zh: "去下载页".into(),
    }];
    Ok(ToolExecResult {
        model_text: model_payload(&msg, &payload),
        message_zh: msg,
        card,
        action: None,
        report: None,
    })
}

async fn install_mod_impl(ctx: &ToolContext<'_>, args: &Value) -> easymc_core::Result<ToolExecResult> {
    let query = s_arg(args, "query").unwrap_or_default();
    if query.is_empty() {
        return Err(EasyMcError::msg("请告诉我要装哪个模组"));
    }
    let inst = need_instance(ctx.paths, instance_hint(ctx, args).as_deref())?;
    let settings = load_settings(ctx.paths).unwrap_or_default();
    easymc_mods::sync_api_key_from_settings(settings.curseforge_api_key.as_deref());
    let result = search_and_install_best(&inst, &query, |_p| {}).await?;
    let msg = if result.skipped {
        format!("「{}」已经装过了（{}）。", result.project_name, result.filename)
    } else {
        format!(
            "已把「{}」{} 装进「{}」（{}）。",
            result.project_name, result.version_number, inst.name, result.filename
        )
    };
    let payload = json!({
        "instance_id": inst.id,
        "instance_name": inst.name,
        "project_id": result.project_id,
        "project_name": result.project_name,
        "version_number": result.version_number,
        "filename": result.filename,
        "path": result.path,
        "skipped": result.skipped,
        "installed": [result.project_name],
        "installed_count": if result.skipped { 0 } else { 1 },
    });
    let card = ok_card(
        "install_mod",
        msg.clone(),
        payload.clone(),
        card_args(&[
            ("query", "关键词", query),
            ("version", "版本", result.version_number.clone()),
            ("instance", "版本", inst.name.clone()),
        ]),
    );
    Ok(ToolExecResult {
        model_text: model_payload(&msg, &payload),
        message_zh: msg,
        card,
        action: None,
        report: None,
    })
}

async fn fetch_deps_impl(ctx: &ToolContext<'_>, args: &Value) -> easymc_core::Result<ToolExecResult> {
    let inst = need_instance(ctx.paths, instance_hint(ctx, args).as_deref())?;
    let settings = load_settings(ctx.paths).unwrap_or_default();
    easymc_mods::sync_api_key_from_settings(settings.curseforge_api_key.as_deref());
    let only = strings_arg(args, "only_ids");
    let r = fetch_missing_deps(&inst, only, |_p| {}).await?;
    let payload = serde_json::to_value(&r).unwrap_or(Value::Null);
    let mut msg = r.message_zh.clone();
    if !r.installed.is_empty() {
        msg.push_str(&format!("\n已补上 {} 个。", r.installed.len()));
    }
    let card = ok_card(
        "fetch_missing_mod_deps",
        msg.clone(),
        payload.clone(),
        card_args(&[("instance", "版本", inst.name.clone())]),
    );
    Ok(ToolExecResult {
        model_text: model_payload(&msg, &payload),
        message_zh: msg,
        card,
        action: None,
        report: None,
    })
}

async fn check_updates_impl(ctx: &ToolContext<'_>, args: &Value) -> easymc_core::Result<ToolExecResult> {
    let inst = need_instance(ctx.paths, instance_hint(ctx, args).as_deref())?;
    let settings = load_settings(ctx.paths).unwrap_or_default();
    easymc_mods::sync_api_key_from_settings(settings.curseforge_api_key.as_deref());
    let report = check_mod_updates(&inst).await?;
    let payload = serde_json::to_value(&report).unwrap_or(Value::Null);
    let msg = if report.update_count == 0 {
        format!("「{}」的模组都是最新兼容版本。", inst.name)
    } else {
        format!(
            "「{}」有 {} 个模组可更新（已是最新 {} 个）。",
            inst.name, report.update_count, report.up_to_date_count
        )
    };
    let card = ok_card(
        "check_mod_updates",
        msg.clone(),
        payload.clone(),
        card_args(&[("instance", "版本", inst.name.clone())]),
    );
    Ok(ToolExecResult {
        model_text: model_payload(&msg, &payload),
        message_zh: msg,
        card,
        action: None,
        report: None,
    })
}

async fn update_mods_impl(ctx: &ToolContext<'_>, args: &Value) -> easymc_core::Result<ToolExecResult> {
    let inst = need_instance(ctx.paths, instance_hint(ctx, args).as_deref())?;
    let settings = load_settings(ctx.paths).unwrap_or_default();
    easymc_mods::sync_api_key_from_settings(settings.curseforge_api_key.as_deref());
    let wanted = strings_arg(args, "filenames").unwrap_or_default();
    let scan = check_mod_updates(&inst).await?;
    let items: Vec<ModUpdateApplyItem> = scan
        .candidates
        .iter()
        .filter(|c| c.latest_version_id.is_some())
        .filter(|c| {
            wanted.is_empty()
                || wanted
                    .iter()
                    .any(|f| c.filename == *f || c.filename.starts_with(f.trim_end_matches(".jar")))
        })
        .filter_map(|c| {
            Some(ModUpdateApplyItem {
                filename: c.filename.clone(),
                source: c.source.clone().unwrap_or_else(|| "modrinth".into()),
                project_id: c.project_id.clone()?,
                version_id: c.latest_version_id.clone(),
            })
        })
        .collect();

    if items.is_empty() {
        let msg = if scan.update_count == 0 {
            format!("「{}」没有可更新的模组。", inst.name)
        } else {
            "你指定的模组没有可更新项。".to_string()
        };
        let payload = json!({ "updated": [], "skipped": [], "failed": [], "message_zh": msg });
        return Ok(ToolExecResult {
            model_text: model_payload(&msg, &payload),
            message_zh: msg.clone(),
            card: ok_card("update_mods", msg, payload, vec![]),
            action: None,
            report: None,
        });
    }

    let r = apply_mod_updates(&inst, &items, |_p| {}).await?;
    let payload = serde_json::to_value(&r).unwrap_or(Value::Null);
    let mut msg = r.message_zh.clone();
    if r.suggest_dep_check {
        msg.push_str("\n更新后建议再查一次依赖。");
    }
    let card = ok_card(
        "update_mods",
        msg.clone(),
        payload.clone(),
        card_args(&[
            ("instance", "版本", inst.name.clone()),
            ("count", "更新数", format!("{}", r.updated.len())),
        ]),
    );
    Ok(ToolExecResult {
        model_text: model_payload(&msg, &payload),
        message_zh: msg,
        card,
        action: None,
        report: None,
    })
}

/// Run the real host-side Java provisioning step.
///
/// Previously `ensure_java` only produced a「正在准备…」answer and relied on the
/// Tauri/CLI host to finish it *after* the loop returned — every non-last step
/// was silently dropped. The agent now performs the work itself and marks the
/// payload `executed: true` so hosts do not run it a second time.
pub async fn ensure_java_action(paths: &Paths, major: u32) -> ActionResult {
    match easymc_launch_desktop::ensure_java(paths, major, |_| {}).await {
        Ok(j) => {
            let path = j.path.display().to_string();
            ActionResult {
                ok: true,
                action: "ensure_java".into(),
                message_zh: format!(
                    "Java {} 已准备好。\n路径：{path}\n来源：{}\n下次启动游戏会用它。",
                    j.major, j.source
                ),
                path: Some(path.clone()),
                value: Some(json!({
                    "major": j.major,
                    "path": path,
                    "source": j.source,
                    "executed": true,
                })),
            }
        }
        Err(e) => ActionResult {
            ok: false,
            action: "ensure_java".into(),
            message_zh: format!(
                "准备 Java {major} 没成功：{e}\n可以再试一次，或检查网络/下载源后重试。"
            ),
            path: None,
            value: Some(json!({
                "major": major,
                "executed": true,
                "error": e.to_string(),
            })),
        },
    }
}

/// Compose环境状态 + install the matching JDK in one step.
pub async fn setup_env_action(paths: &Paths, instance_id: Option<&str>) -> ActionResult {
    let base = env_status_summary(paths, instance_id);
    let value = base.value.clone().unwrap_or(Value::Null);
    let major = value
        .get("major")
        .and_then(|v| v.as_u64())
        .unwrap_or(21) as u32;
    let download_zh = value
        .get("download_source_zh")
        .and_then(|v| v.as_str())
        .unwrap_or("官方")
        .to_string();
    let memory_mb = value
        .get("memory_mb")
        .and_then(|v| v.as_u64())
        .unwrap_or(2048);
    let instance_name = value
        .get("instance_name")
        .and_then(|v| v.as_str())
        .unwrap_or("（未选版本）")
        .to_string();
    match easymc_launch_desktop::ensure_java(paths, major, |_| {}).await {
        Ok(j) => {
            let path = j.path.display().to_string();
            ActionResult {
                ok: true,
                action: "setup_env".into(),
                message_zh: format!(
                    "{}\n\nJava {major} 已就绪。\n路径：{path}\n来源：{}\n下载源：{download_zh}\n内存约 {memory_mb} MB。\n下次启动「{instance_name}」会用这套环境。",
                    base.message_zh, j.source
                ),
                path: Some(path.clone()),
                value: Some(json!({
                    "major": j.major,
                    "path": path,
                    "source": j.source,
                    "download_source_zh": download_zh,
                    "memory_mb": memory_mb,
                    "instance_id": value.get("instance_id"),
                    "instance_name": instance_name,
                    "executed": true,
                })),
            }
        }
        Err(e) => ActionResult {
            ok: false,
            action: "setup_env".into(),
            message_zh: format!(
                "{}\n\n准备 Java {major} 没成功：{e}\n可检查网络后再试一次。",
                base.message_zh
            ),
            path: None,
            value: Some(json!({ "major": major, "executed": true, "error": e.to_string() })),
        },
    }
}

/// Export the resolved instance as a Modrinth `.mrpack`.
pub async fn build_pack_action(paths: &Paths, instance_id: Option<&str>) -> ActionResult {
    let Some(inst) = resolve_instance(paths, instance_id) else {
        return ActionResult {
            ok: false,
            action: "build_pack".into(),
            message_zh: "还没有选版本，没法生成整合包。请先在启动页或助手顶部选一个版本。".into(),
            path: None,
            value: Some(json!({ "executed": true })),
        };
    };
    match easymc_share::export_instance(
        paths,
        &inst,
        easymc_share::ExportOptions {
            mode: easymc_share::ShareMode::Mrpack,
            notes: Some("由助手「制作整合包」导出".into()),
            output: None,
            resolve_modrinth: true,
        },
    )
    .await
    {
        Ok(r) => ActionResult {
            ok: true,
            action: "build_pack".into(),
            message_zh: format!(
                "整合包已生成（Modrinth .mrpack）。\n版本：「{}」\n模组数：{}\n路径：{}\n{}",
                inst.name, r.mod_count, r.path, r.message_zh
            ),
            path: Some(r.path.clone()),
            value: Some(json!({
                "instance_id": inst.id,
                "instance_name": inst.name,
                "path": r.path,
                "mod_count": r.mod_count,
                "bytes": r.bytes,
                "format": "mrpack",
                "resolved": r.resolved,
                "overrides": r.overrides,
                "executed": true,
            })),
        },
        Err(e) => ActionResult {
            ok: false,
            action: "build_pack".into(),
            message_zh: format!("生成整合包失败：{e}"),
            path: None,
            value: Some(json!({ "instance_id": inst.id, "executed": true, "error": e.to_string() })),
        },
    }
}

/// Import a `.mrpack` (or EasyMC bundle) as a new instance.
pub async fn import_mrpack_action(paths: &Paths, raw_path: &str) -> ActionResult {
    let path = raw_path.trim();
    if path.is_empty() {
        return ActionResult {
            ok: false,
            action: "import_mrpack".into(),
            message_zh: "请带上整合包路径，例如：导入整合包 /path/to/pack.mrpack".into(),
            path: None,
            value: Some(json!({ "executed": true })),
        };
    }
    match easymc_share::import_instance(
        paths,
        std::path::Path::new(path),
        easymc_share::ImportOptions {
            name: None,
            apply_memory: false,
            offline: false,
        },
    )
    .await
    {
        Ok(r) => ActionResult {
            ok: true,
            action: "import_mrpack".into(),
            message_zh: format!(
                "整合包已导入为新版本「{}」。\n模组：下载 {} / 覆盖 {}；缺 {}。\n{}",
                r.instance_name,
                r.mods_downloaded,
                r.mods_copied,
                r.mods_missing.len(),
                r.message_zh
            ),
            path: Some(r.path_loaded.clone()),
            value: Some(json!({
                "path": r.path_loaded,
                "instance_id": r.instance_id,
                "instance_name": r.instance_name,
                "mods_downloaded": r.mods_downloaded,
                "mods_copied": r.mods_copied,
                "mods_missing": r.mods_missing,
                "executed": true,
            })),
        },
        Err(e) => ActionResult {
            ok: false,
            action: "import_mrpack".into(),
            message_zh: format!("导入整合包失败：{e}"),
            path: Some(path.to_string()),
            value: Some(json!({ "executed": true, "error": e.to_string() })),
        },
    }
}

fn suggest_theme_query(theme: &str) -> (&'static str, &'static str) {
    match theme.trim().to_ascii_lowercase().as_str() {
        "tech" | "科技" | "工业" => ("create", "科技"),
        "magic" | "魔法" => ("botania", "魔法"),
        "performance" | "性能" | "优化" => ("sodium", "性能"),
        "qos" | "生活质量" | "便利" => ("jade", "生活质量"),
        "adventure" | "冒险" => ("origins", "冒险"),
        "building" | "建筑" => ("chisel", "建筑"),
        "shader" | "光影" => ("iris", "光影"),
        _ => ("farmers-delight", "生存"),
    }
}

fn folder_kind(raw: Option<String>) -> GameFolderKind {
    match raw.as_deref().unwrap_or("mods").to_ascii_lowercase().as_str() {
        "resourcepacks" | "resourcepack" | "资源包" => GameFolderKind::ResourcePacks,
        "shaderpacks" | "shaderpack" | "光影" => GameFolderKind::ShaderPacks,
        "saves" | "存档" => GameFolderKind::Saves,
        "config" | "配置" => GameFolderKind::Config,
        "logs" | "日志" => GameFolderKind::Logs,
        "crash-reports" | "crash_reports" | "崩溃" => GameFolderKind::CrashReports,
        _ => GameFolderKind::Mods,
    }
}

/// Execute a tool by canonical name with typed JSON arguments.
pub async fn execute(ctx: &ToolContext<'_>, requested: &str, args: &Value) -> ToolExecResult {
    if ctx.cancelled() {
        return from_error(requested, "已停止");
    }
    let name = canonical_tool_name(requested);
    // The agent loop gates these behind an explicit player confirmation; a
    // stray direct call must never mutate the disk.
    if !ctx.confirmed
        && matches!(name, "delete_instance" | "delete_mod" | "toggle_mod")
    {
        return from_error(
            requested,
            "这个操作会改动本地文件，需要你确认后才会执行。",
        );
    }
    let out = match name {
        "search_mods" => search_mods_impl(ctx, args).await,
        "install_mod" => install_mod_impl(ctx, args).await,
        "fetch_missing_mod_deps" => fetch_deps_impl(ctx, args).await,
        "check_mod_updates" => check_updates_impl(ctx, args).await,
        "update_mods" => update_mods_impl(ctx, args).await,
        "ensure_java" => {
            let major = u_arg(args, "major").unwrap_or(21);
            Ok(from_action_result(
                requested,
                ensure_java_action(ctx.paths, major).await,
            ))
        }
        "setup_env" => {
            let hint = instance_hint(ctx, args);
            Ok(from_action_result(
                requested,
                setup_env_action(ctx.paths, hint.as_deref()).await,
            ))
        }
        "build_pack" => {
            let hint = instance_hint(ctx, args);
            Ok(from_action_result(
                requested,
                build_pack_action(ctx.paths, hint.as_deref()).await,
            ))
        }
        "import_mrpack" => {
            let path = s_arg(args, "path").unwrap_or_default();
            Ok(from_action_result(
                requested,
                import_mrpack_action(ctx.paths, &path).await,
            ))
        }
        "suggest_mods" => {
            let theme = s_arg(args, "theme").unwrap_or_else(|| "survival".into());
            let (query, theme_zh) = suggest_theme_query(&theme);
            match search_mods_impl(ctx, &json!({ "query": query })).await {
                Ok(mut out) => {
                    let msg = format!("按「{theme_zh}」主题推荐：\n{}", out.message_zh);
                    out.message_zh = msg.clone();
                    out.model_text = format!("{msg}\n[主题] {theme}");
                    out.card.title_zh = "推荐模组".into();
                    out.card.result_zh = Some(msg);
                    Ok(out)
                }
                Err(e) => Err(e),
            }
        }
        "open_store" => {
            let query = s_arg(args, "query").unwrap_or_default();
            let ar = ActionResult {
                ok: true,
                action: "open_store".into(),
                message_zh: "打开下载页继续找。".into(),
                path: None,
                value: Some(json!({ "query": query, "tab": "mod" })),
            };
            Ok(from_action_result(requested, ar))
        }
        _ => {
            let iid = ctx.instance_id.clone();
            let intent = match name {
                "diagnose" => Intent::Diagnose {
                    instance_id: iid,
                    log_path: s_arg(args, "log_path"),
                },
                "list_installed_mods" => Intent::CheckMods { instance_id: iid },
                "check_mod_deps" => Intent::CheckModDeps { instance_id: iid },
                "toggle_mod" => Intent::ToggleMod {
                    query: s_arg(args, "query").unwrap_or_default(),
                    enable: b_arg(args, "enable"),
                    instance_id: iid,
                },
                "delete_mod" => Intent::DeleteMod {
                    query: s_arg(args, "query").unwrap_or_default(),
                    instance_id: iid,
                },
                "list_instances" => Intent::ListInstances,
                "select_instance" => Intent::SelectInstance {
                    query: s_arg(args, "query").unwrap_or_default(),
                },
                "rename_instance" => Intent::RenameInstance {
                    query: s_arg(args, "query"),
                    new_name: s_arg(args, "new_name").unwrap_or_default(),
                },
                "duplicate_instance" => Intent::DuplicateInstance {
                    query: s_arg(args, "query"),
                    new_name: s_arg(args, "new_name"),
                },
                "delete_instance" => Intent::DeleteInstance {
                    query: s_arg(args, "query").unwrap_or_default(),
                },
                "open_latest_log" | "open_log" => Intent::OpenLatestLog { instance_id: iid },
                "open_screenshots" => Intent::OpenScreenshots { instance_id: iid },
                "open_instance_dir" => Intent::OpenInstanceDir { instance_id: iid },
                "show_data_dir" => Intent::ShowDataDir,
                "open_game_folder" => Intent::OpenGameFolder {
                    folder: folder_kind(s_arg(args, "kind")),
                    instance_id: iid,
                },
                "list_accounts" => Intent::ListAccounts,
                "select_account" => Intent::SelectAccount {
                    query: s_arg(args, "query").unwrap_or_default(),
                },
                "bump_memory" => {
                    if let Some(mb) = u_arg(args, "mb").or_else(|| u_arg(args, "memory_mb")) {
                        Intent::BumpMemory { mb }
                    } else {
                        Intent::TuneConfig { instance_id: iid }
                    }
                }
                "propose_changes" => Intent::TuneConfig { instance_id: iid },
                "set_download_source" => Intent::SetDownloadSource {
                    source: s_arg(args, "source").unwrap_or_default(),
                },
                "diagnostics" => Intent::DiagnosticsBundle,
                "help" => Intent::Help,
                other => {
                    // MCP tools are namespaced `mcp__server__tool`.
                    if other.starts_with("mcp__") {
                        return crate::mcp::execute_tool(requested, args, ctx.cancel.as_deref()).await;
                    }
                    return from_error(requested, format!("未知工具 {other}"));
                }
            };
            via_intent(ctx, requested, intent).await
        }
    };
    out.unwrap_or_else(|e| from_error(requested, e))
}

/// Execute a tool that the player already approved in the UI.
pub async fn execute_confirmed(paths: &Paths, name: &str, args: &Value, instance_id: Option<String>) -> ToolExecResult {
    let ctx = ToolContext {
        paths,
        instance_id,
        process_logs: None,
        confirmed: true,
        cancel: None,
    };
    execute(&ctx, name, args).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_paths() -> Paths {
        let dir = std::env::temp_dir().join(format!(
            "easymc-tool-runtime-{}-{}",
            std::process::id(),
            chrono::Local::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Paths::from_root(dir)
    }

    #[test]
    fn legacy_names_map_to_canonical_tools() {
        assert_eq!(canonical_tool_name("check_mods"), "list_installed_mods");
        assert_eq!(canonical_tool_name("mod_dep_graph"), "check_mod_deps");
        assert_eq!(canonical_tool_name("install_missing_deps"), "fetch_missing_mod_deps");
        assert_eq!(canonical_tool_name("batch_update_mods"), "update_mods");
        assert_eq!(canonical_tool_name("analyze_crash_log"), "diagnose");
        assert_eq!(canonical_tool_name("create_modpack"), "build_pack");
        assert_eq!(canonical_tool_name("open_log"), "open_log");
    }

    #[tokio::test]
    async fn unknown_tool_returns_error_card_instead_of_panicking() {
        let paths = temp_paths();
        let out = execute(&ToolContext::new(&paths), "does_not_exist", &json!({})).await;
        assert_eq!(out.card.status, ToolStatus::Error);
        assert!(out.message_zh.contains("未知工具"));
    }

    #[tokio::test]
    async fn install_without_instance_tells_player_what_to_do() {
        let paths = temp_paths();
        let out = execute(
            &ToolContext::new(&paths),
            "install_mod",
            &json!({ "query": "sodium" }),
        )
        .await;
        assert_eq!(out.card.status, ToolStatus::Error);
        assert!(out.message_zh.contains("选版本"), "{}", out.message_zh);
    }

    #[tokio::test]
    async fn cancelled_context_does_not_run_tools() {
        let paths = temp_paths();
        let cancel = Arc::new(AtomicBool::new(true));
        let ctx = ToolContext {
            paths: &paths,
            instance_id: None,
            process_logs: None,
            confirmed: false,
            cancel: Some(cancel),
        };
        let out = execute(&ctx, "list_instances", &json!({})).await;
        assert_eq!(out.card.status, ToolStatus::Error);
        assert!(out.message_zh.contains("已停止"));
    }
}
