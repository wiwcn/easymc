//! Modpack install: Modrinth `.mrpack` and CurseForge `manifest.json` zips → a brand-new
//! instance with the right game version + loader, all mods, and overrides.
//! The game/loader files themselves install on first 「安装并启动」 (same path as any instance).

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use easymc_core::{
    create_named_instance_with, upsert_instance, CreateInstanceOpts, EasyMcError, Instance,
    McVersion, ModLoader, Paths, Result,
};
use easymc_meta::{download_file, InstallProgress};
use serde::{Deserialize, Serialize};

use crate::curseforge;
use crate::http::{curseforge_client, modrinth_client};
use crate::modrinth;
use crate::types::ModSource;

#[derive(Debug, Clone, Serialize)]
pub struct ModpackInstallResult {
    pub instance_id: String,
    pub instance_name: String,
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub files_downloaded: usize,
    pub files_failed: Vec<String>,
    pub overrides: usize,
    pub message_zh: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct MrIndex {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version_id: String,
    #[serde(default)]
    pub files: Vec<MrFile>,
    #[serde(default)]
    pub dependencies: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MrFile {
    pub path: String,
    #[serde(default)]
    pub hashes: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub downloads: Vec<String>,
    #[serde(default)]
    pub file_size: Option<u64>,
    #[serde(default)]
    pub env: Option<std::collections::HashMap<String, String>>,
}

/// Parse loader from mrpack `dependencies`.
pub fn mr_loader(deps: &std::collections::HashMap<String, String>) -> (ModLoader, Option<String>) {
    for (k, l) in [
        ("fabric-loader", ModLoader::Fabric),
        ("quilt-loader", ModLoader::Quilt),
        ("neoforge", ModLoader::NeoForge),
        ("forge", ModLoader::Forge),
    ] {
        if let Some(v) = deps.get(k) {
            return (l, Some(v.clone()));
        }
    }
    (ModLoader::Vanilla, None)
}

/// Reject absolute / `..` paths from pack manifests (zip-slip).
fn safe_rel(p: &str) -> Option<PathBuf> {
    let path = Path::new(p);
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(s) => out.push(s),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

fn progress(cb: &impl Fn(InstallProgress), phase: u32, label: &str, cur: u64, total: u64, detail: String) {
    cb(InstallProgress {
        stage: "modpack".into(),
        stage_label: label.into(),
        phase,
        phase_total: 3,
        current: cur,
        total: Some(total),
        percent: Some(if total == 0 { 100.0 } else { cur as f32 * 100.0 / total as f32 }),
        detail,
        skipped: 0,
        downloaded: cur,
    });
}

/// Extract `<prefix>/…` entries of the zip into `game_dir`. Returns count.
fn extract_overrides(zip_path: &Path, prefixes: &[&str], game_dir: &Path) -> Result<usize> {
    let f = std::fs::File::open(zip_path)?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| EasyMcError::msg(format!("整合包损坏: {e}")))?;
    let mut n = 0;
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| EasyMcError::msg(e.to_string()))?;
        let name = e.name().to_string();
        let Some(rest) = prefixes.iter().find_map(|p| name.strip_prefix(&format!("{p}/"))) else {
            continue;
        };
        let Some(rel) = safe_rel(rest) else { continue };
        let dest = game_dir.join(rel);
        if e.is_dir() {
            std::fs::create_dir_all(&dest)?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::new();
        e.read_to_end(&mut buf)?;
        std::fs::write(&dest, buf)?;
        n += 1;
    }
    Ok(n)
}

fn read_zip_text(zip_path: &Path, name: &str) -> Result<Option<String>> {
    let f = std::fs::File::open(zip_path)?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| EasyMcError::msg(format!("整合包损坏: {e}")))?;
    let Ok(mut e) = z.by_name(name) else { return Ok(None) };
    let mut s = String::new();
    e.read_to_string(&mut s)?;
    Ok(Some(s))
}

fn new_pack_instance(
    paths: &Paths,
    name: &str,
    game: &str,
    loader: ModLoader,
    loader_version: Option<String>,
    icon_url: Option<String>,
) -> Result<Instance> {
    let mut inst = create_named_instance_with(
        paths,
        CreateInstanceOpts {
            name: name.to_string(),
            version: McVersion(game.to_string()),
            loader,
            loader_version,
            version_ready: false,
        },
    )?;
    inst.category = Some("整合包".into());
    inst.icon = icon_url.or(Some("pack".into()));
    inst.description = Some(format!("整合包 · {}", loader.label_zh()));
    upsert_instance(paths, inst.clone())?;
    Ok(inst)
}

/// Install a modpack project from Modrinth or CurseForge into a NEW instance.
pub async fn install_modpack(
    paths: &Paths,
    source: ModSource,
    project_id: &str,
    version_id: Option<&str>,
    name_override: Option<&str>,
    on_progress: impl Fn(InstallProgress),
) -> Result<ModpackInstallResult> {
    let project = match source {
        ModSource::Modrinth => modrinth::get_project(project_id).await?,
        ModSource::CurseForge => curseforge::get_project(project_id).await?,
    };
    let version = match version_id.filter(|s| !s.is_empty()) {
        Some(v) => match source {
            ModSource::Modrinth => modrinth::get_version(v).await?,
            ModSource::CurseForge => curseforge::get_version(project_id, v).await?,
        },
        None => match source {
            ModSource::Modrinth => modrinth::list_versions(project_id, None, None).await?,
            ModSource::CurseForge => curseforge::list_versions(project_id, None, None).await?,
        }
        .into_iter()
        .next()
        .ok_or_else(|| EasyMcError::msg("这个整合包还没有可下载的版本"))?,
    };
    let file = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .cloned()
        .ok_or_else(|| EasyMcError::msg("整合包文件缺失"))?;

    let cache = paths.root.join("cache").join("modpacks");
    tokio::fs::create_dir_all(&cache).await?;
    let zip_path = cache.join(format!("{}-{}.zip", project.id, version.id));
    progress(&on_progress, 1, "下载整合包", 0, 1, file.filename.clone());
    let client = match source {
        ModSource::Modrinth => modrinth_client()?,
        ModSource::CurseForge => curseforge_client(&curseforge::require_api_key()?)?,
    };
    let url = if file.url.is_empty() && source == ModSource::CurseForge {
        curseforge::resolve_download_url(project_id, &version.id, "").await?
    } else {
        file.url.clone()
    };
    download_file(&client, &url, &zip_path, file.sha1.as_deref(), file.size, |_, _| {}).await?;

    let name = name_override
        .filter(|s| !s.trim().is_empty())
        .map(String::from)
        .unwrap_or_else(|| format!("{} {}", project.name, version.version_number));

    if let Some(idx) = read_zip_text(&zip_path, "modrinth.index.json")? {
        let index: MrIndex = serde_json::from_str(&idx)
            .map_err(|e| EasyMcError::msg(format!("整合包清单无法读取: {e}")))?;
        let game = index
            .dependencies
            .get("minecraft")
            .cloned()
            .ok_or_else(|| EasyMcError::msg("整合包没写游戏版本"))?;
        let (loader, lv) = mr_loader(&index.dependencies);
        let inst = new_pack_instance(paths, &name, &game, loader, lv.clone(), project.icon_url.clone())?;
        let game_dir = PathBuf::from(&inst.game_dir);
        let wanted: Vec<&MrFile> = index
            .files
            .iter()
            .filter(|f| {
                f.env
                    .as_ref()
                    .and_then(|e| e.get("client"))
                    .map(|c| c != "unsupported")
                    .unwrap_or(true)
            })
            .collect();
        let total = wanted.len() as u64;
        let mut ok = 0usize;
        let mut failed = Vec::new();
        let mr = modrinth_client()?;
        for (i, f) in wanted.iter().enumerate() {
            progress(&on_progress, 2, "下载整合包里的模组", i as u64, total, f.path.clone());
            let Some(rel) = safe_rel(&f.path) else {
                failed.push(f.path.clone());
                continue;
            };
            let dest = game_dir.join(rel);
            let sha1 = f.hashes.get("sha1").map(String::as_str);
            let mut done = false;
            for u in &f.downloads {
                if download_file(&mr, u, &dest, sha1, f.file_size, |_, _| {}).await.is_ok() {
                    done = true;
                    break;
                }
            }
            if done { ok += 1 } else { failed.push(f.path.clone()) }
        }
        progress(&on_progress, 3, "解压配置文件", 0, 1, "overrides".into());
        let ov = extract_overrides(&zip_path, &["overrides", "client-overrides"], &game_dir)?;
        progress(&on_progress, 3, "完成", 1, 1, name.clone());
        return Ok(ModpackInstallResult {
            message_zh: format!(
                "已创建「{}」：{ok} 个文件{}，点「安装并启动」即可开玩",
                inst.name,
                if failed.is_empty() { String::new() } else { format!("，{} 个没下好", failed.len()) }
            ),
            instance_id: inst.id,
            instance_name: inst.name,
            game_version: game,
            loader,
            loader_version: lv,
            files_downloaded: ok,
            files_failed: failed,
            overrides: ov,
        });
    }

    if let Some(man) = read_zip_text(&zip_path, "manifest.json")? {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct CfManifest {
            minecraft: CfMc,
            #[serde(default)]
            files: Vec<CfEntry>,
            #[serde(default)]
            overrides: Option<String>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct CfMc {
            version: String,
            #[serde(default)]
            mod_loaders: Vec<CfLoader>,
        }
        #[derive(Deserialize)]
        struct CfLoader {
            id: String,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct CfEntry {
            #[serde(rename = "projectID")]
            project_id: u64,
            #[serde(rename = "fileID")]
            file_id: u64,
        }
        let m: CfManifest = serde_json::from_str(&man)
            .map_err(|e| EasyMcError::msg(format!("整合包清单无法读取: {e}")))?;
        let (loader, lv) = m
            .minecraft
            .mod_loaders
            .first()
            .and_then(|l| {
                let (k, v) = l.id.split_once('-')?;
                Some((ModLoader::parse_loose(k)?, Some(v.to_string())))
            })
            .unwrap_or((ModLoader::Vanilla, None));
        let inst = new_pack_instance(paths, &name, &m.minecraft.version, loader, lv.clone(), project.icon_url.clone())?;
        let game_dir = PathBuf::from(&inst.game_dir);
        let mods = game_dir.join("mods");
        let total = m.files.len() as u64;
        let mut ok = 0usize;
        let mut failed = Vec::new();
        for (i, e) in m.files.iter().enumerate() {
            let (pid, fid) = (e.project_id.to_string(), e.file_id.to_string());
            progress(&on_progress, 2, "下载整合包里的模组", i as u64, total, pid.clone());
            let res: Result<()> = async {
                let v = curseforge::get_version(&pid, &fid).await?;
                let f = v.files.first().ok_or_else(|| EasyMcError::msg("无文件"))?;
                let url = curseforge::resolve_download_url(&pid, &fid, &f.url).await?;
                let Some(rel) = safe_rel(&f.filename) else { return Err(EasyMcError::msg("文件名无效")) };
                download_file(&client, &url, &mods.join(rel), f.sha1.as_deref(), f.size, |_, _| {}).await?;
                Ok(())
            }
            .await;
            match res {
                Ok(()) => ok += 1,
                Err(_) => failed.push(format!("{pid}/{fid}")),
            }
        }
        let prefix = m.overrides.unwrap_or_else(|| "overrides".into());
        let ov = extract_overrides(&zip_path, &[prefix.as_str()], &game_dir)?;
        return Ok(ModpackInstallResult {
            message_zh: format!("已创建「{}」：{ok} 个模组，点「安装并启动」即可开玩", inst.name),
            instance_id: inst.id,
            instance_name: inst.name,
            game_version: m.minecraft.version,
            loader,
            loader_version: lv,
            files_downloaded: ok,
            files_failed: failed,
            overrides: ov,
        });
    }

    Err(EasyMcError::msg("不认识这个整合包格式（需要 Modrinth 或 CurseForge 整合包）"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zip_slip() {
        assert!(safe_rel("../evil").is_none());
        assert!(safe_rel("/etc/passwd").is_none());
        assert_eq!(safe_rel("mods/a.jar"), Some(PathBuf::from("mods/a.jar")));
    }

    #[test]
    fn loader_from_deps() {
        let mut d = std::collections::HashMap::new();
        d.insert("minecraft".to_string(), "1.20.1".to_string());
        d.insert("fabric-loader".to_string(), "0.15.11".to_string());
        assert_eq!(mr_loader(&d), (ModLoader::Fabric, Some("0.15.11".into())));
    }
}
