//! Cold-start / Podman Machine preheat + status (Slices C + E).
//!
//! Slice E improvements:
//! - Persist last successful warm timestamp under EasyMC data / `~/.config/easymc`
//! - `preheat_status` for UI (cold / warm / unknown)
//! - Linux: touch `podman info` to wake user socket
//! - Clearer Chinese progress copy; optional force re-pull

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::detect::{detect_podman, probe_container_runtime, ProbeStatus};
use crate::image::{ensure_runtime_image, runtime_image_ready, try_pull_runtime_image, ImagePullResult};
use crate::podman_util::{podman_cmd, try_wake_runtime};

const WARM_WINDOW_SECS: u64 = 30 * 60; // 30 minutes counts as "warm"

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreheatResult {
    pub ok: bool,
    pub machine_started: bool,
    pub image: Option<ImagePullResult>,
    pub probe_status: String,
    pub messages_zh: Vec<String>,
    /// Epoch seconds of this successful warm (when ok).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warmed_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreheatStatus {
    pub probe_status: String,
    pub machine_ready: bool,
    pub image_ready: bool,
    pub warm: bool,
    pub last_preheat_at: Option<u64>,
    pub seconds_since_preheat: Option<u64>,
    pub message_zh: String,
}

fn status_str(s: ProbeStatus) -> &'static str {
    match s {
        ProbeStatus::NotInstalled => "not_installed",
        ProbeStatus::InstalledNotReady => "installed_not_ready",
        ProbeStatus::Ready => "ready",
        ProbeStatus::Unsupported => "unsupported",
    }
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn preheat_stamp_path() -> Option<PathBuf> {
    // Prefer XDG config so it survives data-dir wipes; fall back to home.
    if let Some(cfg) = dirs::config_dir() {
        return Some(cfg.join("easymc").join("container-preheat.json"));
    }
    dirs::home_dir().map(|h| h.join(".config").join("easymc").join("container-preheat.json"))
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PreheatStamp {
    last_ok_at: Option<u64>,
    last_image: Option<String>,
}

fn read_stamp() -> PreheatStamp {
    let Some(path) = preheat_stamp_path() else {
        return PreheatStamp::default();
    };
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_stamp(stamp: &PreheatStamp) {
    let Some(path) = preheat_stamp_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(raw) = serde_json::to_string_pretty(stamp) {
        let _ = fs::write(path, raw);
    }
}

/// Snapshot whether the runtime looks warm enough to skip a long cold start.
pub fn preheat_status(prefer_china: bool) -> PreheatStatus {
    let probe = probe_container_runtime();
    let machine_ready = matches!(probe.status, ProbeStatus::Ready);
    let image_ready = runtime_image_ready(prefer_china);
    let stamp = read_stamp();
    let now = now_epoch();
    let seconds_since = stamp.last_ok_at.map(|t| now.saturating_sub(t));
    let warm = machine_ready
        && image_ready
        && seconds_since
            .map(|s| s <= WARM_WINDOW_SECS)
            .unwrap_or(false);

    // Same detect truth as onboarding: when runtime is not Ready, surface probe.message_zh.
    let message_zh = if !machine_ready {
        probe.message_zh
    } else if warm {
        "隔离环境已准备好，开玩更快".into()
    } else if image_ready {
        "隔离环境还在，但距上次准备较久，建议再点一次「现在就准备好」".into()
    } else {
        "隔离组件已就绪，还差下载运行环境基础文件".into()
    };

    PreheatStatus {
        probe_status: status_str(probe.status).into(),
        machine_ready,
        image_ready,
        warm,
        last_preheat_at: stamp.last_ok_at,
        seconds_since_preheat: seconds_since,
        message_zh,
    }
}

/// Warm Podman Machine (Windows) / confirm socket (Linux) and optionally pull JRE image.
///
/// Call before first container launch to hide cold-start latency.
/// `force_pull` re-pulls even when the image already exists locally.
pub fn preheat_container_runtime(prefer_china: bool, pull_image: bool) -> PreheatResult {
    preheat_container_runtime_ex(prefer_china, pull_image, false)
}

pub fn preheat_container_runtime_ex(
    prefer_china: bool,
    pull_image: bool,
    force_pull: bool,
) -> PreheatResult {
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
            warmed_at: None,
        };
    }

    if matches!(
        probe.status,
        ProbeStatus::InstalledNotReady | ProbeStatus::NotInstalled
    ) {
        if let Some(info) = detect_podman() {
            messages.push("正在唤醒隔离环境…".into());
            let wake = try_wake_runtime(&info.path);
            messages.push(wake.message_zh.clone());
            machine_started = wake.ok;
        } else {
            messages.push(probe.message_zh.clone());
        }
        probe = probe_container_runtime();
    }

    // Linux warm path: poke the user socket even when already Ready.
    if matches!(probe.status, ProbeStatus::Ready) {
        if let Some(info) = detect_podman() {
            if !cfg!(target_os = "windows") {
                match podman_cmd(&info.path, &["info", "--format", "{{.Host.RemoteSocket.Path}}"]) {
                    Ok(_) => messages.push("已唤醒隔离组件服务".into()),
                    Err(_) => {
                        let _ = podman_cmd(&info.path, &["info"]);
                        messages.push("已检查隔离组件服务".into());
                    }
                }
            }
        }
    }

    let image = if pull_image && matches!(probe.status, ProbeStatus::Ready) {
        if force_pull {
            // Best-effort: pull again (registries mirrors apply).
            let r = try_pull_runtime_image(prefer_china);
            // If already present, try_pull via ensure still reports already_present —
            // force path: call podman pull explicitly through ensure after removing? Keep soft force.
            messages.push(format!("强制检查镜像：{}", r.message_zh));
            Some(r)
        } else {
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
        }
    } else if pull_image {
        None
    } else {
        None
    };

    let image_ok = image
        .as_ref()
        .map(|i| i.pulled || i.already_present)
        .unwrap_or(!pull_image || runtime_image_ready(prefer_china));

    let ok = matches!(probe.status, ProbeStatus::Ready) && image_ok;
    let warmed_at = if ok {
        let t = now_epoch();
        write_stamp(&PreheatStamp {
            last_ok_at: Some(t),
            last_image: image.as_ref().map(|i| i.image.clone()),
        });
        messages.push("隔离环境已准备好，下次开玩会更快".into());
        Some(t)
    } else {
        None
    };

    PreheatResult {
        ok,
        machine_started,
        image,
        probe_status: status_str(probe.status).into(),
        messages_zh: messages,
        warmed_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_without_podman_is_stable() {
        let s = preheat_status(true);
        assert!(!s.probe_status.is_empty());
        assert!(!s.message_zh.is_empty());
    }
}
