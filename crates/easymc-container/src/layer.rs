//! Game writable-layer snapshot / apply / discard (Slice D).
//!
//! Semantics:
//! - **Apply**  = commit current layer as the new baseline (snapshot).
//! - **Discard** = restore baseline into the writable game dir (rebuild layer).
//! - **saves/** are excluded from automatic discard/apply by default — player must propose.
//!
//! Apply/discard use a staging directory then rename so a mid-copy failure
//! leaves the previous baseline (or live dirs) intact.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use easymc_core::{Instance, Paths, Result as CoreResult};
use serde::{Deserialize, Serialize};

/// Subdirs of `.minecraft` that participate in layer snapshot (not saves).
pub const LAYER_DIRS: &[&str] = &[
    "mods",
    "config",
    "resourcepacks",
    "shaderpacks",
    "datapacks",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerState {
    pub instance_id: String,
    pub baseline_at: Option<String>,
    pub applied_at: Option<String>,
    pub dirty: bool,
    pub rev: u64,
    pub message_zh: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerOpResult {
    pub ok: bool,
    pub state: LayerState,
    pub message_zh: String,
}

fn easymc_dir(game_dir: &Path) -> PathBuf {
    game_dir.join(".easymc")
}

fn baseline_dir(game_dir: &Path) -> PathBuf {
    easymc_dir(game_dir).join("layer-baseline")
}

fn baseline_staging(game_dir: &Path) -> PathBuf {
    easymc_dir(game_dir).join("layer-baseline.next")
}

fn discard_staging(game_dir: &Path) -> PathBuf {
    easymc_dir(game_dir).join("layer-discard.next")
}

fn state_path(game_dir: &Path) -> PathBuf {
    easymc_dir(game_dir).join("layer-state.json")
}

fn read_state(instance_id: &str, game_dir: &Path) -> LayerState {
    let path = state_path(game_dir);
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(mut s) = serde_json::from_str::<LayerState>(&raw) {
            s.instance_id = instance_id.to_string();
            return s;
        }
    }
    LayerState {
        instance_id: instance_id.into(),
        baseline_at: None,
        applied_at: None,
        dirty: false,
        rev: 0,
        message_zh: "还没有可回退的干净状态".into(),
    }
}

fn write_state(game_dir: &Path, state: &LayerState) -> Result<(), String> {
    let dir = easymc_dir(game_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    // Write via temp + rename for state file atomicity.
    let tmp = dir.join("layer-state.json.tmp");
    fs::write(&tmp, &raw).map_err(|e| e.to_string())?;
    fs::rename(&tmp, state_path(game_dir)).map_err(|e| e.to_string())
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
        } else if ty.is_symlink() {
            // Materialize symlink targets as regular files/dirs so baseline is self-contained.
            let target = fs::read_link(ent.path()).map_err(|e| e.to_string())?;
            let resolved = if target.is_absolute() {
                target
            } else {
                ent.path()
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(target)
            };
            if resolved.is_dir() {
                copy_dir_recursive(&resolved, &to)?;
            } else if resolved.is_file() {
                if let Some(parent) = to.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::copy(&resolved, &to).map_err(|e| e.to_string())?;
            }
        } else if ty.is_file() {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(ent.path(), &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn remove_dir_contents(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    for ent in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let ent = ent.map_err(|e| e.to_string())?;
        let p = ent.path();
        if p.is_dir() {
            fs::remove_dir_all(&p).map_err(|e| e.to_string())?;
        } else {
            fs::remove_file(&p).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn replace_dir(src_staging: &Path, dest: &Path) -> Result<(), String> {
    if dest.exists() {
        let bak = dest.with_extension("easymc-bak");
        let _ = fs::remove_dir_all(&bak);
        fs::rename(dest, &bak).map_err(|e| e.to_string())?;
        match fs::rename(src_staging, dest) {
            Ok(()) => {
                let _ = fs::remove_dir_all(&bak);
                Ok(())
            }
            Err(e) => {
                // Roll back
                let _ = fs::rename(&bak, dest);
                Err(e.to_string())
            }
        }
    } else {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::rename(src_staging, dest).map_err(|e| e.to_string())
    }
}

fn load_container_instance<'a>(
    store: &'a easymc_core::InstanceStore,
    instance_id: &str,
) -> Result<&'a Instance, String> {
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| format!("找不到版本 {instance_id}"))?;
    if inst.container.is_none() {
        return Err("只有隔离版本才能应用或丢弃改动".into());
    }
    Ok(inst)
}

/// Mark the writable layer dirty (Agent wrote something).
pub fn mark_layer_dirty(paths: &Paths, instance_id: &str) -> CoreResult<LayerState> {
    let store = easymc_core::load_instances(paths)?;
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| easymc_core::EasyMcError::Missing(format!("instance {instance_id}")))?;
    let game = PathBuf::from(&inst.game_dir);
    let mut state = read_state(instance_id, &game);
    state.dirty = true;
    state.message_zh = "有未保存的隔离改动".into();
    let _ = write_state(&game, &state);
    Ok(state)
}

pub fn layer_state_for(paths: &Paths, instance_id: &str) -> CoreResult<LayerState> {
    let store = easymc_core::load_instances(paths)?;
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| easymc_core::EasyMcError::Missing(format!("instance {instance_id}")))?;
    Ok(read_state(instance_id, Path::new(&inst.game_dir)))
}

/// Snapshot LAYER_DIRS from `game` into baseline using staging + rename.
pub fn snapshot_layer_dirs(game: &Path) -> Result<(), String> {
    let staging = baseline_staging(game);
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    for name in LAYER_DIRS {
        let src = game.join(name);
        let dst = staging.join(name);
        // Always create the baseline slot so discard can restore empty dirs too.
        fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
        copy_dir_recursive(&src, &dst)?;
    }
    let final_base = baseline_dir(game);
    replace_dir(&staging, &final_base)?;
    Ok(())
}

/// Restore LAYER_DIRS from baseline into `game` using per-dir staging.
pub fn restore_layer_dirs(game: &Path) -> Result<(), String> {
    let base = baseline_dir(game);
    if !base.exists() {
        return Err("还没有可回退的干净状态。请先「应用」一次改动，或重新安装内容。".into());
    }
    let staging_root = discard_staging(game);
    let _ = fs::remove_dir_all(&staging_root);
    fs::create_dir_all(&staging_root).map_err(|e| e.to_string())?;

    for name in LAYER_DIRS {
        let src = base.join(name);
        let staged = staging_root.join(name);
        fs::create_dir_all(&staged).map_err(|e| e.to_string())?;
        copy_dir_recursive(&src, &staged)?;
    }
    // Swap each dir only after the full staging tree is ready.
    for name in LAYER_DIRS {
        let staged = staging_root.join(name);
        let live = game.join(name);
        replace_dir(&staged, &live)?;
    }
    let _ = fs::remove_dir_all(&staging_root);
    Ok(())
}

/// Apply = snapshot current LAYER_DIRS into baseline (commit semantics).
/// Does **not** touch `saves/` (propose-gated by default).
pub fn apply_game_layer(paths: &Paths, instance_id: &str) -> Result<LayerOpResult, String> {
    let store = easymc_core::load_instances(paths).map_err(|e| e.to_string())?;
    let inst = load_container_instance(&store, instance_id)?;
    let game = PathBuf::from(&inst.game_dir);
    snapshot_layer_dirs(&game)?;
    let now = Utc::now().to_rfc3339();
    let mut state = read_state(instance_id, &game);
    state.rev += 1;
    state.baseline_at = Some(now.clone());
    state.applied_at = Some(now);
    state.dirty = false;
    state.message_zh = "已应用改动（干净状态已更新；存档不动）".into();
    write_state(&game, &state)?;
    Ok(LayerOpResult {
        ok: true,
        message_zh: state.message_zh.clone(),
        state,
    })
}

/// Discard = restore baseline into LAYER_DIRS (rebuild writable game layer).
/// Never deletes `saves/` here — that requires an explicit propose/confirm.
pub fn discard_game_layer(paths: &Paths, instance_id: &str) -> Result<LayerOpResult, String> {
    let store = easymc_core::load_instances(paths).map_err(|e| e.to_string())?;
    let inst = load_container_instance(&store, instance_id)?;
    let game = PathBuf::from(&inst.game_dir);
    restore_layer_dirs(&game)?;
    let mut state = read_state(instance_id, &game);
    state.dirty = false;
    state.message_zh = "已丢弃改动，隔离层已恢复（存档未动）".into();
    write_state(&game, &state)?;
    Ok(LayerOpResult {
        ok: true,
        message_zh: state.message_zh.clone(),
        state,
    })
}

/// Ensure a baseline exists (first attach / first agent bind). Idempotent.
pub fn ensure_baseline(paths: &Paths, instance: &Instance) -> Result<LayerState, String> {
    let game = PathBuf::from(&instance.game_dir);
    let mut state = read_state(&instance.id, &game);
    if state.baseline_at.is_some() {
        return Ok(state);
    }
    let op = apply_game_layer(paths, &instance.id)?;
    state = op.state;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use easymc_core::{create_named_instance_with, CreateInstanceOpts, McVersion, ModLoader};
    use tempfile::tempdir;

    #[test]
    fn apply_discard_roundtrip() {
        let dir = tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure().unwrap();
        let mut inst = create_named_instance_with(
            &paths,
            CreateInstanceOpts {
                name: "c".into(),
                version: McVersion("1.21.1".into()),
                loader: ModLoader::Vanilla,
                loader_version: None,
                version_ready: false,
            },
        )
        .unwrap();
        inst.container = Some(easymc_core::ContainerMeta::new_podman("img"));
        easymc_core::upsert_instance(&paths, inst.clone()).unwrap();

        let game = PathBuf::from(&inst.game_dir);
        fs::create_dir_all(game.join("mods")).unwrap();
        fs::write(game.join("mods/a.jar"), b"v1").unwrap();
        apply_game_layer(&paths, &inst.id).unwrap();

        fs::write(game.join("mods/a.jar"), b"v2").unwrap();
        mark_layer_dirty(&paths, &inst.id).unwrap();
        discard_game_layer(&paths, &inst.id).unwrap();
        let body = fs::read(game.join("mods/a.jar")).unwrap();
        assert_eq!(body, b"v1");
        // saves untouched even if present
        fs::create_dir_all(game.join("saves/w")).unwrap();
        fs::write(game.join("saves/w/level.dat"), b"x").unwrap();
        discard_game_layer(&paths, &inst.id).unwrap();
        assert!(game.join("saves/w/level.dat").exists());
    }

    #[test]
    fn apply_uses_staging_keeps_prior_on_success() {
        let dir = tempdir().unwrap();
        let game = dir.path().join("g");
        fs::create_dir_all(game.join("mods")).unwrap();
        fs::write(game.join("mods/a.jar"), b"one").unwrap();
        snapshot_layer_dirs(&game).unwrap();
        assert!(baseline_dir(&game).join("mods/a.jar").exists());
        assert!(!baseline_staging(&game).exists());

        fs::write(game.join("mods/a.jar"), b"two").unwrap();
        snapshot_layer_dirs(&game).unwrap();
        let body = fs::read(baseline_dir(&game).join("mods/a.jar")).unwrap();
        assert_eq!(body, b"two");
        // discard staging leftovers cleaned
        assert!(!discard_staging(&game).exists());
    }

    #[test]
    fn restore_rebuilds_empty_mod_dir() {
        let dir = tempdir().unwrap();
        let game = dir.path().join("g");
        fs::create_dir_all(game.join("mods")).unwrap();
        fs::write(game.join("mods/keep.jar"), b"k").unwrap();
        snapshot_layer_dirs(&game).unwrap();
        fs::write(game.join("mods/extra.jar"), b"x").unwrap();
        restore_layer_dirs(&game).unwrap();
        assert!(game.join("mods/keep.jar").exists());
        assert!(!game.join("mods/extra.jar").exists());
    }
}
