use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};

/// Where Podman was found (if at all).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PodmanInfo {
    pub path: String,
    pub version: String,
    /// `true` when `podman info` succeeds (daemon/machine usable).
    pub ready: bool,
    /// Extra note for UI (machine needed, WSL, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_zh: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStatus {
    /// `podman` binary not on PATH / not installed.
    NotInstalled,
    /// Binary present but machine/socket not ready (common on Windows before Machine init).
    InstalledNotReady,
    /// Ready to create/list containers.
    Ready,
    /// Platform cannot use Podman GUI path (e.g. macOS has no WSLg acceptance path).
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeProbe {
    pub status: ProbeStatus,
    pub podman: Option<PodmanInfo>,
    /// Player-facing Chinese summary.
    pub message_zh: String,
    /// Platform hint: `linux` | `windows` | `macos` | `other`.
    pub platform: String,
}

fn platform_id() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "other"
    }
}

fn podman_bin_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "podman.exe"
    } else {
        "podman"
    }
}

fn look_path(bin: &str) -> Option<PathBuf> {
    if let Ok(p) = which_like(bin) {
        return Some(p);
    }
    // Common Windows / machine install locations (best-effort, no panic).
    #[cfg(target_os = "windows")]
    {
        let candidates = [
            r"C:\Program Files\RedHat\Podman\podman.exe",
            r"C:\Program Files\Podman\podman.exe",
        ];
        for c in candidates {
            let p = PathBuf::from(c);
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

/// Minimal PATH search (avoid extra deps). Mirrors common MIT `which` crate behavior.
fn which_like(bin: &str) -> Result<PathBuf, ()> {
    let path = std::env::var_os("PATH").ok_or(())?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Ok(candidate);
        }
        #[cfg(target_os = "windows")]
        {
            let with_exe = dir.join(format!("{bin}.exe"));
            if with_exe.is_file() {
                return Ok(with_exe);
            }
        }
    }
    Err(())
}

fn run_podman(bin: &PathBuf, args: &[&str]) -> Result<String, String> {
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
        let stdout = String::from_utf8_lossy(&out.stdout);
        return Err(if !err.trim().is_empty() {
            err.trim().to_string()
        } else {
            stdout.trim().to_string()
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Detect Podman binary + version string (does not require a running machine).
pub fn detect_podman() -> Option<PodmanInfo> {
    let path = look_path(podman_bin_name()).or_else(|| look_path("podman"))?;
    let version = run_podman(&path, &["--version"])
        .unwrap_or_else(|_| "podman (version unknown)".into());
    let ready = run_podman(&path, &["info", "--format", "{{.Host.RemoteSocket.Exists}}"]).is_ok()
        || run_podman(&path, &["info"]).is_ok();
    let detail_zh = if ready {
        None
    } else if cfg!(target_os = "windows") {
        Some("隔离组件已装好，但运行环境还没就绪。Windows 请用官方安装包启用运行环境（含图形支持）。".into())
    } else {
        Some("已找到隔离组件，但暂时连不上。请重新打开 EasyMC，或到设置里点「修复隔离环境」。".into())
    };
    Some(PodmanInfo {
        path: path.to_string_lossy().into_owned(),
        version,
        ready,
        detail_zh,
    })
}

/// Full probe used by onboarding + settings.
pub fn probe_container_runtime() -> RuntimeProbe {
    let platform = platform_id().to_string();

    // Approved Win GUI acceptance path is Podman Machine WSL2 + WSLg only.
    // macOS: no accepted GUI container path in the 2026-10-30 decisions — Agent without
    // container remains long-term; probe reports Unsupported so UI can skip gracefully.
    if cfg!(target_os = "macos") {
        return RuntimeProbe {
            status: ProbeStatus::Unsupported,
            podman: detect_podman(),
            message_zh: "当前 macOS 不走隔离环境（可继续用助手 / 标准模式直接开玩）。".into(),
            platform,
        };
    }

    match detect_podman() {
        None => RuntimeProbe {
            status: ProbeStatus::NotInstalled,
            podman: None,
            message_zh: if cfg!(target_os = "windows") {
                "还没装隔离组件。一键配置会引导安装官方安装包并启用运行环境（含图形支持）。".into()
            } else {
                "还没装隔离组件。一键配置会用系统软件源或官方脚本安装。".into()
            },
            platform,
        },
        Some(info) if info.ready => RuntimeProbe {
            status: ProbeStatus::Ready,
            message_zh: format!("隔离环境已就绪（{}）", info.version),
            podman: Some(info),
            platform,
        },
        Some(info) => RuntimeProbe {
            status: ProbeStatus::InstalledNotReady,
            message_zh: info
                .detail_zh
                .clone()
                .unwrap_or_else(|| "隔离组件已装好，但还没准备好".into()),
            podman: Some(info),
            platform,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_returns_platform() {
        let p = probe_container_runtime();
        assert!(!p.platform.is_empty());
        assert!(!p.message_zh.is_empty());
    }
}
