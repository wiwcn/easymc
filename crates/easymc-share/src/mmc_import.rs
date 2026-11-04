//! Prism / MultiMC instance zip import (clean-room; no Prism GPL code).
//!
//! Expected layout (archive root or one nested folder):
//! - `instance.cfg` — INI-like; `name=` for display name
//! - `mmc-pack.json` — components with Minecraft + loader uids
//! - `.minecraft/` or `minecraft/` — game dir contents → EasyMC `game_dir`

use std::fs::{self, File};
use std::io::{copy, Read};
use std::path::{Component, Path, PathBuf};

use zip::ZipArchive;

use easymc_core::{
    create_named_instance_with, upsert_instance, CreateInstanceOpts, EasyMcError, McVersion,
    ModLoader, Paths, Result,
};

use crate::import::{ImportOptions, ImportResult};

#[derive(Debug, Clone)]
struct MmcPackInfo {
    game_version: String,
    loader: ModLoader,
    loader_version: Option<String>,
}

/// True when the zip looks like a MultiMC / Prism instance export.
pub fn looks_like_mmc(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext != "zip" {
        return false;
    }
    let Ok(file) = File::open(path) else {
        return false;
    };
    let Ok(z) = ZipArchive::new(file) else {
        return false;
    };
    let names: Vec<String> = z.file_names().map(|n| n.replace("\\", "/")).collect();


    names.iter().any(|n| is_instance_cfg_entry(n))
}

fn is_instance_cfg_entry(name: &str) -> bool {
    let n = name.trim_end_matches('/');
    let base = n.rsplit('/').next().unwrap_or(n);
    if base != "instance.cfg" {
        return false;
    }
    // Ignore overrides/ copies of the same name if any.
    !n.split('/').any(|p| p.eq_ignore_ascii_case("overrides"))
}

fn zip_names(path: &Path) -> Result<Vec<String>> {
    let file = File::open(path)?;
    let z = ZipArchive::new(file).map_err(|e| EasyMcError::msg(format!("zip: {e}")))?;
    Ok(z.file_names().map(|n| n.replace('\\', "/")).collect())
}

fn find_instance_cfg_entry(names: &[String]) -> Option<String> {
    names
        .iter()
        .find(|n| is_instance_cfg_entry(n))
        .cloned()
}

/// Directory prefix of `instance.cfg` (empty = archive root).
fn instance_prefix(cfg_entry: &str) -> String {
    match cfg_entry.rsplit_once('/') {
        Some((dir, _)) => {
            if dir.is_empty() {
                String::new()
            } else {
                format!("{dir}/")
            }
        }
        None => String::new(),
    }
}

fn read_zip_entry_text(path: &Path, entry: &str) -> Result<Option<String>> {
    let file = File::open(path)?;
    let mut z = ZipArchive::new(file).map_err(|e| EasyMcError::msg(format!("zip: {e}")))?;
    let Ok(mut e) = z.by_name(entry) else {
        // Try forward-slash normalized names (ZipArchive uses stored names).
        let target = entry.replace('\\', "/");
        let mut found = None;
        for i in 0..z.len() {
            let name = z
                .by_index(i)
                .map_err(|e| EasyMcError::msg(format!("zip: {e}")))?
                .name()
                .replace('\\', "/");
            if name == target {
                found = Some(i);
                break;
            }
        }
        let Some(i) = found else {
            return Ok(None);
        };
        let mut e = z
            .by_index(i)
            .map_err(|e| EasyMcError::msg(format!("zip: {e}")))?;
        let mut s = String::new();
        e.read_to_string(&mut s)?;
        return Ok(Some(s));
    };
    let mut s = String::new();
    e.read_to_string(&mut s)?;
    Ok(Some(s))
}

fn parse_ini_name(cfg: &str) -> Option<String> {
    for line in cfg.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim().eq_ignore_ascii_case("name") {
            let name = v.trim().trim_matches('"').trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn parse_mmc_pack(raw: &str) -> Result<MmcPackInfo> {
    #[derive(serde::Deserialize)]
    struct Pack {
        #[serde(default)]
        components: Vec<Comp>,
    }
    #[derive(serde::Deserialize)]
    struct Comp {
        #[serde(default)]
        uid: String,
        #[serde(default)]
        version: String,
    }
    let pack: Pack = serde_json::from_str(raw)
        .map_err(|e| EasyMcError::msg(format!("mmc-pack.json 无法读取: {e}")))?;

    let game_version = pack
        .components
        .iter()
        .find(|c| c.uid.trim() == "net.minecraft")
        .map(|c| c.version.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| EasyMcError::msg("mmc-pack.json 缺少 net.minecraft 版本"))?;

    // First match wins (Fabric → Quilt → NeoForge → Forge).
    let loader_order: &[(&str, ModLoader)] = &[
        ("net.fabricmc.fabric-loader", ModLoader::Fabric),
        ("org.quiltmc.quilt-loader", ModLoader::Quilt),
        ("net.neoforged.fancymodloader", ModLoader::NeoForge),
        ("net.neoforged", ModLoader::NeoForge),
        ("net.minecraftforge", ModLoader::Forge),
    ];

    let mut loader = ModLoader::Vanilla;
    let mut loader_version = None;
    for (uid_exact, kind) in loader_order {
        if let Some(c) = pack.components.iter().find(|c| c.uid.trim() == *uid_exact) {
            loader = *kind;
            let v = c.version.trim();
            if !v.is_empty() {
                loader_version = Some(v.to_string());
            }
            break;
        }
    }

    Ok(MmcPackInfo {
        game_version,
        loader,
        loader_version,
    })
}

/// Reject absolute / `..` paths (zip-slip).
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

fn game_dir_prefixes(instance_prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    for name in [".minecraft", "minecraft"] {
        if instance_prefix.is_empty() {
            out.push(format!("{name}/"));
        } else {
            out.push(format!("{instance_prefix}{name}/"));
        }
    }
    out
}

fn extract_game_dir(zip_path: &Path, prefixes: &[String], game_dir: &Path) -> Result<usize> {
    let file = File::open(zip_path)?;
    let mut z = ZipArchive::new(file).map_err(|e| EasyMcError::msg(format!("zip: {e}")))?;
    let mut n = 0usize;
    for i in 0..z.len() {
        let mut e = z
            .by_index(i)
            .map_err(|e| EasyMcError::msg(format!("zip: {e}")))?;
        let name = e.name().replace('\\', "/");
        let Some(rest) = prefixes.iter().find_map(|p| name.strip_prefix(p.as_str())) else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        let Some(rel) = safe_rel(rest) else {
            continue;
        };
        let dest = game_dir.join(&rel);
        if e.is_dir() || name.ends_with('/') {
            fs::create_dir_all(&dest)?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&dest)?;
        copy(&mut e, &mut out)?;
        n += 1;
    }
    Ok(n)
}

fn zip_stem_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Prism 实例".into())
}

/// Import a Prism / MultiMC instance zip into a new EasyMC instance.
pub async fn import_mmc_instance(
    paths: &Paths,
    bundle_path: &Path,
    opts: ImportOptions,
) -> Result<ImportResult> {
    let names = zip_names(bundle_path)?;
    let cfg_entry = find_instance_cfg_entry(&names)
        .ok_or_else(|| EasyMcError::msg("不是 Prism/MultiMC 实例包（缺少 instance.cfg）"))?;
    let prefix = instance_prefix(&cfg_entry);

    let cfg_raw = read_zip_entry_text(bundle_path, &cfg_entry)?
        .ok_or_else(|| EasyMcError::msg("无法读取 instance.cfg"))?;
    let name_from_cfg = parse_ini_name(&cfg_raw);

    let pack_entry = if prefix.is_empty() {
        "mmc-pack.json".to_string()
    } else {
        format!("{prefix}mmc-pack.json")
    };
    let pack_raw = read_zip_entry_text(bundle_path, &pack_entry)?.ok_or_else(|| {
        EasyMcError::msg("不是 Prism/MultiMC 实例包（缺少 mmc-pack.json）")
    })?;
    let pack = parse_mmc_pack(&pack_raw)?;

    let name = opts
        .name
        .filter(|s| !s.trim().is_empty())
        .or(name_from_cfg)
        .unwrap_or_else(|| zip_stem_name(bundle_path));

    let mut inst = create_named_instance_with(
        paths,
        CreateInstanceOpts {
            name: name.clone(),
            version: McVersion(pack.game_version.clone()),
            loader: pack.loader,
            loader_version: pack.loader_version.clone(),
            version_ready: false,
        },
    )?;
    inst.category = Some("导入".into());
    inst.icon = Some("pack".into());
    inst.description = Some(format!(
        "Prism/MultiMC 导入 · {} · {}",
        pack.game_version,
        pack.loader.label_zh()
    ));
    upsert_instance(paths, inst.clone())?;

    let game_dir = PathBuf::from(&inst.game_dir);
    let prefixes = game_dir_prefixes(&prefix);
    let files_copied = extract_game_dir(bundle_path, &prefixes, &game_dir)?;

    let loader_label = pack.loader.label_zh();
    let message_zh = format!(
        "已从 Prism/MultiMC 导入「{}」（{} · {}）。首次启动会下载游戏与加载器。",
        inst.name, pack.game_version, loader_label
    );

    Ok(ImportResult {
        instance_id: inst.id,
        instance_name: inst.name,
        mods_copied: files_copied,
        mods_downloaded: 0,
        mods_missing: Vec::new(),
        memory_applied: None,
        message_zh,
        path_loaded: bundle_path.display().to_string(),
    })
}

/// True when zip has a CurseForge-style `manifest.json` (and is not mrpack / EasyMC / MMC).
pub fn looks_like_cf_manifest(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext != "zip" {
        return false;
    }
    let Ok(names) = zip_names(path) else {
        return false;
    };
    let has_cf = names.iter().any(|n| {
        let base = n.rsplit('/').next().unwrap_or(n.as_str());
        base == "manifest.json" && !n.split('/').any(|p| p.eq_ignore_ascii_case("overrides"))
    });
    if !has_cf {
        return false;
    }
    let has_mr = names
        .iter()
        .any(|n| n == "modrinth.index.json" || n.ends_with("/modrinth.index.json"));
    let has_emc = names
        .iter()
        .any(|n| n.ends_with("easymc.share.json") || n == "easymc.share.json");
    let has_mmc = names.iter().any(|n| is_instance_cfg_entry(n));
    has_cf && !has_mr && !has_emc && !has_mmc
}

#[cfg(test)]
mod tests {
    use super::*;
    use easymc_core::load_instances;
    use std::io::Write;
        use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn tempfile_dir() -> PathBuf {
        let p = std::env::temp_dir().join(format!("easymc-mmc-test-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_synthetic_mmc(zip_path: &Path, nested: bool) {
        let file = File::create(zip_path).unwrap();
        let mut z = ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let prefix = if nested { "MyPack/" } else { "" };

        z.start_file(format!("{prefix}instance.cfg"), opts).unwrap();
        z.write_all("name=测试Prism包\nOverrideJavaLocation=\n".as_bytes()).unwrap();

        let pack = r#"{
  "formatVersion": 1,
  "components": [
    { "uid": "net.minecraft", "version": "1.20.1" },
    { "uid": "net.fabricmc.fabric-loader", "version": "0.15.11" },
    { "uid": "net.fabricmc.intermediary", "version": "1.20.1" }
  ]
}"#;
        z.start_file(format!("{prefix}mmc-pack.json"), opts).unwrap();
        z.write_all(pack.as_bytes()).unwrap();

        z.start_file(format!("{prefix}.minecraft/mods/hello.jar"), opts)
            .unwrap();
        z.write_all(b"PK\x03\x04tiny-jar").unwrap();

        z.start_file(format!("{prefix}.minecraft/config/hello.txt"), opts)
            .unwrap();
        z.write_all(b"ok").unwrap();

        z.finish().unwrap();
    }

    #[test]
    fn parse_loader_fabric() {
        let raw = r#"{"formatVersion":1,"components":[
            {"uid":"net.minecraft","version":"1.21"},
            {"uid":"net.fabricmc.fabric-loader","version":"0.16.0"}
        ]}"#;
        let p = parse_mmc_pack(raw).unwrap();
        assert_eq!(p.game_version, "1.21");
        assert_eq!(p.loader, ModLoader::Fabric);
        assert_eq!(p.loader_version.as_deref(), Some("0.16.0"));
    }

    #[test]
    fn parse_loader_neoforge() {
        let raw = r#"{"components":[
            {"uid":"net.minecraft","version":"1.21.1"},
            {"uid":"net.neoforged","version":"21.1.0"}
        ]}"#;
        let p = parse_mmc_pack(raw).unwrap();
        assert_eq!(p.loader, ModLoader::NeoForge);
    }

    #[test]
    fn ini_name() {
        assert_eq!(
            parse_ini_name("foo=1\nname=Hello World\n").as_deref(),
            Some("Hello World")
        );
    }

    #[test]
    fn safe_rel_blocks_slip() {
        assert!(safe_rel("../evil").is_none());
        assert!(safe_rel("/etc/passwd").is_none());
        assert_eq!(safe_rel("mods/a.jar"), Some(PathBuf::from("mods/a.jar")));
    }

    #[tokio::test]
    async fn import_synthetic_prism_zip() {
        let dir = tempfile_dir();
        let zip_path = dir.join("prism-test.zip");
        write_synthetic_mmc(&zip_path, true);
        assert!(looks_like_mmc(&zip_path));

        let root = dir.join("data");
        let paths = Paths::from_root(&root);
        paths.ensure().unwrap();

        let res = import_mmc_instance(
            &paths,
            &zip_path,
            ImportOptions {
                name: None,
                apply_memory: false,
                offline: true,
            },
        )
        .await
        .expect("import");

        assert!(res.instance_name.contains("测试Prism") || res.instance_name == "测试Prism包");
        assert!(res.message_zh.contains("Prism/MultiMC"));
        assert!(res.message_zh.contains("1.20.1"));
        assert!(res.message_zh.contains("Fabric"));
        assert!(res.mods_copied >= 2);

        let store = load_instances(&paths).unwrap();
        let inst = store
            .instances
            .iter()
            .find(|i| i.id == res.instance_id)
            .expect("instance");
        assert_eq!(inst.version.as_str(), "1.20.1");
        assert_eq!(inst.loader, ModLoader::Fabric);
        assert_eq!(inst.loader_version.as_deref(), Some("0.15.11"));
        assert!(!inst.version_ready);

        let jar = PathBuf::from(&inst.game_dir).join("mods/hello.jar");
        assert!(jar.is_file(), "missing {jar:?}");
        let cfg = PathBuf::from(&inst.game_dir).join("config/hello.txt");
        assert_eq!(fs::read_to_string(cfg).unwrap(), "ok");

        let _ = fs::remove_dir_all(&dir);
    }
}
