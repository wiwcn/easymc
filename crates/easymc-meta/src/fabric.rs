//! Fabric (and thin Quilt) meta client + profile installer.

use std::sync::Arc;

use easymc_core::{EasyMcError, McVersion, Paths, Result};
use serde::Deserialize;

use crate::download::{download_file, FetchResult};
use crate::install::{
    install_version, library_relative_path, maven_path_from_name, InstallOptions, InstallProgress,
};
use crate::types::{Artifact, LibraryDownloads, VersionInfo};

pub const FABRIC_META: &str = "https://meta.fabricmc.net/v2";
pub const QUILT_META: &str = "https://meta.quiltmc.org/v3";
pub const FABRIC_MAVEN: &str = "https://maven.fabricmc.net/";
pub const QUILT_MAVEN: &str = "https://maven.quiltmc.org/repository/release/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderKind {
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

impl LoaderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Fabric => "Fabric",
            Self::Quilt => "Quilt",
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
        }
    }

    pub fn is_forge_family(self) -> bool {
        matches!(self, Self::Forge | Self::NeoForge)
    }

    fn meta_base(self) -> Result<&'static str> {
        match self {
            Self::Fabric => Ok(FABRIC_META),
            Self::Quilt => Ok(QUILT_META),
            Self::Forge | Self::NeoForge => Err(EasyMcError::msg(
                "Forge/NeoForge 不使用 Fabric meta API",
            )),
        }
    }

    fn default_maven(self) -> Result<&'static str> {
        match self {
            Self::Fabric => Ok(FABRIC_MAVEN),
            Self::Quilt => Ok(QUILT_MAVEN),
            Self::Forge | Self::NeoForge => Err(EasyMcError::msg(
                "Forge/NeoForge 不使用 Fabric maven",
            )),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoaderVersionInfo {
    pub loader: LoaderBuild,
    pub intermediary: IntermediaryBuild,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoaderBuild {
    pub version: String,
    #[serde(default)]
    pub stable: bool,
    #[serde(default)]
    pub maven: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IntermediaryBuild {
    pub version: String,
    #[serde(default)]
    pub maven: Option<String>,
}

pub fn fabric_profile_id(game: &str, loader: &str) -> String {
    format!("fabric-loader-{loader}-{game}")
}

pub fn quilt_profile_id(game: &str, loader: &str) -> String {
    format!("quilt-loader-{loader}-{game}")
}

pub fn profile_id(kind: LoaderKind, game: &str, loader: &str) -> String {
    match kind {
        LoaderKind::Fabric => fabric_profile_id(game, loader),
        LoaderKind::Quilt => quilt_profile_id(game, loader),
        LoaderKind::Forge => crate::forge::forge_profile_id(game, loader),
        LoaderKind::NeoForge => crate::forge::neoforge_profile_id(loader),
    }
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("EasyMC/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| EasyMcError::msg(e.to_string()))
}

/// List loader builds for a game version (stable / newest first).
pub async fn list_loaders(kind: LoaderKind, game: &str) -> Result<Vec<LoaderVersionInfo>> {
    match kind {
        LoaderKind::Forge => return crate::forge::list_forge_versions(game).await,
        LoaderKind::NeoForge => return crate::forge::list_neoforge_versions(game).await,
        LoaderKind::Fabric | LoaderKind::Quilt => {}
    }
    let url = format!("{}/versions/loader/{game}", kind.meta_base()?);
    let client = http_client()?;
    let list: Vec<LoaderVersionInfo> = client
        .get(&url)
        .send()
        .await
        .map_err(|e| EasyMcError::Http(format!("请求 {} 失败: {e}", kind.label_zh())))?
        .error_for_status()
        .map_err(|e| EasyMcError::Http(format!("{} meta HTTP 错误: {e}", kind.label_zh())))?
        .json()
        .await
        .map_err(|e| EasyMcError::Http(format!("解析 {} loader 列表失败: {e}", kind.label_zh())))?;
    if list.is_empty() {
        return Err(EasyMcError::msg(format!(
            "{} 不支持游戏版本 {game}（meta 无 loader）",
            kind.label_zh()
        )));
    }
    Ok(list)
}

/// Pick stable loader, else newest (first entry — Fabric sorts newest first).
pub async fn resolve_loader_version(kind: LoaderKind, game: &str, prefer: Option<&str>) -> Result<String> {
    if kind.is_forge_family() {
        return crate::forge::resolve_forge_family_version(kind, game, prefer).await;
    }
    let list = list_loaders(kind, game).await?;
    if let Some(p) = prefer {
        if list.iter().any(|l| l.loader.version == p) {
            return Ok(p.to_string());
        }
        return Err(EasyMcError::msg(format!(
            "{} loader {p} 不适用于 {game}",
            kind.label_zh()
        )));
    }
    if let Some(stable) = list.iter().find(|l| l.loader.stable) {
        return Ok(stable.loader.version.clone());
    }
    Ok(list[0].loader.version.clone())
}

async fn fetch_profile_json(kind: LoaderKind, game: &str, loader: &str) -> Result<VersionInfo> {
    let url = format!(
        "{}/versions/loader/{game}/{loader}/profile/json",
        kind.meta_base()?
    );
    let client = http_client()?;
    let profile: VersionInfo = client
        .get(&url)
        .send()
        .await
        .map_err(|e| EasyMcError::Http(format!("获取 {} profile 失败: {e}", kind.label_zh())))?
        .error_for_status()
        .map_err(|e| EasyMcError::Http(format!("{} profile HTTP 错误: {e}", kind.label_zh())))?
        .json()
        .await
        .map_err(|e| EasyMcError::Http(format!("解析 {} profile 失败: {e}", kind.label_zh())))?;
    Ok(profile)
}

fn maven_url(base: &str, name: &str) -> String {
    let base = if base.ends_with('/') {
        base.to_string()
    } else {
        format!("{base}/")
    };
    format!("{base}{}", maven_path_from_name(name))
}

fn normalize_profile_libraries(kind: LoaderKind, profile: &mut VersionInfo) {
    for lib in &mut profile.libraries {
        if lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()).is_some() {
            continue;
        }
        let base = lib
            .url
            .clone()
            .unwrap_or_else(|| kind.default_maven().expect("fabric/quilt").to_string());
        let path = maven_path_from_name(&lib.name);
        let url = maven_url(&base, &lib.name);
        lib.downloads = Some(LibraryDownloads {
            artifact: Some(Artifact {
                sha1: lib.sha1.clone().unwrap_or_default(),
                size: lib.size.unwrap_or(0),
                url,
                path: Some(path),
            }),
            classifiers: None,
        });
        lib.url = Some(base);
    }
}

pub struct InstallLoaderOptions {
    pub kind: LoaderKind,
    /// Prefer this loader build; `None` = stable/latest.
    pub loader_version: Option<String>,
    /// Also install vanilla game files first.
    pub vanilla: InstallOptions,
}

/// Install Fabric/Quilt for `game` version. Returns `(profile_id, loader_version)`.
pub async fn install_loader(
    paths: &Paths,
    game: &McVersion,
    opts: InstallLoaderOptions,
    on_progress: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> Result<(String, String)> {
    let kind = opts.kind;
    if kind.is_forge_family() {
        return Err(EasyMcError::msg(
            "请使用 install_forge / install_neoforge（需传入 Java 路径）",
        ));
    }
    let on_progress = Arc::new(on_progress);
    let emit = {
        let on_progress = on_progress.clone();
        move |p: InstallProgress| on_progress(p)
    };

    // Phase plan: resolve loader meta + vanilla install phases + loader libs
    emit(InstallProgress::new(
        "meta",
        1,
        2,
        0,
        Some(1),
        format!("解析 {} loader（游戏 {}）", kind.label_zh(), game),
    ));
    let loader_ver =
        resolve_loader_version(kind, game.as_str(), opts.loader_version.as_deref()).await?;
    emit(InstallProgress::new(
        "meta",
        1,
        2,
        1,
        Some(1),
        format!("{} loader {loader_ver}", kind.label_zh()),
    ));

    // Vanilla (client/libs/natives/assets) — reuse existing installer progress labels.
    let game_clone = game.clone();
    let emit_vanilla = {
        let on_progress = on_progress.clone();
        move |p: InstallProgress| on_progress(p)
    };
    install_version(paths, &game_clone, opts.vanilla, emit_vanilla).await?;

    let mut profile = fetch_profile_json(kind, game.as_str(), &loader_ver).await?;
    if profile.inherits_from.is_none() {
        profile.inherits_from = Some(game.0.clone());
    }
    normalize_profile_libraries(kind, &mut profile);
    let id = profile_id(kind, game.as_str(), &loader_ver);
    // Prefer canonical id even if meta returns something else.
    profile.id = id.clone();

    let vdir = paths.version_dir(&id);
    tokio::fs::create_dir_all(&vdir).await?;
    let json_path = paths.version_json(&id);
    tokio::fs::write(&json_path, serde_json::to_vec_pretty(&profile)?).await?;

    let client = http_client()?;
    let total = profile.libraries.len() as u64;
    let mut skipped = 0u64;
    let mut downloaded = 0u64;
    emit(InstallProgress::new(
        "libraries",
        2,
        2,
        0,
        Some(total),
        format!("下载 {} 依赖库（{total}）", kind.label_zh()),
    ));

    for (i, lib) in profile.libraries.iter().enumerate() {
        let rel = library_relative_path(lib).ok_or_else(|| {
            EasyMcError::msg(format!("无法解析库路径: {}", lib.name))
        })?;
        let dest = paths.library_path(&rel);
        let art = lib
            .downloads
            .as_ref()
            .and_then(|d| d.artifact.as_ref())
            .ok_or_else(|| EasyMcError::msg(format!("库缺少下载信息: {}", lib.name)))?;
        let sha = if art.sha1.is_empty() {
            None
        } else {
            Some(art.sha1.as_str())
        };
        let size = if art.size == 0 { None } else { Some(art.size) };
        let res = download_file(&client, &art.url, &dest, sha, size, |_, _| {}).await;
        match res {
            Ok(FetchResult::Skipped) => skipped += 1,
            Ok(FetchResult::Downloaded) => downloaded += 1,
            Err(e) => {
                let e = EasyMcError::msg(format!(
                    "安装失败（阶段：{} 依赖库）：{} — {e}",
                    kind.label_zh(),
                    lib.name
                ));
                let mut p = InstallProgress::new("error", 2, 2, i as u64, Some(total), e.to_string());
                p.stage_label = format!("失败 · {}", kind.label_zh());
                emit(p);
                return Err(e);
            }
        }
        if i % 1 == 0 || i + 1 == profile.libraries.len() {
            emit(
                InstallProgress::new(
                    "libraries",
                    2,
                    2,
                    (i + 1) as u64,
                    Some(total),
                    lib.name.clone(),
                )
                .with_counts(skipped, downloaded),
            );
        }
    }

    emit(
        InstallProgress::new(
            "done",
            2,
            2,
            1,
            Some(1),
            format!(
                "{} {} @ {} 安装完成（新下 {downloaded}，跳过 {skipped}）",
                kind.label_zh(),
                loader_ver,
                game
            ),
        )
        .with_counts(skipped, downloaded),
    );

    Ok((id, loader_ver))
}

pub async fn install_fabric(
    paths: &Paths,
    game: &McVersion,
    loader_version: Option<String>,
    vanilla: InstallOptions,
    on_progress: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> Result<(String, String)> {
    install_loader(
        paths,
        game,
        InstallLoaderOptions {
            kind: LoaderKind::Fabric,
            loader_version,
            vanilla,
        },
        on_progress,
    )
    .await
}

pub async fn install_quilt(
    paths: &Paths,
    game: &McVersion,
    loader_version: Option<String>,
    vanilla: InstallOptions,
    on_progress: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> Result<(String, String)> {
    install_loader(
        paths,
        game,
        InstallLoaderOptions {
            kind: LoaderKind::Quilt,
            loader_version,
            vanilla,
        },
        on_progress,
    )
    .await
}

/// Whether a loader profile is on disk and parent jar exists.
pub fn is_loader_installed(paths: &Paths, kind: LoaderKind, game: &str, loader: &str) -> bool {
    let id = profile_id(kind, game, loader);
    crate::install::is_version_installed(paths, &id)
}
