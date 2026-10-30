//! EasyMC container mode — Podman rootless detect/setup, China registries,
//! version metadata, launch path (Slice C), and Agent bind / layer (Slice D).
//!
//! Native (non-container) instances keep `Instance.container = None` forever.
//! MIT-friendly reuse: registries.conf shape + public DaoCloud endpoints (not vendored Podman).

#![allow(dead_code)]

mod detect;
mod image;
mod launch;
mod layer;
mod meta;
mod mounts;
mod podman_util;
mod preheat;
mod registries;
mod setup;
mod versions;
mod workspace;

pub use detect::{detect_podman, probe_container_runtime, PodmanInfo, ProbeStatus, RuntimeProbe};
pub use image::{
    ensure_runtime_image, resolve_runtime_image, runtime_image_ready, try_pull_runtime_image,
    ImagePullResult,
};
pub use launch::{
    default_image, gui_mount_hints, plan_container_launch, plan_to_spawn, plan_without_replace,
    provision_for_launch, ContainerLaunchPlan, GuiMountHint,
};
pub use layer::{
    apply_game_layer, discard_game_layer, ensure_baseline, layer_state_for, mark_layer_dirty,
    LayerOpResult, LayerState, LAYER_DIRS,
};
pub use meta::{ContainerMeta, ContainerVolume, CONTAINER_RUNTIME_PODMAN};
pub use mounts::{
    default_volumes, remap_host_path, remap_launch_args, CTR_ASSETS, CTR_LIBRARIES, CTR_MINECRAFT,
    CTR_NATIVES, CTR_VERSIONS,
};
pub use preheat::{preheat_container_runtime, PreheatResult};
pub use registries::{
    china_friendly_registries_toml, prefer_china_image_ref, registries_conf_path,
    write_china_registries_config, ChinaRegistriesResult, DEFAULT_BASE_IMAGE,
    DEFAULT_BASE_IMAGE_MIRROR,
};
pub use setup::{
    apply_one_click_setup, install_guidance, SetupAction, SetupPlan, SetupProgress, SetupResult,
    SetupStep,
};
pub use versions::{
    attach_container_to_instance, create_container_version, delete_container_version,
    list_container_versions, provision_container_version, ContainerVersionSummary, ProvisionResult,
};
pub use workspace::{
    path_is_gated_save, resolve_agent_workspace, saves_propose_message_zh, workspace_for_instance,
    AgentWorkspace, WorkspaceKind,
};
