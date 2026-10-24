//! Find `.minecraft` folders already present on the machine, so an existing
//! install can be reused/imported instead of downloaded from scratch.
//!
//! Detection is path-based (no shelling out), so it works the same on Windows,
//! macOS and Linux.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MinecraftDir {
    /// Absolute path to the `.minecraft` folder.
    pub path: PathBuf,
    /// Where it was found, e.g. `official` / `flatpak`.
    pub source: String,
    /// Number of versions under `versions/<id>/`.
    pub version_count: usize,
}

/// Scan the well-known `.minecraft` locations for the current OS.
///
/// Only folders that actually exist are returned; ones with a populated
/// `versions/` directory sort first, since those are worth importing.
pub fn detect_minecraft_dirs() -> Vec<MinecraftDir> {
    let mut found: Vec<MinecraftDir> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (dir, source) in candidate_dirs() {
        if !seen.insert(dir.to_string_lossy().to_string()) {
            continue;
        }
        if !dir.is_dir() {
            continue;
        }
        found.push(MinecraftDir {
            version_count: count_versions(&dir),
            path: dir,
            source: source.into(),
        });
    }

    found.sort_by(|a, b| b.version_count.cmp(&a.version_count));
    found
}

fn count_versions(dir: &Path) -> usize {
    std::fs::read_dir(dir.join("versions"))
        .map(|rd| rd.flatten().filter(|e| e.path().is_dir()).count())
        .unwrap_or(0)
}

fn candidate_dirs() -> Vec<(PathBuf, &'static str)> {
    let mut v: Vec<(PathBuf, &'static str)> = Vec::new();
    if cfg!(windows) {
        // The official launcher lives in %APPDATA%\.minecraft.
        for base in [dirs::config_dir(), dirs::data_dir()].into_iter().flatten() {
            v.push((base.join(".minecraft"), "official"));
        }
    } else if cfg!(target_os = "macos") {
        // ~/Library/Application Support/minecraft
        if let Some(base) = dirs::data_dir() {
            v.push((base.join("minecraft"), "official"));
        }
    } else {
        if let Some(home) = dirs::home_dir() {
            v.push((home.join(".minecraft"), "official"));
            // Flatpak build of the official launcher.
            v.push((home.join(".var/app/com.mojang.Minecraft/.minecraft"), "flatpak"));
        }
    }
    v
}
