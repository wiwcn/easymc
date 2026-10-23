use std::sync::Arc;

use easymc_core::{
    begin_microsoft_login, create_named_instance_with, create_offline_account, ensure_defaults,
    load_accounts, load_instances, load_settings, save_accounts, save_instances, save_settings,
    set_active_instance, upsert_instance, Account, CreateInstanceOpts, Instance, McVersion,
    MicrosoftAuthConfig, ModLoader, Paths, Settings,
};
use easymc_launch_desktop::{
    detect_java, ensure_java, prepare_and_launch, JavaInstall, LaunchRequest, LaunchStatus,
    ProcessManager,
};
use easymc_meta::{
    install_fabric, install_forge, install_neoforge, install_quilt, install_version,
    is_loader_installed, is_version_installed, list_loaders, InstallOptions, LoaderKind,
    MetaProvider, PistonMetaProvider,
};
use easymc_share::{
    export_instance, import_instance, ExportOptions, ExportResult, ImportOptions, ImportResult,
    ShareMode,
};
use easymc_agent::{
    ask, bump_memory, llm_status, run_diagnose, show_data_dir, try_load_dotenv_llm,
    write_diagnostic_bundle, AskRequest, AskResponse, ChatMessage, DiagnoseReport, LlmStatus,
};
use easymc_mods::{
    api_key, api_key_source, install_mod, list_installed_mods, list_mod_versions, remove_mod,
    search_mods, set_mod_disabled, sync_api_key_from_settings, ApiKeySource, ModInstallResult,
    ModProject, ModSource, ModVersion, SearchQuery, API_KEY_ENV, CONSOLE_URL,
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
        "milestone": "M1.7",
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
        ModLoader::Forge => loader_version
            .as_deref()
            .map(|lv| is_loader_installed(paths, LoaderKind::Forge, &version, lv))
            .unwrap_or(false),
        ModLoader::NeoForge => loader_version
            .as_deref()
            .map(|lv| is_loader_installed(paths, LoaderKind::NeoForge, &version, lv))
            .unwrap_or(false),
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
        "forge" => LoaderKind::Forge,
        "neoforge" | "neo" => LoaderKind::NeoForge,
        other => {
            return Err(format!(
                "不支持的加载器: {other}（fabric / quilt / forge / neoforge）"
            ))
        }
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
        "forge" => LoaderKind::Forge,
        "neoforge" | "neo" => LoaderKind::NeoForge,
        other => return Err(format!("未知加载器: {other}")),
    };
    let vanilla = InstallOptions {
        assets: with_assets.unwrap_or(true),
        natives: true,
    };
    let (profile_id, resolved) = match kind {
        LoaderKind::Fabric => {
            let app2 = app.clone();
            install_fabric(
                &paths,
                &McVersion(game_version.clone()),
                loader_version.clone(),
                vanilla,
                move |p| {
                    let _ = app2.emit("install-progress", &p);
                },
            )
            .await
            .map_err(|e| e.to_string())?
        }
        LoaderKind::Quilt => {
            let app2 = app.clone();
            install_quilt(
                &paths,
                &McVersion(game_version.clone()),
                loader_version.clone(),
                vanilla,
                move |p| {
                    let _ = app2.emit("install-progress", &p);
                },
            )
            .await
            .map_err(|e| e.to_string())?
        }
        LoaderKind::Forge => {
            let java = ensure_java(&paths, 21, |_| {}).await.map_err(|e| e.to_string())?;
            let app2 = app.clone();
            install_forge(
                &paths,
                &McVersion(game_version.clone()),
                loader_version.clone(),
                &java.path,
                vanilla,
                move |p| {
                    let _ = app2.emit("install-progress", &p);
                },
            )
            .await
            .map_err(|e| e.to_string())?
        }
        LoaderKind::NeoForge => {
            let java = ensure_java(&paths, 21, |_| {}).await.map_err(|e| e.to_string())?;
            let app2 = app.clone();
            install_neoforge(
                &paths,
                &McVersion(game_version.clone()),
                loader_version.clone(),
                &java.path,
                vanilla,
                move |p| {
                    let _ = app2.emit("install-progress", &p);
                },
            )
            .await
            .map_err(|e| e.to_string())?
        }
    };
    let loader_enum = match kind {
        LoaderKind::Fabric => ModLoader::Fabric,
        LoaderKind::Quilt => ModLoader::Quilt,
        LoaderKind::Forge => ModLoader::Forge,
        LoaderKind::NeoForge => ModLoader::NeoForge,
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
        ModLoader::Forge => inst
            .loader_version
            .as_deref()
            .map(|lv| is_loader_installed(paths, LoaderKind::Forge, inst.version.as_str(), lv))
            .unwrap_or(false),
        ModLoader::NeoForge => inst
            .loader_version
            .as_deref()
            .map(|lv| is_loader_installed(paths, LoaderKind::NeoForge, inst.version.as_str(), lv))
            .unwrap_or(false),
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


fn parse_mod_source(s: &str) -> Result<ModSource, String> {
    ModSource::parse(s).ok_or_else(|| format!("未知模组源: {s}（modrinth|curseforge）"))
}

#[tauri::command]
fn curseforge_configured(state: State<'_, AppState>) -> serde_json::Value {
    // Refresh from settings in case file changed externally
    if let Ok(settings) = load_settings(state_paths(&state)) {
        sync_api_key_from_settings(settings.curseforge_api_key.as_deref());
    }
    let source = api_key_source();
    let configured = source != ApiKeySource::None;
    let source_str = match source {
        ApiKeySource::Env => "env",
        ApiKeySource::Settings => "settings",
        ApiKeySource::None => "none",
    };
    let key_len = api_key().map(|k| k.len()).unwrap_or(0);
    serde_json::json!({
        "configured": configured,
        "source": source_str,
        "env": API_KEY_ENV,
        "console_url": CONSOLE_URL,
        "key_len": key_len,
        "hint": if configured {
            format!("CurseForge API Key 已配置（来源: {source_str}）")
        } else {
            format!(
                "未配置 CurseForge API Key。请在「设置」中粘贴，或访问 {CONSOLE_URL} 申请。也可设置环境变量 {API_KEY_ENV}。Modrinth 无需 Key。"
            )
        }
    })
}

#[tauri::command]
fn set_curseforge_key_cmd(
    state: State<'_, AppState>,
    key: Option<String>,
) -> Result<serde_json::Value, String> {
    let paths = state_paths(&state);
    let mut settings = load_settings(paths).map_err(|e| e.to_string())?;
    let cleaned = key
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    settings.curseforge_api_key = cleaned.clone();
    save_settings(paths, &settings).map_err(|e| e.to_string())?;
    sync_api_key_from_settings(cleaned.as_deref());
    let source = api_key_source();
    Ok(serde_json::json!({
        "ok": true,
        "configured": source != ApiKeySource::None,
        "source": match source {
            ApiKeySource::Env => "env",
            ApiKeySource::Settings => "settings",
            ApiKeySource::None => "none",
        },
        "key_len": api_key().map(|k| k.len()).unwrap_or(0),
        "message": if cleaned.is_some() {
            "已保存 CurseForge API Key 到本地设置"
        } else {
            "已清除本地 CurseForge API Key"
        }
    }))
}

#[tauri::command]
fn remove_mod_cmd(
    state: State<'_, AppState>,
    filename: String,
    instance_id: Option<String>,
) -> Result<String, String> {
    let paths = state_paths(&state);
    let store = load_instances(paths).map_err(|e| e.to_string())?;
    let id = instance_id
        .or(store.active_id.clone())
        .ok_or_else(|| "无活动实例".to_string())?;
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| format!("实例不存在: {id}"))?;
    remove_mod(inst, &filename).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_mod_disabled_cmd(
    state: State<'_, AppState>,
    filename: String,
    disabled: bool,
    instance_id: Option<String>,
) -> Result<String, String> {
    let paths = state_paths(&state);
    let store = load_instances(paths).map_err(|e| e.to_string())?;
    let id = instance_id
        .or(store.active_id.clone())
        .ok_or_else(|| "无活动实例".to_string())?;
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| format!("实例不存在: {id}"))?;
    set_mod_disabled(inst, &filename, disabled).map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_mods_cmd(
    source: String,
    query: Option<String>,
    game_version: Option<String>,
    loader: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<ModProject>, String> {
    let source = parse_mod_source(&source)?;
    let loader = loader.and_then(|s| ModLoader::parse_loose(&s));
    let q = SearchQuery {
        text: query.unwrap_or_default(),
        game_version,
        loader,
        limit: limit.unwrap_or(20).clamp(1, 50),
        offset: 0,
    };
    search_mods(source, &q).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_mod_versions_cmd(
    source: String,
    project_id: String,
    game_version: Option<String>,
    loader: Option<String>,
) -> Result<Vec<ModVersion>, String> {
    let source = parse_mod_source(&source)?;
    let loader = loader.and_then(|s| ModLoader::parse_loose(&s));
    list_mod_versions(source, &project_id, game_version.as_deref(), loader)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn install_mod_cmd(
    app: AppHandle,
    state: State<'_, AppState>,
    source: String,
    project_id: String,
    version_id: Option<String>,
    instance_id: Option<String>,
) -> Result<ModInstallResult, String> {
    let source = parse_mod_source(&source)?;
    let paths = state_paths(&state);
    let store = load_instances(paths).map_err(|e| e.to_string())?;
    let id = instance_id
        .or(store.active_id.clone())
        .ok_or_else(|| "无活动实例".to_string())?;
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| format!("实例不存在: {id}"))?
        .clone();
    let app2 = app.clone();
    install_mod(
        &inst,
        source,
        &project_id,
        version_id.as_deref(),
        move |p| {
            let _ = app2.emit("install-progress", &p);
        },
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_installed_mods_cmd(
    state: State<'_, AppState>,
    instance_id: Option<String>,
) -> Result<Vec<String>, String> {
    let paths = state_paths(&state);
    let store = load_instances(paths).map_err(|e| e.to_string())?;
    let id = instance_id
        .or(store.active_id.clone())
        .ok_or_else(|| "无活动实例".to_string())?;
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| format!("实例不存在: {id}"))?;
    list_installed_mods(inst).map_err(|e| e.to_string())
}

#[tauri::command]
fn version_installed(state: State<'_, AppState>, version: String) -> bool {
    is_version_installed(state_paths(&state), &version)
}


#[tauri::command]
fn agent_llm_status() -> LlmStatus {
    try_load_dotenv_llm();
    llm_status()
}

#[tauri::command]
async fn agent_diagnose(
    state: State<'_, AppState>,
    instance_id: Option<String>,
) -> Result<DiagnoseReport, String> {
    try_load_dotenv_llm();
    let logs = state.procs.logs_snapshot().await;
    let process_logs = if logs.is_empty() { None } else { Some(logs) };
    Ok(run_diagnose(&state.paths, instance_id, process_logs))
}

#[tauri::command]
async fn agent_ask(
    state: State<'_, AppState>,
    text: String,
    instance_id: Option<String>,
    rules_only: Option<bool>,
    history: Option<Vec<ChatMessage>>,
) -> Result<AskResponse, String> {
    try_load_dotenv_llm();
    let logs = state.procs.logs_snapshot().await;
    let process_logs = if logs.is_empty() { None } else { Some(logs) };
    let resp = ask(
        &state.paths,
        AskRequest {
            text,
            instance_id,
            process_logs,
            rules_only: rules_only.unwrap_or(false),
            history: history.unwrap_or_default(),
        },
    )
    .await
    .map_err(|e| e.to_string())?;
    // Auto-run ensure_java when suggested
    if let Some(action) = &resp.action {
        if action.action == "ensure_java" {
            let major = action
                .value
                .as_ref()
                .and_then(|v| v.get("major"))
                .and_then(|v| v.as_u64())
                .unwrap_or(21) as u32;
            let paths = state.paths.clone();
            let _ = ensure_java(&paths, major, |_| {}).await;
        }
    }
    Ok(resp)
}

#[tauri::command]
fn agent_data_dir(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let ar = show_data_dir(&state.paths);
    Ok(serde_json::json!({ "path": ar.path, "message": ar.message_zh }))
}

#[tauri::command]
fn agent_bump_memory(state: State<'_, AppState>, mb: u32) -> Result<serde_json::Value, String> {
    let ar = bump_memory(&state.paths, mb).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "ok": ar.ok,
        "message": ar.message_zh,
        "value": ar.value,
    }))
}

#[tauri::command]
async fn agent_export_diagnostics(
    state: State<'_, AppState>,
    instance_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let logs = state.procs.logs_snapshot().await;
    let process_logs = if logs.is_empty() { None } else { Some(logs) };
    let report = run_diagnose(&state.paths, instance_id, process_logs);
    let ar = write_diagnostic_bundle(&state.paths, &report).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "ok": ar.ok,
        "path": ar.path,
        "message": ar.message_zh,
        "summary": report.summary_zh,
    }))
}



#[tauri::command]
async fn export_instance_cmd(
    state: State<'_, AppState>,
    instance_id: String,
    pack: Option<bool>,
    notes: Option<String>,
) -> Result<ExportResult, String> {
    let store = load_instances(&state.paths).map_err(|e| e.to_string())?;
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| format!("实例不存在: {instance_id}"))?
        .clone();
    let mode = if pack.unwrap_or(false) {
        ShareMode::Pack
    } else {
        ShareMode::Metadata
    };
    export_instance(
        &state.paths,
        &inst,
        ExportOptions {
            mode,
            notes,
            output: None,
            resolve_modrinth: true,
        },
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn import_instance_cmd(
    state: State<'_, AppState>,
    path: String,
    name: Option<String>,
    apply_memory: Option<bool>,
    offline: Option<bool>,
) -> Result<ImportResult, String> {
    import_instance(
        &state.paths,
        std::path::Path::new(&path),
        ImportOptions {
            name,
            apply_memory: apply_memory.unwrap_or(false),
            offline: offline.unwrap_or(false),
        },
    )
    .await
    .map_err(|e| e.to_string())
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let paths = Paths::discover().expect("data dir");
    paths.ensure().expect("create data dir");
    let _ = ensure_defaults(&paths);
    if let Ok(settings) = load_settings(&paths) {
        sync_api_key_from_settings(settings.curseforge_api_key.as_deref());
    }

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
            curseforge_configured,
            set_curseforge_key_cmd,
            search_mods_cmd,
            list_mod_versions_cmd,
            install_mod_cmd,
            list_installed_mods_cmd,
            remove_mod_cmd,
            set_mod_disabled_cmd,
            agent_llm_status,
            agent_diagnose,
            agent_ask,
            agent_data_dir,
            agent_bump_memory,
            agent_export_diagnostics,
            export_instance_cmd,
            import_instance_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running EasyMC");
}
