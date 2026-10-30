//! EasyMC container mode — Podman rootless detect/setup, China registries, version metadata.
//!
//! Native (non-container) instances keep `Instance.container = None` forever.
//! MIT-friendly reuse: registries.conf shape + public DaoCloud endpoints (not vendored Podman).

#![allow(dead_code)]

mod detect;
mod meta;
mod registries;
mod setup;
mod versions;

pub use detect::{detect_podman, probe_container_runtime, PodmanInfo, ProbeStatus, RuntimeProbe};
pub use meta::{ContainerMeta, ContainerVolume, CONTAINER_RUNTIME_PODMAN};
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
    list_container_versions, ContainerVersionSummary,
};
