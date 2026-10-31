//! Parse Fabric / Quilt / Forge / NeoForge mod metadata from jar files.

use std::collections::HashSet;
use std::io::{Cursor, Read, Seek};
use std::path::Path;

use easymc_core::{EasyMcError, Result};
use serde_json::Value as JsonValue;
use zip::ZipArchive;

/// Which loader family the jar metadata came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModMetaKind {
    Fabric,
    Quilt,
    Forge,
    NeoForge,
    Unknown,
}

impl ModMetaKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DepKind {
    Required,
    Optional,
    Incompatible,
}

impl DepKind {
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Required => "必需",
            Self::Optional => "可选",
            Self::Incompatible => "不兼容",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModDepReq {
    pub mod_id: String,
    pub version_range: Option<String>,
    pub kind: DepKind,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModMeta {
    pub mod_id: String,
    pub version: String,
    pub name: Option<String>,
    pub filename: String,
    pub disabled: bool,
    pub kind: ModMetaKind,
    pub depends: Vec<ModDepReq>,
    pub breaks: Vec<ModDepReq>,
    /// Fabric/Quilt `provides` — alternate ids satisfied by this jar (e.g. fabric-api → fabric).
    #[serde(default)]
    pub provides: Vec<String>,
}

/// Built-in / loader-provided ids that are never "missing mods".
pub fn is_provided_mod_id(id: &str) -> bool {
    matches!(
        id.trim().to_ascii_lowercase().as_str(),
        "minecraft"
            | "java"
            | "fabricloader"
            | "fabric-loader"
            | "quilt_loader"
            | "quilt-loader"
            | "forge"
            | "neoforge"
            | "mcp"
            | "minecraft_server"
            | "client"
            | "server"
    )
}

/// Read primary mod meta plus nested jars listed in fabric.mod.json `jars`.
/// Nested metas inherit the outer filename (with `#nested/<id>`) and disabled flag.
/// Expansion is recursive (LibGui → jankson / libninepatch, etc.).
pub fn read_mod_metas_expanded(path: &Path) -> Result<Vec<ModMeta>> {
    let primary = read_mod_meta(path)?;
    let filename = primary.filename.clone();
    let disabled = primary.disabled;
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(vec![primary]),
    };
    let mut archive = match ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return Ok(vec![primary]),
    };
    let mut out = vec![primary];
    let mut seen: HashSet<String> = out.iter().map(|m| m.mod_id.to_ascii_lowercase()).collect();
    expand_nested_from_archive(&mut archive, &filename, disabled, &mut out, &mut seen, 0)?;
    Ok(out)
}

const MAX_NEST_DEPTH: usize = 4;

fn expand_nested_from_archive<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    outer_filename: &str,
    disabled: bool,
    out: &mut Vec<ModMeta>,
    seen: &mut HashSet<String>,
    depth: usize,
) -> Result<()> {
    if depth >= MAX_NEST_DEPTH {
        return Ok(());
    }
    let raw = match read_zip_string(archive, "fabric.mod.json") {
        Ok(Some(s)) => s,
        _ => match read_zip_string(archive, "quilt.mod.json") {
            Ok(Some(s)) => s,
            _ => return Ok(()),
        },
    };
    let Ok(v) = serde_json::from_str::<JsonValue>(&raw) else {
        return Ok(());
    };
    let root = v.get("quilt_loader").cloned().unwrap_or(v);
    // Merge top-level provides into the first matching outer meta if present
    if let Some(provides) = root.get("provides").and_then(|a| a.as_array()) {
        if let Some(id) = root.get("id").and_then(|x| x.as_str()) {
            if let Some(meta) = out.iter_mut().find(|m| m.mod_id == id) {
                for p in provides.iter().filter_map(|x| x.as_str()) {
                    if !meta.provides.iter().any(|x| x == p) {
                        meta.provides.push(p.to_string());
                    }
                }
            }
        }
    }
    let Some(jars) = root.get("jars").and_then(|j| j.as_array()) else {
        return Ok(());
    };
    for entry in jars {
        let Some(nested_path) = entry.get("file").and_then(|x| x.as_str()) else {
            continue;
        };
        let Ok(Some(bytes)) = read_zip_bytes(archive, nested_path) else {
            continue;
        };
        let cursor = Cursor::new(bytes.clone());
        let Ok(mut nested_zip) = ZipArchive::new(cursor) else {
            continue;
        };
        let mut meta_opt = try_fabric_family(
            &mut nested_zip,
            outer_filename,
            disabled,
            ModMetaKind::Fabric,
        )?;
        if meta_opt.is_none() {
            // re-open because try may have consumed? ZipArchive with Cursor is fine to recreate
            let cursor = Cursor::new(bytes.clone());
            if let Ok(mut nz) = ZipArchive::new(cursor) {
                meta_opt = try_fabric_family(&mut nz, outer_filename, disabled, ModMetaKind::Quilt)?;
            }
        }
        if let Some(mut meta) = meta_opt {
            let key = meta.mod_id.to_ascii_lowercase();
            meta.filename = format!("{outer_filename}#nested/{}", meta.mod_id);
            if seen.contains(&key) {
                // fold provides into existing
                if let Some(existing) = out.iter_mut().find(|m| m.mod_id.eq_ignore_ascii_case(&meta.mod_id))
                {
                    for p in meta.provides.drain(..) {
                        if !existing.provides.contains(&p) {
                            existing.provides.push(p);
                        }
                    }
                }
            } else {
                seen.insert(key);
                for p in &meta.provides {
                    seen.insert(p.to_ascii_lowercase());
                }
                out.push(meta);
            }
        }
        // Recurse into the nested jar bytes for deeper jars
        let cursor = Cursor::new(bytes);
        if let Ok(mut nz) = ZipArchive::new(cursor) {
            expand_nested_from_archive(
                &mut nz,
                outer_filename,
                disabled,
                out,
                seen,
                depth + 1,
            )?;
        }
    }
    Ok(())
}

fn read_zip_bytes<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<Option<Vec<u8>>> {
    let idx = match archive.index_for_name(name) {
        Some(i) => i,
        None => {
            let lower = name.to_ascii_lowercase();
            let found = (0..archive.len()).find(|&i| {
                archive
                    .name_for_index(i)
                    .map(|n| n.to_ascii_lowercase() == lower)
                    .unwrap_or(false)
            });
            match found {
                Some(i) => i,
                None => return Ok(None),
            }
        }
    };
    let mut file = archive
        .by_index(idx)
        .map_err(|e| EasyMcError::msg(format!("读取 {name} 失败: {e}")))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| EasyMcError::msg(format!("读取 {name} 内容失败: {e}")))?;
    Ok(Some(buf))
}

/// Read mod metadata from a jar (or `.jar.disabled`) path.
pub fn read_mod_meta(path: &Path) -> Result<ModMeta> {
    let filename = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    let disabled = filename.ends_with(".jar.disabled");
    let file = std::fs::File::open(path)
        .map_err(|e| EasyMcError::msg(format!("无法打开模组 {filename}: {e}")))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| EasyMcError::msg(format!("无法读取模组包 {filename}: {e}")))?;

    if let Some(meta) = try_fabric_family(&mut archive, &filename, disabled, ModMetaKind::Fabric)? {
        return Ok(meta);
    }
    if let Some(meta) = try_fabric_family(&mut archive, &filename, disabled, ModMetaKind::Quilt)? {
        return Ok(meta);
    }
    if let Some(meta) = try_forge_toml(&mut archive, &filename, disabled, ModMetaKind::NeoForge)? {
        return Ok(meta);
    }
    if let Some(meta) = try_forge_toml(&mut archive, &filename, disabled, ModMetaKind::Forge)? {
        return Ok(meta);
    }

    // Fallback: filename stem as id
    let stem = filename
        .trim_end_matches(".disabled")
        .trim_end_matches(".jar")
        .to_string();
    Ok(ModMeta {
        mod_id: stem.clone(),
        version: "unknown".into(),
        name: Some(stem),
        filename,
        disabled,
        kind: ModMetaKind::Unknown,
        depends: vec![],
        breaks: vec![],
        provides: vec![],
    })
}

fn try_fabric_family<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    filename: &str,
    disabled: bool,
    kind: ModMetaKind,
) -> Result<Option<ModMeta>> {
    let entry_name = match kind {
        ModMetaKind::Fabric => "fabric.mod.json",
        ModMetaKind::Quilt => "quilt.mod.json",
        _ => return Ok(None),
    };
    let Some(raw) = read_zip_string(archive, entry_name)? else {
        return Ok(None);
    };
    let v: JsonValue = serde_json::from_str(&raw)
        .map_err(|e| EasyMcError::msg(format!("解析 {entry_name} 失败 ({filename}): {e}")))?;

    // Quilt uses "quilt_loader" wrapper; Fabric is flat.
    let root = if kind == ModMetaKind::Quilt {
        v.get("quilt_loader").cloned().unwrap_or(v)
    } else {
        v
    };

    let mod_id = root
        .get("id")
        .or_else(|| root.get("mid"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if mod_id.is_empty() {
        return Ok(None);
    }
    let version = root
        .get("version")
        .and_then(|x| x.as_str())
        .unwrap_or("unknown")
        .to_string();
    let name = root
        .get("name")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            root.get("metadata")
                .and_then(|m| m.get("name"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
        });

    let mut depends = Vec::new();
    let mut breaks = Vec::new();

    // fabric: depends / recommends / suggests / breaks / conflicts
    // quilt: depends / breaks under quilt_loader, or depends object
    collect_fabric_map(&root, "depends", DepKind::Required, &mut depends);
    collect_fabric_map(&root, "recommends", DepKind::Optional, &mut depends);
    collect_fabric_map(&root, "suggests", DepKind::Optional, &mut depends);
    collect_fabric_map(&root, "breaks", DepKind::Incompatible, &mut breaks);
    collect_fabric_map(&root, "conflicts", DepKind::Incompatible, &mut breaks);

    // Nested depends for quilt (depends: { versions: ... }) — already covered if flat.
    if let Some(deps) = root.get("depends") {
        if deps.get("versions").is_some() {
            // unusual; skip
        }
    }

    let provides: Vec<String> = root
        .get("provides")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(Some(ModMeta {
        mod_id,
        version,
        name,
        filename: filename.to_string(),
        disabled,
        kind,
        depends,
        breaks,
        provides,
    }))
}

fn collect_fabric_map(root: &JsonValue, key: &str, kind: DepKind, out: &mut Vec<ModDepReq>) {
    let Some(obj) = root.get(key).and_then(|v| v.as_object()) else {
        return;
    };
    for (id, range_val) in obj {
        let range = match range_val {
            JsonValue::String(s) => Some(s.clone()),
            JsonValue::Array(arr) => {
                // Fabric: array means OR of version predicates.
                let parts: Vec<String> = arr
                    .iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect();
                if parts.is_empty() {
                    None
                } else if parts.len() == 1 {
                    Some(parts[0].clone())
                } else {
                    Some(parts.join(" || "))
                }
            }
            JsonValue::Bool(false) => continue, // explicitly not depending
            _ => None,
        };
        out.push(ModDepReq {
            mod_id: id.clone(),
            version_range: range,
            kind,
        });
    }
}

fn try_forge_toml<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    filename: &str,
    disabled: bool,
    kind: ModMetaKind,
) -> Result<Option<ModMeta>> {
    let entry_name = match kind {
        ModMetaKind::NeoForge => "META-INF/neoforge.mods.toml",
        ModMetaKind::Forge => "META-INF/mods.toml",
        _ => return Ok(None),
    };
    let Some(raw) = read_zip_string(archive, entry_name)? else {
        // NeoForge jars sometimes only ship mods.toml
        if kind == ModMetaKind::NeoForge {
            return Ok(None);
        }
        return Ok(None);
    };
    parse_mods_toml(&raw, filename, disabled, kind)
}

fn parse_mods_toml(
    raw: &str,
    filename: &str,
    disabled: bool,
    kind: ModMetaKind,
) -> Result<Option<ModMeta>> {
    let value: toml::Value = raw
        .parse()
        .map_err(|e| EasyMcError::msg(format!("解析 mods.toml 失败 ({filename}): {e}")))?;

    let mods = value
        .get("mods")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let first = mods.first();
    let mod_id = first
        .and_then(|m| m.get("modId"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if mod_id.is_empty() {
        return Ok(None);
    }
    let version = first
        .and_then(|m| m.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let name = first
        .and_then(|m| m.get("displayName"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut depends = Vec::new();
    let mut breaks = Vec::new();

    // [[dependencies.modid]] tables live under value["dependencies"][modid] as array
    if let Some(deps_table) = value.get("dependencies").and_then(|v| v.as_table()) {
        for (_owner, arr) in deps_table {
            let Some(list) = arr.as_array() else { continue };
            for dep in list {
                let dep_id = dep
                    .get("modId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if dep_id.is_empty() {
                    continue;
                }
                let mandatory = dep
                    .get("mandatory")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                // NeoForge uses type = "required" | "optional" | "incompatible" | "discouraged"
                let type_str = dep
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let kind = if type_str == "incompatible" || type_str == "discouraged" {
                    DepKind::Incompatible
                } else if type_str == "optional" || !mandatory {
                    DepKind::Optional
                } else {
                    DepKind::Required
                };
                let range = dep
                    .get("versionRange")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let req = ModDepReq {
                    mod_id: dep_id,
                    version_range: range,
                    kind,
                };
                if kind == DepKind::Incompatible {
                    breaks.push(req);
                } else {
                    depends.push(req);
                }
            }
        }
    }

    Ok(Some(ModMeta {
        mod_id,
        version,
        name,
        filename: filename.to_string(),
        disabled,
        kind,
        depends,
        breaks,
        provides: vec![],
    }))
}

fn read_zip_string<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<Option<String>> {
    let idx = match archive.index_for_name(name) {
        Some(i) => i,
        None => {
            // Case-insensitive fallback
            let lower = name.to_ascii_lowercase();
            let found = (0..archive.len()).find(|&i| {
                archive
                    .name_for_index(i)
                    .map(|n| n.to_ascii_lowercase() == lower)
                    .unwrap_or(false)
            });
            match found {
                Some(i) => i,
                None => return Ok(None),
            }
        }
    };
    let mut file = archive
        .by_index(idx)
        .map_err(|e| EasyMcError::msg(format!("读取 {name} 失败: {e}")))?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)
        .map_err(|e| EasyMcError::msg(format!("读取 {name} 内容失败: {e}")))?;
    Ok(Some(buf))
}

/// Best-effort version range check (Fabric operators + Maven ranges + `*`).
pub fn version_satisfies(installed: &str, range: &str) -> bool {
    let installed = installed.trim();
    let range = range.trim();
    if range.is_empty() || range == "*" {
        return true;
    }
    if installed.is_empty() || installed == "unknown" {
        // Can't verify — treat as satisfied to avoid false conflicts on unknown jars
        return true;
    }
    // Fabric array → OR (we join with " || " in collect_fabric_map)
    if range.contains("||") {
        return range
            .split("||")
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .any(|p| version_satisfies(installed, p));
    }
    // Multiple space-separated predicates (Fabric): ">=1.0 <2.0"
    if range.contains(' ') && !range.starts_with('[') && !range.starts_with('(') {
        return range.split_whitespace().all(|p| version_satisfies(installed, p));
    }
    // Maven range: [1.0,2.0) or (1.0,) etc.
    if range.starts_with('[') || range.starts_with('(') {
        return maven_range_satisfies(installed, range);
    }
    // Wildcard minor: "0.8.x" / "1.21.x"
    if let Some(prefix) = range.strip_suffix(".x").or_else(|| range.strip_suffix(".X")) {
        let prefix = prefix.trim().trim_end_matches('.');
        if !prefix.is_empty() {
            return installed == prefix
                || installed.starts_with(&format!("{prefix}."))
                || same_minor_prefix(installed, prefix);
        }
    }
    // Exact or operator
    for op in [">=", "<=", ">", "<", "=", "~", "^"] {
        if let Some(rest) = range.strip_prefix(op) {
            let target = rest.trim();
            return match op {
                ">=" => cmp_ver(installed, target) >= 0,
                ">" => cmp_ver(installed, target) > 0,
                "<=" => cmp_ver(installed, target) <= 0,
                "<" => cmp_ver(installed, target) < 0,
                "=" => cmp_ver(installed, target) == 0,
                "~" => {
                    // >=target and same major.minor (approx)
                    cmp_ver(installed, target) >= 0 && same_minor_prefix(installed, target)
                }
                "^" => cmp_ver(installed, target) >= 0 && same_major_prefix(installed, target),
                _ => true,
            };
        }
    }
    // Bare version → exact or prefix match
    installed == range || installed.starts_with(&format!("{range}.")) || installed.starts_with(range)
}

fn parse_nums(v: &str) -> Vec<i64> {
    v.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect()
}

fn cmp_ver(a: &str, b: &str) -> i32 {
    let aa = parse_nums(a);
    let bb = parse_nums(b);
    let n = aa.len().max(bb.len());
    for i in 0..n {
        let x = aa.get(i).copied().unwrap_or(0);
        let y = bb.get(i).copied().unwrap_or(0);
        if x < y {
            return -1;
        }
        if x > y {
            return 1;
        }
    }
    0
}

fn same_major_prefix(installed: &str, target: &str) -> bool {
    let a = parse_nums(installed);
    let b = parse_nums(target);
    a.first() == b.first()
}

fn same_minor_prefix(installed: &str, target: &str) -> bool {
    let a = parse_nums(installed);
    let b = parse_nums(target);
    a.get(0) == b.get(0) && a.get(1).or(Some(&0)) == b.get(1).or(Some(&0))
}

fn maven_range_satisfies(installed: &str, range: &str) -> bool {
    // Support single interval: [a,b], (a,b), [a,b), (a,b], [a,), (,b] …
    let range = range.trim();
    let open_incl = range.starts_with('[');
    let close_incl = range.ends_with(']');
    let inner = range
        .trim_start_matches(['[', '('])
        .trim_end_matches([']', ')']);
    let mut parts = inner.splitn(2, ',');
    let low = parts.next().unwrap_or("").trim();
    let high = parts.next().unwrap_or("").trim();
    if !low.is_empty() {
        let c = cmp_ver(installed, low);
        if open_incl {
            if c < 0 {
                return false;
            }
        } else if c <= 0 {
            return false;
        }
    }
    if !high.is_empty() {
        let c = cmp_ver(installed, high);
        if close_incl {
            if c > 0 {
                return false;
            }
        } else if c >= 0 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    fn write_jar(path: &Path, entries: &[(&str, &str)]) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, body) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn parse_fabric_mod_json_depends() {
        let dir = std::env::temp_dir().join(format!(
            "easymc-meta-fabric-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let jar = dir.join("sodium.jar");
        write_jar(
            &jar,
            &[(
                "fabric.mod.json",
                r#"{
                  "schemaVersion": 1,
                  "id": "sodium",
                  "version": "0.5.11",
                  "name": "Sodium",
                  "depends": {
                    "fabricloader": ">=0.15",
                    "minecraft": "~1.21",
                    "fabric-api": "*"
                  },
                  "breaks": { "optifabric": "*" }
                }"#,
            )],
        );
        let meta = read_mod_meta(&jar).unwrap();
        assert_eq!(meta.mod_id, "sodium");
        assert_eq!(meta.version, "0.5.11");
        assert_eq!(meta.kind, ModMetaKind::Fabric);
        assert!(meta
            .depends
            .iter()
            .any(|d| d.mod_id == "fabric-api" && d.kind == DepKind::Required));
        assert!(meta.breaks.iter().any(|d| d.mod_id == "optifabric"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_forge_mods_toml() {
        let dir = std::env::temp_dir().join(format!(
            "easymc-meta-forge-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let jar = dir.join("jei.jar");
        let toml = r#"
modLoader="javafml"
loaderVersion="[47,)"
license="MIT"

[[mods]]
modId="jei"
version="15.0.0"
displayName="Just Enough Items"

[[dependencies.jei]]
modId="forge"
mandatory=true
versionRange="[47,)"
ordering="NONE"
side="BOTH"

[[dependencies.jei]]
modId="minecraft"
mandatory=true
versionRange="[1.20.1,1.21)"
ordering="NONE"
side="BOTH"
"#;
        write_jar(&jar, &[("META-INF/mods.toml", toml)]);
        let meta = read_mod_meta(&jar).unwrap();
        assert_eq!(meta.mod_id, "jei");
        assert_eq!(meta.kind, ModMetaKind::Forge);
        assert!(meta.depends.iter().any(|d| d.mod_id == "forge"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn version_range_checks() {
        assert!(version_satisfies("1.2.3", "*"));
        assert!(version_satisfies("1.2.3", ">=1.2"));
        assert!(!version_satisfies("1.1.0", ">=1.2"));
        assert!(version_satisfies("1.20.1", "[1.20,1.21)"));
        assert!(!version_satisfies("1.21", "[1.20,1.21)"));
        assert!(version_satisfies("0.15.11", ">=0.15"));
        assert!(version_satisfies("1.21.1", "~1.21"));
        // Fabric array OR
        assert!(version_satisfies("0.8.13", "0.8.x || 0.9.x"));
        assert!(version_satisfies("0.9.1", "0.8.x || 0.9.x"));
        assert!(!version_satisfies("0.7.0", "0.8.x || 0.9.x"));
        assert!(version_satisfies("0.8.13", "0.8.x"));
        assert!(version_satisfies("1.21.1", "1.21.x"));
    }

    #[test]
    fn fabric_array_depends_are_or() {
        let dir = std::env::temp_dir().join(format!(
            "easymc-meta-or-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let jar = dir.join("pf.jar");
        write_jar(
            &jar,
            &[(
                "fabric.mod.json",
                r#"{"schemaVersion":1,"id":"presencefootsteps","version":"1.0","depends":{"sodium":["0.8.x","0.9.x"]},"provides":["presence-footsteps"]}"#,
            )],
        );
        let meta = read_mod_meta(&jar).unwrap();
        let dep = meta.depends.iter().find(|d| d.mod_id == "sodium").unwrap();
        assert!(dep.version_range.as_deref().unwrap().contains("||"));
        assert!(meta.provides.iter().any(|p| p == "presence-footsteps"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn provided_ids() {
        assert!(is_provided_mod_id("minecraft"));
        assert!(is_provided_mod_id("FabricLoader"));
        assert!(!is_provided_mod_id("fabric-api"));
    }
}
