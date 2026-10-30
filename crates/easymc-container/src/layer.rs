//! Game writable-layer snapshot / apply / discard (Slice D).
//!
//! Semantics:
//! - **Apply**  = commit current layer as the new baseline (snapshot).
//! - **Discard** = restore baseline into the writable game dir (rebuild layer).
//! - **saves/** are excluded from automatic discard/apply by default — player must propose.

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
        message_zh: "尚未建立基线快照".into(),
    }
}

fn write_state(game_dir: &Path, state: &LayerState) -> Result<(), String> {
    let dir = easymc_dir(game_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    fs::write(state_path(game_dir), raw).map_err(|e| e.to_string())
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
    state.message_zh = "可写层有未应用的更改".into();
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

/// Apply = snapshot current LAYER_DIRS into baseline (commit semantics).
/// Does **not** touch `saves/` (propose-gated by default).
pub fn apply_game_layer(paths: &Paths, instance_id: &str) -> Result<LayerOpResult, String> {
    let store = easymc_core::load_instances(paths).map_err(|e| e.to_string())?;
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| format!("找不到版本 {instance_id}"))?;
    if inst.container.is_none() {
        return Err("仅容器版本支持应用/丢弃可写层".into());
    }
    let game = PathBuf::from(&inst.game_dir);
    let base = baseline_dir(&game);
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    for name in LAYER_DIRS {
        let src = game.join(name);
        let dst = base.join(name);
        let _ = fs::remove_dir_all(&dst);
        copy_dir_recursive(&src, &dst)?;
    }
    let now = Utc::now().to_rfc3339();
    let mut state = read_state(instance_id, &game);
    state.rev += 1;
    state.baseline_at = Some(now.clone());
    state.applied_at = Some(now);
    state.dirty = false;
    state.message_zh = "已应用更改（基线已更新；存档未纳入）".into();
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
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| format!("找不到版本 {instance_id}"))?;
    if inst.container.is_none() {
        return Err("仅容器版本支持应用/丢弃可写层".into());
    }
    let game = PathBuf::from(&inst.game_dir);
    let base = baseline_dir(&game);
    if !base.exists() {
        return Err("还没有基线快照。请先「应用更改」建立基线，或重新安装内容。".into());
    }
    for name in LAYER_DIRS {
        let src = base.join(name);
        let dst = game.join(name);
        if dst.exists() {
            remove_dir_contents(&dst)?;
        }
        copy_dir_recursive(&src, &dst)?;
    }
    let mut state = read_state(instance_id, &game);
    state.dirty = false;
    state.message_zh = "已丢弃更改，可写层已从基线重建（存档未动）".into();
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
}
