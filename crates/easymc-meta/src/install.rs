use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use easymc_core::{EasyMcError, McVersion, Paths, Result};
use zip::ZipArchive;

use crate::download::download_file;
use crate::rules::{is_native_library, rules_allow};
use crate::types::{AssetIndex, Library, VersionInfo};
use crate::{MetaProvider, PistonMetaProvider};

#[derive(Debug, Clone)]
pub struct InstallProgress {
    pub stage: String,
    pub current: u64,
    pub total: Option<u64>,
    pub detail: String,
}

pub type ProgressFn = Box<dyn Fn(InstallProgress) + Send + Sync>;

pub struct InstallOptions {
    /// Download asset objects (required for a full title-screen launch). Default true.
    pub assets: bool,
    /// Extract natives-* jars into natives/<version>. Default true.
    pub natives: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            assets: true,
            natives: true,
        }
    }
}

pub async fn install_version(
    paths: &Paths,
    version: &McVersion,
    opts: InstallOptions,
    on_progress: impl Fn(InstallProgress) + Send + Sync,
) -> Result<VersionInfo> {
    paths.ensure()?;
    let provider = PistonMetaProvider::new();
    on_progress(InstallProgress {
        stage: "meta".into(),
        current: 0,
        total: Some(1),
        detail: format!("获取版本元数据 {version}"),
    });
    let info = provider.resolve(version).await?;

    // Persist version JSON
    let vdir = paths.version_dir(&info.id);
    tokio::fs::create_dir_all(&vdir).await?;
    let json_path = paths.version_json(&info.id);
    tokio::fs::write(&json_path, serde_json::to_vec_pretty(&info)?).await?;

    // Client jar
    let client = &info.downloads.client;
    let jar_path = paths.version_jar(&info.id);
    on_progress(InstallProgress {
        stage: "client".into(),
        current: 0,
        total: Some(client.size),
        detail: format!("下载客户端 {}", info.id),
    });
    download_file(
        provider.client(),
        &client.url,
        &jar_path,
        Some(&client.sha1),
        Some(client.size),
        |c, t| {
            on_progress(InstallProgress {
                stage: "client".into(),
                current: c,
                total: t,
                detail: format!("客户端 {}", info.id),
            });
        },
    )
    .await?;

    // Libraries
    let libs: Vec<&Library> = info
        .libraries
        .iter()
        .filter(|l| rules_allow(l.rules.as_deref(), &[]))
        .collect();
    let lib_total = libs.len() as u64;
    for (i, lib) in libs.iter().enumerate() {
        if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            let rel = artifact
                .path
                .clone()
                .unwrap_or_else(|| maven_path_from_name(&lib.name));
            let dest = paths.library_path(&rel);
            on_progress(InstallProgress {
                stage: "libraries".into(),
                current: i as u64 + 1,
                total: Some(lib_total),
                detail: lib.name.clone(),
            });
            download_file(
                provider.client(),
                &artifact.url,
                &dest,
                Some(&artifact.sha1),
                Some(artifact.size),
                |_, _| {},
            )
            .await?;
        }
        // Legacy classifiers (older versions)
        if let Some(classifiers) = lib.downloads.as_ref().and_then(|d| d.classifiers.as_ref()) {
            if let Some(natives_map) = &lib.natives {
                let key = natives_key(natives_map);
                if let Some(key) = key {
                    if let Some(art) = classifiers.get(&key) {
                        let rel = art.path.clone().unwrap_or_else(|| format!("{key}.jar"));
                        let dest = paths.library_path(&rel);
                        download_file(
                            provider.client(),
                            &art.url,
                            &dest,
                            Some(&art.sha1),
                            Some(art.size),
                            |_, _| {},
                        )
                        .await?;
                    }
                }
            }
        }
    }

    if opts.natives {
        on_progress(InstallProgress {
            stage: "natives".into(),
            current: 0,
            total: None,
            detail: "解压 natives".into(),
        });
        extract_natives(paths, &info)?;
    }

    if opts.assets {
        install_assets(paths, &info, provider.client(), &on_progress).await?;
    }

    on_progress(InstallProgress {
        stage: "done".into(),
        current: 1,
        total: Some(1),
        detail: format!("{} 安装完成", info.id),
    });
    Ok(info)
}

async fn install_assets(
    paths: &Paths,
    info: &VersionInfo,
    client: &reqwest::Client,
    on_progress: &(impl Fn(InstallProgress) + Send + Sync),
) -> Result<()> {
    let idx_ref = &info.asset_index;
    let idx_path = paths.assets_indexes_dir().join(format!("{}.json", idx_ref.id));
    on_progress(InstallProgress {
        stage: "asset-index".into(),
        current: 0,
        total: Some(idx_ref.size),
        detail: format!("资源索引 {}", idx_ref.id),
    });
    download_file(
        client,
        &idx_ref.url,
        &idx_path,
        Some(&idx_ref.sha1),
        Some(idx_ref.size),
        |c, t| {
            on_progress(InstallProgress {
                stage: "asset-index".into(),
                current: c,
                total: t,
                detail: format!("资源索引 {}", idx_ref.id),
            });
        },
    )
    .await?;

    let index: AssetIndex = serde_json::from_str(&std::fs::read_to_string(&idx_path)?)?;
    let objects: Vec<_> = index.objects.values().collect();
    let total = objects.len() as u64;
    // Concurrent downloads with a simple semaphore of 8
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(8));
    let mut joins = Vec::new();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));

    for obj in objects {
        let client = client.clone();
        let hash = obj.hash.clone();
        let size = obj.size;
        let dest = paths
            .assets_objects_dir()
            .join(&hash[..2])
            .join(&hash);
        let sem = sem.clone();
        let done = done.clone();
        let url = format!("https://resources.download.minecraft.net/{}/{}", &hash[..2], hash);
        joins.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.map_err(|e| EasyMcError::msg(e.to_string()))?;
            download_file(&client, &url, &dest, Some(&hash), Some(size), |_, _| {}).await?;
            done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok::<(), EasyMcError>(())
        }));
    }

    for (i, j) in joins.into_iter().enumerate() {
        j.await.map_err(|e| EasyMcError::msg(e.to_string()))??;
        if i % 25 == 0 {
            on_progress(InstallProgress {
                stage: "assets".into(),
                current: done.load(std::sync::atomic::Ordering::Relaxed),
                total: Some(total),
                detail: "下载游戏资源".into(),
            });
        }
    }
    on_progress(InstallProgress {
        stage: "assets".into(),
        current: total,
        total: Some(total),
        detail: "资源下载完成".into(),
    });
    Ok(())
}

pub fn extract_natives(paths: &Paths, info: &VersionInfo) -> Result<()> {
    let out = paths.natives_for_version(&info.id);
    std::fs::create_dir_all(&out)?;
    for lib in &info.libraries {
        if !rules_allow(lib.rules.as_deref(), &[]) {
            continue;
        }
        let mut jars: Vec<PathBuf> = Vec::new();
        if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            let rel = artifact
                .path
                .clone()
                .unwrap_or_else(|| maven_path_from_name(&lib.name));
            if is_native_library(&lib.name, Some(&rel)) {
                jars.push(paths.library_path(&rel));
            }
        }
        if let (Some(classifiers), Some(natives_map)) = (
            lib.downloads.as_ref().and_then(|d| d.classifiers.as_ref()),
            lib.natives.as_ref(),
        ) {
            if let Some(key) = natives_key(natives_map) {
                if let Some(art) = classifiers.get(&key) {
                    let rel = art.path.clone().unwrap_or_default();
                    jars.push(paths.library_path(rel));
                }
            }
        }
        for jar in jars {
            if jar.exists() {
                extract_zip_natives(&jar, &out)?;
            }
        }
    }
    Ok(())
}

fn extract_zip_natives(jar: &Path, out: &Path) -> Result<()> {
    let file = File::open(jar)?;
    let mut archive = ZipArchive::new(file).map_err(|e| EasyMcError::msg(e.to_string()))?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| EasyMcError::msg(e.to_string()))?;
        let name = file.name().to_string();
        if name.ends_with('/') || name.contains("META-INF") {
            continue;
        }
        // Only extract native binaries
        let lower = name.to_ascii_lowercase();
        if !(lower.ends_with(".so")
            || lower.ends_with(".dll")
            || lower.ends_with(".dylib")
            || lower.ends_with(".jnilib"))
        {
            continue;
        }
        let filename = Path::new(&name)
            .file_name()
            .ok_or_else(|| EasyMcError::msg("bad zip entry"))?;
        let dest = out.join(filename);
        let mut outfile = File::create(&dest)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        outfile.write_all(&buf)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = outfile.metadata()?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dest, perms)?;
        }
    }
    Ok(())
}

fn natives_key(map: &std::collections::HashMap<String, String>) -> Option<String> {
    let os = crate::rules::current_os_name();
    map.get(os).cloned().map(|k| {
        // ${arch} substitution rarely used now
        k.replace("${arch}", if cfg!(target_arch = "x86") { "32" } else { "64" })
    })
}

pub fn maven_path_from_name(name: &str) -> String {
    // group:artifact:version[:classifier]
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return format!("{name}.jar");
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let classifier = parts.get(3);
    match classifier {
        Some(c) => format!("{group}/{artifact}/{version}/{artifact}-{version}-{c}.jar"),
        None => format!("{group}/{artifact}/{version}/{artifact}-{version}.jar"),
    }
}

pub fn load_installed_version(paths: &Paths, id: &str) -> Result<VersionInfo> {
    let p = paths.version_json(id);
    let raw = std::fs::read_to_string(&p)
        .map_err(|_| EasyMcError::Missing(format!("version json {}", p.display())))?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn is_version_installed(paths: &Paths, id: &str) -> bool {
    paths.version_jar(id).exists() && paths.version_json(id).exists()
}

/// Classpath entries (non-native library jars + client jar), colon/semicolon joined by caller.
pub fn classpath_jars(paths: &Paths, info: &VersionInfo) -> Result<Vec<PathBuf>> {
    let mut jars = Vec::new();
    for lib in &info.libraries {
        if !rules_allow(lib.rules.as_deref(), &[]) {
            continue;
        }
        if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            let rel = artifact
                .path
                .clone()
                .unwrap_or_else(|| maven_path_from_name(&lib.name));
            if is_native_library(&lib.name, Some(&rel)) {
                continue;
            }
            jars.push(paths.library_path(rel));
        }
    }
    jars.push(paths.version_jar(&info.id));
    Ok(jars)
}
