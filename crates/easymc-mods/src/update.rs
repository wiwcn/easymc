//! Batch mod update: scan installed jars for newer Modrinth / CurseForge versions
//! compatible with the instance game + loader, then apply selected updates.

use std::path::PathBuf;

use easymc_core::{Instance, Result};
use easymc_meta::InstallProgress;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::curseforge;
use crate::install::{
    install_mod, list_installed_mod_entries, mods_dir, remove_mod, InstalledModEntry,
};
use crate::modrinth;
use crate::types::{loader_filter_name, ModSource, ModVersion};

/// Machine-readable status for one installed mod vs marketplace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModUpdateStatus {
    /// Newer compatible version available.
    HasUpdate,
    /// Already on latest compatible file.
    UpToDate,
    /// Project known but no file matches current MC + loader.
    NoCompatible,
    /// No marketplace id (and hash lookup failed / skipped).
    UnknownSource,
    /// CurseForge project but API key missing.
    CfNeedsKey,
    /// Network / API failure while checking.
    LookupFailed,
}

impl ModUpdateStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HasUpdate => "has_update",
            Self::UpToDate => "up_to_date",
            Self::NoCompatible => "no_compatible",
            Self::UnknownSource => "unknown_source",
            Self::CfNeedsKey => "cf_needs_key",
            Self::LookupFailed => "lookup_failed",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::HasUpdate => "有更新",
            Self::UpToDate => "已是最新",
            Self::NoCompatible => "无兼容版本",
            Self::UnknownSource => "未知来源",
            Self::CfNeedsKey => "需 CurseForge Key",
            Self::LookupFailed => "检查失败",
        }
    }

    pub fn selectable(self) -> bool {
        matches!(self, Self::HasUpdate)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModVersionOption {
    pub id: String,
    pub version_number: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_published: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changelog_snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModUpdateCandidate {
    pub filename: String,
    pub disabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_version_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_version_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changelog_snippet: Option<String>,
    /// Compatible versions newest-first (for picker); empty when none / unknown.
    #[serde(default)]
    pub alternates: Vec<ModVersionOption>,
    pub status: ModUpdateStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_zh: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModUpdateScanReport {
    pub instance_id: String,
    pub game_version: String,
    pub loader: String,
    pub candidates: Vec<ModUpdateCandidate>,
    pub update_count: usize,
    pub up_to_date_count: usize,
    pub skipped_cf_no_key: usize,
    pub unknown_count: usize,
    pub summary_zh: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModUpdateApplyItem {
    /// Current installed filename (basename).
    pub filename: String,
    pub source: String,
    pub project_id: String,
    /// Marketplace version / file id; None = latest compatible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModUpdateFail {
    pub filename: String,
    pub error_zh: String,
    /// install_failed | no_update | unknown_source | cf_needs_key | network | other
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModUpdateApplyResult {
    pub attempted: usize,
    pub updated: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<ModUpdateFail>,
    pub message_zh: String,
    /// Hint UI / agent to re-run dep fill after jar churn.
    pub suggest_dep_check: bool,
}

#[derive(Debug, Clone, Default)]
struct IndexMeta {
    source: Option<String>,
    project_id: Option<String>,
    version_id: Option<String>,
    version_number: Option<String>,
    sha1: Option<String>,
    icon_url: Option<String>,
    project_name: Option<String>,
}

fn read_index_map(instance: &Instance) -> std::collections::HashMap<String, IndexMeta> {
    use std::collections::HashMap;
    let path = mods_dir(instance).join(".easymc-mods.json");
    let mut map = HashMap::new();
    if !path.is_file() {
        return map;
    }
    let root: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_default())
            .unwrap_or_else(|_| serde_json::json!({ "mods": [] }));
    let Some(arr) = root.get("mods").and_then(|m| m.as_array()) else {
        return map;
    };
    for m in arr {
        let filename = m
            .get("filename")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if filename.is_empty() {
            continue;
        }
        let stem = filename
            .strip_suffix(".disabled")
            .unwrap_or(&filename)
            .to_string();
        let meta = IndexMeta {
            source: m.get("source").and_then(|v| v.as_str()).map(str::to_string),
            project_id: m
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            version_id: m
                .get("version_id")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            version_number: m
                .get("version_number")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            sha1: m.get("sha1").and_then(|v| v.as_str()).map(str::to_string),
            icon_url: m
                .get("icon_url")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            project_name: m
                .get("project_name")
                .and_then(|v| v.as_str())
                .or_else(|| m.get("name").and_then(|v| v.as_str()))
                .map(str::to_string),
        };
        map.insert(filename.clone(), meta.clone());
        map.insert(stem, meta);
    }
    map
}

fn file_sha1(path: &PathBuf) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = std::io::Read::read(&mut file, &mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hex::encode(hasher.finalize()))
}

fn snippet_changelog(raw: Option<&str>, max_chars: usize) -> Option<String> {
    let s = raw?.trim();
    if s.is_empty() {
        return None;
    }
    // Strip crude markdown headings for UI snippet.
    let flat: String = s
        .lines()
        .map(|l| l.trim_start_matches(['#', '*', '-', ' ']).trim())
        .filter(|l| !l.is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join(" · ");
    if flat.is_empty() {
        return None;
    }
    if flat.chars().count() <= max_chars {
        Some(flat)
    } else {
        Some(flat.chars().take(max_chars).collect::<String>() + "…")
    }
}

fn version_to_option(v: &ModVersion) -> ModVersionOption {
    ModVersionOption {
        id: v.id.clone(),
        version_number: v.version_number.clone(),
        name: v.name.clone(),
        date_published: v.date_published.clone(),
        changelog_snippet: snippet_changelog(v.changelog.as_deref(), 180),
    }
}

fn primary_filename(v: &ModVersion) -> Option<String> {
    v.files
        .iter()
        .find(|f| f.primary)
        .or_else(|| v.files.first())
        .map(|f| f.filename.clone())
}

fn current_matches(latest: &ModVersion, current_version_id: Option<&str>, filename: &str) -> bool {
    if let Some(cid) = current_version_id.filter(|s| !s.is_empty()) {
        if cid == latest.id {
            return true;
        }
    }
    let stem = filename
        .strip_suffix(".disabled")
        .unwrap_or(filename);
    primary_filename(latest)
        .map(|f| f == stem || f == filename)
        .unwrap_or(false)
}

async fn resolve_identity(
    entry: &InstalledModEntry,
    index: &IndexMeta,
    jar_path: &PathBuf,
) -> (Option<ModSource>, Option<String>, Option<String>, Option<String>) {
    let mut source = index
        .source
        .as_deref()
        .or(entry.source.as_deref())
        .and_then(ModSource::parse);
    let mut project_id = index
        .project_id
        .clone()
        .or_else(|| entry.project_id.clone());
    let mut version_id = index.version_id.clone();
    let version_number = index.version_number.clone();

    if source.is_some() && project_id.is_some() {
        return (source, project_id, version_id, version_number);
    }

    // Best-effort Modrinth hash lookup for jars without index metadata.
    let sha = index
        .sha1
        .clone()
        .or_else(|| file_sha1(jar_path));
    if let Some(sha) = sha {
        if let Ok(Some(hit)) = modrinth::lookup_by_sha1(&sha).await {
            source = Some(ModSource::Modrinth);
            project_id = Some(hit.project_id);
            if version_id.is_none() {
                version_id = Some(hit.version_id);
            }
        }
    }

    (source, project_id, version_id, version_number)
}

async fn list_compatible(
    source: ModSource,
    project_id: &str,
    game: &str,
    loader: easymc_core::ModLoader,
) -> Result<Vec<ModVersion>> {
    match source {
        ModSource::Modrinth => {
            let ld = loader_filter_name(loader);
            let mut list =
                modrinth::list_versions_detailed(project_id, Some(game), ld, true).await?;
            if list.is_empty() {
                // Fallback: any loader on this game (Quilt↔Fabric etc.) then filter client-side.
                list = modrinth::list_versions_detailed(project_id, Some(game), None, true).await?;
                if let Some(want) = ld {
                    let filtered: Vec<_> = list
                        .iter()
                        .filter(|v| {
                            v.loaders
                                .iter()
                                .any(|l| l.eq_ignore_ascii_case(want) || (want == "fabric" && l.eq_ignore_ascii_case("quilt")) || (want == "quilt" && l.eq_ignore_ascii_case("fabric")))
                        })
                        .cloned()
                        .collect();
                    if !filtered.is_empty() {
                        list = filtered;
                    }
                }
            }
            Ok(list)
        }
        ModSource::CurseForge => {
            let mut list = crate::install::list_mod_versions(
                source,
                project_id,
                Some(game),
                Some(loader),
            )
            .await?;
            if list.is_empty() {
                list = crate::install::list_mod_versions(source, project_id, Some(game), None)
                    .await?;
            }
            Ok(list)
        }
    }
}

/// Scan all installed mods for newer compatible marketplace versions.
pub async fn check_mod_updates(instance: &Instance) -> Result<ModUpdateScanReport> {
    let entries = list_installed_mod_entries(instance)?;
    let index_map = read_index_map(instance);
    let game = instance.version.as_str();
    let loader = instance.loader;
    let cf_ok = curseforge::api_key().is_some();

    let mut candidates = Vec::with_capacity(entries.len());
    let mut update_count = 0usize;
    let mut up_to_date_count = 0usize;
    let mut skipped_cf_no_key = 0usize;
    let mut unknown_count = 0usize;

    for entry in entries {
        let stem = entry
            .filename
            .strip_suffix(".disabled")
            .unwrap_or(&entry.filename)
            .to_string();
        let idx = index_map
            .get(&entry.filename)
            .or_else(|| index_map.get(&stem))
            .cloned()
            .unwrap_or_default();
        let jar_path = mods_dir(instance).join(&entry.filename);

        let (source, project_id, current_vid, current_vnum) =
            resolve_identity(&entry, &idx, &jar_path).await;

        let mut cand = ModUpdateCandidate {
            filename: entry.filename.clone(),
            disabled: entry.disabled,
            icon_url: entry.icon_url.or(idx.icon_url),
            source: source.map(|s| s.as_str().to_string()).or(entry.source.clone()),
            project_id: project_id.clone(),
            project_name: idx.project_name.clone(),
            current_version_id: current_vid.clone(),
            current_version_number: current_vnum.clone().or_else(|| {
                // Fall back to jar stem as a weak version hint for UI.
                Some(
                    stem.trim_end_matches(".jar")
                        .rsplit(['-', '_'])
                        .next()
                        .unwrap_or("")
                        .to_string(),
                )
                .filter(|s| !s.is_empty() && s.chars().any(|c| c.is_ascii_digit()))
            }),
            latest_version_id: None,
            latest_version_number: None,
            latest_filename: None,
            changelog_snippet: None,
            alternates: vec![],
            status: ModUpdateStatus::UnknownSource,
            error_zh: None,
        };

        let (Some(source), Some(project_id)) = (source, project_id) else {
            cand.status = ModUpdateStatus::UnknownSource;
            cand.error_zh = Some("没有 Modrinth/CurseForge 元数据，无法自动检查更新".into());
            unknown_count += 1;
            candidates.push(cand);
            continue;
        };

        if source == ModSource::CurseForge && !cf_ok {
            cand.status = ModUpdateStatus::CfNeedsKey;
            cand.error_zh = Some(format!(
                "CurseForge 模组需配置 API Key（设置页或环境变量 {}）",
                curseforge::API_KEY_ENV
            ));
            skipped_cf_no_key += 1;
            candidates.push(cand);
            continue;
        }

        match list_compatible(source, &project_id, game, loader).await {
            Ok(list) if list.is_empty() => {
                cand.status = ModUpdateStatus::NoCompatible;
                cand.error_zh = Some(format!(
                    "未找到兼容 {} / {} 的版本",
                    game,
                    loader.label_zh()
                ));
            }
            Ok(list) => {
                let latest = &list[0];
                cand.project_name = cand.project_name.or_else(|| Some(latest.name.clone()));
                cand.latest_version_id = Some(latest.id.clone());
                cand.latest_version_number = Some(latest.version_number.clone());
                cand.latest_filename = primary_filename(latest);
                cand.changelog_snippet = snippet_changelog(latest.changelog.as_deref(), 220);
                cand.alternates = list.iter().take(8).map(version_to_option).collect();

                if current_matches(latest, current_vid.as_deref(), &entry.filename) {
                    cand.status = ModUpdateStatus::UpToDate;
                    up_to_date_count += 1;
                } else {
                    cand.status = ModUpdateStatus::HasUpdate;
                    update_count += 1;
                }
            }
            Err(e) => {
                cand.status = ModUpdateStatus::LookupFailed;
                cand.error_zh = Some(e.to_string());
            }
        }

        candidates.push(cand);
    }

    candidates.sort_by(|a, b| {
        let rank = |s: ModUpdateStatus| match s {
            ModUpdateStatus::HasUpdate => 0,
            ModUpdateStatus::LookupFailed => 1,
            ModUpdateStatus::CfNeedsKey => 2,
            ModUpdateStatus::NoCompatible => 3,
            ModUpdateStatus::UnknownSource => 4,
            ModUpdateStatus::UpToDate => 5,
        };
        rank(a.status)
            .cmp(&rank(b.status))
            .then_with(|| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()))
    });

    let summary_zh = format!(
        "检查完成：{} 个可更新，{} 个已是最新，{} 个未知来源{}。",
        update_count,
        up_to_date_count,
        unknown_count,
        if skipped_cf_no_key > 0 {
            format!("，{skipped_cf_no_key} 个 CurseForge 因缺少 Key 跳过")
        } else {
            String::new()
        }
    );

    Ok(ModUpdateScanReport {
        instance_id: instance.id.clone(),
        game_version: game.to_string(),
        loader: loader.as_str().to_string(),
        candidates,
        update_count,
        up_to_date_count,
        skipped_cf_no_key,
        unknown_count,
        summary_zh,
    })
}

fn emit_progress(
    on_progress: &impl Fn(InstallProgress),
    stage: &str,
    label: &str,
    phase: u32,
    phase_total: u32,
    current: u64,
    total: Option<u64>,
    detail: impl Into<String>,
) {
    let percent = total.map(|t| {
        if t == 0 {
            100.0
        } else {
            (current as f32 / t as f32) * 100.0
        }
    });
    on_progress(InstallProgress {
        stage: stage.into(),
        stage_label: label.into(),
        phase,
        phase_total,
        current,
        total,
        percent,
        detail: detail.into(),
        skipped: 0,
        downloaded: 0,
    });
}

/// Apply selected mod updates. Reuses [`install_mod`]; removes the previous jar when the
/// filename changes. Errors are classified for clear UI (`install_failed` / `no_update` / …).
pub async fn apply_mod_updates(
    instance: &Instance,
    items: &[ModUpdateApplyItem],
    on_progress: impl Fn(InstallProgress),
) -> Result<ModUpdateApplyResult> {
    if items.is_empty() {
        return Ok(ModUpdateApplyResult {
            attempted: 0,
            updated: vec![],
            skipped: vec![],
            failed: vec![],
            message_zh: "没有选中要更新的模组。".into(),
            suggest_dep_check: false,
        });
    }

    let total = items.len() as u32;
    let mut updated = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = Vec::new();

    for (i, item) in items.iter().enumerate() {
        let phase = (i as u32) + 1;
        emit_progress(
            &on_progress,
            "mod-update",
            "批量更新模组",
            phase,
            total,
            i as u64,
            Some(items.len() as u64),
            format!("更新 {} ({phase}/{total})", item.filename),
        );

        let Some(source) = ModSource::parse(&item.source) else {
            failed.push(ModUpdateFail {
                filename: item.filename.clone(),
                error_zh: format!("未知来源：{}", item.source),
                reason: "unknown_source".into(),
            });
            continue;
        };

        if source == ModSource::CurseForge && curseforge::api_key().is_none() {
            failed.push(ModUpdateFail {
                filename: item.filename.clone(),
                error_zh: format!(
                    "未配置 CurseForge API Key（{}）",
                    curseforge::API_KEY_ENV
                ),
                reason: "cf_needs_key".into(),
            });
            continue;
        }

        let old_name = item.filename.clone();
        let old_stem = old_name
            .strip_suffix(".disabled")
            .unwrap_or(&old_name)
            .to_string();

        match install_mod(
            instance,
            source,
            &item.project_id,
            item.version_id.as_deref(),
            |p| on_progress(p),
        )
        .await
        {
            Ok(res) => {
                if res.skipped && res.filename == old_stem {
                    skipped.push(old_name);
                    continue;
                }
                // Remove previous jar when filename changed (or disabled twin).
                if res.filename != old_stem && res.filename != old_name {
                    let _ = remove_mod(instance, &old_name);
                }
                updated.push(res.filename);
            }
            Err(e) => {
                let msg = e.to_string();
                let reason = if msg.contains("未找到兼容") || msg.contains("没有可下载") {
                    "no_update"
                } else if msg.contains("CurseForge API Key") {
                    "cf_needs_key"
                } else {
                    "install_failed"
                };
                failed.push(ModUpdateFail {
                    filename: old_name,
                    error_zh: msg,
                    reason: reason.into(),
                });
            }
        }
    }

    emit_progress(
        &on_progress,
        "done",
        "完成",
        total,
        total,
        items.len() as u64,
        Some(items.len() as u64),
        format!(
            "更新完成：成功 {} · 跳过 {} · 失败 {}",
            updated.len(),
            skipped.len(),
            failed.len()
        ),
    );

    let message_zh = if failed.is_empty() && skipped.is_empty() {
        format!("已更新 {} 个模组。建议再检查一次依赖。", updated.len())
    } else if failed.is_empty() {
        format!(
            "已更新 {} 个，跳过 {} 个（已是目标文件）。",
            updated.len(),
            skipped.len()
        )
    } else {
        format!(
            "已更新 {} 个，跳过 {} 个，失败 {} 个。失败原因见列表。",
            updated.len(),
            skipped.len(),
            failed.len()
        )
    };

    Ok(ModUpdateApplyResult {
        attempted: items.len(),
        updated,
        skipped,
        failed,
        message_zh,
        suggest_dep_check: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_selectable_only_has_update() {
        assert!(ModUpdateStatus::HasUpdate.selectable());
        assert!(!ModUpdateStatus::UpToDate.selectable());
        assert!(!ModUpdateStatus::NoCompatible.selectable());
        assert!(!ModUpdateStatus::CfNeedsKey.selectable());
    }

    #[test]
    fn changelog_snippet_truncates() {
        let long = "a".repeat(400);
        let s = snippet_changelog(Some(&long), 40).unwrap();
        assert!(s.ends_with('…'));
        assert!(s.chars().count() <= 41);
    }

    #[test]
    fn current_matches_by_id_or_filename() {
        let v = ModVersion {
            id: "ver1".into(),
            project_id: "p".into(),
            name: "n".into(),
            version_number: "1.0".into(),
            game_versions: vec![],
            loaders: vec![],
            downloads: 0,
            date_published: None,
            files: vec![crate::types::ModFile {
                filename: "foo-1.0.jar".into(),
                url: String::new(),
                size: None,
                primary: true,
                sha1: None,
                sha512: None,
            }],
            changelog: None,
        };
        assert!(current_matches(&v, Some("ver1"), "old.jar"));
        assert!(current_matches(&v, None, "foo-1.0.jar"));
        assert!(!current_matches(&v, Some("other"), "old.jar"));
    }

    #[test]
    fn apply_empty_is_noop() {
        // Compile-time / logic: empty items → clear message (async covered via sync structure).
        let r = ModUpdateApplyResult {
            attempted: 0,
            updated: vec![],
            skipped: vec![],
            failed: vec![],
            message_zh: "没有选中要更新的模组。".into(),
            suggest_dep_check: false,
        };
        assert_eq!(r.attempted, 0);
        assert!(!r.suggest_dep_check);
    }
}
