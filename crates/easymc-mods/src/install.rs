use std::path::PathBuf;

use easymc_core::{EasyMcError, Instance, ModLoader, Result};
use easymc_meta::{download_file, FetchResult, InstallProgress};

use crate::curseforge;
use crate::modrinth;
use crate::types::{
    loader_filter_name, ModFile, ModInstallResult, ModProject, ModSource, ModVersion, SearchQuery,
};
use crate::http::{modrinth_client, curseforge_client};

pub fn mods_dir(instance: &Instance) -> PathBuf {
    PathBuf::from(&instance.game_dir).join("mods")
}

pub fn list_installed_mods(instance: &Instance) -> Result<Vec<String>> {
    let dir = mods_dir(instance);
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut names = Vec::new();
    for ent in std::fs::read_dir(&dir)? {
        let ent = ent?;
        let name = ent.file_name().to_string_lossy().into_owned();
        if name.ends_with(".jar") || name.ends_with(".jar.disabled") {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

pub async fn search_mods(source: ModSource, query: &SearchQuery) -> Result<Vec<ModProject>> {
    match source {
        ModSource::Modrinth => modrinth::search(query).await,
        ModSource::CurseForge => curseforge::search(query).await,
    }
}

pub async fn list_mod_versions(
    source: ModSource,
    project_id: &str,
    game_version: Option<&str>,
    loader: Option<ModLoader>,
) -> Result<Vec<ModVersion>> {
    match source {
        ModSource::Modrinth => {
            let ld = loader.and_then(loader_filter_name);
            modrinth::list_versions(project_id, game_version, ld).await
        }
        ModSource::CurseForge => curseforge::list_versions(project_id, game_version, loader).await,
    }
}

fn pick_primary_file(version: &ModVersion) -> Result<&ModFile> {
    version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| EasyMcError::msg("该版本没有可下载的文件"))
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
    skipped: u64,
    downloaded: u64,
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
        skipped,
        downloaded,
    });
}

/// Install a mod jar into `instance.game_dir/mods/`.
///
/// - `version_id`: optional marketplace version/file id; if `None`, pick latest matching game+loader.
pub async fn install_mod(
    instance: &Instance,
    source: ModSource,
    project_id: &str,
    version_id: Option<&str>,
    on_progress: impl Fn(InstallProgress),
) -> Result<ModInstallResult> {
    let game = instance.version.as_str();
    let loader = instance.loader;

    emit_progress(
        &on_progress,
        "meta",
        "模组元数据",
        1,
        2,
        0,
        Some(1),
        format!("查询 {} · {project_id}", source.label_zh()),
        0,
        0,
    );

    let project = match source {
        ModSource::Modrinth => modrinth::get_project(project_id).await?,
        ModSource::CurseForge => curseforge::get_project(project_id).await?,
    };

    let version = if let Some(vid) = version_id.filter(|s| !s.is_empty()) {
        match source {
            ModSource::Modrinth => modrinth::get_version(vid).await?,
            ModSource::CurseForge => curseforge::get_version(project_id, vid).await?,
        }
    } else {
        let list = list_mod_versions(source, project_id, Some(game), Some(loader)).await?;
        list.into_iter().next().ok_or_else(|| {
            EasyMcError::msg(format!(
                "未找到兼容 {} / {} {} 的模组版本",
                game,
                loader.label_zh(),
                project.name
            ))
        })?
    };

    let file = pick_primary_file(&version)?.clone();
    let mut url = file.url.clone();
    if url.is_empty() {
        if source == ModSource::CurseForge {
            url = curseforge::resolve_download_url(project_id, &version.id, "").await?;
        } else {
            return Err(EasyMcError::msg("下载地址为空"));
        }
    }

    let dest_dir = mods_dir(instance);
    tokio::fs::create_dir_all(&dest_dir).await?;
    let dest = dest_dir.join(&file.filename);

    emit_progress(
        &on_progress,
        "mod",
        "下载模组",
        2,
        2,
        0,
        file.size,
        format!("下载 {}", file.filename),
        0,
        0,
    );

    let client = match source {
        ModSource::Modrinth => modrinth_client()?,
        ModSource::CurseForge => {
            let key = curseforge::require_api_key()?;
            curseforge_client(&key)?
        }
    };

    let on_progress_ref = &on_progress;
    let filename = file.filename.clone();
    let result = download_file(
        &client,
        &url,
        &dest,
        file.sha1.as_deref(),
        file.size,
        |cur, total| {
            emit_progress(
                on_progress_ref,
                "mod",
                "下载模组",
                2,
                2,
                cur,
                total.or(file.size),
                format!("下载 {filename}"),
                0,
                1,
            );
        },
    )
    .await?;

    let skipped = matches!(result, FetchResult::Skipped);
    emit_progress(
        &on_progress,
        "done",
        "完成",
        2,
        2,
        1,
        Some(1),
        format!(
            "{} {} → {}",
            if skipped { "已存在，跳过" } else { "已安装" },
            project.name,
            dest.display()
        ),
        if skipped { 1 } else { 0 },
        if skipped { 0 } else { 1 },
    );

    let result = ModInstallResult {
        source,
        project_id: project.id.clone(),
        project_name: project.name,
        version_id: version.id.clone(),
        version_number: version.version_number,
        filename: file.filename.clone(),
        path: dest.to_string_lossy().into_owned(),
        skipped,
    };
    let _ = write_mod_index_entry(
        instance,
        &file.filename,
        source,
        &project.id,
        &version.id,
        file.sha1.as_deref(),
    );
    Ok(result)
}

fn write_mod_index_entry(
    instance: &Instance,
    filename: &str,
    source: ModSource,
    project_id: &str,
    version_id: &str,
    sha1: Option<&str>,
) -> Result<()> {
    use serde_json::{json, Value};
    let path = mods_dir(instance).join(".easymc-mods.json");
    let mut root: Value = if path.is_file() {
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_default())
            .unwrap_or_else(|_| json!({ "mods": [] }))
    } else {
        json!({ "mods": [] })
    };
    let entry = json!({
        "filename": filename,
        "disabled": false,
        "sha1": sha1,
        "source": source.as_str(),
        "project_id": project_id,
        "version_id": version_id,
    });
    let mods = root
        .as_object_mut()
        .map(|o| o.entry("mods".to_string()).or_insert_with(|| json!([])))
        .ok_or_else(|| EasyMcError::msg("index corrupt"))?;
    if let Some(arr) = mods.as_array_mut() {
        if let Some(slot) = arr.iter_mut().find(|m| m.get("filename").and_then(|v| v.as_str()) == Some(filename)) {
            *slot = entry;
        } else {
            arr.push(entry);
        }
    }
    std::fs::create_dir_all(mods_dir(instance))?;
    std::fs::write(path, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

/// Reject path traversal / non-basename filenames. Allows `*.jar` and `*.jar.disabled`.
pub fn sanitize_mod_filename(filename: &str) -> Result<String> {
    let name = filename.trim();
    if name.is_empty() {
        return Err(EasyMcError::msg("模组文件名为空"));
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err(EasyMcError::msg("模组文件名不能包含路径分隔符"));
    }
    if name == "." || name == ".." || name.contains("..") {
        return Err(EasyMcError::msg("非法模组文件名"));
    }
    // Basename only — Path::file_name should equal the whole string.
    let as_path = std::path::Path::new(name);
    if as_path.components().count() != 1
        || as_path
            .file_name()
            .and_then(|s| s.to_str())
            != Some(name)
    {
        return Err(EasyMcError::msg("模组文件名必须是纯文件名（basename）"));
    }
    let lower = name.to_ascii_lowercase();
    if !(lower.ends_with(".jar") || lower.ends_with(".jar.disabled")) {
        return Err(EasyMcError::msg("只能卸载 .jar / .jar.disabled 文件"));
    }
    Ok(name.to_string())
}

fn resolve_mod_jar(instance: &Instance, filename: &str) -> Result<PathBuf> {
    let safe = sanitize_mod_filename(filename)?;
    let dir = mods_dir(instance);
    let primary = dir.join(&safe);
    if primary.is_file() {
        return Ok(primary);
    }
    // Accept either form: foo.jar ↔ foo.jar.disabled
    let alt = if safe.ends_with(".jar.disabled") {
        let base = safe.trim_end_matches(".disabled");
        dir.join(base)
    } else if safe.ends_with(".jar") {
        dir.join(format!("{safe}.disabled"))
    } else {
        primary
    };
    if alt.is_file() {
        return Ok(alt);
    }
    Err(EasyMcError::msg(format!(
        "未找到模组文件: {safe}（目录 {}）",
        dir.display()
    )))
}

fn remove_mod_index_entry(instance: &Instance, filename: &str) -> Result<()> {
    use serde_json::{json, Value};
    let path = mods_dir(instance).join(".easymc-mods.json");
    if !path.is_file() {
        return Ok(());
    }
    let mut root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_default())
        .unwrap_or_else(|_| json!({ "mods": [] }));
    let stem = filename
        .strip_suffix(".disabled")
        .unwrap_or(filename);
    if let Some(arr) = root
        .as_object_mut()
        .and_then(|o| o.get_mut("mods"))
        .and_then(|m| m.as_array_mut())
    {
        arr.retain(|m| {
            let f = m.get("filename").and_then(|v| v.as_str()).unwrap_or("");
            let f_stem = f.strip_suffix(".disabled").unwrap_or(f);
            f != filename && f != stem && f_stem != stem
        });
    }
    std::fs::write(path, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

/// Delete an installed mod jar under `instance.game_dir/mods/` by basename.
///
/// Rejects path traversal. Accepts `name.jar` or `name.jar.disabled` (resolves either).
pub fn remove_mod(instance: &Instance, filename: &str) -> Result<String> {
    let path = resolve_mod_jar(instance, filename)?;
    let removed_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(filename)
        .to_string();
    std::fs::remove_file(&path)?;
    let _ = remove_mod_index_entry(instance, &removed_name);
    Ok(removed_name)
}

/// Disable (`true`) or enable (`false`) a mod by renaming `.jar` ↔ `.jar.disabled`.
pub fn set_mod_disabled(instance: &Instance, filename: &str, disabled: bool) -> Result<String> {
    let path = resolve_mod_jar(instance, filename)?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| EasyMcError::msg("无效文件名"))?
        .to_string();
    let currently_disabled = name.ends_with(".jar.disabled");
    if currently_disabled == disabled {
        return Ok(name);
    }
    let new_name = if disabled {
        format!("{name}.disabled")
    } else {
        name.strip_suffix(".disabled")
            .unwrap_or(&name)
            .to_string()
    };
    let dest = mods_dir(instance).join(&new_name);
    if dest.exists() {
        return Err(EasyMcError::msg(format!(
            "目标已存在，无法重命名: {new_name}"
        )));
    }
    std::fs::rename(&path, &dest)?;
    // Best-effort index update
    let _ = write_mod_index_disabled(instance, &name, &new_name, disabled);
    Ok(new_name)
}

fn write_mod_index_disabled(
    instance: &Instance,
    old_filename: &str,
    new_filename: &str,
    disabled: bool,
) -> Result<()> {
    use serde_json::{json, Value};
    let path = mods_dir(instance).join(".easymc-mods.json");
    if !path.is_file() {
        return Ok(());
    }
    let mut root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_default())
        .unwrap_or_else(|_| json!({ "mods": [] }));
    let stem_old = old_filename
        .strip_suffix(".disabled")
        .unwrap_or(old_filename);
    if let Some(arr) = root
        .as_object_mut()
        .and_then(|o| o.get_mut("mods"))
        .and_then(|m| m.as_array_mut())
    {
        for m in arr.iter_mut() {
            let f = m.get("filename").and_then(|v| v.as_str()).unwrap_or("");
            let f_stem = f.strip_suffix(".disabled").unwrap_or(f);
            if f == old_filename || f_stem == stem_old {
                if let Some(obj) = m.as_object_mut() {
                    obj.insert("filename".into(), json!(new_filename.strip_suffix(".disabled").unwrap_or(new_filename)));
                    obj.insert("disabled".into(), json!(disabled));
                }
            }
        }
    }
    std::fs::write(path, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use easymc_core::McVersion;
    use std::fs;

    fn tmp_instance(prefix: &str) -> (tempfile_dir::Tmp, Instance) {
        let dir = tempfile_dir::Tmp::new(prefix);
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("mods")).unwrap();
        let inst = Instance::new("test", "Test", McVersion("1.21.1".into()), game.to_string_lossy().into());
        (dir, inst)
    }

    // Minimal temp dir helper without adding tempfile dep.
    mod tempfile_dir {
        use std::path::{Path, PathBuf};
        pub struct Tmp(PathBuf);
        impl Tmp {
            pub fn new(prefix: &str) -> Self {
                let p = std::env::temp_dir().join(format!(
                    "easymc-mods-{}-{}-{}",
                    prefix,
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0)
                ));
                let _ = std::fs::remove_dir_all(&p);
                std::fs::create_dir_all(&p).unwrap();
                Self(p)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }
        impl Drop for Tmp {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn sanitize_rejects_traversal() {
        assert!(sanitize_mod_filename("../evil.jar").is_err());
        assert!(sanitize_mod_filename("foo/bar.jar").is_err());
        assert!(sanitize_mod_filename("foo\\bar.jar").is_err());
        assert!(sanitize_mod_filename("..").is_err());
        assert!(sanitize_mod_filename("notajar.txt").is_err());
        assert!(sanitize_mod_filename("ok.jar").is_ok());
        assert!(sanitize_mod_filename("ok.jar.disabled").is_ok());
    }

    #[test]
    fn remove_mod_deletes_jar() {
        let (_tmp, inst) = tmp_instance("rm");
        let jar = mods_dir(&inst).join("sodium.jar");
        fs::write(&jar, b"fake").unwrap();
        assert!(jar.is_file());
        let removed = remove_mod(&inst, "sodium.jar").unwrap();
        assert_eq!(removed, "sodium.jar");
        assert!(!jar.exists());
        assert!(list_installed_mods(&inst).unwrap().is_empty());
    }

    #[test]
    fn remove_mod_finds_disabled_variant() {
        let (_tmp, inst) = tmp_instance("rm-dis");
        let jar = mods_dir(&inst).join("iris.jar.disabled");
        fs::write(&jar, b"fake").unwrap();
        let removed = remove_mod(&inst, "iris.jar").unwrap();
        assert_eq!(removed, "iris.jar.disabled");
        assert!(!jar.exists());
    }

    #[test]
    fn set_mod_disabled_renames() {
        let (_tmp, inst) = tmp_instance("dis");
        let jar = mods_dir(&inst).join("lithium.jar");
        fs::write(&jar, b"fake").unwrap();
        let n = set_mod_disabled(&inst, "lithium.jar", true).unwrap();
        assert_eq!(n, "lithium.jar.disabled");
        assert!(!jar.exists());
        assert!(mods_dir(&inst).join("lithium.jar.disabled").is_file());
        let listed = list_installed_mods(&inst).unwrap();
        assert_eq!(listed, vec!["lithium.jar.disabled".to_string()]);
        let n2 = set_mod_disabled(&inst, "lithium.jar.disabled", false).unwrap();
        assert_eq!(n2, "lithium.jar");
        assert!(list_installed_mods(&inst).unwrap() == vec!["lithium.jar".to_string()]);
    }

    #[test]
    fn remove_rejects_missing() {
        let (_tmp, inst) = tmp_instance("miss");
        assert!(remove_mod(&inst, "nope.jar").is_err());
    }
}
