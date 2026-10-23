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

    Ok(ModInstallResult {
        source,
        project_id: project.id,
        project_name: project.name,
        version_id: version.id,
        version_number: version.version_number,
        filename: file.filename,
        path: dest.to_string_lossy().into_owned(),
        skipped,
    })
}
