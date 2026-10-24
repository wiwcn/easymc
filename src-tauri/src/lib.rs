use std::sync::Arc;

use easymc_core::{
    begin_microsoft_login, create_named_instance_with, create_offline_account, duplicate_instance,
    ensure_defaults, load_accounts, load_instances, load_settings, remove_instance, save_accounts,
    save_instances, save_settings, set_active_instance, update_instance_fields, upsert_instance,
    Account, AppMode, CreateInstanceOpts, DownloadSource, Instance, McVersion,
    MicrosoftAuthConfig, ModLoader, Paths, Settings,
};
use easymc_launch_desktop::{
    detect_java, ensure_java, prepare_and_launch, write_launch_script, JavaInstall, LaunchRequest,
    LaunchStatus, ProcessManager,
};
use easymc_meta::{
    install_fabric, install_forge, install_neoforge, install_quilt, install_version,
    is_loader_installed, is_version_installed, list_loaders_with_source, InstallOptions,
    LoaderKind, MetaProvider, PistonMetaProvider,
};
use easymc_share::{
    export_instance, import_instance, ExportOptions, ExportResult, ImportOptions, ImportResult,
    ShareMode,
};
use easymc_agent::{
    ask, bump_memory, llm_status, run_diagnose, show_data_dir, try_load_dotenv_llm,
    write_diagnostic_bundle, ActionResult, AskRequest, AskResponse, ChatMessage, DiagnoseReport,
    LlmStatus,
};
use easymc_mods::{
    api_key, api_key_source, delete_local, install_content, install_mod, install_modpack,
    list_installed_mods, list_local, list_mod_versions, local_dir, remove_mod, search_and_install_best,
    search_mods, set_mod_disabled, sync_api_key_from_settings, ApiKeySource, ContentInstallResult,
    LocalEntry, ModInstallResult, ModProject, ModSource, ModVersion, ModpackInstallResult,
    ProjectType, SearchQuery, API_KEY_ENV, CONSOLE_URL,
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
    let settings = load_settings(state_paths(&state)).ok();
    let msa = MicrosoftAuthConfig::resolve(settings.as_ref().and_then(|s| s.msa_client_id.as_deref()));
    serde_json::json!({
        "name": "EasyMC",
        "version": env!("CARGO_PKG_VERSION"),
        "edition": "Java",
        "milestone": "M1.7",
        "data_dir": state.paths.root,
        "msa_configured": msa.is_configured(),
        "msa_temporary_client": msa.using_temporary_client_id,
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
    let acc = create_offline_account(&username).map_err(|e| e.to_string())?;
    let mut store = load_accounts(paths).map_err(|e| e.to_string())?;
    store.accounts.retain(|a| a.username != acc.username);
    store.active_id = Some(acc.id.clone());
    store.accounts.push(acc.clone());
    save_accounts(paths, &store).map_err(|e| e.to_string())?;
    Ok(acc)
}


#[derive(serde::Serialize)]
struct AccountsStateDto {
    accounts: Vec<Account>,
    active_id: Option<String>,
}

#[tauri::command]
fn list_accounts_state(state: State<'_, AppState>) -> Result<AccountsStateDto, String> {
    let (_, store) = ensure_defaults(state_paths(&state)).map_err(|e| e.to_string())?;
    Ok(AccountsStateDto {
        accounts: store.accounts,
        active_id: store.active_id,
    })
}

#[tauri::command]
fn set_active_account(state: State<'_, AppState>, account_id: String) -> Result<AccountsStateDto, String> {
    let paths = state_paths(&state);
    let mut store = load_accounts(paths).map_err(|e| e.to_string())?;
    if !store.accounts.iter().any(|a| a.id == account_id) {
        return Err("账户不存在".into());
    }
    store.active_id = Some(account_id);
    save_accounts(paths, &store).map_err(|e| e.to_string())?;
    Ok(AccountsStateDto {
        accounts: store.accounts,
        active_id: store.active_id,
    })
}

#[tauri::command]
fn remove_account(state: State<'_, AppState>, account_id: String) -> Result<AccountsStateDto, String> {
    let paths = state_paths(&state);
    let mut store = load_accounts(paths).map_err(|e| e.to_string())?;
    let before = store.accounts.len();
    store.accounts.retain(|a| a.id != account_id);
    if store.accounts.len() == before {
        return Err("账户不存在".into());
    }
    if store.active_id.as_deref() == Some(account_id.as_str()) {
        store.active_id = store.accounts.first().map(|a| a.id.clone());
    }
    save_accounts(paths, &store).map_err(|e| e.to_string())?;
    Ok(AccountsStateDto {
        accounts: store.accounts,
        active_id: store.active_id,
    })
}

#[tauri::command]
async fn start_microsoft_login(state: State<'_, AppState>) -> Result<Account, String> {
    let settings = load_settings(state_paths(&state)).map_err(|e| e.to_string())?;
    let cfg = MicrosoftAuthConfig::resolve(settings.msa_client_id.as_deref());
    begin_microsoft_login(&cfg)
        .await
        .map_err(|e| e.to_string())
}

fn msa_cfg(state: &AppState) -> Result<MicrosoftAuthConfig, String> {
    let settings = load_settings(state_paths(state)).map_err(|e| e.to_string())?;
    Ok(MicrosoftAuthConfig::resolve(settings.msa_client_id.as_deref()))
}

/// Step 1 of 正版登录: get the code the player types at microsoft.com/link.
#[tauri::command]
async fn msa_device_code_cmd(state: State<'_, AppState>) -> Result<easymc_core::msa::DeviceCodeInfo, String> {
    let cfg = msa_cfg(&state)?;
    easymc_core::msa::request_device_code(&cfg).await.map_err(|e| e.to_string())
}

/// Step 2: wait for the browser sign-in, run Xbox/XSTS/Minecraft chain, save + activate account.
#[tauri::command]
async fn msa_poll_cmd(
    state: State<'_, AppState>,
    info: easymc_core::msa::DeviceCodeInfo,
) -> Result<Account, String> {
    let cfg = msa_cfg(&state)?;
    let acc = easymc_core::msa::poll_device_code(&cfg, &info)
        .await
        .map_err(|e| e.to_string())?;
    let paths = state_paths(&state);
    let mut store = load_accounts(paths).map_err(|e| e.to_string())?;
    store.accounts.retain(|a| a.id != acc.id);
    store.active_id = Some(acc.id.clone());
    store.accounts.push(acc.clone());
    save_accounts(paths, &store).map_err(|e| e.to_string())?;
    Ok(acc)
}

fn settings_download_source(paths: &Paths) -> DownloadSource {
    load_settings(paths)
        .map(|s| s.download_source)
        .unwrap_or_default()
}

#[derive(serde::Deserialize)]
struct UpdateSettingsPatch {
    mode: Option<String>,
    onboarding_complete: Option<bool>,
    preferred_login: Option<String>,
    memory_mb: Option<u32>,
    msa_client_id: Option<String>,
    clear_msa_client_id: Option<bool>,
    download_source: Option<String>,
}

#[tauri::command]
fn update_settings_cmd(
    state: State<'_, AppState>,
    patch: UpdateSettingsPatch,
) -> Result<Settings, String> {
    let paths = state_paths(&state);
    let mut settings = load_settings(paths).map_err(|e| e.to_string())?;
    if let Some(mode) = patch.mode.as_deref() {
        settings.mode = AppMode::parse(mode).ok_or_else(|| format!("未知模式: {mode}"))?;
    }
    if let Some(done) = patch.onboarding_complete {
        settings.onboarding_complete = done;
    }
    if let Some(login) = patch.preferred_login {
        let t = login.trim().to_string();
        settings.preferred_login = if t.is_empty() { None } else { Some(t) };
    }
    if let Some(mb) = patch.memory_mb {
        settings.memory_mb = mb.clamp(512, 32768);
    }
    if patch.clear_msa_client_id.unwrap_or(false) {
        settings.msa_client_id = None;
    } else if let Some(id) = patch.msa_client_id {
        let t = id.trim().to_string();
        settings.msa_client_id = if t.is_empty() { None } else { Some(t) };
    }
    if let Some(src) = patch.download_source.as_deref() {
        settings.download_source = DownloadSource::parse(src)
            .ok_or_else(|| format!("未知下载源: {src}（official / bmclapi）"))?;
    }
    save_settings(paths, &settings).map_err(|e| e.to_string())?;
    Ok(settings)
}

/// Persist first-run choices and mark the onboarding gate complete.
#[tauri::command]
fn complete_onboarding_cmd(
    state: State<'_, AppState>,
    mode: String,
    preferred_login: Option<String>,
) -> Result<Settings, String> {
    let paths = state_paths(&state);
    let accounts = load_accounts(paths).map_err(|e| e.to_string())?;
    if accounts.accounts.is_empty() {
        return Err("请先添加至少一个账户再进入主页".into());
    }
    let mut settings = load_settings(paths).map_err(|e| e.to_string())?;
    settings.mode = AppMode::parse(&mode).ok_or_else(|| format!("未知模式: {mode}"))?;
    settings.preferred_login = preferred_login
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    settings.onboarding_complete = true;
    save_settings(paths, &settings).map_err(|e| e.to_string())?;
    Ok(settings)
}

#[tauri::command]
fn msa_status_cmd(state: State<'_, AppState>) -> serde_json::Value {
    let settings = load_settings(state_paths(&state)).ok();
    let cfg = MicrosoftAuthConfig::resolve(settings.as_ref().and_then(|s| s.msa_client_id.as_deref()));
    serde_json::json!({
        "configured": cfg.is_configured(),
        "using_temporary_client_id": cfg.using_temporary_client_id,
        "oauth_ready": cfg.is_configured(),
        "message_zh": if cfg.is_configured() {
            "使用微软账户登录 Minecraft（正版）"
        } else {
            "正版登录即将可用，可先离线游玩"
        },
    })
}

#[tauri::command]
async fn list_versions(
    state: State<'_, AppState>,
    include_snapshots: Option<bool>,
) -> Result<Vec<VersionListItem>, String> {
    let include_snapshots = include_snapshots.unwrap_or(false);
    let paths = state_paths(&state);
    let p = PistonMetaProvider::with_source(settings_download_source(paths));
    let mut list = p.list_versions().await.map_err(|e| e.to_string())?;
    if !include_snapshots {
        list.retain(|v| v.version_type == "release");
    }
    // Prefer recent entries; piston-meta is newest-first.
    let limit = if include_snapshots { 120 } else { 80 };
    list.truncate(limit);
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
        download_source: settings_download_source(&paths),
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
    state: State<'_, AppState>,
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
    let source = settings_download_source(state_paths(&state));
    let list = list_loaders_with_source(kind, &game_version, source)
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
        download_source: settings_download_source(&paths),
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
    // 正版账户：令牌快过期时先刷新，再用真实 accessToken / uuid / 名字启动。
    let acc = if easymc_core::msa::needs_refresh(&acc) {
        let cfg = MicrosoftAuthConfig::resolve(settings.msa_client_id.as_deref());
        let fresh = easymc_core::msa::refresh_account(&cfg, &acc)
            .await
            .map_err(|e| e.to_string())?;
        let mut store = load_accounts(paths).map_err(|e| e.to_string())?;
        for a in store.accounts.iter_mut() {
            if a.id == acc.id {
                *a = fresh.clone();
            }
        }
        save_accounts(paths, &store).map_err(|e| e.to_string())?;
        fresh
    } else {
        acc
    };

    let procs = state.procs.clone();
    let app2 = app.clone();
    let status: LaunchStatus = prepare_and_launch(
        paths,
        &inst,
        &acc,
        &settings,
        &LaunchRequest {
            memory_mb: inst.memory_mb.unwrap_or(settings.memory_mb),
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
            "CurseForge 密钥已保存".to_string()
        } else {
            format!(
                "浏览 CurseForge 需要密钥：在「设置 · 高级」中粘贴（可在 {CONSOLE_URL} 申请）。Modrinth 无需密钥。"
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
                project_type: Default::default(),
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
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
    instance_id: Option<String>,
    rules_only: Option<bool>,
    history: Option<Vec<ChatMessage>>,
) -> Result<AskResponse, String> {
    try_load_dotenv_llm();
    let logs = state.procs.logs_snapshot().await;
    let process_logs = if logs.is_empty() { None } else { Some(logs) };
    let mut resp = ask(
        &state.paths,
        AskRequest {
            text,
            instance_id: instance_id.clone(),
            process_logs,
            rules_only: rules_only.unwrap_or(false),
            history: history.unwrap_or_default(),
        },
    )
    .await
    .map_err(|e| e.to_string())?;
    // Host-side side effects for actions that need real work (Java / mod install).
    if let Some(action) = resp.action.clone() {
        if action.action == "ensure_java" {
            let major = action
                .value
                .as_ref()
                .and_then(|v| v.get("major"))
                .and_then(|v| v.as_u64())
                .unwrap_or(21) as u32;
            let paths = state.paths.clone();
            let app2 = app.clone();
            match ensure_java(&paths, major, move |line| {
                let _ = app2.emit("launch-log", line);
            })
            .await
            {
                Ok(j) => {
                    let msg = format!(
                        "Java {} 已准备好。\n路径：{}\n来源：{}\n下次启动游戏会用它。",
                        j.major,
                        j.path.display(),
                        j.source
                    );
                    resp.reply_zh = msg.clone();
                    resp.action = Some(ActionResult {
                        ok: true,
                        action: "ensure_java".into(),
                        message_zh: msg,
                        path: Some(j.path.display().to_string()),
                        value: Some(serde_json::json!({
                            "major": j.major,
                            "path": j.path.display().to_string(),
                            "source": j.source,
                        })),
                    });
                }
                Err(e) => {
                    let msg = format!(
                        "准备 Java {major} 没成功：{e}\n可以再点一次「修好 Java」，或检查网络后重试。"
                    );
                    resp.reply_zh = msg.clone();
                    resp.action = Some(ActionResult {
                        ok: false,
                        action: "ensure_java".into(),
                        message_zh: msg,
                        path: None,
                        value: Some(serde_json::json!({ "major": major })),
                    });
                }
            }
        } else if action.action == "install_mod" {
            let query = action
                .value
                .as_ref()
                .and_then(|v| v.get("query"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            // Sync CF key so search can include CurseForge when configured.
            if let Ok(settings) = load_settings(&state.paths) {
                sync_api_key_from_settings(settings.curseforge_api_key.as_deref());
            }
            let store = match load_instances(&state.paths) {
                Ok(s) => s,
                Err(e) => {
                    let msg = format!("读取版本列表失败：{e}");
                    resp.reply_zh = msg.clone();
                    resp.action = Some(ActionResult {
                        ok: false,
                        action: "install_mod".into(),
                        message_zh: msg,
                        path: None,
                        value: Some(serde_json::json!({ "query": query })),
                    });
                    return Ok(resp);
                }
            };
            let id = instance_id
                .clone()
                .filter(|s| !s.is_empty())
                .or(store.active_id.clone());
            let Some(id) = id else {
                let msg = format!(
                    "还没有选版本，没法直接安装「{query}」。\n请先在启动页选一个版本，或在助手顶部下拉里选，然后再说「装 {query}」。\n也可以去「下载」页手动搜索。"
                );
                resp.reply_zh = msg.clone();
                resp.action = Some(ActionResult {
                    ok: false,
                    action: "open_store".into(),
                    message_zh: msg,
                    path: None,
                    value: Some(serde_json::json!({ "query": query, "tab": "mod" })),
                });
                return Ok(resp);
            };
            let Some(inst) = store.instances.iter().find(|i| i.id == id).cloned() else {
                let msg = format!("找不到版本 {id}，没法安装「{query}」。");
                resp.reply_zh = msg.clone();
                resp.action = Some(ActionResult {
                    ok: false,
                    action: "install_mod".into(),
                    message_zh: msg,
                    path: None,
                    value: Some(serde_json::json!({ "query": query, "instance_id": id })),
                });
                return Ok(resp);
            };
            if matches!(inst.loader, ModLoader::Vanilla) {
                let msg = format!(
                    "当前版本「{}」是原版，通常装不了模组。\n请先换成 Fabric / Quilt / Forge / NeoForge 版本，再说「装 {query}」。",
                    inst.name
                );
                resp.reply_zh = msg.clone();
                resp.action = Some(ActionResult {
                    ok: false,
                    action: "install_mod".into(),
                    message_zh: msg,
                    path: None,
                    value: Some(serde_json::json!({
                        "query": query,
                        "instance_id": inst.id,
                        "loader": "vanilla",
                    })),
                });
                return Ok(resp);
            }
            let app2 = app.clone();
            match search_and_install_best(&inst, &query, move |p| {
                let _ = app2.emit("install-progress", &p);
            })
            .await
            {
                Ok(r) => {
                    let skipped = if r.skipped { "（已存在，跳过下载）" } else { "" };
                    let msg = format!(
                        "已把「{}」装进版本「{}」。{}\n来源：{}\n版本：{}\n文件：{}\n路径：{}",
                        r.project_name,
                        inst.name,
                        skipped,
                        r.source.label_zh(),
                        r.version_number,
                        r.filename,
                        r.path
                    );
                    resp.reply_zh = msg.clone();
                    resp.action = Some(ActionResult {
                        ok: true,
                        action: "install_mod".into(),
                        message_zh: msg,
                        path: Some(r.path.clone()),
                        value: Some(serde_json::json!({
                            "query": query,
                            "source": r.source.as_str(),
                            "project_id": r.project_id,
                            "project_name": r.project_name,
                            "version_id": r.version_id,
                            "version_number": r.version_number,
                            "filename": r.filename,
                            "path": r.path,
                            "instance_id": inst.id,
                            "instance_name": inst.name,
                            "skipped": r.skipped,
                        })),
                    });
                }
                Err(e) => {
                    let msg = format!(
                        "安装「{query}」到「{}」没成功：{e}\n可以换个关键词再试，或点下方去下载页手动搜。",
                        inst.name
                    );
                    resp.reply_zh = msg.clone();
                    resp.action = Some(ActionResult {
                        ok: false,
                        action: "open_store".into(),
                        message_zh: msg,
                        path: None,
                        value: Some(serde_json::json!({
                            "query": query,
                            "tab": "mod",
                            "instance_id": inst.id,
                            "error": e.to_string(),
                        })),
                    });
                }
            }
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


#[derive(serde::Deserialize)]
struct InstancePatch {
    name: Option<String>,
    icon: Option<String>,
    category: Option<String>,
    description: Option<String>,
    memory_mb: Option<u32>,
    clear_memory: Option<bool>,
    java_path: Option<String>,
    clear_java: Option<bool>,
    width: Option<u32>,
    height: Option<u32>,
    clear_resolution: Option<bool>,
    jvm_args: Option<String>,
    clear_jvm_args: Option<bool>,
    account_id: Option<String>,
}

#[tauri::command]
fn update_instance_cmd(
    state: State<'_, AppState>,
    instance_id: String,
    patch: InstancePatch,
) -> Result<Instance, String> {
    let paths = state_paths(&state);
    update_instance_fields(paths, &instance_id, |inst| {
        if let Some(n) = &patch.name {
            let t = n.trim();
            if !t.is_empty() { inst.name = t.to_string(); }
        }
        if let Some(i) = &patch.icon { inst.icon = if i.is_empty() { None } else { Some(i.clone()) }; }
        if let Some(c) = &patch.category { inst.category = if c.is_empty() { None } else { Some(c.clone()) }; }
        if let Some(d) = &patch.description { inst.description = if d.is_empty() { None } else { Some(d.clone()) }; }
        if patch.clear_memory == Some(true) { inst.memory_mb = None; }
        else if let Some(m) = patch.memory_mb { inst.memory_mb = Some(m.max(512)); }
        if patch.clear_java == Some(true) { inst.java_path = None; }
        else if let Some(j) = &patch.java_path { inst.java_path = if j.trim().is_empty() { None } else { Some(j.clone()) }; }
        if patch.clear_resolution == Some(true) { inst.width = None; inst.height = None; }
        else {
            if let Some(w) = patch.width { inst.width = Some(w); }
            if let Some(h) = patch.height { inst.height = Some(h); }
        }
        if patch.clear_jvm_args == Some(true) { inst.jvm_args = None; }
        else if let Some(a) = &patch.jvm_args { inst.jvm_args = if a.trim().is_empty() { None } else { Some(a.clone()) }; }
        if let Some(a) = &patch.account_id { inst.account_id = if a.is_empty() { None } else { Some(a.clone()) }; }
    }).map_err(|e| e.to_string())
}

#[tauri::command]
fn rename_instance_cmd(state: State<'_, AppState>, instance_id: String, name: String) -> Result<Instance, String> {
    update_instance_cmd(state, instance_id, InstancePatch {
        name: Some(name), icon: None, category: None, description: None,
        memory_mb: None, clear_memory: None, java_path: None, clear_java: None,
        width: None, height: None, clear_resolution: None, jvm_args: None, clear_jvm_args: None,
        account_id: None,
    })
}

#[tauri::command]
fn duplicate_instance_cmd(state: State<'_, AppState>, instance_id: String, name: Option<String>) -> Result<Instance, String> {
    duplicate_instance(state_paths(&state), &instance_id, name.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_instance_cmd(state: State<'_, AppState>, instance_id: String, delete_files: Option<bool>) -> Result<(), String> {
    remove_instance(state_paths(&state), &instance_id, delete_files.unwrap_or(true)).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_instance_folder_cmd(state: State<'_, AppState>, instance_id: String, kind: Option<String>) -> Result<String, String> {
    let paths = state_paths(&state);
    let store = load_instances(paths).map_err(|e| e.to_string())?;
    let inst = store.instances.iter().find(|i| i.id == instance_id).ok_or_else(|| format!("找不到版本: {instance_id}"))?;
    let dir = match kind.as_deref().unwrap_or("root") {
        "root" | "game" => std::path::PathBuf::from(&inst.game_dir),
        k => local_dir(inst, k).map_err(|e| e.to_string())?,
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    open::that(&dir).map_err(|e| format!("无法打开文件夹: {e}"))?;
    Ok(dir.to_string_lossy().into_owned())
}

#[tauri::command]
fn write_launch_script_cmd(state: State<'_, AppState>, instance_id: String) -> Result<serde_json::Value, String> {
    let paths = state_paths(&state);
    let store = load_instances(paths).map_err(|e| e.to_string())?;
    let accounts = load_accounts(paths).map_err(|e| e.to_string())?;
    let settings = load_settings(paths).map_err(|e| e.to_string())?;
    let inst = store.instances.iter().find(|i| i.id == instance_id).ok_or_else(|| format!("找不到版本: {instance_id}"))?;
    let acc_id = inst.account_id.clone().or(accounts.active_id.clone()).ok_or_else(|| "请先添加一个账户".to_string())?;
    let acc = accounts.accounts.iter().find(|a| a.id == acc_id).ok_or_else(|| "账户不存在".to_string())?;
    let mem = inst.memory_mb.unwrap_or(settings.memory_mb);
    let path = write_launch_script(paths, inst, acc, &settings, &LaunchRequest { memory_mb: mem, extra_jvm_args: vec![] }).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "path": path, "message_zh": format!("已生成启动脚本（令牌已脱敏）\\n{}", path.display()) }))
}

#[tauri::command]
fn list_local_content_cmd(state: State<'_, AppState>, instance_id: String, kind: String) -> Result<Vec<LocalEntry>, String> {
    let store = load_instances(state_paths(&state)).map_err(|e| e.to_string())?;
    let inst = store.instances.iter().find(|i| i.id == instance_id).ok_or_else(|| format!("找不到版本: {instance_id}"))?;
    list_local(inst, &kind).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_local_content_cmd(state: State<'_, AppState>, instance_id: String, kind: String, name: String) -> Result<String, String> {
    let store = load_instances(state_paths(&state)).map_err(|e| e.to_string())?;
    let inst = store.instances.iter().find(|i| i.id == instance_id).ok_or_else(|| format!("找不到版本: {instance_id}"))?;
    delete_local(inst, &kind, &name).map_err(|e| e.to_string())
}

fn parse_project_type(s: &str) -> Result<ProjectType, String> {
    ProjectType::parse(s).ok_or_else(|| format!("未知类型: {s}"))
}

#[tauri::command]
async fn search_content_cmd(
    source: String,
    project_type: String,
    query: Option<String>,
    game_version: Option<String>,
    loader: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<ModProject>, String> {
    let source = parse_mod_source(&source)?;
    let pt = parse_project_type(&project_type)?;
    let q = SearchQuery {
        project_type: pt,
        text: query.unwrap_or_default(),
        game_version,
        loader: loader.and_then(|s| ModLoader::parse_loose(&s)),
        limit: limit.unwrap_or(20).clamp(1, 50),
        offset: 0,
    };
    search_mods(source, &q).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn install_content_cmd(
    state: State<'_, AppState>,
    source: String,
    project_type: String,
    project_id: String,
    version_id: Option<String>,
    instance_id: Option<String>,
    world: Option<String>,
) -> Result<ContentInstallResult, String> {
    let source = parse_mod_source(&source)?;
    let pt = parse_project_type(&project_type)?;
    let store = load_instances(state_paths(&state)).map_err(|e| e.to_string())?;
    let id = instance_id.or(store.active_id.clone()).ok_or_else(|| "请先选择一个版本".to_string())?;
    let inst = store.instances.iter().find(|i| i.id == id).ok_or_else(|| format!("找不到版本: {id}"))?.clone();
    install_content(&inst, source, pt, &project_id, version_id.as_deref(), world.as_deref()).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn install_modpack_cmd(
    app: AppHandle,
    state: State<'_, AppState>,
    source: String,
    project_id: String,
    version_id: Option<String>,
    name: Option<String>,
) -> Result<ModpackInstallResult, String> {
    let source = parse_mod_source(&source)?;
    let paths = state_paths(&state).clone();
    let app2 = app.clone();
    install_modpack(&paths, source, &project_id, version_id.as_deref(), name.as_deref(), move |p| {
        let _ = app2.emit("install-progress", &p);
    }).await.map_err(|e| e.to_string())
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
            list_accounts_state,
            set_active_account,
            remove_account,
            create_offline,
            start_microsoft_login,
            update_settings_cmd,
            complete_onboarding_cmd,
            msa_status_cmd,
            msa_device_code_cmd,
            msa_poll_cmd,
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
            update_instance_cmd,
            rename_instance_cmd,
            duplicate_instance_cmd,
            delete_instance_cmd,
            open_instance_folder_cmd,
            write_launch_script_cmd,
            list_local_content_cmd,
            delete_local_content_cmd,
            search_content_cmd,
            install_content_cmd,
            install_modpack_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running EasyMC");
}
