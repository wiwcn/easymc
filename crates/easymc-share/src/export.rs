//! Export instance → metadata JSON or pack zip.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::Local;
use sha1::{Digest, Sha1};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use easymc_core::{load_settings, EasyMcError, Instance, Paths, Result};
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

        let from_index = index.mods.iter().find(|m| m.filename == filename || m.filename == filename.trim_end_matches(".disabled"));

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
            if let Ok(Some(hit)) = easymc_mods::lookup_modrinth_by_sha1(entry.sha1.as_deref().unwrap()).await
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
    let manifest =
        build_manifest(paths, instance, opts.mode, opts.notes, opts.resolve_modrinth).await?;
    fs::create_dir_all(shares_dir(paths))?;

    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let slug = slugify(&instance.name);

    match opts.mode {
        ShareMode::Metadata => {
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
            })
        }
        ShareMode::Pack => {
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
                    "已导出整合包（含 {} 个模组 jar，{} 字节）→ {}",
                    manifest.mods.len(),
                    bytes,
                    path.display()
                ),
            })
        }
    }
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
