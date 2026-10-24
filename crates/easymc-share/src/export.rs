//! Export instance → metadata JSON, EasyMC pack zip, or Modrinth `.mrpack`.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::Local;
use sha1::{Digest, Sha1};
use sha2::Sha512;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use easymc_core::{load_settings, EasyMcError, Instance, ModLoader, Paths, Result};
use easymc_mods::list_installed_mods;

use crate::format::{
    InstanceSpec, JvmSpec, ModEntry, ShareManifest, ShareMode, FORMAT_ID, FORMAT_VERSION,
    MANIFEST_NAME,
};
use crate::index::load_index;

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub mode: ShareMode,
    pub notes: Option<String>,
    /// Output file path; if None, write under `paths.root/shares/`.
    pub output: Option<PathBuf>,
    /// Best-effort Modrinth hash lookup when index lacks ids (needs network).
    pub resolve_modrinth: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            mode: ShareMode::Metadata,
            notes: None,
            output: None,
            resolve_modrinth: true,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExportResult {
    pub path: String,
    pub mode: ShareMode,
    pub mod_count: usize,
    pub bytes: u64,
    pub message_zh: String,
    /// Mods referenced via Modrinth CDN downloads (mrpack only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved: Option<usize>,
    /// Mods / files packed under `overrides/` (mrpack only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overrides: Option<usize>,
}

fn file_sha1(path: &Path) -> Option<String> {
    let mut f = File::open(path).ok()?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

fn file_hashes(path: &Path) -> Option<(String, String, u64)> {
    let mut f = File::open(path).ok()?;
    let mut sha1 = Sha1::new();
    let mut sha512 = Sha512::new();
    let mut buf = [0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        sha1.update(&buf[..n]);
        sha512.update(&buf[..n]);
        size += n as u64;
    }
    Some((
        format!("{:x}", sha1.finalize()),
        format!("{:x}", sha512.finalize()),
        size,
    ))
}

fn slugify(s: &str) -> String {
    let s: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "instance".into()
    } else {
        s.chars().take(40).collect()
    }
}

pub fn shares_dir(paths: &Paths) -> PathBuf {
    paths.root.join("shares")
}

fn loader_dep_key(loader: ModLoader) -> Option<&'static str> {
    match loader {
        ModLoader::Fabric => Some("fabric-loader"),
        ModLoader::Quilt => Some("quilt-loader"),
        ModLoader::Forge => Some("forge"),
        ModLoader::NeoForge => Some("neoforge"),
        ModLoader::Vanilla => None,
    }
}

pub async fn build_manifest(
    paths: &Paths,
    instance: &Instance,
    mode: ShareMode,
    notes: Option<String>,
    resolve_modrinth: bool,
) -> Result<ShareManifest> {
    let settings = load_settings(paths).unwrap_or_default();
    let index = load_index(instance);
    let names = list_installed_mods(instance)?;
    let mods_path = PathBuf::from(&instance.game_dir).join("mods");

    let mut mods = Vec::new();
    for name in names {
        let disabled = name.ends_with(".jar.disabled");
        let filename = name.clone();
        let jar_path = mods_path.join(&filename);
        let sha1 = file_sha1(&jar_path);

        let from_index = index.mods.iter().find(|m| {
            m.filename == filename || m.filename == filename.trim_end_matches(".disabled")
        });

        let mut entry = ModEntry {
            filename: filename.clone(),
            disabled,
            sha1: sha1.clone().or_else(|| from_index.and_then(|m| m.sha1.clone())),
            source: from_index.and_then(|m| m.source.clone()),
            project_id: from_index.and_then(|m| m.project_id.clone()),
            version_id: from_index.and_then(|m| m.version_id.clone()),
        };

        if resolve_modrinth
            && entry.project_id.is_none()
            && entry.sha1.as_ref().map(|s| s.len() == 40).unwrap_or(false)
        {
            if let Ok(Some(hit)) =
                easymc_mods::lookup_modrinth_by_sha1(entry.sha1.as_deref().unwrap()).await
            {
                entry.source = Some("modrinth".into());
                entry.project_id = Some(hit.project_id);
                entry.version_id = Some(hit.version_id);
            }
        }
        mods.push(entry);
    }

    Ok(ShareManifest {
        format: FORMAT_ID.into(),
        format_version: FORMAT_VERSION,
        mode,
        exported_at: Local::now().to_rfc3339(),
        notes,
        instance: InstanceSpec {
            name: instance.name.clone(),
            version: instance.version.as_str().to_string(),
            loader: instance.loader.as_str().to_string(),
            loader_version: instance.loader_version.clone(),
            description: instance.description.clone(),
        },
        jvm: JvmSpec {
            memory_mb: settings.memory_mb,
        },
        mods,
    })
}

pub async fn export_instance(
    paths: &Paths,
    instance: &Instance,
    opts: ExportOptions,
) -> Result<ExportResult> {
    fs::create_dir_all(shares_dir(paths))?;

    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let slug = slugify(&instance.name);

    match opts.mode {
        ShareMode::Metadata => {
            let manifest = build_manifest(
                paths,
                instance,
                opts.mode,
                opts.notes,
                opts.resolve_modrinth,
            )
            .await?;
            let path = opts.output.unwrap_or_else(|| {
                shares_dir(paths).join(format!("easymc-share-{slug}-{stamp}.json"))
            });
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let raw = serde_json::to_string_pretty(&manifest)?;
            fs::write(&path, &raw)?;
            let bytes = raw.len() as u64;
            Ok(ExportResult {
                path: path.display().to_string(),
                mode: ShareMode::Metadata,
                mod_count: manifest.mods.len(),
                bytes,
                message_zh: format!(
                    "已导出元数据分享包（{} 个模组）→ {}",
                    manifest.mods.len(),
                    path.display()
                ),
                resolved: None,
                overrides: None,
            })
        }
        ShareMode::Pack => {
            let manifest = build_manifest(
                paths,
                instance,
                opts.mode,
                opts.notes,
                opts.resolve_modrinth,
            )
            .await?;
            let path = opts.output.unwrap_or_else(|| {
                shares_dir(paths).join(format!("easymc-share-{slug}-{stamp}.zip"))
            });
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            write_pack_zip(&path, &manifest, instance)?;
            let bytes = fs::metadata(&path)?.len();
            Ok(ExportResult {
                path: path.display().to_string(),
                mode: ShareMode::Pack,
                mod_count: manifest.mods.len(),
                bytes,
                message_zh: format!(
                    "已导出 EasyMC 分享包（含 {} 个模组 jar，{} 字节）→ {}",
                    manifest.mods.len(),
                    bytes,
                    path.display()
                ),
                resolved: None,
                overrides: None,
            })
        }
        ShareMode::Mrpack => {
            let path = opts.output.unwrap_or_else(|| {
                shares_dir(paths).join(format!("{slug}-{stamp}.mrpack"))
            });
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let built = write_mrpack(
                &path,
                instance,
                opts.notes.as_deref(),
                opts.resolve_modrinth,
            )
            .await?;
            let bytes = fs::metadata(&path)?.len();
            Ok(ExportResult {
                path: path.display().to_string(),
                mode: ShareMode::Mrpack,
                mod_count: built.mod_count,
                bytes,
                message_zh: format!(
                    "已导出 Modrinth 整合包 .mrpack（{} 个模组：{} 个走 CDN，{} 个放 overrides，{} 字节）→ {}",
                    built.mod_count,
                    built.resolved,
                    built.overrides,
                    bytes,
                    path.display()
                ),
                resolved: Some(built.resolved),
                overrides: Some(built.overrides),
            })
        }
    }
}

struct MrpackBuilt {
    mod_count: usize,
    resolved: usize,
    overrides: usize,
}

async fn write_mrpack(
    path: &Path,
    instance: &Instance,
    notes: Option<&str>,
    resolve_modrinth: bool,
) -> Result<MrpackBuilt> {
    let names = list_installed_mods(instance)?;
    let mods_path = PathBuf::from(&instance.game_dir).join("mods");
    let game_dir = PathBuf::from(&instance.game_dir);

    let mut files: Vec<serde_json::Value> = Vec::new();
    let mut override_jars: Vec<(String, PathBuf)> = Vec::new();
    let mut resolved = 0usize;
    let mut mod_count = 0usize;

    for name in names {
        if name.ends_with(".jar.disabled") {
            continue;
        }
        let jar_path = mods_path.join(&name);
        if !jar_path.is_file() {
            continue;
        }
        mod_count += 1;
        let Some((sha1, sha512, size)) = file_hashes(&jar_path) else {
            override_jars.push((name, jar_path));
            continue;
        };

        let mut download_url: Option<String> = None;
        let mut api_sha1 = sha1.clone();
        let mut api_sha512 = sha512.clone();
        let mut api_size = size;

        if resolve_modrinth {
            if let Ok(Some(hit)) = easymc_mods::lookup_modrinth_by_sha1(&sha1).await {
                if let Some(url) = hit.download_url.filter(|u| !u.is_empty()) {
                    download_url = Some(url);
                    if let Some(h) = hit.sha1 {
                        api_sha1 = h;
                    }
                    if let Some(h) = hit.sha512 {
                        api_sha512 = h;
                    }
                    if let Some(s) = hit.file_size {
                        api_size = s;
                    }
                }
            }
        }

        if let Some(url) = download_url {
            let mut hashes = HashMap::new();
            hashes.insert("sha1".to_string(), api_sha1);
            hashes.insert("sha512".to_string(), api_sha512);
            let mut env = HashMap::new();
            env.insert("client".to_string(), "required".to_string());
            env.insert("server".to_string(), "optional".to_string());
            files.push(serde_json::json!({
                "path": format!("mods/{name}"),
                "hashes": hashes,
                "env": env,
                "downloads": [url],
                "fileSize": api_size,
            }));
            resolved += 1;
        } else {
            override_jars.push((name, jar_path));
        }
    }

    let version_id = Local::now().format("%Y%m%d.%H%M%S").to_string();
    let summary = notes
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| instance.description.clone());

    let mut dependencies = HashMap::new();
    dependencies.insert(
        "minecraft".to_string(),
        instance.version.as_str().to_string(),
    );
    if let Some(key) = loader_dep_key(instance.loader) {
        if let Some(lv) = instance
            .loader_version
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            dependencies.insert(key.to_string(), lv.to_string());
        }
    }

    let mut index = serde_json::json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": version_id,
        "name": instance.name,
        "files": files,
        "dependencies": dependencies,
    });
    if let Some(s) = summary {
        index
            .as_object_mut()
            .unwrap()
            .insert("summary".into(), serde_json::Value::String(s));
    }

    let file = File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let zopts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let json = serde_json::to_vec_pretty(&index)?;
    zip.start_file("modrinth.index.json", zopts)
        .map_err(|e| EasyMcError::msg(format!("zip: {e}")))?;
    zip.write_all(&json)?;

    let mut overrides = 0usize;
    for (name, src) in &override_jars {
        let mut f = File::open(src)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        let entry = format!("overrides/mods/{name}");
        zip.start_file(&entry, zopts)
            .map_err(|e| EasyMcError::msg(format!("zip: {e}")))?;
        zip.write_all(&buf)?;
        overrides += 1;
    }

    // Bundle local config/ so packs keep settings (optional, best-effort).
    let config_dir = game_dir.join("config");
    if config_dir.is_dir() {
        overrides += add_dir_to_zip(&mut zip, &config_dir, "overrides/config", zopts)?;
    }

    zip.finish()
        .map_err(|e| EasyMcError::msg(format!("zip finish: {e}")))?;

    Ok(MrpackBuilt {
        mod_count,
        resolved,
        overrides,
    })
}

fn add_dir_to_zip(
    zip: &mut ZipWriter<File>,
    dir: &Path,
    zip_prefix: &str,
    opts: SimpleFileOptions,
) -> Result<usize> {
    let mut n = 0usize;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(cur) = stack.pop() {
        let rd = match fs::read_dir(&cur) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for ent in rd.flatten() {
            let p = ent.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if !p.is_file() {
                continue;
            }
            let rel = match p.strip_prefix(dir) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let mut name = String::from(zip_prefix);
            for c in rel.components() {
                if let std::path::Component::Normal(s) = c {
                    name.push('/');
                    name.push_str(&s.to_string_lossy());
                }
            }
            let mut f = File::open(&p)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            zip.start_file(&name, opts)
                .map_err(|e| EasyMcError::msg(format!("zip: {e}")))?;
            zip.write_all(&buf)?;
            n += 1;
        }
    }
    Ok(n)
}

fn write_pack_zip(path: &Path, manifest: &ShareManifest, instance: &Instance) -> Result<()> {
    let file = File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let json = serde_json::to_vec_pretty(manifest)?;
    zip.start_file(MANIFEST_NAME, opts)
        .map_err(|e| EasyMcError::msg(format!("zip: {e}")))?;
    zip.write_all(&json)?;

    let mods_path = PathBuf::from(&instance.game_dir).join("mods");
    for m in &manifest.mods {
        let src = mods_path.join(&m.filename);
        if !src.is_file() {
            continue;
        }
        let mut f = File::open(&src)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        let name = format!("mods/{}", m.filename);
        zip.start_file(name, opts)
            .map_err(|e| EasyMcError::msg(format!("zip: {e}")))?;
        zip.write_all(&buf)?;
    }
    zip.finish()
        .map_err(|e| EasyMcError::msg(format!("zip finish: {e}")))?;
    Ok(())
}

/// Export path as a short clipboard-friendly string.
pub fn path_for_clipboard(path: &str) -> String {
    path.to_string()
}

pub fn ensure_instance<'a>(
    store: &'a easymc_core::InstanceStore,
    id: &str,
) -> Result<&'a Instance> {
    store
        .instances
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| EasyMcError::Missing(format!("instance {id}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_keys() {
        assert_eq!(loader_dep_key(ModLoader::Fabric), Some("fabric-loader"));
        assert_eq!(loader_dep_key(ModLoader::Vanilla), None);
    }
}
