//! Create / delete / list / provision container-backed versions (Slices B + C).
//! Native instances stay untouched; container meta is optional on [`Instance`].

use easymc_core::{
    create_named_instance_with, load_instances, remove_instance, update_instance_fields,
    CreateInstanceOpts, Instance, McVersion, ModLoader, Paths, Result as CoreResult,
};
use serde::{Deserialize, Serialize};

use crate::detect::{detect_podman, probe_container_runtime, ProbeStatus};
use easymc_core::{ContainerMeta, ContainerVolume, CONTAINER_RUNTIME_PODMAN};
use crate::image::ensure_runtime_image;
use crate::layer::ensure_baseline;
use crate::mounts::default_volumes;
use crate::podman_util::{container_name_for, podman_cmd};
use crate::registries::{prefer_china_image_ref, DEFAULT_BASE_IMAGE, DEFAULT_BASE_IMAGE_MIRROR};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerVersionSummary {
    pub instance_id: String,
    pub name: String,
    pub version: String,
    pub loader: String,
    pub runtime: String,
    pub container_id: Option<String>,
    pub image: Option<String>,
    pub volumes: Vec<ContainerVolume>,
    pub provisioned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionResult {
    pub instance: Instance,
    pub image_pulled: bool,
    pub message_zh: String,
}

/// List instances that carry container metadata.
pub fn list_container_versions(paths: &Paths) -> CoreResult<Vec<ContainerVersionSummary>> {
    let store = load_instances(paths)?;
    Ok(store
        .instances
        .iter()
        .filter_map(|i| {
            let c = i.container.as_ref()?;
            Some(ContainerVersionSummary {
                instance_id: i.id.clone(),
                name: i.name.clone(),
                version: i.version.as_str().to_string(),
                loader: i.loader.as_str().to_string(),
                runtime: c.runtime.clone(),
                container_id: c.container_id.clone(),
                image: c.image.clone(),
                volumes: c.volumes.clone(),
                provisioned: c.provisioned,
            })
        })
        .collect())
}

fn create_podman_container(
    bin: &str,
    cname: &str,
    image: &str,
    volumes: &[ContainerVolume],
) -> Result<String, String> {
    let _ = podman_cmd(bin, &["rm", "-f", cname]);
    let mut args: Vec<String> = vec![
        "create".into(),
        "--name".into(),
        cname.to_string(),
        "--replace".into(),
        "-w".into(),
        "/data/.minecraft".into(),
    ];
    for v in volumes {
        args.push("-v".into());
        args.push(format!("{}:{}:{}", v.host, v.container, v.mode));
    }
    args.push(image.to_string());
    args.push("sleep".into());
    args.push("infinity".into());
    match podman_cmd(bin, &args.iter().map(|s| s.as_str()).collect::<Vec<_>>()) {
        Ok(id) => Ok(id),
        Err(_) => {
            // Retry without --replace for older podman.
            let mut args2: Vec<String> = vec![
                "create".into(),
                "--name".into(),
                cname.to_string(),
                "-w".into(),
                "/data/.minecraft".into(),
            ];
            for v in volumes {
                args2.push("-v".into());
                args2.push(format!("{}:{}:{}", v.host, v.container, v.mode));
            }
            args2.push(image.to_string());
            args2.push("sleep".into());
            args2.push("infinity".into());
            podman_cmd(bin, &args2.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        }
    }
}

/// Attach container metadata (+ best-effort `podman create`) to an existing instance.
pub fn attach_container_to_instance(
    paths: &Paths,
    instance_id: &str,
    prefer_china_mirror: bool,
) -> CoreResult<Instance> {
    let image = if prefer_china_mirror {
        DEFAULT_BASE_IMAGE_MIRROR.to_string()
    } else {
        prefer_china_image_ref(DEFAULT_BASE_IMAGE, false)
    };

    let inst_snap = {
        let store = load_instances(paths)?;
        store
            .instances
            .iter()
            .find(|i| i.id == instance_id)
            .cloned()
            .ok_or_else(|| easymc_core::EasyMcError::Missing(format!("instance {instance_id}")))?
    };

    let cname = container_name_for(instance_id);
    let volumes = default_volumes(paths, &inst_snap);

    let mut meta = ContainerMeta::new_podman(image.clone());
    meta.volumes = volumes.clone();
    meta.container_id = Some(cname.clone());

    let probe = probe_container_runtime();
    if matches!(probe.status, ProbeStatus::Ready) {
        if let Some(pod) = detect_podman() {
            // Best-effort pull so create has the image.
            let _ = ensure_runtime_image(prefer_china_mirror);
            match create_podman_container(&pod.path, &cname, &image, &volumes) {
                Ok(id) => {
                    meta.provisioned = true;
                    if !id.is_empty() && id.len() >= 12 {
                        // keep stable name in meta; id optional
                        meta.container_id = Some(cname.clone());
                    }
                }
                Err(e) => {
                    tracing::warn!("podman create failed: {e}");
                }
            }
        }
    }

    let inst = update_instance_fields(paths, instance_id, |inst| {
        inst.container = Some(meta.clone());
    })?;
    let _ = ensure_baseline(paths, &inst);
    Ok(inst)
}

/// Pull runtime image + recreate container with full volume set (game files live on host mounts).
///
/// MC jars/libraries/assets are installed into EasyMC data dirs (BMCLAPI-aware via
/// existing `install_version`); those dirs **are** the container volumes.
pub fn provision_container_version(
    paths: &Paths,
    instance_id: &str,
    prefer_china_mirror: bool,
) -> CoreResult<ProvisionResult> {
    let pull = ensure_runtime_image(prefer_china_mirror).map_err(easymc_core::EasyMcError::msg)?;
    let inst = attach_container_to_instance(paths, instance_id, prefer_china_mirror)?;
    Ok(ProvisionResult {
        instance: inst,
        image_pulled: pull.pulled,
        message_zh: format!(
            "{}；版本文件在宿主卷中，容器内直接可见。",
            pull.message_zh
        ),
    })
}

/// Create a new named instance and mark it as container-backed.
pub fn create_container_version(
    paths: &Paths,
    name: impl Into<String>,
    version: McVersion,
    loader: ModLoader,
    loader_version: Option<String>,
    prefer_china_mirror: bool,
) -> CoreResult<Instance> {
    let inst = create_named_instance_with(
        paths,
        CreateInstanceOpts {
            name: name.into(),
            version,
            loader,
            loader_version,
            version_ready: false,
        },
    )?;
    attach_container_to_instance(paths, &inst.id, prefer_china_mirror)
}

/// Delete a container-backed version: remove podman container (best-effort) + instance row.
pub fn delete_container_version(
    paths: &Paths,
    instance_id: &str,
    delete_files: bool,
) -> CoreResult<()> {
    let store = load_instances(paths)?;
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| easymc_core::EasyMcError::Missing(format!("instance {instance_id}")))?;
    if let Some(c) = &inst.container {
        if c.runtime.eq_ignore_ascii_case(CONTAINER_RUNTIME_PODMAN) {
            if let Some(pod) = detect_podman() {
                let name = c
                    .container_id
                    .clone()
                    .unwrap_or_else(|| container_name_for(instance_id));
                let _ = podman_cmd(&pod.path, &["rm", "-f", &name]);
            }
        }
    }
    let _ = update_instance_fields(paths, instance_id, |i| {
        i.container = None;
    });
    remove_instance(paths, instance_id, delete_files)?;
    Ok(())
}
