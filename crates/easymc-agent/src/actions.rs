//! Safe guided actions: memory tip, data dir, diagnostic bundle, mirror, mods peek.

use std::fs;
use std::path::PathBuf;

use chrono::Local;
use serde::{Deserialize, Serialize};

use easymc_core::{
    load_accounts, load_instances, load_settings, save_settings, set_active_instance,
    AccountKind, DownloadSource, Paths, Result,
};

use crate::diagnose::DiagnoseReport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub ok: bool,
    pub action: String,
    pub message_zh: String,
    pub path: Option<String>,
    pub value: Option<serde_json::Value>,
}

pub fn show_data_dir(paths: &Paths) -> ActionResult {
    ActionResult {
        ok: true,
        action: "show_data_dir".into(),
        message_zh: format!("数据目录：{}", paths.root.display()),
        path: Some(paths.root.display().to_string()),
        value: None,
    }
}

/// Show selected / active instance game folder path.
pub fn show_instance_dir(paths: &Paths, instance_id: Option<&str>) -> ActionResult {
    let store = match load_instances(paths) {
        Ok(s) => s,
        Err(e) => {
            return ActionResult {
                ok: false,
                action: "open_instance_dir".into(),
                message_zh: format!("读取版本列表失败：{e}"),
                path: None,
                value: None,
            };
        }
    };
    let id = instance_id
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .or(store.active_id.clone());
    let Some(id) = id else {
        return ActionResult {
            ok: false,
            action: "open_instance_dir".into(),
            message_zh: "还没有选版本。请先在启动页或助手顶部选一个版本。".into(),
            path: None,
            value: None,
        };
    };
    let Some(inst) = store.instances.iter().find(|i| i.id == id) else {
        return ActionResult {
            ok: false,
            action: "open_instance_dir".into(),
            message_zh: format!("找不到版本 {id}"),
            path: None,
            value: None,
        };
    };
    ActionResult {
        ok: true,
        action: "open_instance_dir".into(),
        message_zh: format!(
            "版本「{}」文件夹：\n{}",
            inst.name, inst.game_dir
        ),
        path: Some(inst.game_dir.clone()),
        value: Some(serde_json::json!({
            "instance_id": inst.id,
            "instance_name": inst.name,
            "game_dir": inst.game_dir,
        })),
    }
}

/// Write suggested memory into settings.json (clamped). Immediate apply (shortcut).
pub fn bump_memory(paths: &Paths, mb: u32) -> Result<ActionResult> {
    let mb = mb.clamp(1024, 32768);
    let mut s = load_settings(paths)?;
    let old = s.memory_mb;
    s.memory_mb = mb;
    save_settings(paths, &s)?;
    Ok(ActionResult {
        ok: true,
        action: "bump_memory".into(),
        message_zh: format!("已把游戏内存从 {old} MB 调到 {mb} MB。请重新启动游戏后生效。"),
        path: Some(paths.settings_file().display().to_string()),
        value: Some(serde_json::json!({ "memory_mb": mb, "previous": old })),
    })
}

/// Switch download_source (official / bmclapi).
pub fn set_download_source(paths: &Paths, source: &str) -> Result<ActionResult> {
    let parsed = DownloadSource::parse(source).ok_or_else(|| {
        easymc_core::EasyMcError::msg(format!(
            "未知下载源：{source}（可用 official / bmclapi）"
        ))
    })?;
    let mut s = load_settings(paths)?;
    let old = s.download_source;
    s.download_source = parsed;
    save_settings(paths, &s)?;
    Ok(ActionResult {
        ok: true,
        action: "set_download_source".into(),
        message_zh: format!(
            "下载源已从「{}」切换为「{}」。新下载会走这个源。",
            old.label_zh(),
            parsed.label_zh()
        ),
        path: Some(paths.settings_file().display().to_string()),
        value: Some(serde_json::json!({
            "download_source": parsed.as_str(),
            "previous": old.as_str(),
            "label_zh": parsed.label_zh(),
        })),
    })
}

/// Light mods check: count jars under instance mods/ and nudge diagnose if many.
pub fn check_mods(paths: &Paths, instance_id: Option<&str>) -> ActionResult {
    let store = match load_instances(paths) {
        Ok(s) => s,
        Err(e) => {
            return ActionResult {
                ok: false,
                action: "check_mods".into(),
                message_zh: format!("读取版本列表失败：{e}"),
                path: None,
                value: None,
            };
        }
    };
    let id = instance_id
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .or(store.active_id.clone());
    let Some(id) = id else {
        return ActionResult {
            ok: false,
            action: "check_mods".into(),
            message_zh: "还没有选版本，没法检查模组。请先选一个版本。".into(),
            path: None,
            value: None,
        };
    };
    let Some(inst) = store.instances.iter().find(|i| i.id == id) else {
        return ActionResult {
            ok: false,
            action: "check_mods".into(),
            message_zh: format!("找不到版本 {id}"),
            path: None,
            value: None,
        };
    };
    let mods_dir = PathBuf::from(&inst.game_dir).join("mods");
    let mut names: Vec<String> = Vec::new();
    if mods_dir.is_dir() {
        if let Ok(rd) = fs::read_dir(&mods_dir) {
            for ent in rd.flatten() {
                let name = ent.file_name().to_string_lossy().to_string();
                let lower = name.to_ascii_lowercase();
                if lower.ends_with(".jar") || lower.ends_with(".jar.disabled") {
                    names.push(name);
                }
            }
        }
    }
    names.sort();
    let count = names.len();
    let preview: Vec<_> = names.iter().take(12).cloned().collect();
    let more = if count > preview.len() {
        format!("\n…还有 {} 个", count - preview.len())
    } else {
        String::new()
    };
    let tip = if count == 0 {
        "这个版本还没装模组。".to_string()
    } else if count >= 40 {
        format!(
            "模组比较多（{count} 个），若启动变慢或闪退，可点「查看崩溃」做一次诊断。"
        )
    } else if count >= 15 {
        format!("共 {count} 个模组。若最近加过新模组后崩了，建议「查看崩溃」。")
    } else {
        format!("共 {count} 个模组，看起来还好。")
    };
    let list = if preview.is_empty() {
        String::new()
    } else {
        format!("\n{}", preview.join("\n"))
    };
    ActionResult {
        ok: true,
        action: "check_mods".into(),
        message_zh: format!(
            "版本「{}」模组目录：{}\n{}{}{}",
            inst.name,
            mods_dir.display(),
            tip,
            list,
            more
        ),
        path: Some(mods_dir.display().to_string()),
        value: Some(serde_json::json!({
            "instance_id": inst.id,
            "instance_name": inst.name,
            "count": count,
            "mods": names,
            "suggest_diagnose": count >= 15,
        })),
    }
}

/// Pack diagnose report + log excerpt into `diagnostics/easymc-diag-*.txt`.
pub fn write_diagnostic_bundle(paths: &Paths, report: &DiagnoseReport) -> Result<ActionResult> {
    let dir = paths.root.join("diagnostics");
    fs::create_dir_all(&dir)?;
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let path: PathBuf = dir.join(format!("easymc-diag-{stamp}.txt"));
    let mut body = String::new();
    body.push_str(&format!("EasyMC diagnostic bundle\n生成时间: {}\n", Local::now()));
    body.push_str(&format!("来源: {}\n", report.source_label));
    body.push_str(&format!("摘要: {}\n\n", report.summary_zh));
    body.push_str("=== findings ===\n");
    for f in &report.findings {
        body.push_str(&format!(
            "- [{}] {} — {}\n  建议: {}\n  证据: {}\n",
            format!("{:?}", f.severity).to_ascii_lowercase(),
            f.title,
            f.summary,
            f.fixes.join(" | "),
            f.evidence.as_deref().unwrap_or("(无)")
        ));
    }
    body.push_str("\n=== sources ===\n");
    for s in &report.sources {
        body.push_str(&format!("- {s}\n"));
    }
    body.push_str("\n=== log excerpt ===\n");
    body.push_str(&report.log_excerpt);
    fs::write(&path, body)?;
    Ok(ActionResult {
        ok: true,
        action: "diagnostics".into(),
        message_zh: format!(
            "问题报告已保存到：{}
可发给朋友，或自己留档排查。",
            path.display()
        ),
        path: Some(path.display().to_string()),
        value: None,
    })
}

/// Text-only memory suggestion (no write).
pub fn suggest_memory_text(current_mb: u32, suggested_mb: u32) -> ActionResult {
    ActionResult {
        ok: true,
        action: "suggest_memory".into(),
        message_zh: format!(
            "当前内存约 {current_mb} MB。建议调到 {suggested_mb} MB（助手里说「内存 {suggested_mb}」即可）。"
        ),
        path: None,
        value: Some(serde_json::json!({ "current": current_mb, "suggested": suggested_mb })),
    }
}

/// Compose a status summary used by Setup env (before host ensure_java).
pub fn env_status_summary(paths: &Paths, instance_id: Option<&str>) -> ActionResult {
    let settings = load_settings(paths).unwrap_or_default();
    let store = load_instances(paths).ok();
    let id = instance_id
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| store.as_ref().and_then(|s| s.active_id.clone()));
    let (name, version, major) = if let (Some(id), Some(store)) = (&id, &store) {
        if let Some(inst) = store.instances.iter().find(|i| i.id == *id) {
            let ver = inst.version.0.clone();
            let major = crate::propose::java_major_for_mc(&ver);
            (inst.name.clone(), ver, major)
        } else {
            ("（未知）".into(), "1.21.1".into(), 21u32)
        }
    } else {
        ("（未选版本）".into(), "1.21.1".into(), 21u32)
    };
    let mem = if let (Some(id), Some(store)) = (&id, &store) {
        store
            .instances
            .iter()
            .find(|i| i.id == *id)
            .and_then(|i| i.memory_mb)
            .unwrap_or(settings.memory_mb)
    } else {
        settings.memory_mb
    };
    ActionResult {
        ok: true,
        action: "setup_env".into(),
        message_zh: format!(
            "一键环境：版本「{name}」（{version}）→ Java {major}；当前内存约 {mem} MB；下载源：{}。",
            settings.download_source.label_zh()
        ),
        path: None,
        value: Some(serde_json::json!({
            "major": major,
            "instance_id": id,
            "instance_name": name,
            "mc_version": version,
            "memory_mb": mem,
            "download_source": settings.download_source.as_str(),
            "download_source_zh": settings.download_source.label_zh(),
        })),
    }
}


/// Enable or disable a mod jar by renaming `.jar` ↔ `.jar.disabled` (PCL/HMCL-style).
/// `query` matches substring of filename (case-insensitive). `enable`: Some(true/false) or None to flip.
pub fn toggle_mod(
    paths: &Paths,
    instance_id: Option<&str>,
    query: &str,
    enable: Option<bool>,
) -> ActionResult {
    let q = query.trim();
    if q.is_empty() {
        return ActionResult {
            ok: false,
            action: "toggle_mod".into(),
            message_zh: "请告诉我模组文件名关键词，例如「禁用 sodium」或「启用 iris」。".into(),
            path: None,
            value: None,
        };
    }
    let store = match load_instances(paths) {
        Ok(s) => s,
        Err(e) => {
            return ActionResult {
                ok: false,
                action: "toggle_mod".into(),
                message_zh: format!("读取版本列表失败：{e}"),
                path: None,
                value: None,
            };
        }
    };
    let id = instance_id
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .or(store.active_id.clone());
    let Some(id) = id else {
        return ActionResult {
            ok: false,
            action: "toggle_mod".into(),
            message_zh: "还没有选版本。请先在启动页或助手顶部选一个版本。".into(),
            path: None,
            value: None,
        };
    };
    let Some(inst) = store.instances.iter().find(|i| i.id == id) else {
        return ActionResult {
            ok: false,
            action: "toggle_mod".into(),
            message_zh: format!("找不到版本 {id}"),
            path: None,
            value: None,
        };
    };
    let mods_dir = PathBuf::from(&inst.game_dir).join("mods");
    if !mods_dir.is_dir() {
        return ActionResult {
            ok: false,
            action: "toggle_mod".into(),
            message_zh: format!("版本「{}」还没有 mods 文件夹。", inst.name),
            path: Some(mods_dir.display().to_string()),
            value: None,
        };
    }
    let q_l = q.to_ascii_lowercase();
    let mut hits: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = fs::read_dir(&mods_dir) {
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().to_string();
            let lower = name.to_ascii_lowercase();
            if !(lower.ends_with(".jar") || lower.ends_with(".jar.disabled")) {
                continue;
            }
            if lower.contains(&q_l) {
                hits.push(ent.path());
            }
        }
    }
    hits.sort();
    if hits.is_empty() {
        return ActionResult {
            ok: false,
            action: "toggle_mod".into(),
            message_zh: format!(
                "在「{}」的模组里没找到包含「{q}」的文件。可以说「已装模组」先看列表。",
                inst.name
            ),
            path: Some(mods_dir.display().to_string()),
            value: Some(serde_json::json!({
                "instance_id": inst.id,
                "instance_name": inst.name,
                "query": q,
            })),
        };
    }
    if hits.len() > 1 {
        let names: Vec<String> = hits
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        let preview = names.iter().take(8).cloned().collect::<Vec<_>>().join("\n");
        return ActionResult {
            ok: false,
            action: "toggle_mod".into(),
            message_zh: format!(
                "找到多个匹配「{q}」的模组，请说得更具体一点：\n{preview}"
            ),
            path: Some(mods_dir.display().to_string()),
            value: Some(serde_json::json!({
                "instance_id": inst.id,
                "instance_name": inst.name,
                "query": q,
                "matches": names,
            })),
        };
    }
    let src = &hits[0];
    let name = src.file_name().unwrap().to_string_lossy().to_string();
    let lower = name.to_ascii_lowercase();
    let currently_disabled = lower.ends_with(".jar.disabled");
    let want_enable = enable.unwrap_or(currently_disabled);
    if want_enable && !currently_disabled {
        return ActionResult {
            ok: true,
            action: "toggle_mod".into(),
            message_zh: format!("「{name}」本来就是启用状态，不用改。"),
            path: Some(src.display().to_string()),
            value: Some(serde_json::json!({
                "instance_id": inst.id,
                "instance_name": inst.name,
                "filename": name,
                "enabled": true,
                "changed": false,
            })),
        };
    }
    if !want_enable && currently_disabled {
        return ActionResult {
            ok: true,
            action: "toggle_mod".into(),
            message_zh: format!("「{name}」本来就是禁用状态，不用改。"),
            path: Some(src.display().to_string()),
            value: Some(serde_json::json!({
                "instance_id": inst.id,
                "instance_name": inst.name,
                "filename": name,
                "enabled": false,
                "changed": false,
            })),
        };
    }
    let dest_name = if want_enable {
        name.trim_end_matches(".disabled").to_string()
    } else {
        format!("{name}.disabled")
    };
    let dest = mods_dir.join(&dest_name);
    if dest.exists() {
        return ActionResult {
            ok: false,
            action: "toggle_mod".into(),
            message_zh: format!("目标文件已存在：{dest_name}，没法改名。"),
            path: Some(mods_dir.display().to_string()),
            value: None,
        };
    }
    match fs::rename(src, &dest) {
        Ok(()) => {
            let verb = if want_enable { "已启用" } else { "已禁用" };
            ActionResult {
                ok: true,
                action: "toggle_mod".into(),
                message_zh: format!(
                    "{verb}模组「{dest_name}」（版本「{}」）。下次启动游戏生效。",
                    inst.name
                ),
                path: Some(dest.display().to_string()),
                value: Some(serde_json::json!({
                    "instance_id": inst.id,
                    "instance_name": inst.name,
                    "filename": dest_name,
                    "previous": name,
                    "enabled": want_enable,
                    "changed": true,
                })),
            }
        }
        Err(e) => ActionResult {
            ok: false,
            action: "toggle_mod".into(),
            message_zh: format!("改名失败：{e}"),
            path: Some(src.display().to_string()),
            value: None,
        },
    }
}

/// Summarize installed versions for the agent card (PCL-style quick glance).
pub fn list_instances(paths: &Paths) -> ActionResult {
    let store = match load_instances(paths) {
        Ok(s) => s,
        Err(e) => {
            return ActionResult {
                ok: false,
                action: "list_instances".into(),
                message_zh: format!("读取版本列表失败：{e}"),
                path: None,
                value: None,
            };
        }
    };
    if store.instances.is_empty() {
        return ActionResult {
            ok: true,
            action: "list_instances".into(),
            message_zh: "还没有安装任何版本。去「下载」页装一个原版或加载器版本吧。".into(),
            path: None,
            value: Some(serde_json::json!({ "count": 0, "instances": [] })),
        };
    }
    let active = store.active_id.clone();
    let mut lines = Vec::new();
    let mut items = Vec::new();
    for inst in &store.instances {
        let mark = if Some(&inst.id) == active.as_ref() {
            "★ "
        } else {
            ""
        };
        let loader = inst.loader.as_str();
        let line = format!(
            "{mark}{} · {} · {}",
            inst.name,
            inst.version.0,
            loader
        );
        lines.push(line);
        items.push(serde_json::json!({
            "id": inst.id,
            "name": inst.name,
            "version": inst.version.0,
            "loader": loader,
            "active": Some(&inst.id) == active.as_ref(),
        }));
    }
    ActionResult {
        ok: true,
        action: "list_instances".into(),
        message_zh: format!(
            "已安装 {} 个版本（★ 为当前）：\n{}",
            store.instances.len(),
            lines.join("\n")
        ),
        path: None,
        value: Some(serde_json::json!({
            "count": store.instances.len(),
            "active_id": active,
            "instances": items,
        })),
    }
}


/// Switch the active / current game version by name, id, or version string (PCL-style).
pub fn select_instance(paths: &Paths, query: &str) -> ActionResult {
    let q = query.trim();
    if q.is_empty() {
        return ActionResult {
            ok: false,
            action: "select_instance".into(),
            message_zh: "请告诉我要切换到哪个版本，例如：切换版本 1.21.1".into(),
            path: None,
            value: None,
        };
    }
    let store = match load_instances(paths) {
        Ok(s) => s,
        Err(e) => {
            return ActionResult {
                ok: false,
                action: "select_instance".into(),
                message_zh: format!("读取版本列表失败：{e}"),
                path: None,
                value: None,
            };
        }
    };
    if store.instances.is_empty() {
        return ActionResult {
            ok: false,
            action: "select_instance".into(),
            message_zh: "还没有安装任何版本。去「下载」页装一个吧。".into(),
            path: None,
            value: Some(serde_json::json!({ "count": 0 })),
        };
    }
    let ql = q.to_lowercase();
    let matches: Vec<_> = store
        .instances
        .iter()
        .filter(|inst| {
            inst.id.to_lowercase().contains(&ql)
                || inst.name.to_lowercase().contains(&ql)
                || inst.version.0.to_lowercase().contains(&ql)
                || inst.loader.as_str().to_lowercase().contains(&ql)
        })
        .collect();
    if matches.is_empty() {
        let names: Vec<_> = store.instances.iter().map(|i| i.name.as_str()).collect();
        return ActionResult {
            ok: false,
            action: "select_instance".into(),
            message_zh: format!(
                "没找到匹配「{q}」的版本。已安装：{}",
                names.join("、")
            ),
            path: None,
            value: Some(serde_json::json!({
                "query": q,
                "count": store.instances.len(),
            })),
        };
    }
    if matches.len() > 1 {
        let lines: Vec<_> = matches
            .iter()
            .map(|i| format!("{} · {} · {}", i.name, i.version.0, i.loader.as_str()))
            .collect();
        return ActionResult {
            ok: false,
            action: "select_instance".into(),
            message_zh: format!(
                "「{q}」匹配到多个版本，请说得更具体一点：\n{}",
                lines.join("\n")
            ),
            path: None,
            value: Some(serde_json::json!({
                "query": q,
                "matches": matches.iter().map(|i| serde_json::json!({
                    "id": i.id,
                    "name": i.name,
                    "version": i.version.0,
                    "loader": i.loader.as_str(),
                })).collect::<Vec<_>>(),
            })),
        };
    }
    let target = matches[0];
    if store.active_id.as_deref() == Some(target.id.as_str()) {
        return ActionResult {
            ok: true,
            action: "select_instance".into(),
            message_zh: format!(
                "当前已经是「{}」（{} · {}）。",
                target.name,
                target.version.0,
                target.loader.as_str()
            ),
            path: Some(target.game_dir.clone()),
            value: Some(serde_json::json!({
                "instance_id": target.id,
                "instance_name": target.name,
                "version": target.version.0,
                "loader": target.loader.as_str(),
                "changed": false,
            })),
        };
    }
    match set_active_instance(paths, &target.id) {
        Ok(_) => ActionResult {
            ok: true,
            action: "select_instance".into(),
            message_zh: format!(
                "已切换到「{}」（{} · {}）。启动页会跟着重显示这个版本。",
                target.name,
                target.version.0,
                target.loader.as_str()
            ),
            path: Some(target.game_dir.clone()),
            value: Some(serde_json::json!({
                "instance_id": target.id,
                "instance_name": target.name,
                "version": target.version.0,
                "loader": target.loader.as_str(),
                "changed": true,
            })),
        },
        Err(e) => ActionResult {
            ok: false,
            action: "select_instance".into(),
            message_zh: format!("切换版本失败：{e}"),
            path: None,
            value: None,
        },
    }
}

/// Locate latest.log or the newest crash-report for the current / given instance.
pub fn open_latest_log(paths: &Paths, instance_id: Option<&str>) -> ActionResult {
    let store = match load_instances(paths) {
        Ok(s) => s,
        Err(e) => {
            return ActionResult {
                ok: false,
                action: "open_latest_log".into(),
                message_zh: format!("读取版本列表失败：{e}"),
                path: None,
                value: None,
            };
        }
    };
    let id = instance_id
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .or(store.active_id.clone());
    let Some(id) = id else {
        return ActionResult {
            ok: false,
            action: "open_latest_log".into(),
            message_zh: "还没有选版本。请先在启动页选一个，或说「版本列表」。".into(),
            path: None,
            value: None,
        };
    };
    let Some(inst) = store.instances.iter().find(|i| i.id == id) else {
        return ActionResult {
            ok: false,
            action: "open_latest_log".into(),
            message_zh: format!("找不到版本 {id}"),
            path: None,
            value: None,
        };
    };
    let game = paths.instance_game_dir(&id);
    let latest = game.join("logs").join("latest.log");
    let crash_dir = game.join("crash-reports");
    let mut newest_crash: Option<(std::time::SystemTime, PathBuf)> = None;
    if let Ok(rd) = fs::read_dir(&crash_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("txt") {
                continue;
            }
            if let Ok(mt) = e.metadata().and_then(|m| m.modified()) {
                if newest_crash
                    .as_ref()
                    .map(|(t, _)| mt > *t)
                    .unwrap_or(true)
                {
                    newest_crash = Some((mt, p));
                }
            }
        }
    }
    let (kind, path) = if latest.is_file() {
        ("latest.log", latest)
    } else if let Some((_, p)) = newest_crash {
        ("crash-report", p)
    } else {
        return ActionResult {
            ok: false,
            action: "open_latest_log".into(),
            message_zh: format!(
                "版本「{}」还没有 latest.log 或崩溃报告。先启动一次游戏，或从「查看崩溃」粘贴日志。",
                inst.name
            ),
            path: Some(game.display().to_string()),
            value: Some(serde_json::json!({
                "instance_id": inst.id,
                "instance_name": inst.name,
                "game_dir": inst.game_dir,
            })),
        };
    };
    let kind_zh = if kind == "latest.log" {
        "最新日志 latest.log"
    } else {
        "最近一份崩溃报告"
    };
    ActionResult {
        ok: true,
        action: "open_latest_log".into(),
        message_zh: format!(
            "版本「{}」的{}：\n{}",
            inst.name,
            kind_zh,
            path.display()
        ),
        path: Some(path.display().to_string()),
        value: Some(serde_json::json!({
            "instance_id": inst.id,
            "instance_name": inst.name,
            "kind": kind,
            "path": path.display().to_string(),
        })),
    }
}

/// List saved Minecraft accounts (no tokens) for the agent card.
pub fn list_accounts(paths: &Paths) -> ActionResult {
    let store = match load_accounts(paths) {
        Ok(s) => s,
        Err(e) => {
            return ActionResult {
                ok: false,
                action: "list_accounts".into(),
                message_zh: format!("读取账户失败：{e}"),
                path: None,
                value: None,
            };
        }
    };
    if store.accounts.is_empty() {
        return ActionResult {
            ok: true,
            action: "list_accounts".into(),
            message_zh: "还没有账户。点侧栏左下角当前账号卡，添加正版、离线或皮肤站登录。".into(),
            path: None,
            value: Some(serde_json::json!({ "count": 0, "accounts": [] })),
        };
    }
    let active = store.active_id.clone();
    let mut lines = Vec::new();
    let mut items = Vec::new();
    for acc in &store.accounts {
        let kind_zh = match acc.kind {
            AccountKind::Microsoft => "正版",
            AccountKind::Offline => "离线",
        };
        let mark = if Some(&acc.id) == active.as_ref() {
            "★ "
        } else {
            ""
        };
        lines.push(format!("{mark}{}（{kind_zh}）", acc.username));
        items.push(serde_json::json!({
            "id": acc.id,
            "username": acc.username,
            "kind": kind_zh,
            "active": Some(&acc.id) == active.as_ref(),
        }));
    }
    ActionResult {
        ok: true,
        action: "list_accounts".into(),
        message_zh: format!(
            "已保存 {} 个账户（★ 为当前）：\n{}",
            store.accounts.len(),
            lines.join("\n")
        ),
        path: None,
        value: Some(serde_json::json!({
            "count": store.accounts.len(),
            "active_id": active,
            "accounts": items,
        })),
    }
}
