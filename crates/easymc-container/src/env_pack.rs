//! Container environment export / import (`.easymc-env`) — Slice E.
//!
//! Portable profile for a container-backed version: instance identity, image/runtime
//! prefs, and optional LAYER_DIRS payload (never saves by default).
//! Native instances stay `container: None`; import creates/attaches container meta only.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::Local;
use easymc_core::{
    create_named_instance_with, load_instances, load_settings, CreateInstanceOpts, Instance,
    McVersion, ModLoader, Paths, Result as CoreResult,
};
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::layer::LAYER_DIRS;
use crate::registries::{DEFAULT_BASE_IMAGE, DEFAULT_BASE_IMAGE_MIRROR};
use crate::versions::attach_container_to_instance;

pub const ENV_FORMAT_ID: &str = "easymc.env";
pub const ENV_FORMAT_VERSION: u32 = 1;
pub const ENV_MANIFEST_NAME: &str = "easymc.env.json";
pub const ENV_EXT: &str = "easymc-env";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvPackMode {
    /// JSON only — recreate container meta + instance shell; mods listed by name.
    Metadata,
    /// Zip with manifest + layer/{mods,config,…} (no saves/).
    Pack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvInstanceSpec {
    pub name: String,
    pub version: String,
    #[serde(default = "default_loader")]
    pub loader: String,
    #[serde(default)]
    pub loader_version: Option<String>,
}

fn default_loader() -> String {
    "vanilla".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvContainerSpec {
    #[serde(default = "default_runtime")]
    pub runtime: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default = "default_true")]
    pub prefer_china_mirrors: bool,
}

fn default_runtime() -> String {
    "podman".into()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvLayerSpec {
    #[serde(default)]
    pub dirs: Vec<String>,
    #[serde(default)]
    pub include_saves: bool,
    #[serde(default)]
    pub mod_filenames: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvSettingsHint {
    #[serde(default = "default_true")]
    pub container_china_mirrors: bool,
    #[serde(default = "default_true")]
    pub container_saves_require_propose: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvManifest {
    pub format: String,
    pub format_version: u32,
    pub mode: EnvPackMode,
    pub exported_at: String,
    #[serde(default)]
    pub notes: Option<String>,
    pub instance: EnvInstanceSpec,
    pub container: EnvContainerSpec,
    #[serde(default)]
    pub layer: EnvLayerSpec,
    #[serde(default)]
    pub settings_hint: Option<EnvSettingsHint>,
}

impl Default for EnvLayerSpec {
    fn default() -> Self {
        Self {
            dirs: LAYER_DIRS.iter().map(|s| (*s).to_string()).collect(),
            include_saves: false,
            mod_filenames: Vec::new(),
        }
    }
}

impl EnvManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.format != ENV_FORMAT_ID {
            return Err(format!("未知环境包格式: {}", self.format));
        }
        if self.format_version == 0 || self.format_version > ENV_FORMAT_VERSION {
            return Err(format!("不支持的 format_version: {}", self.format_version));
        }
        if self.instance.name.trim().is_empty() {
            return Err("instance.name 为空".into());
        }
        if self.instance.version.trim().is_empty() {
            return Err("instance.version 为空".into());
        }
        if self.layer.include_saves {
            return Err("环境包不允许打包存档（include_saves 必须为 false）".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct EnvExportOptions {
    pub mode: EnvPackMode,
    pub notes: Option<String>,
    pub output: Option<PathBuf>,
}

impl Default for EnvExportOptions {
    fn default() -> Self {
        Self {
            mode: EnvPackMode::Metadata,
            notes: None,
            output: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvExportResult {
    pub path: String,
    pub mode: EnvPackMode,
    pub bytes: u64,
    pub message_zh: String,
}

#[derive(Debug, Clone)]
pub struct EnvImportOptions {
    /// Create a new instance (default). If Some(id), attach/import layer onto that id.
    pub target_instance_id: Option<String>,
    pub prefer_china_mirrors: Option<bool>,
}

impl Default for EnvImportOptions {
    fn default() -> Self {
        Self {
            target_instance_id: None,
            prefer_china_mirrors: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvImportResult {
    pub instance: Instance,
    pub mode: EnvPackMode,
    pub layer_files: usize,
    pub message_zh: String,
}

pub fn env_packs_dir(paths: &Paths) -> PathBuf {
    paths.root.join("env-packs")
}

fn slugify(s: &str) -> String {
    let s: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else if c.is_whitespace() || c == '-' || c == '_' {
                '-'
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "easymc-env".into()
    } else {
        s
    }
}

fn list_mod_filenames(game_dir: &Path) -> Vec<String> {
    let mods = game_dir.join("mods");
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(&mods) else {
        return out;
    };
    for ent in rd.flatten() {
        let name = ent.file_name().to_string_lossy().into_owned();
        if name.ends_with(".jar") || name.ends_with(".jar.disabled") {
            out.push(name);
        }
    }
    out.sort();
    out
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for ent in fs::read_dir(src).map_err(|e| e.to_string())? {
        let ent = ent.map_err(|e| e.to_string())?;
        let ty = ent.file_type().map_err(|e| e.to_string())?;
        let to = dst.join(ent.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&ent.path(), &to)?;
        } else if ty.is_file() {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(ent.path(), &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn add_dir_to_zip(zip: &mut ZipWriter<File>, src: &Path, prefix: &str) -> Result<usize, String> {
    if !src.exists() {
        return Ok(0);
    }
    let mut count = 0usize;
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    fn walk(
        zip: &mut ZipWriter<File>,
        dir: &Path,
        prefix: &str,
        opts: SimpleFileOptions,
        count: &mut usize,
    ) -> Result<(), String> {
        for ent in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let ent = ent.map_err(|e| e.to_string())?;
            let name = ent.file_name();
            let name_s = name.to_string_lossy();
            let rel = if prefix.is_empty() {
                name_s.to_string()
            } else {
                format!("{prefix}/{name_s}")
            };
            let path = ent.path();
            if path.is_dir() {
                walk(zip, &path, &rel, opts, count)?;
            } else if path.is_file() {
                zip.start_file(&rel, opts).map_err(|e| e.to_string())?;
                let mut f = File::open(&path).map_err(|e| e.to_string())?;
                let mut buf = Vec::new();
                f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
                zip.write_all(&buf).map_err(|e| e.to_string())?;
                *count += 1;
            }
        }
        Ok(())
    }
    walk(zip, src, prefix, opts, &mut count)?;
    Ok(count)
}

/// Build manifest for a container-backed instance (errors if not container).
pub fn build_env_manifest(
    paths: &Paths,
    instance_id: &str,
    mode: EnvPackMode,
    notes: Option<String>,
) -> CoreResult<EnvManifest> {
    let store = load_instances(paths)?;
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| easymc_core::EasyMcError::Missing(format!("instance {instance_id}")))?;
    let c = inst.container.as_ref().ok_or_else(|| {
        easymc_core::EasyMcError::msg("仅容器版本可导出环境包；原生路径请用「发给好友」整合包")
    })?;
    let settings = load_settings(paths).unwrap_or_default();
    let game = Path::new(&inst.game_dir);
    let prefer = settings.container_china_mirrors;
    let image = c.image.clone().or_else(|| {
        Some(if prefer {
            DEFAULT_BASE_IMAGE_MIRROR.to_string()
        } else {
            DEFAULT_BASE_IMAGE.to_string()
        })
    });
    Ok(EnvManifest {
        format: ENV_FORMAT_ID.into(),
        format_version: ENV_FORMAT_VERSION,
        mode,
        exported_at: Local::now().to_rfc3339(),
        notes,
        instance: EnvInstanceSpec {
            name: inst.name.clone(),
            version: inst.version.as_str().to_string(),
            loader: inst.loader.as_str().to_string(),
            loader_version: inst.loader_version.clone(),
        },
        container: EnvContainerSpec {
            runtime: c.runtime.clone(),
            image,
            prefer_china_mirrors: prefer,
        },
        layer: EnvLayerSpec {
            dirs: LAYER_DIRS.iter().map(|s| (*s).to_string()).collect(),
            include_saves: false,
            mod_filenames: list_mod_filenames(game),
        },
        settings_hint: Some(EnvSettingsHint {
            container_china_mirrors: settings.container_china_mirrors,
            container_saves_require_propose: settings.container_saves_require_propose,
        }),
    })
}

/// Export container environment to `.easymc-env` (JSON) or `.easymc-env.zip` (pack).
pub fn export_env_pack(
    paths: &Paths,
    instance_id: &str,
    opts: EnvExportOptions,
) -> Result<EnvExportResult, String> {
    let manifest = build_env_manifest(paths, instance_id, opts.mode, opts.notes)
        .map_err(|e| e.to_string())?;
    manifest.validate()?;

    let store = load_instances(paths).map_err(|e| e.to_string())?;
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| format!("找不到版本 {instance_id}"))?;
    let game = PathBuf::from(&inst.game_dir);

    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let slug = slugify(&inst.name);
    fs::create_dir_all(env_packs_dir(paths)).map_err(|e| e.to_string())?;

    match opts.mode {
        EnvPackMode::Metadata => {
            let out = opts.output.unwrap_or_else(|| {
                env_packs_dir(paths).join(format!("{slug}-{stamp}.{ENV_EXT}"))
            });
            let raw = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
            fs::write(&out, &raw).map_err(|e| e.to_string())?;
            let bytes = raw.len() as u64;
            Ok(EnvExportResult {
                path: out.to_string_lossy().into_owned(),
                mode: EnvPackMode::Metadata,
                bytes,
                message_zh: format!(
                    "已导出容器环境（元数据）→ {}（{} 字节）。好友导入后会重建容器版本，不含存档。",
                    out.display(),
                    bytes
                ),
            })
        }
        EnvPackMode::Pack => {
            let out = opts.output.unwrap_or_else(|| {
                env_packs_dir(paths).join(format!("{slug}-{stamp}.{ENV_EXT}.zip"))
            });
            let file = File::create(&out).map_err(|e| e.to_string())?;
            let mut zip = ZipWriter::new(file);
            let opts_f =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            let raw = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
            zip.start_file(ENV_MANIFEST_NAME, opts_f)
                .map_err(|e| e.to_string())?;
            zip.write_all(raw.as_bytes()).map_err(|e| e.to_string())?;

            let mut file_count = 0usize;
            for dir in LAYER_DIRS {
                let src = game.join(dir);
                file_count += add_dir_to_zip(&mut zip, &src, &format!("layer/{dir}"))?;
            }
            zip.finish().map_err(|e| e.to_string())?;
            let bytes = fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
            Ok(EnvExportResult {
                path: out.to_string_lossy().into_owned(),
                mode: EnvPackMode::Pack,
                bytes,
                message_zh: format!(
                    "已导出容器环境包（含模组/配置等 {} 个文件，不含存档）→ {}（{} 字节）",
                    file_count,
                    out.display(),
                    bytes
                ),
            })
        }
    }
}

fn read_manifest_json(raw: &str) -> Result<EnvManifest, String> {
    let m: EnvManifest = serde_json::from_str(raw).map_err(|e| format!("环境包解析失败：{e}"))?;
    m.validate()?;
    Ok(m)
}

/// Peek manifest from `.easymc-env` / `.easymc-env.zip` / `easymc.env.json`.
pub fn read_env_manifest_from_path(path: &Path) -> Result<EnvManifest, String> {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".zip") || name.ends_with(&format!(".{ENV_EXT}.zip")) {
        let f = File::open(path).map_err(|e| e.to_string())?;
        let mut zip = ZipArchive::new(f).map_err(|e| format!("无法打开环境包 zip：{e}"))?;
        let mut entry = zip
            .by_name(ENV_MANIFEST_NAME)
            .map_err(|_| format!("zip 内缺少 {ENV_MANIFEST_NAME}"))?;
        let mut raw = String::new();
        entry.read_to_string(&mut raw).map_err(|e| e.to_string())?;
        return read_manifest_json(&raw);
    }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    read_manifest_json(&raw)
}

fn extract_layer_from_zip(zip_path: &Path, game_dir: &Path) -> Result<usize, String> {
    let f = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut zip = ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut count = 0usize;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if !name.starts_with("layer/") || name.ends_with('/') {
            continue;
        }
        // Refuse saves/ even if a malicious pack includes it.
        if name.contains("/saves/") || name.starts_with("layer/saves/") {
            continue;
        }
        let rel = name.trim_start_matches("layer/");
        let dest = game_dir.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = File::create(&dest).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        count += 1;
    }
    Ok(count)
}

/// Import `.easymc-env` — create/attach container version; optional layer files from pack zip.
/// Never writes into `saves/`. Does not change native instances that stay without container.
pub fn import_env_pack(
    paths: &Paths,
    bundle_path: &Path,
    opts: EnvImportOptions,
) -> Result<EnvImportResult, String> {
    let manifest = read_env_manifest_from_path(bundle_path)?;
    let prefer = opts
        .prefer_china_mirrors
        .unwrap_or(manifest.container.prefer_china_mirrors);

    let loader = ModLoader::parse_loose(&manifest.instance.loader).unwrap_or(ModLoader::Vanilla);
    let version = McVersion(manifest.instance.version.clone());

    let inst = if let Some(id) = opts.target_instance_id.as_deref() {
        let store = load_instances(paths).map_err(|e| e.to_string())?;
        let existing = store
            .instances
            .iter()
            .find(|i| i.id == id)
            .cloned()
            .ok_or_else(|| format!("找不到目标版本 {id}"))?;
        if existing.container.is_none() {
            attach_container_to_instance(paths, id, prefer).map_err(|e| e.to_string())?
        } else {
            existing
        }
    } else {
        let created = create_named_instance_with(
            paths,
            CreateInstanceOpts {
                name: manifest.instance.name.clone(),
                version,
                loader,
                loader_version: manifest.instance.loader_version.clone(),
                version_ready: false,
            },
        )
        .map_err(|e| e.to_string())?;
        attach_container_to_instance(paths, &created.id, prefer).map_err(|e| e.to_string())?
    };

    let game = PathBuf::from(&inst.game_dir);
    let mut layer_files = 0usize;
    let is_zip = bundle_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
        || bundle_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase().ends_with(&format!(".{ENV_EXT}.zip")))
            .unwrap_or(false);

    if matches!(manifest.mode, EnvPackMode::Pack) || is_zip {
        layer_files = extract_layer_from_zip(bundle_path, &game)?;
    }

    // Prefer image from manifest when present (attach already set a default).
    if let Some(img) = &manifest.container.image {
        let _ = easymc_core::update_instance_fields(paths, &inst.id, |i| {
            if let Some(c) = i.container.as_mut() {
                c.image = Some(img.clone());
            }
        });
    }

    let store = load_instances(paths).map_err(|e| e.to_string())?;
    let final_inst = store
        .instances
        .iter()
        .find(|i| i.id == inst.id)
        .cloned()
        .unwrap_or(inst);

    Ok(EnvImportResult {
        instance: final_inst,
        mode: manifest.mode,
        layer_files,
        message_zh: if layer_files > 0 {
            format!(
                "已导入容器环境「{}」：挂上容器元数据，并恢复 {} 个层文件（存档未导入）。请再安装游戏文件后开玩。",
                manifest.instance.name, layer_files
            )
        } else {
            format!(
                "已导入容器环境「{}」：已创建/挂上容器版本（元数据）。模组需自行安装或使用完整环境包。",
                manifest.instance.name
            )
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use easymc_core::{ContainerMeta, CONTAINER_RUNTIME_PODMAN};
    use tempfile::tempdir;

    #[test]
    fn metadata_roundtrip() {
        let dir = tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure().unwrap();
        let inst = create_named_instance_with(
            &paths,
            CreateInstanceOpts {
                name: "测试环境".into(),
                version: McVersion("1.21.1".into()),
                loader: ModLoader::Fabric,
                loader_version: Some("0.16.0".into()),
                version_ready: false,
            },
        )
        .unwrap();
        easymc_core::update_instance_fields(&paths, &inst.id, |i| {
            i.container = Some(ContainerMeta::new_podman(DEFAULT_BASE_IMAGE));
        })
        .unwrap();
        let game = PathBuf::from(&load_instances(&paths).unwrap().instances[0].game_dir);
        fs::create_dir_all(game.join("mods")).unwrap();
        fs::write(game.join("mods").join("demo.jar"), b"jar").unwrap();

        let out = dir.path().join("round.easymc-env");
        let exp = export_env_pack(
            &paths,
            &inst.id,
            EnvExportOptions {
                mode: EnvPackMode::Metadata,
                notes: Some("unit".into()),
                output: Some(out.clone()),
            },
        )
        .unwrap();
        assert!(exp.path.ends_with(".easymc-env"));
        let m = read_env_manifest_from_path(&out).unwrap();
        assert_eq!(m.format, ENV_FORMAT_ID);
        assert_eq!(m.instance.version, "1.21.1");
        assert!(m.layer.mod_filenames.iter().any(|f| f == "demo.jar"));
        assert!(!m.layer.include_saves);

        let imp = import_env_pack(&paths, &out, EnvImportOptions::default()).unwrap();
        assert!(imp.instance.container.is_some());
        assert_eq!(
            imp.instance.container.as_ref().unwrap().runtime,
            CONTAINER_RUNTIME_PODMAN
        );
    }

    #[test]
    fn rejects_saves_flag() {
        let mut m = EnvManifest {
            format: ENV_FORMAT_ID.into(),
            format_version: 1,
            mode: EnvPackMode::Metadata,
            exported_at: "t".into(),
            notes: None,
            instance: EnvInstanceSpec {
                name: "a".into(),
                version: "1.20".into(),
                loader: "vanilla".into(),
                loader_version: None,
            },
            container: EnvContainerSpec {
                runtime: "podman".into(),
                image: None,
                prefer_china_mirrors: true,
            },
            layer: EnvLayerSpec {
                dirs: vec![],
                include_saves: true,
                mod_filenames: vec![],
            },
            settings_hint: None,
        };
        assert!(m.validate().is_err());
        m.layer.include_saves = false;
        assert!(m.validate().is_ok());
    }
}
