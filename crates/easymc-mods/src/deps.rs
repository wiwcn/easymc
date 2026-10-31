//! Mod dependency graph: scan installed jars, mark missing / conflicts, fetch from Modrinth.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use easymc_core::{Instance, Result};
use easymc_meta::InstallProgress;
use serde::{Deserialize, Serialize};

use crate::install::{install_mod, mods_dir};
use crate::metadata::{
    is_provided_mod_id, read_mod_metas_expanded, version_satisfies, DepKind, ModDepReq, ModMeta,
};
use crate::modrinth;
use crate::types::{ModInstallResult, ModSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DepStatus {
    Ok,
    Missing,
    Conflict,
    Provided,
    Disabled,
    Unknown,
}

impl DepStatus {
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Ok => "已满足",
            Self::Missing => "缺失",
            Self::Conflict => "冲突",
            Self::Provided => "环境提供",
            Self::Disabled => "已禁用",
            Self::Unknown => "未知",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepNode {
    pub mod_id: String,
    pub display_name: String,
    pub version: Option<String>,
    pub filename: Option<String>,
    pub status: DepStatus,
    pub message_zh: Option<String>,
    pub version_range: Option<String>,
    pub dep_kind: Option<DepKind>,
    /// True when status is Missing and Modrinth slug/id looks fetchable (best-effort).
    pub fetchable: bool,
    pub children: Vec<DepNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepGraphReport {
    pub roots: Vec<DepNode>,
    pub installed: Vec<ModMeta>,
    pub missing_ids: Vec<String>,
    pub conflict_messages: Vec<String>,
    pub summary_zh: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchMissingResult {
    pub attempted: Vec<String>,
    pub installed: Vec<ModInstallResult>,
    pub failed: Vec<FetchFail>,
    pub message_zh: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchFail {
    pub mod_id: String,
    pub error_zh: String,
    /// Machine-readable cause: breaks_not_missing | provided_nested | alias_miss |
    /// modrinth_miss | loader_mismatch | version_mismatch | network | unknown
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Resolved Modrinth slug actually queried (after alias map), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tried_slug: Option<String>,
}

/// Scan `instance.game_dir/mods/` and build a dependency tree.
pub fn build_dep_graph(instance: &Instance) -> Result<DepGraphReport> {
    let dir = mods_dir(instance);
    let mut installed: Vec<ModMeta> = Vec::new();
    if dir.is_dir() {
        let mut jars: Vec<PathBuf> = std::fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                let n = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                n.ends_with(".jar") || n.ends_with(".jar.disabled")
            })
            .collect();
        jars.sort();
        for path in jars {
            match read_mod_metas_expanded(&path) {
                Ok(metas) => installed.extend(metas),
                Err(e) => {
                    tracing::warn!("跳过无法解析的模组 {}: {e}", path.display());
                    let filename = path
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let disabled = filename.ends_with(".jar.disabled");
                    let stem = filename
                        .trim_end_matches(".disabled")
                        .trim_end_matches(".jar")
                        .to_string();
                    installed.push(ModMeta {
                        mod_id: stem.clone(),
                        version: "unknown".into(),
                        name: Some(stem),
                        filename,
                        disabled,
                        kind: crate::metadata::ModMetaKind::Unknown,
                        depends: vec![],
                        breaks: vec![],
                        provides: vec![],
                    });
                }
            }
        }
    }

    // Index by mod_id AND by every `provides` alias (fabric-api → fabric, etc.)
    let mut index: HashMap<String, &ModMeta> = HashMap::new();
    for m in &installed {
        index.insert(m.mod_id.to_ascii_lowercase(), m);
        for p in &m.provides {
            index.entry(p.to_ascii_lowercase()).or_insert(m);
        }
        // Bidirectional known aliases so entitytexturefeatures satisfies entity_texture_features
        for slug in modrinth_slug_aliases(&m.mod_id) {
            index.entry(slug).or_insert(m);
        }
    }

    let mut missing_ids: HashSet<String> = HashSet::new();
    let mut conflict_messages: Vec<String> = Vec::new();

    let mut roots: Vec<DepNode> = Vec::new();
    for meta in &installed {
        let mut children = Vec::new();
        for dep in &meta.depends {
            children.push(resolve_dep_node(dep, &index, &mut missing_ids, &mut conflict_messages));
        }
        for brk in &meta.breaks {
            // Do NOT pass missing_ids — breaks must never pollute the fetch list.
            let mut node = resolve_dep_node(brk, &index, &mut HashSet::new(), &mut Vec::new());
            // breaks that are present → conflict (also check version range if any)
            if node.status == DepStatus::Ok || node.status == DepStatus::Disabled {
                let range_hits = brk
                    .version_range
                    .as_deref()
                    .map(|r| {
                        node.version
                            .as_deref()
                            .map(|v| version_satisfies(v, r))
                            .unwrap_or(true)
                    })
                    .unwrap_or(true);
                if range_hits {
                    node.status = DepStatus::Conflict;
                    let msg = format!(
                        "「{}」与已安装的「{}」不兼容",
                        meta.name.as_deref().unwrap_or(&meta.mod_id),
                        node.display_name
                    );
                    node.message_zh = Some(msg.clone());
                    conflict_messages.push(msg);
                } else {
                    // Installed but outside the breaks range → OK
                    node.status = DepStatus::Ok;
                    node.message_zh = Some("已安装但不在不兼容版本范围内".into());
                    node.fetchable = false;
                }
            } else if node.status == DepStatus::Missing {
                // incompatible target not installed — fine; show as ok
                node.status = DepStatus::Ok;
                node.message_zh = Some("未安装（不兼容项，这样正好）".into());
                node.fetchable = false;
            } else if node.status == DepStatus::Provided {
                // loader-provided id listed in breaks — ignore
                node.status = DepStatus::Ok;
                node.message_zh = Some("环境提供（跳过不兼容检查）".into());
                node.fetchable = false;
            }
            children.push(node);
        }

        let status = if meta.disabled {
            DepStatus::Disabled
        } else {
            DepStatus::Ok
        };
        roots.push(DepNode {
            mod_id: meta.mod_id.clone(),
            display_name: meta
                .name
                .clone()
                .unwrap_or_else(|| meta.mod_id.clone()),
            version: Some(meta.version.clone()),
            filename: Some(meta.filename.clone()),
            status,
            message_zh: None,
            version_range: None,
            dep_kind: None,
            fetchable: false,
            children,
        });
    }

    // Also surface orphan missing that somehow only appear as deps — already in tree.

    let mut missing_sorted: Vec<String> = missing_ids.into_iter().collect();
    missing_sorted.sort();
    conflict_messages.sort();
    conflict_messages.dedup();

    let summary_zh = format!(
        "已解析 {} 个模组 · 缺失 {} · 冲突 {}",
        installed.len(),
        missing_sorted.len(),
        conflict_messages.len()
    );

    Ok(DepGraphReport {
        roots,
        installed,
        missing_ids: missing_sorted,
        conflict_messages,
        summary_zh,
    })
}

fn resolve_dep_node(
    dep: &ModDepReq,
    index: &HashMap<String, &ModMeta>,
    missing_ids: &mut HashSet<String>,
    conflict_messages: &mut Vec<String>,
) -> DepNode {
    let id_key = dep.mod_id.to_ascii_lowercase();
    let display = dep.mod_id.clone();

    if is_provided_mod_id(&dep.mod_id) {
        return DepNode {
            mod_id: dep.mod_id.clone(),
            display_name: display,
            version: None,
            filename: None,
            status: DepStatus::Provided,
            message_zh: Some("由游戏 / 加载器提供".into()),
            version_range: dep.version_range.clone(),
            dep_kind: Some(dep.kind),
            fetchable: false,
            children: vec![],
        };
    }

    if dep.kind == DepKind::Optional {
        if let Some(meta) = index.get(&id_key) {
            let ok_range = dep
                .version_range
                .as_deref()
                .map(|r| version_satisfies(&meta.version, r))
                .unwrap_or(true);
            let status = if !ok_range {
                DepStatus::Conflict
            } else if meta.disabled {
                DepStatus::Disabled
            } else {
                DepStatus::Ok
            };
            let message_zh = if !ok_range {
                let msg = format!(
                    "可选依赖「{}」版本 {} 不满足 {}",
                    dep.mod_id,
                    meta.version,
                    dep.version_range.as_deref().unwrap_or("?")
                );
                conflict_messages.push(msg.clone());
                Some(msg)
            } else {
                None
            };
            return DepNode {
                mod_id: dep.mod_id.clone(),
                display_name: meta.name.clone().unwrap_or_else(|| dep.mod_id.clone()),
                version: Some(meta.version.clone()),
                filename: Some(meta.filename.clone()),
                status,
                message_zh,
                version_range: dep.version_range.clone(),
                dep_kind: Some(dep.kind),
                fetchable: false,
                children: vec![],
            };
        }
        return DepNode {
            mod_id: dep.mod_id.clone(),
            display_name: display,
            version: None,
            filename: None,
            status: DepStatus::Ok,
            message_zh: Some("可选，未安装".into()),
            version_range: dep.version_range.clone(),
            dep_kind: Some(dep.kind),
            fetchable: false,
            children: vec![],
        };
    }

    // Incompatible (breaks): never mark as missing — caller decides conflict vs ok.
    if dep.kind == DepKind::Incompatible {
        if let Some(meta) = index.get(&id_key) {
            let status = if meta.disabled {
                DepStatus::Disabled
            } else {
                DepStatus::Ok
            };
            return DepNode {
                mod_id: dep.mod_id.clone(),
                display_name: meta.name.clone().unwrap_or_else(|| dep.mod_id.clone()),
                version: Some(meta.version.clone()),
                filename: Some(meta.filename.clone()),
                status,
                message_zh: None,
                version_range: dep.version_range.clone(),
                dep_kind: Some(dep.kind),
                fetchable: false,
                children: vec![],
            };
        }
        return DepNode {
            mod_id: dep.mod_id.clone(),
            display_name: display,
            version: None,
            filename: None,
            status: DepStatus::Missing, // caller will flip to Ok
            message_zh: None,
            version_range: dep.version_range.clone(),
            dep_kind: Some(dep.kind),
            fetchable: false,
            children: vec![],
        };
    }

    // Required
    if let Some(meta) = index.get(&id_key) {
        let ok_range = dep
            .version_range
            .as_deref()
            .map(|r| version_satisfies(&meta.version, r))
            .unwrap_or(true);
        if !ok_range {
            let msg = format!(
                "「{}」需要 {} {}，当前为 {}",
                dep.mod_id,
                dep.mod_id,
                dep.version_range.as_deref().unwrap_or("*"),
                meta.version
            );
            conflict_messages.push(msg.clone());
            return DepNode {
                mod_id: dep.mod_id.clone(),
                display_name: meta.name.clone().unwrap_or_else(|| dep.mod_id.clone()),
                version: Some(meta.version.clone()),
                filename: Some(meta.filename.clone()),
                status: DepStatus::Conflict,
                message_zh: Some(msg),
                version_range: dep.version_range.clone(),
                dep_kind: Some(dep.kind),
                fetchable: false,
                children: vec![],
            };
        }
        let status = if meta.disabled {
            // Required but disabled → treat as conflict/warning
            let msg = format!("必需依赖「{}」已禁用", dep.mod_id);
            conflict_messages.push(msg.clone());
            DepStatus::Disabled
        } else {
            DepStatus::Ok
        };
        return DepNode {
            mod_id: dep.mod_id.clone(),
            display_name: meta.name.clone().unwrap_or_else(|| dep.mod_id.clone()),
            version: Some(meta.version.clone()),
            filename: Some(meta.filename.clone()),
            status,
            message_zh: if status == DepStatus::Disabled {
                Some(format!("必需依赖「{}」已禁用", dep.mod_id))
            } else {
                None
            },
            version_range: dep.version_range.clone(),
            dep_kind: Some(dep.kind),
            fetchable: false,
            children: vec![],
        };
    }

    missing_ids.insert(dep.mod_id.clone());
    DepNode {
        mod_id: dep.mod_id.clone(),
        display_name: display,
        version: None,
        filename: None,
        status: DepStatus::Missing,
        message_zh: Some(format!(
            "缺少必需依赖「{}」{}",
            dep.mod_id,
            dep.version_range
                .as_ref()
                .map(|r| format!("（{r}）"))
                .unwrap_or_default()
        )),
        version_range: dep.version_range.clone(),
        dep_kind: Some(dep.kind),
        fetchable: true, // try Modrinth by slug/id
        children: vec![],
    }
}

/// Known Fabric mod-id → Modrinth slug aliases (underscore vs hyphen, old ids, etc.).
fn modrinth_slug_aliases(mod_id: &str) -> Vec<String> {
    let id = mod_id.trim().to_ascii_lowercase();
    let mut out = vec![id.clone()];
    // hyphen ↔ underscore
    if id.contains('_') {
        out.push(id.replace('_', "-"));
    }
    if id.contains('-') {
        out.push(id.replace('-', "_"));
    }
    // Well-known id ≠ slug
    const ALIASES: &[(&str, &str)] = &[
        ("fabric", "fabric-api"),
        ("fabric-api", "fabric-api"),
        ("entity_texture_features", "entitytexturefeatures"),
        ("entity-texture-features", "entitytexturefeatures"),
        ("entitytexturefeatures", "entitytexturefeatures"),
        ("presencefootsteps", "presence-footsteps"),
        ("presence_footsteps", "presence-footsteps"),
        ("cloth-config2", "cloth-config"),
        ("cloth_config", "cloth-config"),
        ("modmenu", "modmenu"),
        ("sodium", "sodium"),
        ("iris", "iris"),
        ("indium", "indium"),
        ("xaerolib", "xaeros-library"),
        ("xaero_library", "xaeros-library"),
        ("xaerominimap", "xaeros-minimap"),
        ("xaeroworldmap", "xaeros-world-map"),
        ("xaerobetterpvp", "xaeros-betterpvp"),
        ("dungeonconfig", "almanac"),
        ("trinkets", "trinkets"),
        ("cardinal-components-base", "cardinal-components-api"),
        ("cardinal_components", "cardinal-components-api"),
        ("fabric-key-binding-api-v1", "fabric-api"),
        ("fabric-renderer-api-v1", "fabric-api"),
        ("fabric-rendering-v1", "fabric-api"),
        ("fabric-resource-loader-v0", "fabric-api"),
        ("bookshelf", "bookshelf"),
        ("konkrete", "konkrete"),
        ("fancymenu", "fancymenu"),
        ("drippyloadingscreen", "drippy-loading-screen"),
        ("continuity", "continuity"),
        ("lambdynlights", "lambdynamiclights"),
        ("lambdynamiclights", "lambdynamiclights"),
    ];
    for (k, v) in ALIASES {
        if id == *k {
            out.push((*v).to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn fetch_fail(mod_id: &str, error_zh: String, reason: &str, tried_slug: Option<String>) -> FetchFail {
    FetchFail {
        mod_id: mod_id.to_string(),
        error_zh,
        reason: Some(reason.to_string()),
        tried_slug,
    }
}

/// Download missing required deps from Modrinth when project id/slug resolves.
pub async fn fetch_missing_deps(
    instance: &Instance,
    only_ids: Option<Vec<String>>,
    on_progress: impl Fn(InstallProgress),
) -> Result<FetchMissingResult> {
    let report = build_dep_graph(instance)?;
    let targets: Vec<String> = if let Some(ids) = only_ids {
        ids
    } else {
        report.missing_ids.clone()
    };

    let mut installed = Vec::new();
    let mut failed = Vec::new();
    let mut attempted = Vec::new();

    // Skip ids already satisfied via provides / nested index after graph build
    let satisfied: HashSet<String> = report
        .installed
        .iter()
        .flat_map(|m| {
            std::iter::once(m.mod_id.to_ascii_lowercase())
                .chain(m.provides.iter().map(|p| p.to_ascii_lowercase()))
        })
        .collect();

    for mod_id in &targets {
        if is_provided_mod_id(mod_id) {
            continue;
        }
        if satisfied.contains(&mod_id.to_ascii_lowercase()) {
            continue;
        }
        // fabric-api submodule ids → already covered when fabric-api is installed
        if mod_id.starts_with("fabric-") && mod_id.contains("-api-") {
            if satisfied.contains("fabric-api") || satisfied.contains("fabric") {
                continue;
            }
        }
        attempted.push(mod_id.clone());

        let aliases = modrinth_slug_aliases(mod_id);
        let mut last_err: Option<String> = None;
        let mut last_slug: Option<String> = None;
        let mut done = false;

        for slug in &aliases {
            last_slug = Some(slug.clone());
            match modrinth::get_project(slug).await {
                Ok(project) => {
                    match install_mod(
                        instance,
                        ModSource::Modrinth,
                        &project.id,
                        None,
                        &on_progress,
                    )
                    .await
                    {
                        Ok(res) => {
                            installed.push(res);
                            done = true;
                            break;
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            let reason = if msg.contains("没有匹配")
                                || msg.to_ascii_lowercase().contains("no compatible")
                                || msg.contains("无可用版本")
                                || msg.contains("loader")
                            {
                                "loader_or_version_mismatch"
                            } else if msg.to_ascii_lowercase().contains("network")
                                || msg.contains("超时")
                                || msg.contains("timed out")
                            {
                                "network"
                            } else {
                                "install_failed"
                            };
                            last_err = Some(msg.clone());
                            // keep trying other aliases only on project-resolve miss;
                            // install failure on a resolved project is final for this id
                            failed.push(fetch_fail(
                                mod_id,
                                format!("已找到「{slug}」但安装失败: {msg}"),
                                reason,
                                Some(slug.clone()),
                            ));
                            done = true;
                            break;
                        }
                    }
                }
                Err(e) => {
                    last_err = Some(e.to_string());
                    continue;
                }
            }
        }

        if done {
            continue;
        }

        // Fallback: search by text using original id + aliases
        let mut hit_project = None;
        for slug in &aliases {
            let q = crate::types::SearchQuery {
                text: slug.clone(),
                game_version: Some(instance.version.as_str().to_string()),
                loader: Some(instance.loader),
                limit: 8,
                ..Default::default()
            };
            match crate::install::search_mods(ModSource::Modrinth, &q).await {
                Ok(hits) => {
                    let hit = hits.into_iter().find(|h| {
                        aliases.iter().any(|a| {
                            h.slug.eq_ignore_ascii_case(a)
                                || h.id.eq_ignore_ascii_case(a)
                                || h.name.eq_ignore_ascii_case(a)
                                || h.slug.eq_ignore_ascii_case(mod_id)
                        }) || h.slug.replace('-', "_").eq_ignore_ascii_case(&mod_id.replace('-', "_"))
                    });
                    if let Some(p) = hit {
                        hit_project = Some(p);
                        break;
                    }
                }
                Err(e2) => {
                    last_err = Some(format!("{} / {e2}", last_err.unwrap_or_default()));
                }
            }
        }

        if let Some(project) = hit_project {
            match install_mod(
                instance,
                ModSource::Modrinth,
                &project.id,
                None,
                &on_progress,
            )
            .await
            {
                Ok(res) => installed.push(res),
                Err(e) => failed.push(fetch_fail(
                    mod_id,
                    format!("搜索命中「{}」但安装失败: {e}", project.slug),
                    "install_failed",
                    Some(project.slug),
                )),
            }
        } else {
            let hint = if mod_id.eq_ignore_ascii_case("bookshelf") {
                "（Bookshelf 的 Fabric 版可能仅在 CurseForge；Modrinth 常无对应 loader 版本）"
            } else if aliases.iter().any(|a| a.contains("fabric-api")) {
                "（可能是 Fabric API 子模块 id，应由 fabric-api 的 provides/嵌套 jar 满足）"
            } else {
                ""
            };
            failed.push(fetch_fail(
                mod_id,
                format!(
                    "Modrinth 上找不到「{mod_id}」{}{}",
                    hint,
                    last_err
                        .as_ref()
                        .map(|e| format!(": {e}"))
                        .unwrap_or_default()
                ),
                "modrinth_miss",
                last_slug,
            ));
        }
    }

    let message_zh = format!(
        "尝试 {} 个 · 成功 {} · 失败 {}",
        attempted.len(),
        installed.len(),
        failed.len()
    );
    Ok(FetchMissingResult {
        attempted,
        installed,
        failed,
        message_zh,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use easymc_core::McVersion;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    fn tmp_instance(prefix: &str) -> (PathBuf, Instance) {
        let dir = std::env::temp_dir().join(format!(
            "easymc-deps-{}-{}-{}",
            prefix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mods")).unwrap();
        let game = dir.join("game");
        // put mods under game/mods
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(game.join("mods")).unwrap();
        let inst = Instance::new(
            "test",
            "Test",
            McVersion("1.21.1".into()),
            game.to_string_lossy().into(),
        );
        (dir, inst)
    }

    fn write_jar(path: &std::path::Path, entries: &[(&str, &str)]) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, body) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn graph_marks_missing_and_ok() {
        let (dir, mut inst) = tmp_instance("graph");
        inst.loader = easymc_core::ModLoader::Fabric;
        let mods = mods_dir(&inst);
        write_jar(
            &mods.join("sodium.jar"),
            &[(
                "fabric.mod.json",
                r#"{"schemaVersion":1,"id":"sodium","version":"0.5.11","name":"Sodium","depends":{"fabricloader":">=0.15","minecraft":"~1.21","fabric-api":"*"}}"#,
            )],
        );
        write_jar(
            &mods.join("fabric-api.jar"),
            &[(
                "fabric.mod.json",
                r#"{"schemaVersion":1,"id":"fabric-api","version":"0.100.0+1.21.1","name":"Fabric API","depends":{"fabricloader":">=0.15","minecraft":"~1.21"}}"#,
            )],
        );

        let report = build_dep_graph(&inst).unwrap();
        assert_eq!(report.installed.len(), 2);
        assert!(report.missing_ids.is_empty(), "{:?}", report.missing_ids);

        // Remove fabric-api → missing
        std::fs::remove_file(mods.join("fabric-api.jar")).unwrap();
        let report2 = build_dep_graph(&inst).unwrap();
        assert!(report2.missing_ids.iter().any(|id| id == "fabric-api"));
        let sodium = report2
            .roots
            .iter()
            .find(|r| r.mod_id == "sodium")
            .unwrap();
        assert!(sodium
            .children
            .iter()
            .any(|c| c.mod_id == "fabric-api" && c.status == DepStatus::Missing));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn graph_detects_version_conflict() {
        let (dir, mut inst) = tmp_instance("conflict");
        inst.loader = easymc_core::ModLoader::Fabric;
        let mods = mods_dir(&inst);
        write_jar(
            &mods.join("parent.jar"),
            &[(
                "fabric.mod.json",
                r#"{"schemaVersion":1,"id":"parent","version":"1.0.0","depends":{"child":">=2.0.0"}}"#,
            )],
        );
        write_jar(
            &mods.join("child.jar"),
            &[(
                "fabric.mod.json",
                r#"{"schemaVersion":1,"id":"child","version":"1.0.0"}"#,
            )],
        );
        let report = build_dep_graph(&inst).unwrap();
        assert!(!report.conflict_messages.is_empty());
        let parent = report.roots.iter().find(|r| r.mod_id == "parent").unwrap();
        assert!(parent
            .children
            .iter()
            .any(|c| c.mod_id == "child" && c.status == DepStatus::Conflict));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn graph_detects_breaks() {
        let (dir, mut inst) = tmp_instance("breaks");
        inst.loader = easymc_core::ModLoader::Fabric;
        let mods = mods_dir(&inst);
        write_jar(
            &mods.join("a.jar"),
            &[(
                "fabric.mod.json",
                r#"{"schemaVersion":1,"id":"moda","version":"1.0.0","breaks":{"modb":"*"}}"#,
            )],
        );
        write_jar(
            &mods.join("b.jar"),
            &[(
                "fabric.mod.json",
                r#"{"schemaVersion":1,"id":"modb","version":"1.0.0"}"#,
            )],
        );
        let report = build_dep_graph(&inst).unwrap();
        assert!(!report.conflict_messages.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn breaks_not_listed_as_missing() {
        let (dir, mut inst) = tmp_instance("breaks-miss");
        inst.loader = easymc_core::ModLoader::Fabric;
        let mods = mods_dir(&inst);
        write_jar(
            &mods.join("a.jar"),
            &[(
                "fabric.mod.json",
                r#"{"schemaVersion":1,"id":"moda","version":"1.0.0","breaks":{"optifabric":"*","hydrogen":"*"}}"#,
            )],
        );
        let report = build_dep_graph(&inst).unwrap();
        assert!(
            report.missing_ids.is_empty(),
            "breaks must not be missing: {:?}",
            report.missing_ids
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn provides_satisfies_dependency() {
        let (dir, mut inst) = tmp_instance("provides");
        inst.loader = easymc_core::ModLoader::Fabric;
        let mods = mods_dir(&inst);
        write_jar(
            &mods.join("need-fabric.jar"),
            &[(
                "fabric.mod.json",
                r#"{"schemaVersion":1,"id":"needfab","version":"1.0","depends":{"fabric":"*"}}"#,
            )],
        );
        write_jar(
            &mods.join("fabric-api.jar"),
            &[(
                "fabric.mod.json",
                r#"{"schemaVersion":1,"id":"fabric-api","version":"0.100.0","provides":["fabric"]}"#,
            )],
        );
        let report = build_dep_graph(&inst).unwrap();
        assert!(
            report.missing_ids.is_empty(),
            "fabric provide should satisfy: {:?}",
            report.missing_ids
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

