use easymc_core::{EasyMcError, Result};
use serde::Deserialize;

use crate::http::modrinth_client;
use crate::types::{
    loader_filter_name, ModFile, ModProject, ModSource, ModVersion, SearchQuery,
};

const BASE: &str = "https://api.modrinth.com/v2";

#[derive(Debug, Deserialize)]
struct SearchResponse {
    hits: Vec<SearchHit>,
}

#[derive(Debug, Deserialize)]
struct SearchHit {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    categories: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct VersionDto {
    id: String,
    project_id: String,
    name: String,
    version_number: String,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    date_published: Option<String>,
    #[serde(default)]
    files: Vec<FileDto>,
}

#[derive(Debug, Deserialize)]
struct FileDto {
    url: String,
    filename: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: HashesDto,
}

#[derive(Debug, Deserialize, Default)]
struct HashesDto {
    #[serde(default)]
    sha1: Option<String>,
}

fn map_project(h: SearchHit) -> ModProject {
    ModProject {
        id: h.project_id,
        slug: h.slug,
        name: h.title,
        description: h.description,
        source: ModSource::Modrinth,
        downloads: h.downloads,
        author: h.author,
        icon_url: h.icon_url,
        categories: h.categories,
    }
}

fn map_version(v: VersionDto) -> ModVersion {
    ModVersion {
        id: v.id,
        project_id: v.project_id,
        name: v.name,
        version_number: v.version_number,
        game_versions: v.game_versions,
        loaders: v.loaders,
        downloads: v.downloads,
        date_published: v.date_published,
        files: v
            .files
            .into_iter()
            .map(|f| ModFile {
                filename: f.filename,
                url: f.url,
                size: if f.size > 0 { Some(f.size) } else { None },
                primary: f.primary,
                sha1: f.hashes.sha1,
            })
            .collect(),
    }
}

fn build_facets(query: &SearchQuery) -> String {
    let mut facets: Vec<Vec<String>> =
        vec![vec![format!("project_type:{}", query.project_type.modrinth_facet())]];
    if let Some(ref gv) = query.game_version {
        if !gv.is_empty() {
            facets.push(vec![format!("versions:{gv}")]);
        }
    }
    // Loader facets only make sense for jar mods (modpacks: filter client-side by deps).
    let loader = if query.project_type == crate::types::ProjectType::Mod { query.loader } else { None };
    if let Some(loader) = loader.and_then(loader_filter_name) {
        facets.push(vec![format!("categories:{loader}")]);
    }
    serde_json::to_string(&facets).unwrap_or_else(|_| "[[\"project_type:mod\"]]".into())
}

pub async fn search(query: &SearchQuery) -> Result<Vec<ModProject>> {
    let client = modrinth_client()?;
    let limit = query.limit.clamp(1, 100);
    // Empty query → downloads (popular/hot); otherwise relevance.
    let index = if query.text.trim().is_empty() {
        "downloads"
    } else {
        "relevance"
    };
    let resp = client
        .get(format!("{BASE}/search"))
        .query(&[
            ("query", query.text.as_str()),
            ("facets", &build_facets(query)),
            ("limit", &limit.to_string()),
            ("offset", &query.offset.to_string()),
            ("index", index),
        ])
        .send()
        .await
        .map_err(|e| EasyMcError::Http(format!("Modrinth 搜索失败: {e}")))?
        .error_for_status()
        .map_err(|e| EasyMcError::Http(format!("Modrinth 搜索 HTTP 错误: {e}")))?;
    let body: SearchResponse = resp
        .json()
        .await
        .map_err(|e| EasyMcError::Http(format!("解析 Modrinth 搜索结果失败: {e}")))?;
    Ok(body.hits.into_iter().map(map_project).collect())
}

pub async fn get_project(id_or_slug: &str) -> Result<ModProject> {
    let client = modrinth_client()?;
    #[derive(Deserialize)]
    struct ProjectDto {
        id: String,
        slug: String,
        title: String,
        description: String,
        #[serde(default)]
        downloads: u64,
        #[serde(default)]
        icon_url: Option<String>,
        #[serde(default)]
        categories: Vec<String>,
    }
    let p: ProjectDto = client
        .get(format!("{BASE}/project/{id_or_slug}"))
        .send()
        .await
        .map_err(|e| EasyMcError::Http(format!("Modrinth 项目请求失败: {e}")))?
        .error_for_status()
        .map_err(|e| EasyMcError::Http(format!("Modrinth 项目不存在或不可用: {e}")))?
        .json()
        .await
        .map_err(|e| EasyMcError::Http(format!("解析 Modrinth 项目失败: {e}")))?;
    Ok(ModProject {
        id: p.id,
        slug: p.slug,
        name: p.title,
        description: p.description,
        source: ModSource::Modrinth,
        downloads: p.downloads,
        author: None,
        icon_url: p.icon_url,
        categories: p.categories,
    })
}

pub async fn list_versions(
    id_or_slug: &str,
    game_version: Option<&str>,
    loader: Option<&str>,
) -> Result<Vec<ModVersion>> {
    let client = modrinth_client()?;
    let mut req = client.get(format!("{BASE}/project/{id_or_slug}/version"));
    if let Some(gv) = game_version.filter(|s| !s.is_empty()) {
        let arr = serde_json::to_string(&vec![gv]).unwrap();
        req = req.query(&[("game_versions", arr)]);
    }
    if let Some(ld) = loader.filter(|s| !s.is_empty()) {
        let arr = serde_json::to_string(&vec![ld]).unwrap();
        req = req.query(&[("loaders", arr)]);
    }
    req = req.query(&[("include_changelog", "false")]);
    let list: Vec<VersionDto> = req
        .send()
        .await
        .map_err(|e| EasyMcError::Http(format!("Modrinth 版本列表失败: {e}")))?
        .error_for_status()
        .map_err(|e| EasyMcError::Http(format!("Modrinth 版本列表 HTTP 错误: {e}")))?
        .json()
        .await
        .map_err(|e| EasyMcError::Http(format!("解析 Modrinth 版本失败: {e}")))?;
    Ok(list.into_iter().map(map_version).collect())
}

pub async fn get_version(version_id: &str) -> Result<ModVersion> {
    let client = modrinth_client()?;
    let v: VersionDto = client
        .get(format!("{BASE}/version/{version_id}"))
        .send()
        .await
        .map_err(|e| EasyMcError::Http(format!("Modrinth 版本请求失败: {e}")))?
        .error_for_status()
        .map_err(|e| EasyMcError::Http(format!("Modrinth 版本不存在: {e}")))?
        .json()
        .await
        .map_err(|e| EasyMcError::Http(format!("解析 Modrinth 版本失败: {e}")))?;
    Ok(map_version(v))
}

#[derive(Debug, Clone)]
pub struct HashLookup {
    pub project_id: String,
    pub version_id: String,
    pub filename: Option<String>,
}

/// Resolve a local jar SHA-1 via Modrinth `version_file` API. Returns None on 404.
pub async fn lookup_by_sha1(sha1_hex: &str) -> Result<Option<HashLookup>> {
    let sha1_hex = sha1_hex.trim().to_ascii_lowercase();
    if sha1_hex.len() != 40 || !sha1_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(None);
    }
    let client = modrinth_client()?;
    let resp = client
        .get(format!("{BASE}/version_file/{sha1_hex}"))
        .query(&[("algorithm", "sha1")])
        .send()
        .await
        .map_err(|e| EasyMcError::Http(format!("Modrinth hash 查询失败: {e}")))?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let resp = resp
        .error_for_status()
        .map_err(|e| EasyMcError::Http(format!("Modrinth hash HTTP: {e}")))?;
    let v: VersionDto = resp
        .json()
        .await
        .map_err(|e| EasyMcError::Http(format!("解析 Modrinth hash 结果失败: {e}")))?;
    let filename = v
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| v.files.first())
        .map(|f| f.filename.clone());
    Ok(Some(HashLookup {
        project_id: v.project_id,
        version_id: v.id,
        filename,
    }))
}
