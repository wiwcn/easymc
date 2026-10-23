use std::sync::Arc;

use easymc_core::{
    begin_microsoft_login, create_named_instance_with, create_offline_account, ensure_defaults,
    load_accounts, load_instances, load_settings, save_accounts, save_instances, set_active_instance,
    upsert_instance, Account, CreateInstanceOpts, Instance, McVersion, MicrosoftAuthConfig,
    ModLoader, Paths, Settings,
};
use easymc_launch_desktop::{
    detect_java, ensure_java, prepare_and_launch, JavaInstall, LaunchRequest, LaunchStatus,
    ProcessManager,
};
use easymc_meta::{
    install_fabric, install_quilt, install_version, is_loader_installed, is_version_installed,
    list_loaders, InstallOptions, LoaderKind, MetaProvider, PistonMetaProvider,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

pub struct AppState {
    pub paths: Paths,
    pub procs: Arc<ProcessManager>,
}

#[derive(Serialize)]
struct LaunchResult {
    ok: bool,
    message: String,
    pid: Option<u32>,
}

fn state_paths(state: &AppState) -> &Paths {
    &state.paths
}

#[tauri::command]
fn app_info(state: State<'_, AppState>) -> serde_json::Value {
    serde_json::json!({
        "name": "EasyMC",
        "version": env!("CARGO_PKG_VERSION"),
        "edition": "Java",
        "milestone": "M1.3",
        "data_dir": state.paths.root,
        "msa_configured": MicrosoftAuthConfig::default().is_configured(),
    })
}

#[derive(Serialize)]
struct InstancesStateDto {
    instances: Vec<Instance>,
    active_id: Option<String>,
}

#[derive(Serialize)]
struct VersionListItem {
    id: String,
    #[serde(rename = "type")]
    version_type: String,
    url: String,
    installed: bool,
}

#[tauri::command]
fn list_instances(state: State<'_, AppState>) -> Result<InstancesStateDto, String> {
    let (store, _) = ensure_defaults(state_paths(&state)).map_err(|e| e.to_string())?;
    Ok(InstancesStateDto {
        instances: store.instances,
        active_id: store.active_id,
    })
}

#[tauri::command]
fn create_instance(
    state: State<'_, AppState>,
    name: String,
    version: String,
    loader: Option<String>,
    loader_version: Option<String>,
) -> Result<Instance, String> {
    let paths = state_paths(&state);
    ensure_defaults(paths).map_err(|e| e.to_string())?;
    let loader = loader
        .as_deref()
        .and_then(ModLoader::parse_loose)
        .unwrap_or(ModLoader::Vanilla);
    let loader_version = loader_version.filter(|s| !s.trim().is_empty());
    let ready = match loader {
        ModLoader::Vanilla => is_version_installed(paths, &version),
        ModLoader::Fabric => loader_version
            .as_deref()
            .map(|lv| is_loader_installed(paths, LoaderKind::Fabric, &version, lv))
            .unwrap_or(false),
        ModLoader::Quilt => loader_version
            .as_deref()
            .map(|lv| is_loader_installed(paths, LoaderKind::Quilt, &version, lv))
            .unwrap_or(false),
        ModLoader::Forge | ModLoader::NeoForge => false,
    };
    create_named_instance_with(
        paths,
        CreateInstanceOpts {
            name,
            version: McVersion(version),
            loader,
            loader_version,
            version_ready: ready,
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_active_instance_cmd(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<InstancesStateDto, String> {
    let paths = state_paths(&state);
    ensure_defaults(paths).map_err(|e| e.to_string())?;
    let store = set_active_instance(paths, &instance_id).map_err(|e| e.to_string())?;
    Ok(InstancesStateDto {
        instances: store.instances,
        active_id: store.active_id,
    })
}

#[tauri::command]
fn list_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, String> {
    let (_, store) = ensure_defaults(state_paths(&state)).map_err(|e| e.to_string())?;
    Ok(store.accounts)
}

#[tauri::command]
fn create_offline(state: State<'_, AppState>, username: String) -> Result<Account, String> {
    let paths = state_paths(&state);
    let acc = create_offline_account(&username);
    let mut store = load_accounts(paths).map_err(|e| e.to_string())?;
    store.accounts.retain(|a| a.username != username);
    store.active_id = Some(acc.id.clone());
    store.accounts.push(acc.clone());
    save_accounts(paths, &store).map_err(|e| e.to_string())?;
    Ok(acc)
}

#[tauri::command]
async fn start_microsoft_login() -> Result<Account, String> {
    begin_microsoft_login(&MicrosoftAuthConfig::default())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_versions(
    state: State<'_, AppState>,
    include_snapshots: Option<bool>,
) -> Result<Vec<VersionListItem>, String> {
    let include_snapshots = include_snapshots.unwrap_or(false);
    let p = PistonMetaProvider::new();
    let mut list = p.list_versions().await.map_err(|e| e.to_string())?;
    if !include_snapshots {
        list.retain(|v| v.version_type == "release");
    }
    // Prefer recent entries; piston-meta is newest-first.
    let limit = if include_snapshots { 120 } else { 80 };
    list.truncate(limit);
    let paths = state_paths(&state);
    Ok(list
        .into_iter()
        .map(|v| VersionListItem {
            installed: is_version_installed(paths, &v.id),
            id: v.id,
            version_type: v.version_type,
            url: v.url,
        })
        .collect())
}

#[tauri::command]
async fn install_mc_version(
    app: AppHandle,
    state: State<'_, AppState>,
    version: String,
    with_assets: Option<bool>,
) -> Result<String, String> {
    let paths = state.paths.clone();
    let opts = InstallOptions {
        assets: with_assets.unwrap_or(true),
        natives: true,
    };
    install_version(&paths, &McVersion(version.clone()), opts, move |p| {
        let _ = app.emit("install-progress", &p);
    })
    .await
    .map_err(|e| {
        // Already includes stage context from meta.
        e.to_string()
    })?;

    let mut store = load_instances(&paths).map_err(|e| e.to_string())?;
    for inst in &mut store.instances {
        if inst.version.0 == version {
            inst.version_ready = true;
        }
    }
    save_instances(&paths, &store).map_err(|e| e.to_string())?;
    let assets_note = if with_assets.unwrap_or(true) {
        "客户端/依赖/资源已就绪"
    } else {
        "客户端/依赖已就绪（已跳过资源）"
    };
    Ok(format!("已安装 {version}（{assets_note}）"))
}

#[derive(Serialize)]
struct LoaderListItem {
    version: String,
    stable: bool,
    intermediary: String,
}

#[tauri::command]
async fn list_loader_versions(
    loader: String,
    game_version: String,
) -> Result<Vec<LoaderListItem>, String> {
    let kind = match loader.to_ascii_lowercase().as_str() {
        "fabric" => LoaderKind::Fabric,
        "quilt" => LoaderKind::Quilt,
        other => return Err(format!("不支持的加载器: {other}（目前：fabric / quilt）")),
    };
    let list = list_loaders(kind, &game_version)
        .await
        .map_err(|e| e.to_string())?;
    Ok(list
        .into_iter()
        .take(30)
        .map(|l| LoaderListItem {
            version: l.loader.version,
            stable: l.loader.stable,
            intermediary: l.intermediary.version,
        })
        .collect())
}

#[tauri::command]
async fn install_loader_version(
    app: AppHandle,
    state: State<'_, AppState>,
    game_version: String,
    loader: String,
    loader_version: Option<String>,
    with_assets: Option<bool>,
) -> Result<String, String> {
    let paths = state.paths.clone();
    let kind = match loader.to_ascii_lowercase().as_str() {
        "fabric" => LoaderKind::Fabric,
        "quilt" => LoaderKind::Quilt,
        "forge" | "neoforge" => {
            return Err("Forge / NeoForge 尚未实现，请先使用 Fabric 或原版".into());
        }
        other => return Err(format!("未知加载器: {other}")),
    };
    let vanilla = InstallOptions {
        assets: with_assets.unwrap_or(true),
        natives: true,
    };
    let app2 = app.clone();
    let (profile_id, resolved) = match kind {
        LoaderKind::Fabric => install_fabric(
            &paths,
            &McVersion(game_version.clone()),
            loader_version.clone(),
            vanilla,
            move |p| {
                let _ = app2.emit("install-progress", &p);
            },
        )
        .await
        .map_err(|e| e.to_string())?,
        LoaderKind::Quilt => {
            let app3 = app.clone();
            install_quilt(
                &paths,
                &McVersion(game_version.clone()),
                loader_version.clone(),
                vanilla,
                move |p| {
                    let _ = app3.emit("install-progress", &p);
                },
            )
            .await
            .map_err(|e| e.to_string())?
        }
    };
    let loader_enum = match kind {
        LoaderKind::Fabric => ModLoader::Fabric,
        LoaderKind::Quilt => ModLoader::Quilt,
    };
    let mut store = load_instances(&paths).map_err(|e| e.to_string())?;
    for inst in &mut store.instances {
        if inst.version.0 == game_version && inst.loader == loader_enum {
            inst.loader_version = Some(resolved.clone());
            inst.version_ready = true;
        }
    }
    save_instances(&paths, &store).map_err(|e| e.to_string())?;
    Ok(format!(
        "已安装 {} {} @ {}（配置 {profile_id}）",
        kind.label_zh(),
        resolved,
        game_version
    ))
}

#[tauri::command]
fn detect_java_cmd() -> Vec<JavaInstall> {
    detect_java()
}

#[tauri::command]
async fn ensure_java_cmd(app: AppHandle, state: State<'_, AppState>, major: Option<u32>) -> Result<JavaInstall, String> {
    let major = major.unwrap_or(21);
    let paths = state.paths.clone();
    ensure_java(&paths, major, move |line| {
        let _ = app.emit("launch-log", line);
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn launch_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<LaunchResult, String> {
    let paths = state_paths(&state);
    ensure_defaults(paths).map_err(|e| e.to_string())?;
    let store = load_instances(paths).map_err(|e| e.to_string())?;
    let accounts = load_accounts(paths).map_err(|e| e.to_string())?;
    let settings = load_settings(paths).map_err(|e| e.to_string())?;

    let mut inst = store
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .cloned()
        .ok_or_else(|| format!("unknown instance: {instance_id}"))?;

    let ready = match inst.loader {
        ModLoader::Vanilla => is_version_installed(paths, inst.version.as_str()),
        ModLoader::Fabric => inst
            .loader_version
            .as_deref()
            .map(|lv| is_loader_installed(paths, LoaderKind::Fabric, inst.version.as_str(), lv))
            .unwrap_or(false),
        ModLoader::Quilt => inst
            .loader_version
            .as_deref()
            .map(|lv| is_loader_installed(paths, LoaderKind::Quilt, inst.version.as_str(), lv))
            .unwrap_or(false),
        ModLoader::Forge | ModLoader::NeoForge => false,
    };
    if !ready {
        return Err(format!(
            "{} {} 未安装。请先在库/启动页安装。",
            inst.loader.label_zh(),
            inst.version
        ));
    }
    inst.version_ready = true;

    let acc_id = inst
        .account_id
        .clone()
        .or(accounts.active_id.clone())
        .ok_or_else(|| "没有可用账户，请先创建离线账户".to_string())?;
    let acc = accounts
        .accounts
        .iter()
        .find(|a| a.id == acc_id)
        .cloned()
        .ok_or_else(|| format!("account missing: {acc_id}"))?;

    let procs = state.procs.clone();
    let app2 = app.clone();
    let status: LaunchStatus = prepare_and_launch(
        paths,
        &inst,
        &acc,
        &settings,
        &LaunchRequest {
            memory_mb: settings.memory_mb,
            extra_jvm_args: vec![],
        },
        procs,
        move |line| {
            let _ = app2.emit("launch-log", line);
        },
    )
    .await
    .map_err(|e| e.to_string())?;

    // bump last_played
    inst.last_played = Some(chrono::Utc::now());
    let _ = upsert_instance(paths, inst);

    Ok(LaunchResult {
        ok: true,
        message: status.message,
        pid: status.pid,
    })
}

#[tauri::command]
async fn stop_instance(state: State<'_, AppState>) -> Result<(), String> {
    state.procs.stop().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_logs(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(state.procs.logs_snapshot().await)
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    load_settings(state_paths(&state)).map_err(|e| e.to_string())
}

#[tauri::command]
fn version_installed(state: State<'_, AppState>, version: String) -> bool {
    is_version_installed(state_paths(&state), &version)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let paths = Paths::discover().expect("data dir");
    paths.ensure().expect("create data dir");
    let _ = ensure_defaults(&paths);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            paths,
            procs: Arc::new(ProcessManager::new()),
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            list_instances,
            create_instance,
            set_active_instance_cmd,
            list_accounts,
            create_offline,
            start_microsoft_login,
            list_versions,
            install_mc_version,
            list_loader_versions,
            install_loader_version,
            detect_java_cmd,
            ensure_java_cmd,
            launch_instance,
            stop_instance,
            get_logs,
            get_settings,
            version_installed,
        ])
        .run(tauri::generate_context!())
        .expect("error while running EasyMC");
}
