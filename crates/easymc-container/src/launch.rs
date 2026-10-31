//! Launch Minecraft inside a Podman container (Slice C).
//!
//! Linux first (X11/Wayland + /dev/dri). Windows GUI acceptance path is
//! Podman Machine + WSL2 + WSLg — env/mounts are prepared; full GPU verify
//! is host-dependent and may remain blocked until WSLg is confirmed on device.

use std::path::PathBuf;

use easymc_core::{ContainerMeta, ContainerVolume, Instance, Paths};
use serde::{Deserialize, Serialize};

use crate::detect::{probe_container_runtime, ProbeStatus};
use crate::image::{ensure_runtime_image, resolve_runtime_image};
use crate::mounts::{default_volumes, remap_launch_args, CTR_MINECRAFT};
use crate::podman_util::{container_name_for, podman_bin, podman_cmd};
use crate::preheat::preheat_container_runtime;
use crate::registries::DEFAULT_BASE_IMAGE;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLaunchPlan {
    /// Absolute path to podman binary.
    pub podman: String,
    /// Full argv for `podman` (does not include the binary): `run … image java …`.
    pub argv: Vec<String>,
    pub container_name: String,
    pub image: String,
    pub workdir: String,
    pub message_zh: String,
    /// Platform notes (e.g. Win GUI pending).
    pub warnings_zh: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiMountHint {
    pub display: Option<String>,
    pub wayland: Option<String>,
    pub xdg_runtime_dir: Option<String>,
    pub xauthority: Option<String>,
    pub has_dri: bool,
    pub note_zh: String,
}

/// Probe display / GPU passthrough hints for the current host.
pub fn gui_mount_hints() -> GuiMountHint {
    let display = std::env::var("DISPLAY").ok().filter(|s| !s.is_empty());
    let wayland = std::env::var("WAYLAND_DISPLAY").ok().filter(|s| !s.is_empty());
    let xdg = std::env::var("XDG_RUNTIME_DIR").ok().filter(|s| !s.is_empty());
    let xauthority = std::env::var("XAUTHORITY")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            // Common default when DISPLAY is set but XAUTHORITY is unset.
            dirs::home_dir().map(|h| h.join(".Xauthority")).and_then(|p| {
                if p.is_file() {
                    Some(p.to_string_lossy().into_owned())
                } else {
                    None
                }
            })
        });
    let has_dri = std::path::Path::new("/dev/dri").exists();
    let note_zh = gui_note_zh(
        cfg!(target_os = "windows"),
        cfg!(target_os = "macos"),
        display.is_some(),
        wayland.is_some(),
        has_dri,
    );
    GuiMountHint {
        display,
        wayland,
        xdg_runtime_dir: xdg,
        xauthority,
        has_dri,
        note_zh,
    }
}

/// Player-facing GUI notes (Chinese). Keep jargon out of Windows copy.
pub fn gui_note_zh(
    is_windows: bool,
    is_macos: bool,
    has_display: bool,
    has_wayland: bool,
    has_dri: bool,
) -> String {
    if is_windows {
        return "Windows 开隔离环境需要官方安装包的运行环境（含图形支持）。请确认窗口能正常弹出后再开玩。".into();
    }
    if is_macos {
        return "Mac 不走隔离窗口路径，请用普通方式开玩。".into();
    }
    match (has_display, has_wayland, has_dri) {
        (true, _, true) | (_, true, true) => {
            "已检测到图形显示与显卡直通接口，隔离窗口会尽量用本机加速。".into()
        }
        (true, false, false) => {
            "已检测到 X11 显示，但没有显卡直通接口，可能会用软件渲染（更慢）。".into()
        }
        (false, true, false) => {
            "已检测到 Wayland 显示，但没有显卡直通接口，可能会用软件渲染（更慢）。".into()
        }
        (true, true, false) => {
            "已检测到图形显示，但没有显卡直通接口，可能会用软件渲染（更慢）。".into()
        }
        _ => "未检测到图形显示变量；无窗口环境只能做非界面验证。".into(),
    }
}

fn push_gui_args(argv: &mut Vec<String>, hints: &GuiMountHint) {
    let mut display_set = false;
    if let Some(d) = &hints.display {
        argv.push("-e".into());
        argv.push(format!("DISPLAY={d}"));
        display_set = true;
        // X11 socket — Linux host; on Podman Machine WSLg the socket lives inside the VM.
        if std::path::Path::new("/tmp/.X11-unix").exists() {
            argv.push("-v".into());
            argv.push("/tmp/.X11-unix:/tmp/.X11-unix:ro".into());
        }
    }
    if let Some(xa) = &hints.xauthority {
        if std::path::Path::new(xa).is_file() {
            argv.push("-e".into());
            argv.push(format!("XAUTHORITY={xa}"));
            argv.push("-v".into());
            argv.push(format!("{xa}:{xa}:ro"));
        }
    }
    if let Some(w) = &hints.wayland {
        argv.push("-e".into());
        argv.push(format!("WAYLAND_DISPLAY={w}"));
    }
    if let Some(xdg) = &hints.xdg_runtime_dir {
        argv.push("-e".into());
        argv.push(format!("XDG_RUNTIME_DIR={xdg}"));
        if std::path::Path::new(xdg).is_dir() {
            argv.push("-v".into());
            argv.push(format!("{xdg}:{xdg}"));
        }
        // PulseAudio native socket (best-effort)
        let pulse = format!("{xdg}/pulse/native");
        if std::path::Path::new(&pulse).exists() {
            argv.push("-e".into());
            argv.push(format!("PULSE_SERVER=unix:{pulse}"));
        }
    }
    if hints.has_dri {
        argv.push("--device".into());
        argv.push("/dev/dri".into());
    }
    // Explicit PULSE_SERVER from host env wins / supplements.
    if let Ok(pulse) = std::env::var("PULSE_SERVER") {
        if !pulse.is_empty() {
            argv.push("-e".into());
            argv.push(format!("PULSE_SERVER={pulse}"));
        }
    }
    if cfg!(target_os = "windows") && !display_set {
        // Design hooks for Podman Machine + WSLg acceptance path.
        // Only set when host did not already provide DISPLAY.
        argv.push("-e".into());
        argv.push("DISPLAY=:0".into());
    }
}

fn stable_container_name(meta: &ContainerMeta, instance_id: &str) -> String {
    let cname = meta
        .container_id
        .clone()
        .filter(|s| s.starts_with("easymc-") || !s.is_empty())
        .unwrap_or_else(|| container_name_for(instance_id));
    // Prefer stable name over raw id from older creates
    if cname.len() == 64 && cname.chars().all(|c| c.is_ascii_hexdigit()) {
        container_name_for(instance_id)
    } else {
        cname
    }
}

/// Pure argv assembly (no Podman probe). Used by `plan_container_launch` and unit tests.
pub fn assemble_container_run_argv(
    image: &str,
    container_name: &str,
    volumes: &[ContainerVolume],
    java_args_host: &[String],
    hints: &GuiMountHint,
) -> Vec<String> {
    let remapped = remap_launch_args(java_args_host, volumes);
    let mut argv: Vec<String> = vec![
        "run".into(),
        "--rm".into(),
        "--name".into(),
        container_name.to_string(),
        "--replace".into(),
        "-w".into(),
        CTR_MINECRAFT.into(),
        "--network".into(),
        "host".into(),
        "--ipc".into(),
        "host".into(),
    ];
    push_gui_args(&mut argv, hints);
    for v in volumes {
        argv.push("-v".into());
        argv.push(format!("{}:{}:{}", v.host, v.container, v.mode));
    }
    argv.push(image.to_string());
    argv.push("java".into());
    argv.extend(remapped);
    argv
}

/// Build a `podman run --rm … <image> java <args…>` plan.
///
/// `java_args` must already use **host** paths; they are remapped to `/data/…`.
pub fn plan_container_launch(
    paths: &Paths,
    instance: &Instance,
    java_args_host: &[String],
    prefer_china: bool,
    preheat: bool,
) -> Result<ContainerLaunchPlan, String> {
    let meta = instance
        .container
        .as_ref()
        .ok_or_else(|| "该版本未启用容器（Instance.container 为空）".to_string())?;
    if !meta.is_podman() {
        return Err(format!("不支持的容器运行时：{}", meta.runtime));
    }

    let mut warnings = Vec::new();
    let hints = gui_mount_hints();
    warnings.push(hints.note_zh.clone());

    if preheat {
        let ph = preheat_container_runtime(prefer_china, true);
        warnings.extend(ph.messages_zh);
        if !ph.ok {
            return Err("隔离环境提前准备失败，暂时无法启动".into());
        }
    } else {
        let probe = probe_container_runtime();
        if !matches!(probe.status, ProbeStatus::Ready) {
            return Err(format!("容器未就绪：{}", probe.message_zh));
        }
        let _ = ensure_runtime_image(prefer_china)?;
    }

    let image = meta
        .image
        .clone()
        .unwrap_or_else(|| resolve_runtime_image(prefer_china));
    let volumes = if meta.volumes.is_empty() {
        default_volumes(paths, instance)
    } else {
        meta.volumes.clone()
    };
    let cname = stable_container_name(meta, &instance.id);
    let bin = podman_bin()?;
    let argv = assemble_container_run_argv(&image, &cname, &volumes, java_args_host, &hints);

    // Older podman may not support --replace on run; caller can strip on retry.
    Ok(ContainerLaunchPlan {
        podman: bin,
        argv,
        container_name: cname,
        image,
        workdir: CTR_MINECRAFT.into(),
        message_zh: "已生成隔离环境启动计划".into(),
        warnings_zh: warnings,
    })
}

/// Ensure container metadata volumes are complete + image pulled; update instance meta.
pub fn provision_for_launch(
    paths: &Paths,
    instance: &Instance,
    prefer_china: bool,
) -> Result<ContainerMeta, String> {
    let pull = ensure_runtime_image(prefer_china)?;
    let image = pull.image;
    let volumes = default_volumes(paths, instance);
    let cname = container_name_for(&instance.id);
    let bin = podman_bin()?;

    // Best-effort create stopped container so `podman start` warm path works later.
    let _ = podman_cmd(&bin, &["rm", "-f", &cname]);
    let mut args: Vec<String> = vec![
        "create".into(),
        "--name".into(),
        cname.clone(),
        "-w".into(),
        CTR_MINECRAFT.into(),
    ];
    for v in &volumes {
        args.push("-v".into());
        args.push(format!("{}:{}:{}", v.host, v.container, v.mode));
    }
    args.push(image.clone());
    args.push("sleep".into());
    args.push("infinity".into());
    let provisioned = podman_cmd(
        &bin,
        &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    )
    .is_ok();

    Ok(ContainerMeta {
        runtime: "podman".into(),
        container_id: Some(cname),
        image: Some(image),
        volumes,
        provisioned,
    })
}

/// Strip `--replace` from a plan (older Podman compatibility).
pub fn plan_without_replace(plan: &ContainerLaunchPlan) -> ContainerLaunchPlan {
    let mut p = plan.clone();
    p.argv.retain(|a| a != "--replace");
    p
}

/// Convenience: podman binary + argv as PathBuf / Vec for ProcessManager::spawn.
pub fn plan_to_spawn(plan: &ContainerLaunchPlan) -> (PathBuf, Vec<String>) {
    (PathBuf::from(&plan.podman), plan.argv.clone())
}

/// Fallback image constant for tests / docs.
pub fn default_image() -> &'static str {
    DEFAULT_BASE_IMAGE
}

#[cfg(test)]
mod tests {
    use super::*;
    use easymc_core::{ContainerVolume, McVersion, ModLoader};
    use crate::mounts::CTR_LIBRARIES;

    #[test]
    fn gui_hints_have_note() {
        let h = gui_mount_hints();
        assert!(!h.note_zh.is_empty());
    }

    #[test]
    fn gui_note_x11_vs_wayland() {
        let x11 = gui_note_zh(false, false, true, false, false);
        assert!(x11.contains("X11") || x11.contains("软件渲染"));
        let wl = gui_note_zh(false, false, false, true, false);
        assert!(wl.contains("Wayland") || wl.contains("软件渲染"));
        let win = gui_note_zh(true, false, false, false, false);
        assert!(win.contains("Windows") || win.contains("官方安装包"));
        assert!(!win.to_lowercase().contains("wsl"));
        assert!(!win.to_lowercase().contains("podman"));
    }

    #[test]
    fn plan_without_replace_strips() {
        let plan = ContainerLaunchPlan {
            podman: "podman".into(),
            argv: vec!["run".into(), "--replace".into(), "img".into()],
            container_name: "easymc-x".into(),
            image: "img".into(),
            workdir: CTR_MINECRAFT.into(),
            message_zh: String::new(),
            warnings_zh: vec![],
        };
        let p2 = plan_without_replace(&plan);
        assert!(!p2.argv.iter().any(|a| a == "--replace"));
    }

    #[test]
    fn assemble_remaps_classpath_and_volumes() {
        let vols = vec![
            ContainerVolume::rw("/game", CTR_MINECRAFT),
            ContainerVolume::ro("/libs", CTR_LIBRARIES),
        ];
        let hints = GuiMountHint {
            display: None,
            wayland: None,
            xdg_runtime_dir: None,
            xauthority: None,
            has_dri: false,
            note_zh: "test".into(),
        };
        let argv = assemble_container_run_argv(
            "img:21",
            "easymc-t",
            &vols,
            &[
                "-cp".into(),
                "/libs/a.jar:/libs/b.jar".into(),
                "-Djava.library.path=/libs/n".into(),
                "net.minecraft.client.main.Main".into(),
                "--gameDir".into(),
                "/game".into(),
            ],
            &hints,
        );
        assert!(argv.iter().any(|a| a == "run"));
        assert!(argv.iter().any(|a| a.starts_with("/game:")));
        assert!(argv.iter().any(|a| a == "java"));
        let cp_idx = argv.iter().position(|a| a == "-cp").expect("cp");
        assert_eq!(
            argv[cp_idx + 1],
            "/data/libraries/a.jar:/data/libraries/b.jar"
        );
        assert!(argv
            .iter()
            .any(|a| a == "-Djava.library.path=/data/libraries/n"));
        assert!(argv.iter().any(|a| a == CTR_MINECRAFT));
        let _ = McVersion("1.21.1".into());
        let _ = ModLoader::Vanilla;
    }
}
