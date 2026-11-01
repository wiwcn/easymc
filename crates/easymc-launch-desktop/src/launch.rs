use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use easymc_core::{Account, AccountKind, EasyMcError, Instance, ModLoader, Paths, QuickPlay, Result, Settings};
use easymc_meta::rules_allow;
use easymc_meta::{
    classpath_jars, extract_natives, fabric_profile_id, forge_profile_id, is_version_installed,
    load_installed_version, neoforge_profile_id, quilt_profile_id, VersionInfo,
};
use serde_json::Value;

use crate::java::{ensure_java, select_java, JavaInstall};
use crate::process::{LaunchStatus, ProcessManager};

#[derive(Debug, Clone)]
pub struct LaunchRequest {
    pub extra_jvm_args: Vec<String>,
    pub memory_mb: u32,
}

impl Default for LaunchRequest {
    fn default() -> Self {
        Self {
            extra_jvm_args: vec![],
            memory_mb: 2048,
        }
    }
}

/// Resolve the version JSON id to launch for this instance (vanilla or loader profile).
pub fn resolve_launch_version(paths: &Paths, instance: &Instance) -> Result<VersionInfo> {
    let id = match instance.loader {
        ModLoader::Vanilla => instance.version.0.clone(),
        ModLoader::Fabric => {
            let loader_ver = instance.loader_version.as_deref().ok_or_else(|| {
                EasyMcError::msg("Fabric 实例缺少 loader_version，请重新安装 Fabric")
            })?;
            fabric_profile_id(instance.version.as_str(), loader_ver)
        }
        ModLoader::Quilt => {
            let loader_ver = instance.loader_version.as_deref().ok_or_else(|| {
                EasyMcError::msg("Quilt 实例缺少 loader_version，请重新安装 Quilt")
            })?;
            quilt_profile_id(instance.version.as_str(), loader_ver)
        }
        ModLoader::Forge => {
            let loader_ver = instance.loader_version.as_deref().ok_or_else(|| {
                EasyMcError::msg("Forge 实例缺少 loader_version，请重新安装 Forge")
            })?;
            forge_profile_id(instance.version.as_str(), loader_ver)
        }
        ModLoader::NeoForge => {
            let loader_ver = instance.loader_version.as_deref().ok_or_else(|| {
                EasyMcError::msg("NeoForge 实例缺少 loader_version，请重新安装 NeoForge")
            })?;
            neoforge_profile_id(loader_ver)
        }
    };
    if !is_version_installed(paths, &id) {
        // Fall back: vanilla id check message
        if instance.loader == ModLoader::Vanilla {
            return Err(EasyMcError::msg(format!(
                "版本 {} 尚未安装，请先安装",
                instance.version
            )));
        }
        return Err(EasyMcError::msg(format!(
            "加载器配置 {} 尚未安装，请先安装 {} {}",
            id,
            instance.loader.label_zh(),
            instance.version
        )));
    }
    load_installed_version(paths, &id)
}

pub async fn prepare_and_launch(
    paths: &Paths,
    instance: &Instance,
    account: &Account,
    settings: &Settings,
    req: &LaunchRequest,
    procs: Arc<ProcessManager>,
    on_log: impl Fn(String) + Send + Sync + 'static,
) -> Result<LaunchStatus> {
    let info = resolve_launch_version(paths, instance)?;
    // Ensure natives extracted (always from the jar / parent version)
    let natives = paths.natives_for_version(info.jar_id());
    if !natives.exists() || std::fs::read_dir(&natives).map(|mut d| d.next().is_none()).unwrap_or(true)
    {
        on_log("解压 natives…".into());
        extract_natives(paths, &info)?;
    }

    let required = info
        .java_version
        .as_ref()
        .map(|j| j.major_version)
        .unwrap_or(21);
    std::fs::create_dir_all(&instance.game_dir)?;

    // Active wardrobe → staged PNG + CustomSkinLoader LocalSkin (when CSL is present).
    match easymc_core::stage_active_for_launch(
        paths,
        Some(std::path::Path::new(&instance.game_dir)),
        Some(account.username.as_str()),
    ) {
        Ok(Some(staged)) => {
            on_log(format!(
                "已套用衣柜皮肤「{}」{}",
                staged.name,
                staged
                    .csl_skin_path
                    .as_ref()
                    .map(|_| "（含 CSL LocalSkin）")
                    .unwrap_or("")
            ));
        }
        Ok(None) => {}
        Err(e) => on_log(format!("衣柜皮肤未套用：{e}")),
    }

    // Slice C: container-backed → JRE comes from the runtime image (no host Java required).
    if instance.container.is_some() {
        on_log(format!("容器模式将使用镜像内 Java（目标主版本 {required}）"));
        let stub = JavaInstall {
            path: PathBuf::from("java"),
            major: required,
            version_string: format!("{required}+ (container)"),
            source: "container".into(),
        };
        let args = build_jvm_and_game_args(paths, &info, instance, account, &stub, req)?;
        return launch_via_container(paths, instance, &args, settings, procs, on_log).await;
    }

    let preferred = instance
        .java_path
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .or(settings.preferred_java.as_ref())
        .map(PathBuf::from);
    let java = match select_java(required, preferred.as_deref()) {
        Ok(j) => j,
        Err(_) => {
            on_log(format!("未找到 Java {required}+，尝试下载 Temurin…"));
            ensure_java(paths, required, |s| on_log(s)).await?
        }
    };
    on_log(format!("使用 Java {}: {}", java.major, java.path.display()));

    let args = build_jvm_and_game_args(paths, &info, instance, account, &java, req)?;

    procs
        .spawn(
            instance.id.clone(),
            java.path.clone(),
            args,
            Path::new(&instance.game_dir),
            HashMap::new(),
            on_log,
        )
        .await
}

async fn launch_via_container(
    paths: &Paths,
    instance: &Instance,
    java_args_host: &[String],
    settings: &Settings,
    procs: Arc<ProcessManager>,
    on_log: impl Fn(String) + Send + Sync + 'static,
) -> Result<LaunchStatus> {
    let prefer = settings.container_china_mirrors;
    let on_log = std::sync::Arc::new(on_log);
    (on_log)("容器模式：预热 Podman / 拉取运行时镜像…".into());
    let mut plan = easymc_container::plan_container_launch(
        paths,
        instance,
        java_args_host,
        prefer,
        true,
    )
    .map_err(EasyMcError::msg)?;
    // Prefer broad Podman compatibility: strip --replace; best-effort rm stale name.
    plan = easymc_container::plan_without_replace(&plan);
    for w in &plan.warnings_zh {
        (on_log)(w.clone());
    }
    (on_log)(format!(
        "容器启动：{} → {}",
        plan.container_name, plan.image
    ));
    let _ = std::process::Command::new(&plan.podman)
        .args(["rm", "-f", &plan.container_name])
        .output();
    let (bin, argv) = easymc_container::plan_to_spawn(&plan);
    let on_log2 = Arc::clone(&on_log);
    procs
        .spawn(
            instance.id.clone(),
            bin,
            argv,
            Path::new(&instance.game_dir),
            HashMap::new(),
            move |s| on_log2(s),
        )
        .await
}

pub fn build_jvm_and_game_args(
    paths: &Paths,
    info: &VersionInfo,
    instance: &Instance,
    account: &Account,
    java: &JavaInstall,
    req: &LaunchRequest,
) -> Result<Vec<String>> {
    let _ = java;
    let natives = paths.natives_for_version(info.jar_id());
    let jars = classpath_jars(paths, info)?;
    let sep = if cfg!(windows) { ";" } else { ":" };
    let classpath = jars
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(sep);

    let mut vars: HashMap<String, String> = HashMap::new();
    vars.insert(
        "natives_directory".into(),
        natives.to_string_lossy().into_owned(),
    );
    vars.insert("launcher_name".into(), "EasyMC".into());
    vars.insert("launcher_version".into(), env!("CARGO_PKG_VERSION").into());
    vars.insert("classpath".into(), classpath.clone());
    vars.insert(
        "auth_player_name".into(),
        account.username.clone(),
    );
    vars.insert("version_name".into(), info.id.clone());
    vars.insert("game_directory".into(), instance.game_dir.clone());
    vars.insert(
        "assets_root".into(),
        paths.assets_dir().to_string_lossy().into_owned(),
    );
    let assets_index = info
        .asset_index
        .as_ref()
        .map(|a| a.id.clone())
        .ok_or_else(|| EasyMcError::msg(format!("版本 {} 缺少 assetIndex", info.id)))?;
    vars.insert("assets_index_name".into(), assets_index);
    vars.insert("auth_uuid".into(), account.uuid.clone());
    vars.insert("auth_access_token".into(), account.access_token.clone());
    vars.insert("clientid".into(), "easymc".into());
    vars.insert("auth_xuid".into(), "0".into());
    let user_type = match account.kind {
        AccountKind::Offline => "legacy",
        AccountKind::Microsoft => "msa",
    };
    vars.insert("user_type".into(), user_type.into());
    vars.insert("version_type".into(), info.version_type.clone());
    vars.insert("library_directory".into(), paths.libraries_dir().to_string_lossy().into_owned());
    vars.insert("classpath_separator".into(), sep.into());

    let mut args = Vec::new();
    let mem = instance.memory_mb.filter(|m| *m >= 512).unwrap_or(req.memory_mb);
    args.push(format!("-Xms{}M", mem.min(1024)));
    args.push(format!("-Xmx{}M", mem));
    args.extend(req.extra_jvm_args.iter().cloned());
    if let Some(extra) = instance.jvm_args.as_deref() {
        args.extend(extra.split_whitespace().map(String::from));
    }

    if let Some(arguments) = &info.arguments {
        for item in &arguments.jvm {
            push_arg_value(&mut args, item, &vars, &[])?;
        }
    } else {
        // Legacy
        args.push(format!("-Djava.library.path={}", natives.display()));
        args.push("-cp".into());
        args.push(classpath);
    }

    args.push(info.main_class.clone());

    if let Some(arguments) = &info.arguments {
        for item in &arguments.game {
            // Skip demo / custom resolution feature flags by default
            push_arg_value(
                &mut args,
                item,
                &vars,
                &[("is_demo_user", false), ("has_custom_resolution", false)],
            )?;
        }
    } else if let Some(legacy) = &info.minecraft_arguments {
        for token in legacy.split_whitespace() {
            args.push(substitute(token, &vars));
        }
    } else {
        return Err(EasyMcError::msg("version json has no arguments"));
    }
    if let (Some(w), Some(h)) = (instance.width, instance.height) {
        if w > 0 && h > 0 {
            args.extend(["--width".into(), w.to_string(), "--height".into(), h.to_string()]);
        }
    }
    if let Some(qp) = &instance.quick_play {
        append_quick_play_args(&mut args, qp, instance.version.as_str());
    }

    Ok(args)
}

/// Append Quick Play / legacy server join args. Minecraft 1.20+ supports `--quickPlay*`.
pub fn append_quick_play_args(args: &mut Vec<String>, quick: &QuickPlay, mc_version: &str) {
    let modern = mc_version_at_least(mc_version, 1, 20);
    match quick {
        QuickPlay::World { name } => {
            if modern {
                args.push("--quickPlaySingleplayer".into());
                args.push(name.clone());
            }
        }
        QuickPlay::Server { address } => {
            if modern {
                args.push("--quickPlayMultiplayer".into());
                args.push(address.clone());
            } else {
                let (host, port) = split_host_port(address);
                args.push("--server".into());
                args.push(host);
                if let Some(p) = port {
                    args.push("--port".into());
                    args.push(p);
                }
            }
        }
        QuickPlay::Realms { id } => {
            if modern {
                args.push("--quickPlayRealms".into());
                args.push(id.clone());
            }
        }
    }
}

fn split_host_port(address: &str) -> (String, Option<String>) {
    let a = address.trim();
    if let Some(rest) = a.strip_prefix('[') {
        if let Some((host, port)) = rest.split_once("]:") {
            return (format!("[{host}]"), Some(port.to_string()));
        }
        return (a.to_string(), None);
    }
    if let Some((host, port)) = a.rsplit_once(':') {
        if !host.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            return (host.to_string(), Some(port.to_string()));
        }
    }
    (a.to_string(), None)
}

fn mc_version_at_least(version: &str, major: u32, minor: u32) -> bool {
    let mut nums = Vec::new();
    let mut cur = String::new();
    for c in version.chars() {
        if c.is_ascii_digit() {
            cur.push(c);
        } else if !cur.is_empty() {
            if let Ok(n) = cur.parse::<u32>() {
                nums.push(n);
            }
            cur.clear();
            if c != '.' && nums.len() >= 2 {
                break;
            }
            if c != '.' {
                nums.clear();
            }
        }
    }
    if !cur.is_empty() {
        if let Ok(n) = cur.parse::<u32>() {
            nums.push(n);
        }
    }
    if nums.len() < 2 {
        return false;
    }
    nums[0] > major || (nums[0] == major && nums[1] >= minor)
}


fn push_arg_value(
    out: &mut Vec<String>,
    value: &Value,
    vars: &HashMap<String, String>,
    features: &[(&str, bool)],
) -> Result<()> {
    match value {
        Value::String(s) => {
            out.push(substitute(s, vars));
        }
        Value::Object(map) => {
            let rules = map.get("rules").and_then(|r| {
                serde_json::from_value::<Vec<easymc_meta::Rule>>(r.clone()).ok()
            });
            if !rules_allow(rules.as_deref(), features) {
                return Ok(());
            }
            match map.get("value") {
                Some(Value::String(s)) => out.push(substitute(s, vars)),
                Some(Value::Array(arr)) => {
                    for v in arr {
                        if let Value::String(s) = v {
                            out.push(substitute(s, vars));
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}

fn substitute(template: &str, vars: &HashMap<String, String>) -> String {
    let mut out = template.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("${{{k}}}"), v);
    }
    out
}

/// HMCL-style 「生成启动脚本」: write a shell / batch script that launches this instance
/// without the launcher. The access token is redacted (offline-style "0") so the file never
/// carries a secret; 正版 servers need launching from EasyMC.
pub fn write_launch_script(
    paths: &Paths,
    instance: &Instance,
    account: &Account,
    settings: &Settings,
    req: &LaunchRequest,
) -> Result<PathBuf> {
    let info = resolve_launch_version(paths, instance)?;
    let required = info.java_version.as_ref().map(|j| j.major_version).unwrap_or(21);
    let preferred = instance
        .java_path
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .or(settings.preferred_java.as_ref())
        .map(PathBuf::from);
    let java = select_java(required, preferred.as_deref())
        .map_err(|_| EasyMcError::msg(format!("还没有 Java {required}，先启动一次游戏让 EasyMC 自动准备")))?;
    let mut safe = account.clone();
    safe.access_token = "0".into();
    let args = build_jvm_and_game_args(paths, &info, instance, &safe, &java, req)?;
    let win = cfg!(windows);
    let quote = |s: &str| {
        if win {
            format!("\"{}\"", s.replace('"', "\\\""))
        } else {
            format!("'{}'", s.replace('\'', "'\\''"))
        }
    };
    let mut body = String::new();
    if win {
        body.push_str("@echo off\r\n");
        body.push_str(&format!("cd /d {}\r\n", quote(&instance.game_dir)));
    } else {
        body.push_str("#!/bin/sh\n# EasyMC 生成的启动脚本\n");
        body.push_str(&format!("cd {} || exit 1\n", quote(&instance.game_dir)));
    }
    body.push_str(&quote(&java.path.to_string_lossy()));
    for a in &args {
        body.push(' ');
        body.push_str(&quote(a));
    }
    body.push_str(if win { "\r\n" } else { "\n" });
    let out = PathBuf::from(&instance.game_dir).join(if win { "launch.bat" } else { "launch.sh" });
    std::fs::write(&out, body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o755));
    }
    Ok(out)
}


/// Sanitize a name for use in a desktop / Start Menu filename.
pub fn sanitize_shortcut_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.');
    if trimmed.is_empty() {
        "EasyMC".into()
    } else {
        format!("EasyMC - {trimmed}")
    }
}

/// Write a launch script, then place a desktop (+ Start Menu / applications) shortcut
/// that runs it. Quick Play and instance JVM overrides are baked into the script.
pub fn create_desktop_shortcut(
    paths: &Paths,
    instance: &Instance,
    account: &Account,
    settings: &Settings,
    req: &LaunchRequest,
) -> Result<PathBuf> {
    let script = write_launch_script(paths, instance, account, settings, req)?;
    let desktop = dirs::desktop_dir().ok_or_else(|| EasyMcError::msg("找不到桌面目录"))?;
    std::fs::create_dir_all(&desktop)?;
    let stem = sanitize_shortcut_stem(&instance.name);

    #[cfg(windows)]
    {
        let bat = desktop.join(format!("{stem}.bat"));
        let body = format!(
            "@echo off\r\nrem EasyMC desktop shortcut — token redacted in launch script\r\ncall \"{}\"\r\n",
            script.display()
        );
        std::fs::write(&bat, body)?;
        if let Some(data) = dirs::data_dir() {
            let sm = data.join("Microsoft").join("Windows").join("Start Menu").join("Programs").join("EasyMC");
            let _ = std::fs::create_dir_all(&sm);
            let _ = std::fs::copy(&bat, sm.join(format!("{stem}.bat")));
        }
        return Ok(bat);
    }

    #[cfg(target_os = "macos")]
    {
        let cmd_path = desktop.join(format!("{stem}.command"));
        let body = format!(
            "#!/bin/sh\n# EasyMC desktop shortcut\nexec '{}'\n",
            script.to_string_lossy().replace('\'', "'\\''")
        );
        std::fs::write(&cmd_path, body)?;
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&cmd_path, std::fs::Permissions::from_mode(0o755));
        return Ok(cmd_path);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let desk = desktop.join(format!("{stem}.desktop"));
        let exec = format!("\"{}\"", script.display());
        let content = format!(
            "[Desktop Entry]\nType=Application\nVersion=1.0\nName={name}\nComment=EasyMC — {ver}\nExec={exec}\nPath={cwd}\nTerminal=false\nCategories=Game;\nStartupNotify=true\n",
            name = instance.name.replace('\n', " "),
            ver = instance.version,
            exec = exec,
            cwd = instance.game_dir,
        );
        std::fs::write(&desk, &content)?;
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&desk, std::fs::Permissions::from_mode(0o755));
        if let Some(home) = dirs::home_dir() {
            let apps = home.join(".local/share/applications");
            let _ = std::fs::create_dir_all(&apps);
            let dest = apps.join(format!("easymc-{}.desktop", instance.id));
            let _ = std::fs::write(&dest, &content);
            let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
        }
        return Ok(desk);
    }

    #[allow(unreachable_code)]
    {
        let _ = script;
        Err(EasyMcError::msg("当前平台暂不支持桌面快捷方式"))
    }
}



#[cfg(test)]
mod quick_play_tests {
    use super::*;

    #[test]
    fn modern_server() {
        let mut args = vec![];
        append_quick_play_args(
            &mut args,
            &QuickPlay::Server {
                address: "play.example.com:25566".into(),
            },
            "1.21.1",
        );
        assert_eq!(args, vec!["--quickPlayMultiplayer", "play.example.com:25566"]);
    }

    #[test]
    fn legacy_server_splits_port() {
        let mut args = vec![];
        append_quick_play_args(
            &mut args,
            &QuickPlay::Server {
                address: "mc.hypixel.net:25565".into(),
            },
            "1.16.5",
        );
        assert_eq!(
            args,
            vec!["--server", "mc.hypixel.net", "--port", "25565"]
        );
    }

    #[test]
    fn world_modern_only() {
        let mut args = vec![];
        append_quick_play_args(
            &mut args,
            &QuickPlay::World {
                name: "New World".into(),
            },
            "1.20",
        );
        assert_eq!(args, vec!["--quickPlaySingleplayer", "New World"]);
        let mut old = vec![];
        append_quick_play_args(
            &mut old,
            &QuickPlay::World {
                name: "New World".into(),
            },
            "1.19.4",
        );
        assert!(old.is_empty());
    }

    #[test]
    fn version_parse() {
        assert!(mc_version_at_least("1.20.1", 1, 20));
        assert!(!mc_version_at_least("1.19.4", 1, 20));
    }
}


#[cfg(test)]
mod shortcut_tests {
    use super::*;

    #[test]
    fn sanitize_strips_bad_chars() {
        assert_eq!(sanitize_shortcut_stem("生存/创造:测试"), "EasyMC - 生存_创造_测试");
        assert_eq!(sanitize_shortcut_stem("   "), "EasyMC");
        assert_eq!(sanitize_shortcut_stem("..."), "EasyMC");
    }
}
