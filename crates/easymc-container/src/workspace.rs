//! Agent workspace bind for container-backed versions (Slice D).

use std::path::{Path, PathBuf};

use easymc_core::{load_instances, load_settings, Instance, Paths, Result as CoreResult};
use serde::{Deserialize, Serialize};

use crate::layer::{ensure_baseline, layer_state_for, LayerState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceKind {
    Native,
    Container,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentWorkspace {
    pub instance_id: String,
    pub kind: WorkspaceKind,
    /// Absolute path Agent should treat as the writable root (`.minecraft`).
    pub root: String,
    pub container_id: Option<String>,
    pub image: Option<String>,
    /// When true, Agent must propose before mutating `saves/`.
    pub saves_require_propose: bool,
    pub layer: Option<LayerState>,
    pub message_zh: String,
}

fn settings_saves_gate(paths: &Paths) -> bool {
    load_settings(paths)
        .map(|s| s.container_saves_require_propose)
        .unwrap_or(true)
}

/// Resolve Agent workspace for an instance.
///
/// Container mode + `Instance.container` ⇒ workspace = game writable layer.
/// Otherwise ⇒ native game_dir (unchanged behavior).
pub fn resolve_agent_workspace(
    paths: &Paths,
    instance_id: &str,
) -> CoreResult<AgentWorkspace> {
    let store = load_instances(paths)?;
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| easymc_core::EasyMcError::Missing(format!("instance {instance_id}")))?;
    Ok(workspace_for_instance(paths, inst))
}

pub fn workspace_for_instance(paths: &Paths, inst: &Instance) -> AgentWorkspace {
    let saves_gate = settings_saves_gate(paths);
    match &inst.container {
        Some(c) => {
            let layer = ensure_baseline(paths, inst).ok().or_else(|| {
                layer_state_for(paths, &inst.id).ok()
            });
            AgentWorkspace {
                instance_id: inst.id.clone(),
                kind: WorkspaceKind::Container,
                root: inst.game_dir.clone(),
                container_id: c.container_id.clone(),
                image: c.image.clone(),
                saves_require_propose: saves_gate,
                layer,
                message_zh: "助手工作区已绑到这个隔离版本的可写层（游戏目录）".into(),
            }
        }
        None => AgentWorkspace {
            instance_id: inst.id.clone(),
            kind: WorkspaceKind::Native,
            root: inst.game_dir.clone(),
            container_id: None,
            image: None,
            saves_require_propose: false,
            layer: None,
            message_zh: "普通版本：助手工作区为该版本游戏目录".into(),
        },
    }
}

/// True when `path` is under the instance saves folder and propose is required.
pub fn path_is_gated_save(workspace: &AgentWorkspace, path: &Path) -> bool {
    if !workspace.saves_require_propose {
        return false;
    }
    let saves = PathBuf::from(&workspace.root).join("saves");
    path.starts_with(&saves)
}

/// Player-facing tip when Agent tries to touch saves under container mode.
pub fn saves_propose_message_zh() -> &'static str {
    "容器模式下修改存档需要先提出建议，经你确认后再写入（可在设置关闭「存档需确认」）。"
}

#[cfg(test)]
mod tests {
    use super::*;
    use easymc_core::{ContainerMeta, McVersion, ModLoader};

    #[test]
    fn native_workspace() {
        let inst = Instance {
            id: "a".into(),
            name: "a".into(),
            version: McVersion("1.21".into()),
            loader: ModLoader::Vanilla,
            loader_version: None,
            game_dir: "/tmp/g".into(),
            java_path: None,
            jvm_args: None,
            memory_mb: None,
            width: None,
            height: None,
            account_id: None,
            last_played: None,
            created_at: chrono::Utc::now(),
            version_ready: false,
            description: None,
            icon: None,
            category: None,
            container: None,
        };
        // Can't call workspace_for_instance without real paths settings — construct manually.
        let ws = AgentWorkspace {
            instance_id: inst.id.clone(),
            kind: WorkspaceKind::Native,
            root: inst.game_dir.clone(),
            container_id: None,
            image: None,
            saves_require_propose: false,
            layer: None,
            message_zh: String::new(),
        };
        assert!(!path_is_gated_save(
            &ws,
            Path::new("/tmp/g/saves/w")
        ));
        let mut cws = ws.clone();
        cws.saves_require_propose = true;
        cws.kind = WorkspaceKind::Container;
        cws.root = "/tmp/g".into();
        assert!(path_is_gated_save(&cws, Path::new("/tmp/g/saves/w")));
        assert!(!path_is_gated_save(&cws, Path::new("/tmp/g/mods/a.jar")));
        let _ = ContainerMeta::new_podman("x");
    }
}
