//! Import share JSON / zip → create instance + restore mods.

use std::fs::{self, File};
use std::io::{copy, Read};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use easymc_core::{
    create_named_instance_with, load_settings, save_settings, CreateInstanceOpts, EasyMcError,
    Instance, McVersion, ModLoader, Paths, Result,
};
use easymc_mods::{install_mod, mods_dir, ModSource};

use crate::format::{ShareManifest, ShareMode, MANIFEST_NAME};
use crate::index::{save_index, ModsIndex};

#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// Override display name.
    pub name: Option<String>,
    /// Apply bundle JVM memory into settings.json.
    pub apply_memory: bool,
    /// Skip remote Modrinth/CF downloads (only copy jars from pack).
    pub offline: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportResult {
    pub instance_id: String,
    pub instance_name: String,
    pub mods_copied: usize,
    pub mods_downloaded: usize,
    pub mods_missing: Vec<String>,
    pub memory_applied: Option<u32>,
    pub message_zh: String,
    pub path_loaded: String,
}

pub fn read_manifest_from_path(path: &Path) -> Result<(ShareManifest, Option<PathBuf>)> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext == "zip" {
        let file = File::open(path)?;
        let mut archive = ZipArchive::new(file).map_err(|e| EasyMcError::msg(format!("zip: {e}")))?;
        let mut manifest_raw = String::new();
        let mut found = false;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| EasyMcError::msg(format!("zip: {e}")))?;
            let n = entry.name().replace('\\', "/");
            let base = n.rsplit('/').next().unwrap_or(&n);
            if base == MANIFEST_NAME {
                entry.read_to_string(&mut manifest_raw)?;
                found = true;
                break;
            }
        }
        if !found {
            return Err(EasyMcError::msg(format!("zip 中缺少 {MANIFEST_NAME}")));
        }
        let manifest: ShareManifest = serde_json::from_str(&manifest_raw)?;
        manifest.validate().map_err(EasyMcError::msg)?;
        Ok((manifest, Some(path.to_path_buf())))
    } else {
        let raw = fs::read_to_string(path)?;
        let manifest: ShareManifest = serde_json::from_str(&raw)?;
        manifest.validate().map_err(EasyMcError::msg)?;
        Ok((manifest, None))
    }
}

fn parse_loader(s: &str) -> ModLoader {
    ModLoader::parse_loose(s).unwrap_or(ModLoader::Vanilla)
}


fn looks_like_mrpack(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext == "mrpack" {
        return true;
    }
    if ext == "zip" {
        if let Ok(file) = File::open(path) {
            if let Ok(z) = ZipArchive::new(file) {
                let names: Vec<String> = z
                    .file_names()
                    .map(|n| n.replace('\\', "/"))
                    .collect();
                let has_mr = names
                    .iter()
                    .any(|n| n == "modrinth.index.json" || n.ends_with("/modrinth.index.json"));
                let has_emc = names.iter().any(|n| {
                    n == MANIFEST_NAME || n.ends_with(&format!("/{MANIFEST_NAME}"))
                });
                return has_mr && !has_emc;
            }
        }
    }
    false
}

pub async fn import_instance(
    paths: &Paths,
    bundle_path: &Path,
    opts: ImportOptions,
) -> Result<ImportResult> {
    if looks_like_mrpack(bundle_path) {
        let r = easymc_mods::install_mrpack_file(
            paths,
            bundle_path,
            opts.name.as_deref(),
            |_| {},
        )
        .await?;
        return Ok(ImportResult {
            instance_id: r.instance_id,
            instance_name: r.instance_name,
            mods_copied: r.overrides,
            mods_downloaded: r.files_downloaded,
            mods_missing: r.files_failed,
            memory_applied: None,
            message_zh: r.message_zh,
            path_loaded: bundle_path.display().to_string(),
        });
    }

    let (manifest, zip_path) = read_manifest_from_path(bundle_path)?;
    let name = opts
        .name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| manifest.instance.name.clone());

    let loader = parse_loader(&manifest.instance.loader);
    let inst = create_named_instance_with(
        paths,
        CreateInstanceOpts {
            name: name.clone(),
            version: McVersion(manifest.instance.version.clone()),
            loader,
            loader_version: manifest.instance.loader_version.clone(),
            version_ready: false,
        },
    )?;

    // Refresh description from bundle if present
    if let Some(desc) = &manifest.instance.description {
        let mut store = easymc_core::load_instances(paths)?;
        if let Some(slot) = store.instances.iter_mut().find(|i| i.id == inst.id) {
            slot.description = Some(format!("导入 · {desc}"));
            let _ = easymc_core::save_instances(paths, &store);
        }
    }

    let mut mods_copied = 0usize;
    let mut mods_downloaded = 0usize;
    let mut mods_missing = Vec::new();
    let mut index = ModsIndex::default();

    // Extract / copy jars from pack first
    if let Some(ref zp) = zip_path {
        mods_copied += extract_mods_from_zip(zp, &inst)?;
    }

    for entry in &manifest.mods {
        let dest = mods_dir(&inst).join(&entry.filename);
        if dest.is_file() {
            index.mods.push(entry.clone());
            continue;
        }

        // Try remote resolve
        let can_download = !opts.offline
            && entry.project_id.as_ref().map(|s| !s.is_empty()).unwrap_or(false)
            && entry.source.as_ref().map(|s| !s.is_empty()).unwrap_or(false);

        if can_download {
            let source = match entry.source.as_deref().unwrap_or("") {
                "modrinth" | "mr" => ModSource::Modrinth,
                "curseforge" | "cf" => ModSource::CurseForge,
                other => {
                    mods_missing.push(format!("{} (未知源 {other})", entry.filename));
                    continue;
                }
            };
            match install_mod(
                &inst,
                source,
                entry.project_id.as_deref().unwrap(),
                entry.version_id.as_deref(),
                |_| {},
            )
            .await
            {
                Ok(res) => {
                    mods_downloaded += 1;
                    let mut e = entry.clone();
                    e.filename = res.filename;
                    e.project_id = Some(res.project_id);
                    e.version_id = Some(res.version_id);
                    e.source = Some(source.as_str().into());
                    index.mods.push(e);
                }
                Err(err) => {
                    mods_missing.push(format!("{} ({err})", entry.filename));
                }
            }
        } else if matches!(manifest.mode, ShareMode::Metadata) {
            mods_missing.push(entry.filename.clone());
        } else {
            mods_missing.push(format!("{} (包内无文件且无商店 id)", entry.filename));
        }
    }

    let _ = save_index(&inst, &index);

    let mut memory_applied = None;
    if opts.apply_memory {
        let mut s = load_settings(paths)?;
        s.memory_mb = manifest.jvm.memory_mb.clamp(1024, 32768);
        save_settings(paths, &s)?;
        memory_applied = Some(s.memory_mb);
    }

    let message_zh = format!(
        "已导入实例「{}」({})\n模组：复制 {mods_copied} · 下载 {mods_downloaded} · 缺失 {}\n内存建议 {} MB{}",
        inst.name,
        inst.id,
        mods_missing.len(),
        manifest.jvm.memory_mb,
        memory_applied
            .map(|m| format!("（已写入 settings={m}）"))
            .unwrap_or_default()
    );

    Ok(ImportResult {
        instance_id: inst.id,
        instance_name: inst.name,
        mods_copied,
        mods_downloaded,
        mods_missing,
        memory_applied,
        message_zh,
        path_loaded: bundle_path.display().to_string(),
    })
}

fn extract_mods_from_zip(zip_path: &Path, instance: &Instance) -> Result<usize> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| EasyMcError::msg(format!("zip: {e}")))?;
    let dest_dir = mods_dir(instance);
    fs::create_dir_all(&dest_dir)?;
    let mut n = 0usize;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| EasyMcError::msg(format!("zip entry: {e}")))?;
        let name = entry.name().replace('\\', "/");
        let fname = name.strip_prefix("mods/").or_else(|| name.strip_prefix("./mods/"));
        let Some(fname) = fname else { continue };
        if fname.is_empty() || fname.contains("..") || fname.contains('/') {
            continue;
        }
        if !(fname.ends_with(".jar") || fname.ends_with(".jar.disabled")) {
            continue;
        }
        let out = dest_dir.join(fname);
        let mut out_f = File::create(&out)?;
        copy(&mut entry, &mut out_f)?;
        n += 1;
    }
    Ok(n)
}
