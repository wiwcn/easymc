//! Shared Podman CLI helpers (CREATE_NO_WINDOW on Windows GUI).

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crate::detect::detect_podman;

pub fn podman_bin() -> Result<String, String> {
    detect_podman()
        .map(|p| p.path)
        .ok_or_else(|| "还没找到隔离组件，请先在设置里完成一键配置".into())
}

pub fn podman_cmd(bin: &str, args: &[&str]) -> Result<String, String> {
    podman_cmd_timeout(bin, args, None)
}

pub fn podman_cmd_timeout(
    bin: &str,
    args: &[&str],
    timeout: Option<Duration>,
) -> Result<String, String> {
    let mut cmd = Command::new(bin);
    cmd.args(args);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    // std::process::Command has no built-in timeout on all platforms without wait_timeout crate;
    // for long pulls we rely on caller progress. Optional soft deadline via thread+kill is heavy —
    // document timeout and ignore None.
    let _ = timeout;
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

pub fn container_name_for(instance_id: &str) -> String {
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

/// Look up `podman` for building a tokio::process::Command later.
pub fn podman_path_buf() -> Result<PathBuf, String> {
    Ok(PathBuf::from(podman_bin()?))
}

#[derive(Debug, Clone)]
pub struct WakeAttempt {
    pub tried: bool,
    pub ok: bool,
    pub message_zh: String,
}

/// Best-effort wake so runtime probe can succeed.
///
/// - Windows: `podman machine start` (acceptance = Machine + WSL2 + WSLg)
/// - Linux: `systemctl --user start podman.socket` then `podman info`
/// - Never downloads installers. Player messages avoid jargon.
pub fn try_wake_runtime(bin: &str) -> WakeAttempt {
    #[cfg(target_os = "windows")]
    {
        match podman_cmd(bin, &["machine", "start"]) {
            Ok(_) => WakeAttempt {
                tried: true,
                ok: true,
                message_zh: "运行环境已启动".into(),
            },
            Err(e) => match podman_cmd(bin, &["machine", "init", "--now"]) {
                Ok(_) => WakeAttempt {
                    tried: true,
                    ok: true,
                    message_zh: "已初始化并启动运行环境".into(),
                },
                Err(e2) => WakeAttempt {
                    tried: true,
                    ok: false,
                    message_zh: format!(
                        "启动运行环境失败（{e}；初始化：{e2}）。请确认官方安装包已启用并含图形支持。"
                    ),
                },
            },
        }
    }
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("systemctl")
            .args(["--user", "start", "podman.socket"])
            .output();
        let _ = Command::new("systemctl")
            .args(["--user", "start", "podman"])
            .output();
        match podman_cmd(bin, &["info"]) {
            Ok(_) => WakeAttempt {
                tried: true,
                ok: true,
                message_zh: "已唤醒隔离组件".into(),
            },
            Err(e) => WakeAttempt {
                tried: true,
                ok: false,
                message_zh: format!(
                    "隔离组件唤醒失败：{e}。请到设置里点「修复隔离环境」，或重新打开 EasyMC。"
                ),
            },
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = bin;
        WakeAttempt {
            tried: false,
            ok: false,
            message_zh: "当前系统不走隔离环境唤醒路径".into(),
        }
    }
}

