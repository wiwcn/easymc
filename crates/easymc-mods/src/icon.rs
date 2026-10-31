//! Resolve display icons for marketplace hits and local packs/mods.
//!
//! Sources when listing local files:
//! 1. Cached HTTP URL from Modrinth / CurseForge (`.easymc-*.json` written on install)
//! 2. `pack.png` / `pack.jpg` on disk or inside a zip/jar
//! 3. Fabric/Quilt `icon` path inside the jar

use std::io::Read;
use std::path::Path;

use base64::Engine;
use zip::ZipArchive;

const MAX_ICON_BYTES: usize = 256 * 1024;

pub fn bytes_to_data_url(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() || bytes.len() > MAX_ICON_BYTES {
        return None;
    }
    let mime = sniff_mime(bytes)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{mime};base64,{b64}"))
}

fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']) {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else {
        None
    }
}

fn read_capped(path: &Path) -> Option<Vec<u8>> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() == 0 || meta.len() as usize > MAX_ICON_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    if sniff_mime(&bytes).is_some() {
        Some(bytes)
    } else {
        None
    }
}

fn norm_zip_name(name: &str) -> String {
    name.trim_start_matches("./").replace('\\', "/").to_ascii_lowercase()
}

fn zip_find_bytes(archive: &mut ZipArchive<std::fs::File>, candidates: &[&str]) -> Option<Vec<u8>> {
    let wanted: Vec<String> = candidates.iter().map(|c| c.to_ascii_lowercase()).collect();
    for i in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(i) else { continue };
        if entry.is_dir() {
            continue;
        }
        let name = norm_zip_name(entry.name());
        if !wanted.iter().any(|w| name == *w || name.ends_with(&format!("/{w}"))) {
            continue;
        }
        if entry.size() == 0 || entry.size() as usize > MAX_ICON_BYTES {
            continue;
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        if entry.read_to_end(&mut buf).is_ok() && sniff_mime(&buf).is_some() {
            return Some(buf);
        }
    }
    None
}

fn fabric_icon_path(archive: &mut ZipArchive<std::fs::File>) -> Option<String> {
    for meta_name in ["fabric.mod.json", "quilt.mod.json"] {
        let Ok(mut entry) = archive.by_name(meta_name) else {
            continue;
        };
        let mut s = String::new();
        if entry.read_to_string(&mut s).is_err() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) else {
            continue;
        };
        if let Some(icon) = v.get("icon").and_then(|x| x.as_str()) {
            let t = icon.trim().trim_start_matches('/');
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

/// Extract a displayable icon from a local folder, zip, or jar.
pub fn extract_local_icon(path: &Path) -> Option<String> {
    if path.is_dir() {
        for name in ["pack.png", "pack.jpg", "pack.jpeg", "icon.png", "icon.jpg"] {
            if let Some(bytes) = read_capped(&path.join(name)) {
                return bytes_to_data_url(&bytes);
            }
        }
        return None;
    }
    if !path.is_file() {
        return None;
    }
    let lower = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "pack.png" | "pack.jpg" | "pack.jpeg" | "icon.png" | "icon.jpg"
    ) {
        return read_capped(path).and_then(|b| bytes_to_data_url(&b));
    }
    let is_archive = lower.ends_with(".zip")
        || lower.ends_with(".jar")
        || lower.ends_with(".jar.disabled");
    if !is_archive {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;

    if let Some(bytes) = zip_find_bytes(
        &mut archive,
        &["pack.png", "pack.jpg", "pack.jpeg", "icon.png", "icon.jpg"],
    ) {
        return bytes_to_data_url(&bytes);
    }
    if let Some(rel) = fabric_icon_path(&mut archive) {
        if let Some(bytes) = zip_find_bytes(&mut archive, &[rel.as_str()]) {
            return bytes_to_data_url(&bytes);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn tiny_png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xd7, 0x63, 0xf8, 0xff, 0xff, 0x3f, 0x00, 0x05, 0xfe, 0x02, 0xfe, 0xdc, 0xcc, 0x59,
            0xe7, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ]
    }

    #[test]
    fn data_url_from_png() {
        let u = bytes_to_data_url(&tiny_png()).unwrap();
        assert!(u.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn extract_from_dir_pack_png() {
        let dir = std::env::temp_dir().join(format!("easymc-icon-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pack.png"), tiny_png()).unwrap();
        let url = extract_local_icon(&dir).unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extract_from_zip_pack_png() {
        let zip_path = std::env::temp_dir().join(format!("easymc-icon-{}.zip", std::process::id()));
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut z = ZipWriter::new(f);
            z.start_file("pack.png", SimpleFileOptions::default()).unwrap();
            z.write_all(&tiny_png()).unwrap();
            z.finish().unwrap();
        }
        let url = extract_local_icon(&zip_path).unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
        let _ = std::fs::remove_file(&zip_path);
    }
}
