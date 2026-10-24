//! Forge / NeoForge: list builds from official Maven, install via official installer JAR
//! (`--installClient`) so processors (binarypatcher, etc.) run correctly.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use easymc_core::{DownloadSource, EasyMcError, McVersion, Paths, Result};
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::download::{download_with_source, get_with_source};
use crate::fabric::{IntermediaryBuild, LoaderBuild, LoaderKind, LoaderVersionInfo};
use crate::install::{install_version, InstallOptions, InstallProgress};

pub const FORGE_MAVEN: &str = "https://maven.minecraftforge.net";
pub const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases";
pub const NEOFORGE_VERSIONS_API: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";

#[derive(Debug, Deserialize)]
struct NeoForgeVersionsApi {
    versions: Vec<String>,
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("EasyMC/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| EasyMcError::msg(e.to_string()))
}

/// NeoForge uses `21.1.x` for Minecraft `1.21.1`, `21.0.x` for `1.21`, etc.
/// Returns the version prefix to filter (e.g. `"21.1."`). Not used for MC ≤1.20.1
/// (those used the transitional `net.neoforged:forge` artifact — unsupported here).
pub fn neoforge_version_prefix(mc: &str) -> Result<String> {
    let rest = mc.strip_prefix("1.").unwrap_or(mc);
    let parts: Vec<&str> = rest.split('.').filter(|p| !p.is_empty()).collect();
    match parts.as_slice() {
        [maj, min, ..] => {
            // 1.20.1 and older NeoForge used a different GAV; refuse with a clear tip.
            let maj_n: u32 = maj.parse().unwrap_or(0);
            let min_n: u32 = min.parse().unwrap_or(0);
            if maj_n < 20 || (maj_n == 20 && min_n <= 1) {
                return Err(EasyMcError::msg(format!(
                    "NeoForge EasyMC 支持 MC ≥1.20.2（当前 {mc}）。1.20.1 请用 Forge，或自行装官方 NeoForge installer。"
                )));
            }
            Ok(format!("{maj}.{min}."))
        }
        [maj] => Ok(format!("{maj}.0.")),
        _ => Err(EasyMcError::msg(format!(
            "无法从游戏版本 {mc} 推导 NeoForge 版本前缀"
        ))),
    }
}

/// Profile id written by the official NeoForge installer.
pub fn neoforge_profile_id(loader: &str) -> String {
    format!("neoforge-{loader}")
}

/// Profile id written by the official Forge installer (`{mc}-forge-{forge}`).
pub fn forge_profile_id(mc: &str, loader: &str) -> String {
    format!("{mc}-forge-{loader}")
}

pub fn forge_family_profile_id(kind: LoaderKind, game: &str, loader: &str) -> String {
    match kind {
        LoaderKind::Forge => forge_profile_id(game, loader),
        LoaderKind::NeoForge => neoforge_profile_id(loader),
        _ => unreachable!("forge_family_profile_id only for Forge/NeoForge"),
    }
}

/// Maven coordinate version for Forge installer: `{mc}-{loader}` (e.g. `1.21.1-52.1.20`).
pub fn forge_maven_version(mc: &str, loader: &str) -> String {
    if loader.starts_with(&format!("{mc}-")) {
        loader.to_string()
    } else {
        format!("{mc}-{loader}")
    }
}

/// Strip `{mc}-` prefix from a Forge maven version to get the loader build id.
pub fn forge_loader_from_maven(mc: &str, maven_ver: &str) -> String {
    let prefix = format!("{mc}-");
    maven_ver
        .strip_prefix(&prefix)
        .unwrap_or(maven_ver)
        .to_string()
}

fn forge_installer_url(mc: &str, loader: &str) -> String {
    let ver = forge_maven_version(mc, loader);
    format!("{FORGE_MAVEN}/net/minecraftforge/forge/{ver}/forge-{ver}-installer.jar")
}

fn neoforge_installer_url(loader: &str) -> String {
    format!("{NEOFORGE_MAVEN}/net/neoforged/neoforge/{loader}/neoforge-{loader}-installer.jar")
}

/// List Forge builds for a Minecraft version (newest first). `loader.version` is the
/// Forge build only (e.g. `52.1.20`); `intermediary.version` echoes the game version.
pub async fn list_forge_versions(game: &str) -> Result<Vec<LoaderVersionInfo>> {
    list_forge_versions_with_source(game, DownloadSource::Official).await
}

pub async fn list_forge_versions_with_source(
    game: &str,
    source: DownloadSource,
) -> Result<Vec<LoaderVersionInfo>> {
    let url = format!("{FORGE_MAVEN}/net/minecraftforge/forge/maven-metadata.xml");
    let client = http_client()?;
    let text = get_with_source(&client, &url, source)
        .await
        .map_err(|e| EasyMcError::Http(format!("请求 Forge maven-metadata 失败: {e}")))?
        .text()
        .await
        .map_err(|e| EasyMcError::Http(format!("读取 Forge maven-metadata 失败: {e}")))?;
    let prefix = format!("{game}-");
    let mut versions = Vec::new();
    for chunk in text.split("<version>").skip(1) {
        if let Some(end) = chunk.find("</version>") {
            let v = chunk[..end].trim();
            if !v.is_empty() {
                versions.push(v.to_string());
            }
        }
    }
    if versions.is_empty() {
        return Err(EasyMcError::msg("解析 Forge maven-metadata 失败：未找到 <version>"));
    }
    let mut out: Vec<LoaderVersionInfo> = versions
        .into_iter()
        .filter(|v| v.starts_with(&prefix))
        .map(|maven_ver| {
            let loader = forge_loader_from_maven(game, &maven_ver);
            LoaderVersionInfo {
                loader: LoaderBuild {
                    version: loader,
                    stable: false,
                    maven: Some(format!("net.minecraftforge:forge:{maven_ver}")),
                },
                intermediary: IntermediaryBuild {
                    version: game.to_string(),
                    maven: None,
                },
            }
        })
        .collect();
    // Maven metadata is usually oldest→newest; reverse so newest first.
    out.reverse();
    if out.is_empty() {
        return Err(EasyMcError::msg(format!(
            "Forge 不支持游戏版本 {game}（maven 无匹配构建）"
        )));
    }
    // Mark first (newest) as preferred "stable" for UI auto-pick.
    if let Some(first) = out.first_mut() {
        first.loader.stable = true;
    }
    Ok(out)
}

/// List NeoForge builds for a Minecraft version (newest first).
pub async fn list_neoforge_versions(game: &str) -> Result<Vec<LoaderVersionInfo>> {
    list_neoforge_versions_with_source(game, DownloadSource::Official).await
}

pub async fn list_neoforge_versions_with_source(
    game: &str,
    source: DownloadSource,
) -> Result<Vec<LoaderVersionInfo>> {
    let prefix = neoforge_version_prefix(game)?;
    let client = http_client()?;
    // NeoForge versions API is not mirrored on BMCLAPI — get_with_source leaves it official.
    let api: NeoForgeVersionsApi = get_with_source(&client, NEOFORGE_VERSIONS_API, source)
        .await
        .map_err(|e| EasyMcError::Http(format!("请求 NeoForge 版本列表失败: {e}")))?
        .json()
        .await
        .map_err(|e| EasyMcError::Http(format!("解析 NeoForge 版本列表失败: {e}")))?;
    let mut out: Vec<LoaderVersionInfo> = api
        .versions
        .into_iter()
        .filter(|v| v.starts_with(&prefix) && !v.contains('-')) // skip betas like 21.1.0-beta
        .map(|loader| LoaderVersionInfo {
            loader: LoaderBuild {
                version: loader.clone(),
                stable: false,
                maven: Some(format!("net.neoforged:neoforge:{loader}")),
            },
            intermediary: IntermediaryBuild {
                version: game.to_string(),
                maven: None,
            },
        })
        .collect();
    // API returns ascending; newest last → reverse.
    out.reverse();
    if out.is_empty() {
        return Err(EasyMcError::msg(format!(
            "NeoForge 不支持游戏版本 {game}（无 {prefix}* 构建）"
        )));
    }
    if let Some(first) = out.first_mut() {
        first.loader.stable = true;
    }
    Ok(out)
}

pub async fn resolve_forge_family_version(
    kind: LoaderKind,
    game: &str,
    prefer: Option<&str>,
) -> Result<String> {
    resolve_forge_family_version_with_source(kind, game, prefer, DownloadSource::Official).await
}

pub async fn resolve_forge_family_version_with_source(
    kind: LoaderKind,
    game: &str,
    prefer: Option<&str>,
    source: DownloadSource,
) -> Result<String> {
    let list = match kind {
        LoaderKind::Forge => list_forge_versions_with_source(game, source).await?,
        LoaderKind::NeoForge => list_neoforge_versions_with_source(game, source).await?,
        _ => {
            return Err(EasyMcError::msg(
                "resolve_forge_family_version 仅用于 Forge/NeoForge",
            ))
        }
    };
    if let Some(p) = prefer {
        // Accept either bare loader (`52.1.20`) or full maven (`1.21.1-52.1.20`).
        let normalized = match kind {
            LoaderKind::Forge => forge_loader_from_maven(game, p),
            _ => p.to_string(),
        };
        if list.iter().any(|l| l.loader.version == normalized) {
            return Ok(normalized);
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

fn ensure_launcher_profiles(root: &Path) -> Result<()> {
    let p = root.join("launcher_profiles.json");
    if !p.exists() {
        std::fs::write(&p, b"{}").map_err(|e| {
            EasyMcError::msg(format!("写入 launcher_profiles.json: {e}"))
        })?;
    }
    Ok(())
}

async fn run_official_installer(
    java_bin: &Path,
    installer_jar: &Path,
    client_dir: &Path,
    label: &str,
    on_progress: Arc<dyn Fn(InstallProgress) + Send + Sync>,
) -> Result<()> {
    ensure_launcher_profiles(client_dir)?;
    on_progress(InstallProgress::new(
        "processors",
        3,
        3,
        0,
        Some(1),
        format!("运行官方 {label} installer（--installClient）…"),
    ));

    let mut child = tokio::process::Command::new(java_bin)
        .arg("-jar")
        .arg(installer_jar)
        .arg("--installClient")
        .arg(client_dir)
        .current_dir(client_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            EasyMcError::msg(format!(
                "无法启动 {label} installer（Java {}）: {e}",
                java_bin.display()
            ))
        })?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    if let Some(out) = stdout {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(out).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx.send(line);
            }
        });
    }
    if let Some(err) = stderr {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(err).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx.send(line);
            }
        });
    }
    drop(tx);

    let mut last_lines = 0u64;
    while let Some(line) = rx.recv().await {
        last_lines += 1;
        // Keep progress noisy but capped — installer dumps class names.
        let short = if line.len() > 120 {
            format!("{}…", &line[..117])
        } else {
            line
        };
        if last_lines % 25 == 1 || short.contains("Successfully") || short.contains("Patching") {
            on_progress(InstallProgress::new(
                "processors",
                3,
                3,
                0,
                Some(1),
                format!("{label}: {short}"),
            ));
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| EasyMcError::msg(format!("等待 {label} installer: {e}")))?;
    if !status.success() {
        return Err(EasyMcError::msg(format!(
            "{label} installer 失败（exit {:?}）。请确认已安装对应原版客户端，且 Java 可用。",
            status.code()
        )));
    }
    Ok(())
}

/// Install Forge or NeoForge via official installer. `java_bin` must be a JDK/JRE ≥17
/// (21 recommended for 1.21.x). Returns `(profile_id, loader_version)`.
pub async fn install_forge_family(
    paths: &Paths,
    game: &McVersion,
    kind: LoaderKind,
    loader_version: Option<String>,
    java_bin: &Path,
    vanilla: InstallOptions,
    on_progress: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> Result<(String, String)> {
    if !matches!(kind, LoaderKind::Forge | LoaderKind::NeoForge) {
        return Err(EasyMcError::msg(
            "install_forge_family 仅用于 Forge/NeoForge",
        ));
    }
    let on_progress = Arc::new(on_progress);
    let emit = {
        let on_progress = on_progress.clone();
        move |p: InstallProgress| on_progress(p)
    };

    emit(InstallProgress::new(
        "meta",
        1,
        3,
        0,
        Some(1),
        format!("解析 {}（游戏 {}）", kind.label_zh(), game),
    ));
    let source = vanilla.download_source;
    let loader_ver = resolve_forge_family_version_with_source(
        kind,
        game.as_str(),
        loader_version.as_deref(),
        source,
    )
    .await?;
    emit(InstallProgress::new(
        "meta",
        1,
        3,
        1,
        Some(1),
        format!("{} {loader_ver}", kind.label_zh()),
    ));

    let game_clone = game.clone();
    let emit_vanilla = {
        let on_progress = on_progress.clone();
        move |p: InstallProgress| on_progress(p)
    };
    install_version(paths, &game_clone, vanilla, emit_vanilla).await?;

    let (url, jar_name) = match kind {
        LoaderKind::Forge => (
            forge_installer_url(game.as_str(), &loader_ver),
            format!("forge-{}-{}-installer.jar", game, loader_ver),
        ),
        LoaderKind::NeoForge => (
            neoforge_installer_url(&loader_ver),
            format!("neoforge-{loader_ver}-installer.jar"),
        ),
        _ => unreachable!(),
    };

    let installer_dir = paths.meta_dir().join("installers");
    tokio::fs::create_dir_all(&installer_dir).await?;
    let installer_jar: PathBuf = installer_dir.join(&jar_name);

    emit(InstallProgress::new(
        "libraries",
        2,
        3,
        0,
        Some(1),
        format!("下载 {} installer…", kind.label_zh()),
    ));
    let client = http_client()?;
    download_with_source(&client, &url, &installer_jar, None, None, source, |_, _| {})
        .await
        .map_err(|e| {
            EasyMcError::msg(format!(
                "下载 {} installer 失败（{url}）: {e}",
                kind.label_zh()
            ))
        })?;
    emit(InstallProgress::new(
        "libraries",
        2,
        3,
        1,
        Some(1),
        format!("{} installer 已就绪", kind.label_zh()),
    ));

    let label = kind.label_zh().to_string();
    run_official_installer(
        java_bin,
        &installer_jar,
        &paths.root,
        &label,
        on_progress.clone(),
    )
    .await?;

    let profile_id = forge_family_profile_id(kind, game.as_str(), &loader_ver);
    if !paths.version_json(&profile_id).exists() {
        return Err(EasyMcError::msg(format!(
            "{} installer 完成但未找到配置 {profile_id}.json — 版本号可能与官方命名不一致",
            kind.label_zh()
        )));
    }

    emit(InstallProgress::new(
        "done",
        3,
        3,
        1,
        Some(1),
        format!(
            "{} {} @ {} 安装完成 → {profile_id}",
            kind.label_zh(),
            loader_ver,
            game
        ),
    ));

    Ok((profile_id, loader_ver))
}

pub async fn install_forge(
    paths: &Paths,
    game: &McVersion,
    loader_version: Option<String>,
    java_bin: &Path,
    vanilla: InstallOptions,
    on_progress: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> Result<(String, String)> {
    install_forge_family(
        paths,
        game,
        LoaderKind::Forge,
        loader_version,
        java_bin,
        vanilla,
        on_progress,
    )
    .await
}

pub async fn install_neoforge(
    paths: &Paths,
    game: &McVersion,
    loader_version: Option<String>,
    java_bin: &Path,
    vanilla: InstallOptions,
    on_progress: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> Result<(String, String)> {
    install_forge_family(
        paths,
        game,
        LoaderKind::NeoForge,
        loader_version,
        java_bin,
        vanilla,
        on_progress,
    )
    .await
}
