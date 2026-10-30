//! Cold-start / Podman Machine preheat (Slice C).

use serde::{Deserialize, Serialize};

use crate::detect::{detect_podman, probe_container_runtime, ProbeStatus};
use crate::image::{ensure_runtime_image, ImagePullResult};
use crate::podman_util::podman_cmd;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreheatResult {
    pub ok: bool,
    pub machine_started: bool,
    pub image: Option<ImagePullResult>,
    pub probe_status: String,
    pub messages_zh: Vec<String>,
}

fn status_str(s: ProbeStatus) -> &'static str {
    match s {
        ProbeStatus::NotInstalled => "not_installed",
        ProbeStatus::InstalledNotReady => "installed_not_ready",
        ProbeStatus::Ready => "ready",
        ProbeStatus::Unsupported => "unsupported",
    }
}

/// Warm Podman Machine (Windows) / confirm socket (Linux) and optionally pull JRE image.
///
/// Call before first container launch to hide cold-start latency.
pub fn preheat_container_runtime(prefer_china: bool, pull_image: bool) -> PreheatResult {
    let mut messages = Vec::new();
    let mut machine_started = false;

    let mut probe = probe_container_runtime();
    if matches!(probe.status, ProbeStatus::Unsupported) {
        return PreheatResult {
            ok: false,
            machine_started: false,
            image: None,
            probe_status: status_str(probe.status).into(),
            messages_zh: vec![probe.message_zh],
        };
    }

    if matches!(
        probe.status,
        ProbeStatus::InstalledNotReady | ProbeStatus::NotInstalled
    ) {
        if let Some(info) = detect_podman() {
            if cfg!(target_os = "windows") {
                messages.push("正在启动 Podman Machine（冷启动可能需要几十秒）…".into());
                match podman_cmd(&info.path, &["machine", "start"]) {
                    Ok(_) => {
                        machine_started = true;
                        messages.push("Podman Machine 已启动".into());
                    }
                    Err(e) => {
                        // init --now if no machine yet
                        let _ = podman_cmd(&info.path, &["machine", "init", "--now"]);
                        machine_started = true;
                        messages.push(format!("已尝试初始化/启动 Machine：{e}"));
                    }
                }
            } else {
                messages.push("Podman 已安装但未就绪，请检查 rootless 用户命名空间".into());
            }
        } else {
            messages.push("尚未安装 Podman".into());
        }
        probe = probe_container_runtime();
    }

    let image = if pull_image && matches!(probe.status, ProbeStatus::Ready) {
        match ensure_runtime_image(prefer_china) {
            Ok(r) => {
                messages.push(r.message_zh.clone());
                Some(r)
            }
            Err(e) => {
                messages.push(e);
                None
            }
        }
    } else {
        None
    };

    let ok = matches!(probe.status, ProbeStatus::Ready);
    if ok {
        messages.push("容器运行时已预热".into());
    }

    PreheatResult {
        ok,
        machine_started,
        image,
        probe_status: status_str(probe.status).into(),
        messages_zh: messages,
    }
}
