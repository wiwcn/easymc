//! Optional per-instance mods index (`.easymc-mods.json`) for share round-trips.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use easymc_core::{Instance, Result};
use easymc_mods::mods_dir;

use crate::format::ModEntry;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModsIndex {
    #[serde(default)]
    pub mods: Vec<ModEntry>,
}

pub fn index_path(instance: &Instance) -> PathBuf {
    mods_dir(instance).join(".easymc-mods.json")
}

pub fn load_index(instance: &Instance) -> ModsIndex {
    let p = index_path(instance);
    read_index_file(&p).unwrap_or_default()
}

pub fn read_index_file(path: &Path) -> Option<ModsIndex> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_index(instance: &Instance, index: &ModsIndex) -> Result<()> {
    let dir = mods_dir(instance);
    fs::create_dir_all(&dir)?;
    let p = index_path(instance);
    let raw = serde_json::to_string_pretty(index)?;
    fs::write(p, raw)?;
    Ok(())
}

pub fn upsert_index_entry(instance: &Instance, entry: ModEntry) -> Result<()> {
    let mut idx = load_index(instance);
    if let Some(slot) = idx
        .mods
        .iter_mut()
        .find(|m| m.filename == entry.filename)
    {
        *slot = entry;
    } else {
        idx.mods.push(entry);
    }
    save_index(instance, &idx)
}
