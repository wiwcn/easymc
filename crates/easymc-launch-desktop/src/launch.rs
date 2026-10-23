use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use easymc_core::{Account, AccountKind, EasyMcError, Instance, ModLoader, Paths, Result, Settings};
use easymc_meta::rules_allow;
use easymc_meta::{
    classpath_jars, extract_natives, fabric_profile_id, is_version_installed, load_installed_version,
    quilt_profile_id, VersionInfo,
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
        ModLoader::Forge | ModLoader::NeoForge => {
            return Err(EasyMcError::NotImplemented(match instance.loader {
                ModLoader::Forge => "Forge 启动",
                _ => "NeoForge 启动",
            }));
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
    let preferred = settings
        .preferred_java
        .as_ref()
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
    std::fs::create_dir_all(&instance.game_dir)?;

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
    args.push(format!("-Xms{}M", req.memory_mb.min(1024)));
    args.push(format!("-Xmx{}M", req.memory_mb));
    args.extend(req.extra_jvm_args.iter().cloned());

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

    Ok(args)
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
