//! Shared Podman CLI helpers (CREATE_NO_WINDOW on Windows GUI).

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crate::detect::detect_podman;

pub fn podman_bin() -> Result<String, String> {
    detect_podman()
        .map(|p| p.path)
        .ok_or_else(|| "未找到 Podman，请先完成容器一键配置".into())
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
