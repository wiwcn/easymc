//! Create / delete / list container-backed versions (Slice B).
//! Native instances stay untouched; container meta is optional on [`Instance`].

use std::process::Command;

use easymc_core::{
    create_named_instance_with, load_instances, remove_instance, update_instance_fields,
    CreateInstanceOpts, Instance, McVersion, ModLoader, Paths, Result as CoreResult,
};
use serde::{Deserialize, Serialize};

use crate::detect::{detect_podman, probe_container_runtime, ProbeStatus};
use easymc_core::{ContainerMeta, ContainerVolume, CONTAINER_RUNTIME_PODMAN};
use crate::registries::{
    prefer_china_image_ref, DEFAULT_BASE_IMAGE, DEFAULT_BASE_IMAGE_MIRROR,
};

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

fn container_name_for(instance_id: &str) -> String {
    // Podman name: [a-zA-Z0-9][a-zA-Z0-9_.-]*
    let slug: String = instance_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("easymc-{slug}")
}

fn podman_cmd(bin: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new(bin);
    cmd.args(args);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
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

    let game_dir = {
        let store = load_instances(paths)?;
        let inst = store
            .instances
            .iter()
            .find(|i| i.id == instance_id)
            .ok_or_else(|| easymc_core::EasyMcError::Missing(format!("instance {instance_id}")))?;
        inst.game_dir.clone()
    };

    let cname = container_name_for(instance_id);
    let volumes = vec![
        ContainerVolume::rw(&game_dir, "/data/.minecraft"),
        ContainerVolume::ro(
            paths.libraries_dir().to_string_lossy().as_ref(),
            "/data/libraries",
        ),
        ContainerVolume::ro(
            paths.assets_dir().to_string_lossy().as_ref(),
            "/data/assets",
        ),
    ];

    let mut meta = ContainerMeta::new_podman(image.clone());
    meta.volumes = volumes.clone();
    meta.container_id = Some(cname.clone());

    // Best-effort provision when Podman is ready; metadata is still saved if create fails
    // so the home badge and list work, and setup can retry later.
    let probe = probe_container_runtime();
    if matches!(probe.status, ProbeStatus::Ready) {
        if let Some(pod) = detect_podman() {
            let _ = podman_cmd(&pod.path, &["rm", "-f", &cname]);
            let mut args: Vec<String> = vec![
                "create".into(),
                "--name".into(),
                cname.clone(),
                "--replace".into(),
            ];
            for v in &volumes {
                args.push("-v".into());
                args.push(format!("{}:{}:{}", v.host, v.container, v.mode));
            }
            args.push(image);
            // `--replace` may be unsupported on older podman — retry without it.
            match podman_cmd(
                &pod.path,
                &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            ) {
                Ok(id) => {
                    meta.provisioned = true;
                    if !id.is_empty() {
                        meta.container_id = Some(id);
                    }
                }
                Err(_) => {
                    // Retry without --replace
                    let mut args2: Vec<String> = vec!["create".into(), "--name".into(), cname.clone()];
                    for v in &volumes {
                        args2.push("-v".into());
                        args2.push(format!("{}:{}:{}", v.host, v.container, v.mode));
                    }
                    args2.push(meta.image.clone().unwrap_or_else(|| DEFAULT_BASE_IMAGE.into()));
                    if let Ok(id) = podman_cmd(
                        &pod.path,
                        &args2.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                    ) {
                        meta.provisioned = true;
                        if !id.is_empty() {
                            meta.container_id = Some(id);
                        }
                    }
                }
            }
        }
    }

    update_instance_fields(paths, instance_id, |inst| {
        inst.container = Some(meta.clone());
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
    // Clear meta first so a partial failure doesn't leave a ghost badge expectation
    let _ = update_instance_fields(paths, instance_id, |i| {
        i.container = None;
    });
    remove_instance(paths, instance_id, delete_files)?;
    Ok(())
}
