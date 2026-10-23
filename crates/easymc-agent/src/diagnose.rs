//! Collect launch / instance logs and produce a zh-CN diagnosis report.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use easymc_core::Paths;

use crate::patterns::{scan_log, Finding, FindingSeverity};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnoseRequest {
    /// Optional in-memory process log lines (from ProcessManager).
    pub process_logs: Option<Vec<String>>,
    /// Instance id under data dir (reads `.minecraft/logs/latest.log`).
    pub instance_id: Option<String>,
    /// Extra log file paths to include.
    pub extra_paths: Vec<PathBuf>,
    /// Max characters of log body to keep in report.
    pub max_log_chars: usize,
}

impl Default for DiagnoseRequest {
    fn default() -> Self {
        Self {
            process_logs: None,
            instance_id: None,
            extra_paths: Vec::new(),
            max_log_chars: 120_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnoseReport {
    pub ok: bool,
    pub source_label: String,
    pub findings: Vec<Finding>,
    pub summary_zh: String,
    pub log_excerpt: String,
    pub log_chars: usize,
    pub sources: Vec<String>,
}

fn read_tail(path: &Path, max_chars: usize) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    if raw.len() <= max_chars {
        Some(raw)
    } else {
        let start = raw.len() - max_chars;
        // align to char boundary
        let start = raw[start..]
            .char_indices()
            .next()
            .map(|(i, _)| start + i)
            .unwrap_or(start);
        Some(format!("…(截断)\n{}", &raw[start..]))
    }
}

fn crash_reports_dir(paths: &Paths, instance_id: &str) -> PathBuf {
    paths.instance_game_dir(instance_id).join("crash-reports")
}

fn latest_log_path(paths: &Paths, instance_id: &str) -> PathBuf {
    paths.instance_game_dir(instance_id).join("logs").join("latest.log")
}

/// Newest crash-report *.txt if any.
fn newest_crash_report(dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    let rd = fs::read_dir(dir).ok()?;
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("txt") {
            continue;
        }
        let mt = e.metadata().ok()?.modified().ok()?;
        if best.as_ref().map(|(t, _)| mt > *t).unwrap_or(true) {
            best = Some((mt, p));
        }
    }
    best.map(|(_, p)| p)
}

pub fn collect_log_text(paths: &Paths, req: &DiagnoseRequest) -> (String, Vec<String>, String) {
    let mut chunks: Vec<String> = Vec::new();
    let mut sources: Vec<String> = Vec::new();
    let mut label = String::from("（无日志）");

    if let Some(lines) = &req.process_logs {
        if !lines.is_empty() {
            chunks.push(lines.join("\n"));
            sources.push("process_memory".into());
            label = "内存中的最近启动日志".into();
        }
    }

    if let Some(id) = &req.instance_id {
        let lp = latest_log_path(paths, id);
        if let Some(t) = read_tail(&lp, req.max_log_chars) {
            chunks.push(t);
            sources.push(lp.display().to_string());
            if label.starts_with('（') {
                label = format!("实例 {id} 的 latest.log");
            }
        }
        let cr = crash_reports_dir(paths, id);
        if let Some(p) = newest_crash_report(&cr) {
            if let Some(t) = read_tail(&p, req.max_log_chars / 2) {
                chunks.push(format!("\n===== crash-report: {} =====\n{t}", p.display()));
                sources.push(p.display().to_string());
                if label.starts_with('（') {
                    label = format!("实例 {id} 的 crash-report");
                }
            }
        }
    }

    for p in &req.extra_paths {
        if let Some(t) = read_tail(p, req.max_log_chars) {
            chunks.push(t);
            sources.push(p.display().to_string());
        }
    }

    let mut text = chunks.join("\n");
    if text.len() > req.max_log_chars {
        let start = text.len() - req.max_log_chars;
        let start = text[start..]
            .char_indices()
            .next()
            .map(|(i, _)| start + i)
            .unwrap_or(start);
        text = format!("…(截断)\n{}", &text[start..]);
    }
    (text, sources, label)
}

fn summarize_zh(findings: &[Finding], label: &str, empty_log: bool) -> String {
    if empty_log {
        return "未找到可分析的日志。请先启动一次实例，或指定实例 id。".into();
    }
    if findings.is_empty() {
        return format!(
            "已分析「{label}」，未匹配到已知崩溃模式。若仍有问题，可导出诊断包查看完整日志。"
        );
    }
    let errors = findings
        .iter()
        .filter(|f| matches!(f.severity, FindingSeverity::Error))
        .count();
    let warns = findings
        .iter()
        .filter(|f| matches!(f.severity, FindingSeverity::Warn))
        .count();
    let titles: Vec<&str> = findings.iter().map(|f| f.title.as_str()).collect();
    format!(
        "已分析「{label}」：发现 {} 项（错误 {errors} / 警告 {warns}）。主要问题：{}。",
        findings.len(),
        titles.join("；")
    )
}

pub fn diagnose(paths: &Paths, req: DiagnoseRequest) -> DiagnoseReport {
    let (text, sources, label) = collect_log_text(paths, &req);
    let empty = text.trim().is_empty();
    let findings = if empty {
        Vec::new()
    } else {
        scan_log(&text)
    };
    let excerpt_len = 4000.min(text.len());
    let excerpt = if text.len() <= excerpt_len {
        text.clone()
    } else {
        let start = text.len() - excerpt_len;
        let start = text[start..]
            .char_indices()
            .next()
            .map(|(i, _)| start + i)
            .unwrap_or(start);
        format!("…\n{}", &text[start..])
    };
    let summary_zh = summarize_zh(&findings, &label, empty);
    DiagnoseReport {
        ok: !findings
            .iter()
            .any(|f| matches!(f.severity, FindingSeverity::Error)),
        source_label: label,
        findings,
        summary_zh,
        log_excerpt: excerpt,
        log_chars: text.len(),
        sources,
    }
}
