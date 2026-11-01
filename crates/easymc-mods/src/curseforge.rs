use easymc_core::{EasyMcError, ModLoader, Result};
use serde::Deserialize;

use crate::http::curseforge_client;
use crate::types::{ModFile, ModProject, ModSource, ModVersion, SearchQuery};

const BASE: &str = "https://api.curseforge.com/v1";
/// Minecraft Java
const GAME_ID: u32 = 432;
/// Mods class

pub const API_KEY_ENV: &str = "EASYMC_CURSEFORGE_API_KEY";
pub const CONSOLE_URL: &str = "https://console.curseforge.com/";

/// Key pasted in Settings / saved to local settings.json (not env).
static STORED_API_KEY: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

pub fn api_key_from_env() -> Option<String> {
    std::env::var(API_KEY_ENV)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Install / refresh the settings-file key used when env is unset.
pub fn set_stored_api_key(key: Option<String>) {
    let cleaned = key
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Ok(mut g) = STORED_API_KEY.write() {
        *g = cleaned;
    }
}

pub fn stored_api_key() -> Option<String> {
    STORED_API_KEY.read().ok().and_then(|g| g.clone())
}

/// Resolve key: env overrides settings-file / in-memory stored key.
pub fn api_key() -> Option<String> {
    api_key_from_env().or_else(stored_api_key)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeySource {
    Env,
    Settings,
    None,
}

pub fn api_key_source() -> ApiKeySource {
    if api_key_from_env().is_some() {
        ApiKeySource::Env
    } else if stored_api_key().is_some() {
        ApiKeySource::Settings
    } else {
        ApiKeySource::None
    }
}

pub fn require_api_key() -> Result<String> {
    api_key().ok_or_else(|| {
        EasyMcError::msg(format!(
            "未配置 CurseForge API Key。请在「设置」中粘贴 Key，或设置环境变量 {API_KEY_ENV}（申请: {CONSOLE_URL}）。Modrinth 搜索/安装无需此 Key。"
        ))
    })
}

pub fn sync_api_key_from_settings(settings_key: Option<&str>) {
    set_stored_api_key(settings_key.map(|s| s.to_string()));
}

fn mod_loader_type(loader: ModLoader) -> Option<u32> {
    match loader {
        ModLoader::Forge => Some(1),
        ModLoader::Fabric => Some(4),
        ModLoader::Quilt => Some(5),
        ModLoader::NeoForge => Some(6),
        ModLoader::Vanilla => None,
    }
}

#[derive(Debug, Deserialize)]
struct CfData<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMod {
    id: u64,
    name: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    download_count: f64,
    #[serde(default)]
    authors: Vec<CfAuthor>,
    #[serde(default)]
    logo: Option<CfLogo>,
    #[serde(default)]
    categories: Vec<CfCategory>,
}

#[derive(Debug, Deserialize)]
struct CfAuthor {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfLogo {
    #[serde(default)]
    thumbnail_url: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CfCategory {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    id: u64,
    #[serde(default)]
    mod_id: u64,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    file_name: String,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    file_length: u64,
    #[serde(default)]
    download_count: f64,
    #[serde(default)]
    file_date: Option<String>,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    hashes: Vec<CfHash>,
}

#[derive(Debug, Deserialize)]
struct CfHash {
    value: String,
    /// 1 = Sha1, 2 = Md5
    algo: u32,
}

fn map_mod(m: CfMod) -> ModProject {
    let icon = m
        .logo
        .as_ref()
        .and_then(|l| l.thumbnail_url.clone().or(l.url.clone()));
    let author = m.authors.first().map(|a| a.name.clone());
    ModProject {
        id: m.id.to_string(),
        slug: if m.slug.is_empty() {
            m.id.to_string()
        } else {
            m.slug
        },
        name: m.name,
        description: m.summary,
        source: ModSource::CurseForge,
        downloads: m.download_count as u64,
        author,
        icon_url: icon,
        categories: m.categories.into_iter().map(|c| c.name).collect(),
    }
}

fn map_file(f: CfFile) -> ModVersion {
    let sha1 = f
        .hashes
        .iter()
        .find(|h| h.algo == 1)
        .map(|h| h.value.clone());
    let loaders: Vec<String> = f
        .game_versions
        .iter()
        .filter(|v| {
            let l = v.to_ascii_lowercase();
            matches!(
                l.as_str(),
                "fabric" | "quilt" | "forge" | "neoforge" | "rift" | "liteloader"
            )
        })
        .map(|v| v.to_ascii_lowercase())
        .collect();
    let game_versions: Vec<String> = f
        .game_versions
        .iter()
        .filter(|v| {
            let l = v.to_ascii_lowercase();
            !matches!(
                l.as_str(),
                "fabric" | "quilt" | "forge" | "neoforge" | "rift" | "liteloader" | "client" | "server"
            )
        })
        .cloned()
        .collect();
    let url = f.download_url.clone().unwrap_or_default();
    ModVersion {
        id: f.id.to_string(),
        project_id: f.mod_id.to_string(),
        name: if f.display_name.is_empty() {
            f.file_name.clone()
        } else {
            f.display_name
        },
        version_number: f.file_name.clone(),
        game_versions,
        loaders,
        downloads: f.download_count as u64,
        date_published: f.file_date,
        files: vec![ModFile {
            filename: f.file_name,
            url,
            size: if f.file_length > 0 {
                Some(f.file_length)
            } else {
                None
            },
            primary: true,
            sha1,
            sha512: None,
        }],
        changelog: None,
    }
}

pub async fn search(query: &SearchQuery) -> Result<Vec<ModProject>> {
    let key = require_api_key()?;
    let client = curseforge_client(&key)?;
    let limit = query.limit.clamp(1, 50);
    let mut req = client
        .get(format!("{BASE}/mods/search"))
        .query(&[
            ("gameId", GAME_ID.to_string()),
            ("classId", query.project_type.curseforge_class_id().to_string()),
            ("pageSize", limit.to_string()),
            ("index", query.offset.to_string()),
            ("sortField", "1".into()), // Popularity
            ("sortOrder", "desc".into()),
        ]);
    if !query.text.is_empty() {
        req = req.query(&[("searchFilter", query.text.as_str())]);
    }
    if let Some(ref gv) = query.game_version {
        if !gv.is_empty() {
            req = req.query(&[("gameVersion", gv.as_str())]);
        }
    }
    if let Some(loader) = query.loader.filter(|_| query.project_type == crate::types::ProjectType::Mod) {
        if let Some(t) = mod_loader_type(loader) {
            req = req.query(&[("modLoaderType", t.to_string())]);
        }
    }
    let body: CfData<Vec<CfMod>> = req
        .send()
        .await
        .map_err(|e| EasyMcError::Http(format!("CurseForge 搜索失败: {e}")))?
        .error_for_status()
        .map_err(|e| EasyMcError::Http(format!("CurseForge 搜索 HTTP 错误: {e}")))?
        .json()
        .await
        .map_err(|e| EasyMcError::Http(format!("解析 CurseForge 搜索结果失败: {e}")))?;
    Ok(body.data.into_iter().map(map_mod).collect())
}

pub async fn get_project(mod_id: &str) -> Result<ModProject> {
    let key = require_api_key()?;
    let client = curseforge_client(&key)?;
    let id: u64 = mod_id
        .parse()
        .map_err(|_| EasyMcError::msg(format!("CurseForge 模组 ID 应为数字，收到: {mod_id}")))?;
    let body: CfData<CfMod> = client
        .get(format!("{BASE}/mods/{id}"))
        .send()
        .await
        .map_err(|e| EasyMcError::Http(format!("CurseForge 项目请求失败: {e}")))?
        .error_for_status()
        .map_err(|e| EasyMcError::Http(format!("CurseForge 项目不存在: {e}")))?
        .json()
        .await
        .map_err(|e| EasyMcError::Http(format!("解析 CurseForge 项目失败: {e}")))?;
    Ok(map_mod(body.data))
}

pub async fn list_versions(
    mod_id: &str,
    game_version: Option<&str>,
    loader: Option<ModLoader>,
) -> Result<Vec<ModVersion>> {
    let key = require_api_key()?;
    let client = curseforge_client(&key)?;
    let id: u64 = mod_id
        .parse()
        .map_err(|_| EasyMcError::msg(format!("CurseForge 模组 ID 应为数字，收到: {mod_id}")))?;
    let mut req = client
        .get(format!("{BASE}/mods/{id}/files"))
        .query(&[("pageSize", "50")]);
    if let Some(gv) = game_version.filter(|s| !s.is_empty()) {
        req = req.query(&[("gameVersion", gv)]);
    }
    if let Some(ld) = loader.and_then(mod_loader_type) {
        req = req.query(&[("modLoaderType", ld.to_string())]);
    }
    let body: CfData<Vec<CfFile>> = req
        .send()
        .await
        .map_err(|e| EasyMcError::Http(format!("CurseForge 文件列表失败: {e}")))?
        .error_for_status()
        .map_err(|e| EasyMcError::Http(format!("CurseForge 文件列表 HTTP 错误: {e}")))?
        .json()
        .await
        .map_err(|e| EasyMcError::Http(format!("解析 CurseForge 文件失败: {e}")))?;
    // Ensure mod_id on files
    Ok(body
        .data
        .into_iter()
        .map(|mut f| {
            if f.mod_id == 0 {
                f.mod_id = id;
            }
            map_file(f)
        })
        .collect())
}

pub async fn get_version(mod_id: &str, file_id: &str) -> Result<ModVersion> {
    let key = require_api_key()?;
    let client = curseforge_client(&key)?;
    let mid: u64 = mod_id
        .parse()
        .map_err(|_| EasyMcError::msg(format!("无效 CurseForge mod id: {mod_id}")))?;
    let fid: u64 = file_id
        .parse()
        .map_err(|_| EasyMcError::msg(format!("无效 CurseForge file id: {file_id}")))?;
    let body: CfData<CfFile> = client
        .get(format!("{BASE}/mods/{mid}/files/{fid}"))
        .send()
        .await
        .map_err(|e| EasyMcError::Http(format!("CurseForge 文件请求失败: {e}")))?
        .error_for_status()
        .map_err(|e| EasyMcError::Http(format!("CurseForge 文件不存在: {e}")))?
        .json()
        .await
        .map_err(|e| EasyMcError::Http(format!("解析 CurseForge 文件失败: {e}")))?;
    let mut f = body.data;
    if f.mod_id == 0 {
        f.mod_id = mid;
    }
    let mut ver = map_file(f);
    // Resolve download URL if empty
    if ver.files.first().map(|x| x.url.is_empty()).unwrap_or(true) {
        let url_body: CfData<String> = client
            .get(format!("{BASE}/mods/{mid}/files/{fid}/download-url"))
            .send()
            .await
            .map_err(|e| EasyMcError::Http(format!("CurseForge 下载地址失败: {e}")))?
            .error_for_status()
            .map_err(|e| EasyMcError::Http(format!("CurseForge 下载地址 HTTP 错误: {e}")))?
            .json()
            .await
            .map_err(|e| EasyMcError::Http(format!("解析 CurseForge 下载地址失败: {e}")))?;
        if let Some(file) = ver.files.first_mut() {
            file.url = url_body.data;
        }
    }
    Ok(ver)
}

pub async fn resolve_download_url(mod_id: &str, file_id: &str, current: &str) -> Result<String> {
    if !current.is_empty() {
        return Ok(current.to_string());
    }
    let key = require_api_key()?;
    let client = curseforge_client(&key)?;
    let mid: u64 = mod_id.parse().map_err(|_| EasyMcError::msg("无效 mod id"))?;
    let fid: u64 = file_id.parse().map_err(|_| EasyMcError::msg("无效 file id"))?;
    let url_body: CfData<String> = client
        .get(format!("{BASE}/mods/{mid}/files/{fid}/download-url"))
        .send()
        .await
        .map_err(|e| EasyMcError::Http(format!("CurseForge 下载地址失败: {e}")))?
        .error_for_status()
        .map_err(|e| EasyMcError::Http(format!("CurseForge 下载地址 HTTP 错误: {e}")))?
        .json()
        .await
        .map_err(|e| EasyMcError::Http(format!("解析 CurseForge 下载地址失败: {e}")))?;
    Ok(url_body.data)
}
