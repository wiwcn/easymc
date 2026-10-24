//! Safe guided actions: memory tip, data dir, diagnostic bundle.

use std::fs;
use std::path::PathBuf;

use chrono::Local;
use serde::{Deserialize, Serialize};

use easymc_core::{load_settings, save_settings, Paths, Result};

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

/// Write suggested memory into settings.json (clamped).
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
        message_zh: format!("问题报告已保存到：{}
可发给朋友，或自己留档排查。", path.display()),
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
