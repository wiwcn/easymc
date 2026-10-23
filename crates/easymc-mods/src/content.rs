//! Non-jar content: resource packs, shader packs, datapacks — plus local file listing
//! for the instance 「修改」 pane (saves / resourcepacks / shaderpacks).

use std::path::{Path, PathBuf};

use easymc_core::{EasyMcError, Instance, Result};
use easymc_meta::{download_file, FetchResult};
use serde::Serialize;

use crate::curseforge;
use crate::http::{curseforge_client, modrinth_client};
use crate::modrinth;
use crate::types::{ModSource, ModVersion, ProjectType};

#[derive(Debug, Clone, Serialize)]
pub struct ContentInstallResult {
    pub project_type: ProjectType,
    pub project_name: String,
    pub version_number: String,
    pub filename: String,
    pub path: String,
    pub skipped: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<String>,
}

/// Local folder kinds the instance pane manages.
pub fn local_dir(instance: &Instance, kind: &str) -> Result<PathBuf> {
    let base = PathBuf::from(&instance.game_dir);
    let sub = match kind {
        "saves" => "saves",
        "resourcepacks" => "resourcepacks",
        "shaderpacks" => "shaderpacks",
        "mods" => "mods",
        "root" => "",
        other => return Err(EasyMcError::msg(format!("未知目录: {other}"))),
    };
    Ok(if sub.is_empty() { base } else { base.join(sub) })
}

fn safe_name(name: &str) -> Result<&str> {
    let n = name.trim();
    if n.is_empty() || n == "." || n == ".." || n.contains('/') || n.contains('\\') {
        return Err(EasyMcError::msg("文件名无效"));
    }
    Ok(n)
}

fn dir_size(p: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(p) else { return 0 };
    rd.flatten()
        .map(|e| {
            let m = e.metadata().ok();
            match m {
                Some(m) if m.is_dir() => dir_size(&e.path()),
                Some(m) => m.len(),
                None => 0,
            }
        })
        .sum()
}

pub fn list_local(instance: &Instance, kind: &str) -> Result<Vec<LocalEntry>> {
    let dir = local_dir(instance, kind)?;
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for ent in std::fs::read_dir(&dir)? {
        let ent = ent?;
        let name = ent.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name.ends_with(".part") || name.ends_with(".txt") {
            continue;
        }
        let meta = ent.metadata()?;
        let is_dir = meta.is_dir();
        if kind == "saves" && !is_dir {
            continue;
        }
        let modified = meta
            .modified()
            .ok()
            .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());
        out.push(LocalEntry {
            size: if is_dir { dir_size(&ent.path()) } else { meta.len() },
            name,
            is_dir,
            modified,
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

pub fn delete_local(instance: &Instance, kind: &str, name: &str) -> Result<String> {
    let dir = local_dir(instance, kind)?;
    if kind == "root" {
        return Err(EasyMcError::msg("不能删除游戏目录本身"));
    }
    let p = dir.join(safe_name(name)?);
    if p.is_dir() {
        std::fs::remove_dir_all(&p)?;
    } else if p.is_file() {
        std::fs::remove_file(&p)?;
    } else {
        return Err(EasyMcError::msg(format!("找不到 {name}")));
    }
    Ok(format!("已删除 {name}"))
}

/// Destination folder for a content download. Datapacks go into `saves/<world>/datapacks`
/// when a world is given, else the instance-wide `datapacks/` staging folder.
pub fn content_dest(instance: &Instance, kind: ProjectType, world: Option<&str>) -> Result<PathBuf> {
    let base = PathBuf::from(&instance.game_dir);
    Ok(match kind {
        ProjectType::Datapack => match world.filter(|w| !w.trim().is_empty()) {
            Some(w) => base.join("saves").join(safe_name(w)?).join("datapacks"),
            None => base.join("datapacks"),
        },
        ProjectType::Modpack => return Err(EasyMcError::msg("整合包请用「安装到新实例」")),
        k => base.join(k.folder()),
    })
}

async fn pick_version(
    source: ModSource,
    kind: ProjectType,
    project_id: &str,
    version_id: Option<&str>,
    game: &str,
) -> Result<ModVersion> {
    if let Some(vid) = version_id.filter(|s| !s.is_empty()) {
        return match source {
            ModSource::Modrinth => modrinth::get_version(vid).await,
            ModSource::CurseForge => curseforge::get_version(project_id, vid).await,
        };
    }
    let mr_loader = match kind {
        ProjectType::Datapack => Some("datapack"),
        _ => None,
    };
    let list = match source {
        ModSource::Modrinth => {
            let mut l = modrinth::list_versions(project_id, Some(game), mr_loader).await?;
            if l.is_empty() {
                // Packs often don't tag every patch version — fall back to latest overall.
                l = modrinth::list_versions(project_id, None, mr_loader).await?;
            }
            l
        }
        ModSource::CurseForge => {
            let mut l = curseforge::list_versions(project_id, Some(game), None).await?;
            if l.is_empty() {
                l = curseforge::list_versions(project_id, None, None).await?;
            }
            l
        }
    };
    list.into_iter()
        .next()
        .ok_or_else(|| EasyMcError::msg("这个资源暂时没有可下载的版本"))
}

pub async fn install_content(
    instance: &Instance,
    source: ModSource,
    kind: ProjectType,
    project_id: &str,
    version_id: Option<&str>,
    world: Option<&str>,
) -> Result<ContentInstallResult> {
    if kind == ProjectType::Mod {
        return Err(EasyMcError::msg("模组请用模组安装"));
    }
    let dest_dir = content_dest(instance, kind, world)?;
    let project = match source {
        ModSource::Modrinth => modrinth::get_project(project_id).await?,
        ModSource::CurseForge => curseforge::get_project(project_id).await?,
    };
    let version = pick_version(source, kind, project_id, version_id, instance.version.as_str()).await?;
    let file = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .cloned()
        .ok_or_else(|| EasyMcError::msg("该版本没有可下载的文件"))?;
    let mut url = file.url.clone();
    let client = match source {
        ModSource::Modrinth => modrinth_client()?,
        ModSource::CurseForge => {
            if url.is_empty() {
                url = curseforge::resolve_download_url(project_id, &version.id, "").await?;
            }
            curseforge_client(&curseforge::require_api_key()?)?
        }
    };
    tokio::fs::create_dir_all(&dest_dir).await?;
    let fname = safe_name(&file.filename)?.to_string();
    let dest = dest_dir.join(&fname);
    let r = download_file(&client, &url, &dest, file.sha1.as_deref(), file.size, |_, _| {}).await?;
    Ok(ContentInstallResult {
        project_type: kind,
        project_name: project.name,
        version_number: version.version_number,
        filename: fname,
        path: dest.to_string_lossy().into_owned(),
        skipped: matches!(r, FetchResult::Skipped),
    })
}
