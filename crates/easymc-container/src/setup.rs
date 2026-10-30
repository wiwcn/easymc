//! One-click Podman setup hooks (player-facing). Does not require CloudAgent.
//!
//! Install scripts / guidance lean on official Podman docs + distro packages.
//! We do **not** shell-pipe remote installers blindly in GUI — we return a plan
//! the UI can run step-by-step with progress copy.

use serde::{Deserialize, Serialize};

use crate::detect::{detect_podman, probe_container_runtime, ProbeStatus, RuntimeProbe};
use crate::registries::{write_china_registries_config, ChinaRegistriesResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupAction {
    /// Open / show install URL or package command.
    InstallPodman,
    /// `podman machine init && start` (Windows / macOS machine path; Win = WSL2).
    InitMachine,
    /// Write China registries drop-in.
    WriteRegistries,
    /// Re-probe after changes.
    Reprobe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupPlan {
    pub actions: Vec<SetupStep>,
    pub probe: RuntimeProbe,
    pub summary_zh: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupStep {
    pub action: SetupAction,
    pub title_zh: String,
    pub detail_zh: String,
    /// Optional shell command the host may run (never auto-piped from the network).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Optional https URL (Podman Desktop / docs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupProgress {
    pub step: String,
    pub message_zh: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupResult {
    pub ok: bool,
    pub probe: RuntimeProbe,
    pub registries: Option<ChinaRegistriesResult>,
    pub messages_zh: Vec<String>,
    pub pending: Vec<SetupStep>,
}

/// Player-facing install guidance (no remote script execution).
pub fn install_guidance() -> SetupPlan {
    let probe = probe_container_runtime();
    let mut actions = Vec::new();

    match probe.status {
        ProbeStatus::Unsupported => {
            return SetupPlan {
                actions,
                summary_zh: probe.message_zh.clone(),
                probe,
            };
        }
        ProbeStatus::NotInstalled => {
            if cfg!(target_os = "windows") {
                actions.push(SetupStep {
                    action: SetupAction::InstallPodman,
                    title_zh: "安装 Podman Desktop".into(),
                    detail_zh: "Windows 图形界面只接受 Podman Machine（WSL2）+ WSLg。请安装官方 Podman Desktop，并启用 Machine。".into(),
                    command: None,
                    url: Some("https://podman-desktop.io/downloads".into()),
                });
                actions.push(SetupStep {
                    action: SetupAction::InitMachine,
                    title_zh: "初始化 Podman Machine".into(),
                    detail_zh: "在 Podman Desktop 里创建 Machine，或在终端运行下列命令（需已启用 WSL2）。".into(),
                    command: Some("podman machine init --now".into()),
                    url: Some("https://podman-desktop.io/docs/podman/creating-a-podman-machine".into()),
                });
            } else if cfg!(target_os = "linux") {
                actions.push(SetupStep {
                    action: SetupAction::InstallPodman,
                    title_zh: "安装 rootless Podman".into(),
                    detail_zh: "用发行版包装上 Podman（rootless）。装好后重新打开 EasyMC。".into(),
                    command: Some(linux_install_hint()),
                    url: Some("https://podman.io/docs/installation".into()),
                });
            } else {
                actions.push(SetupStep {
                    action: SetupAction::InstallPodman,
                    title_zh: "安装 Podman".into(),
                    detail_zh: probe.message_zh.clone(),
                    command: None,
                    url: Some("https://podman.io/docs/installation".into()),
                });
            }
        }
        ProbeStatus::InstalledNotReady => {
            if cfg!(target_os = "windows") {
                actions.push(SetupStep {
                    action: SetupAction::InitMachine,
                    title_zh: "启动 Podman Machine".into(),
                    detail_zh: "Machine 未就绪。请在 Podman Desktop 启动，或运行命令（WSL2 + WSLg）。".into(),
                    command: Some("podman machine start".into()),
                    url: None,
                });
            } else {
                actions.push(SetupStep {
                    action: SetupAction::InitMachine,
                    title_zh: "检查 Podman 服务".into(),
                    detail_zh: "二进制在，但连不上。可尝试 `podman info` 查看报错。".into(),
                    command: Some("podman info".into()),
                    url: None,
                });
            }
        }
        ProbeStatus::Ready => {}
    }

    actions.push(SetupStep {
        action: SetupAction::WriteRegistries,
        title_zh: "配置国内镜像加速".into(),
        detail_zh: "写入 DaoCloud 公共镜像前缀，拉取 docker.io / ghcr 等更快。".into(),
        command: None,
        url: Some("https://github.com/DaoCloud/public-image-mirror".into()),
    });

    actions.push(SetupStep {
        action: SetupAction::Reprobe,
        title_zh: "再次检测".into(),
        detail_zh: "装好或启动 Machine 后点此确认。".into(),
        command: None,
        url: None,
    });

    let summary_zh = match probe.status {
        ProbeStatus::Ready => "容器环境已就绪，只需确认国内镜像加速。".into(),
        ProbeStatus::NotInstalled => "还差安装 Podman，按步骤做完就能用容器版本。".into(),
        ProbeStatus::InstalledNotReady => "Podman 已装，还差启动 Machine / 服务。".into(),
        ProbeStatus::Unsupported => probe.message_zh.clone(),
    };

    SetupPlan {
        actions,
        probe,
        summary_zh,
    }
}

fn linux_install_hint() -> String {
    // Best-effort hints; UI shows as copy-paste, never auto-run with curl|sh.
    if std::path::Path::new("/etc/debian_version").exists() {
        "sudo apt-get update && sudo apt-get install -y podman".into()
    } else if std::path::Path::new("/etc/fedora-release").exists()
        || std::path::Path::new("/etc/redhat-release").exists()
    {
        "sudo dnf install -y podman".into()
    } else if std::path::Path::new("/etc/arch-release").exists() {
        "sudo pacman -S --noconfirm podman".into()
    } else {
        "podman  # 请用发行版包装安装 rootless Podman".into()
    }
}

/// Apply safe local steps: China registries + optional `podman machine start` when already installed.
/// Never downloads remote installers automatically.
pub fn apply_one_click_setup(prefer_china_mirrors: bool) -> SetupResult {
    let mut messages = Vec::new();
    let mut pending = Vec::new();
    let plan = install_guidance();

    let mut registries = None;
    if prefer_china_mirrors {
        match write_china_registries_config() {
            Ok(r) => {
                messages.push(r.message_zh.clone());
                registries = Some(r);
            }
            Err(e) => messages.push(format!("镜像加速配置失败：{e}")),
        }
    }

    // Try machine start when binary exists but not ready (Windows acceptance path).
    if matches!(
        plan.probe.status,
        ProbeStatus::InstalledNotReady | ProbeStatus::NotInstalled
    ) {
        if let Some(info) = detect_podman() {
            if !info.ready && cfg!(target_os = "windows") {
                let _ = std::process::Command::new(&info.path)
                    .args(["machine", "start"])
                    .output();
                messages.push("已尝试启动 Podman Machine".into());
            }
        }
    }

    let probe = probe_container_runtime();
    if !matches!(probe.status, ProbeStatus::Ready) {
        for step in plan.actions {
            if matches!(
                step.action,
                SetupAction::InstallPodman | SetupAction::InitMachine
            ) {
                pending.push(step);
            }
        }
    }

    let ok = matches!(probe.status, ProbeStatus::Ready)
        || (registries.as_ref().map(|r| r.wrote || !r.path.is_empty()).unwrap_or(false)
            && prefer_china_mirrors
            && matches!(probe.status, ProbeStatus::InstalledNotReady));

    // "ok" for partial: registries written even if still waiting on install — UI shows pending.
    let ok = matches!(probe.status, ProbeStatus::Ready) || (!pending.is_empty() && registries.is_some()) || ok;

    SetupResult {
        ok,
        probe,
        registries,
        messages_zh: messages,
        pending,
    }
}
