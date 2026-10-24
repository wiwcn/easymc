//! Safe guided actions: memory tip, data dir, diagnostic bundle, mirror, mods peek.

use std::fs;
use std::path::PathBuf;

use chrono::Local;
use serde::{Deserialize, Serialize};

use easymc_core::{
    load_instances, load_settings, save_settings, DownloadSource, Paths, Result,
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
