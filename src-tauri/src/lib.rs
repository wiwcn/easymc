use std::sync::Arc;

use easymc_core::{
    begin_microsoft_login, create_named_instance_with, create_offline_account, duplicate_instance,
    ensure_defaults, load_accounts, load_instances, load_settings, remove_instance, save_accounts,
    save_instances, save_settings, set_active_instance, update_instance_fields, upsert_instance,
    add_instance_playtime,
    Account, AppMode, CreateInstanceOpts, DownloadSource, Instance, McVersion,
    MicrosoftAuthConfig, ModLoader, Paths, Settings,
    fetch_littleskin_by_name,
    fetch_official,
    set_active_wardrobe,
    stage_active_for_launch,
    active_wardrobe_dto,
    wardrobe_import_resolved,
    wardrobe_remove,
    wardrobe_replace,
    wardrobe_state,
    wardrobe_upload,
    ResolvedSkin,
    WardrobeEntryDto,
    WardrobeStateDto,
    SkinModel,
    read_local_file,
};
use easymc_launch_desktop::{
    create_desktop_shortcut, detect_java, detect_minecraft_dirs, ensure_java, prepare_and_launch,
    write_launch_script, JavaInstall, LaunchRequest, LaunchStatus, MinecraftDir, ProcessManager,
};
use easymc_meta::{
    install_fabric, install_forge, install_neoforge, install_quilt,
    install_version, is_loader_installed, is_version_installed, list_download_mirror_candidates,
    list_loaders_with_source, probe_download_mirrors, InstallOptions, LoaderKind, MetaProvider,
    PistonMetaProvider,
};
use easymc_share::{
    export_instance, import_instance, ExportOptions, ExportResult, ImportOptions, ImportResult,
    ShareMode,
};
use easymc_agent::{
    apply_proposal, ask_with_progress, bump_memory, clear_llm_config, llm_status, parse_proposal, read_llm_config,
    run_diagnose, save_llm_config, show_data_dir, try_load_dotenv_llm, undo_proposal,
    write_diagnostic_bundle, refresh_ask_tool, ActionResult, AskRequest, AskResponse, ChatMessage, DiagnoseReport,
    AgentStepEvent, LlmConfigView, LlmStatus,
};
use easymc_container::{
    apply_game_layer, apply_one_click_setup, attach_container_to_instance,
    create_container_version, delete_container_version, discard_game_layer,
    ensure_runtime_image, export_env_pack, import_env_pack, install_guidance, layer_state_for,
    list_container_versions, list_mirror_profiles, list_registry_mirror_candidates,
    preheat_container_runtime, preheat_container_runtime_ex, preheat_status,
    probe_container_runtime, probe_registry_mirrors, provision_container_version,
    read_env_manifest_from_path, repair_container_runtime_with_profile,
    resolve_agent_workspace, write_china_registries_config, write_registries_config_for_profile,
    EnvExportOptions, EnvImportOptions, EnvPackMode, MirrorProfile,
};
use easymc_mods::{
    api_key, api_key_source, build_dep_graph, delete_local, fetch_missing_deps, install_content,
    install_mod, install_modpack, list_installed_mod_entries, list_local, list_mod_versions, local_dir,
    remove_mod, resolve_instance_icon, search_and_install_best, search_mods, set_mod_disabled,
    sync_api_key_from_settings, ApiKeySource, ContentInstallResult, DepGraphReport, FetchMissingResult,
    InstalledModEntry, LocalEntry, ModInstallResult, ModProject, ModSource, ModVersion,
    ModpackInstallResult, ProjectType, SearchQuery, API_KEY_ENV, CONSOLE_URL,
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
        "milestone": "M1.8",
        "data_dir": state.paths.root,
        "msa_configured": msa.is_configured(),
        "msa_temporary_client": msa.using_temporary_client_id,
    })
}

/// Instance row for UI — core fields plus resolved display `icon_url` (PCL-style).
#[derive(Serialize)]
struct InstanceListItem {
    #[serde(flatten)]
    instance: Instance,
    /// Resolved pack / marketplace / local icon (`http(s)` or `data:`). `None` → UI loader default.
    #[serde(skip_serializing_if = "Option::is_none")]
    icon_url: Option<String>,
}

#[derive(Serialize)]
struct InstancesStateDto {
    instances: Vec<InstanceListItem>,
    active_id: Option<String>,
}

fn instance_list_item(inst: Instance) -> InstanceListItem {
    let icon_url = resolve_instance_icon(&inst);
    InstanceListItem {
        instance: inst,
        icon_url,
    }
}

#[derive(Serialize)]
struct VersionListItem {
    id: String,
    #[serde(rename = "type")]
    version_type: String,
    url: String,
    installed: bool,
}

// ——— Skin wardrobe (3D preview data + official / LittleSkin import) ———

#[tauri::command]
fn wardrobe_list_cmd(state: State<'_, AppState>) -> Result<WardrobeStateDto, String> {
    let paths = state_paths(&state);
    let _ = ensure_defaults(paths);
    wardrobe_state(paths).map_err(|e| e.to_string())
}

#[tauri::command]
fn wardrobe_set_active_cmd(
    state: State<'_, AppState>,
    id: Option<String>,
) -> Result<WardrobeStateDto, String> {
    set_active_wardrobe(state_paths(&state), id.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn wardrobe_upload_cmd(
    state: State<'_, AppState>,
    skin_path: String,
    cape_path: Option<String>,
    name: Option<String>,
    model: Option<String>,
) -> Result<WardrobeEntryDto, String> {
    let paths = state_paths(&state);
    let skin = read_local_file(&skin_path).map_err(|e| e.to_string())?;
    let cape = match cape_path.as_deref() {
        Some(p) if !p.is_empty() => Some(read_local_file(p).map_err(|e| e.to_string())?),
        _ => None,
    };
    let m = model.map(|s| SkinModel::parse(&s));
    wardrobe_upload(paths, &skin, cape.as_deref(), name.as_deref(), m).map_err(|e| e.to_string())
}

#[tauri::command]
fn wardrobe_replace_cmd(
    state: State<'_, AppState>,
    id: String,
    skin_path: Option<String>,
    cape_path: Option<String>,
    name: Option<String>,
    model: Option<String>,
    clear_cape: Option<bool>,
) -> Result<WardrobeEntryDto, String> {
    let paths = state_paths(&state);
    let skin = match skin_path.as_deref() {
        Some(p) if !p.is_empty() => Some(read_local_file(p).map_err(|e| e.to_string())?),
        _ => None,
    };
    let cape = match cape_path.as_deref() {
        Some(p) if !p.is_empty() => Some(read_local_file(p).map_err(|e| e.to_string())?),
        _ => None,
    };
    let m = model.map(|s| SkinModel::parse(&s));
    wardrobe_replace(
        paths,
        &id,
        skin.as_deref(),
        cape.as_deref(),
        name.as_deref(),
        m,
        clear_cape.unwrap_or(false),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn wardrobe_remove_cmd(state: State<'_, AppState>, id: String) -> Result<WardrobeStateDto, String> {
    wardrobe_remove(state_paths(&state), &id).map_err(|e| e.to_string())
}

#[tauri::command]
fn wardrobe_active_cmd(
    state: State<'_, AppState>,
) -> Result<Option<easymc_core::WardrobeEntryDto>, String> {
    active_wardrobe_dto(state_paths(&state)).map_err(|e| e.to_string())
}

#[tauri::command]
fn wardrobe_stage_active_cmd(
    state: State<'_, AppState>,
    game_dir: Option<String>,
    player_name: Option<String>,
) -> Result<Option<easymc_core::StagedActiveSkin>, String> {
    let gd = game_dir.as_deref().map(std::path::Path::new);
    stage_active_for_launch(state_paths(&state), gd, player_name.as_deref())
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
struct ResolvedSkinDto {
    name: String,
    uuid: Option<String>,
    model: String,
    skin_url: String,
    cape_url: Option<String>,
    source: String,
    source_label: String,
    skin_data_url: Option<String>,
    cape_data_url: Option<String>,
}

async fn resolved_to_dto(r: ResolvedSkin, with_bytes: bool) -> Result<ResolvedSkinDto, String> {
    use base64::Engine;
    let mut skin_data_url = None;
    let mut cape_data_url = None;
    if with_bytes {
        let skin = easymc_core::download_texture_bytes(&r.skin_url)
            .await
            .map_err(|e| e.to_string())?;
        skin_data_url = Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&skin)
        ));
        if let Some(ref u) = r.cape_url {
            if let Ok(c) = easymc_core::download_texture_bytes(u).await {
                cape_data_url = Some(format!(
                    "data:image/png;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(&c)
                ));
            }
        }
    }
    Ok(ResolvedSkinDto {
        name: r.name,
        uuid: r.uuid,
        model: r.model.as_str().into(),
        skin_url: r.skin_url,
        cape_url: r.cape_url,
        source: r.source.as_str().into(),
        source_label: r.source.label_zh().into(),
        skin_data_url,
        cape_data_url,
    })
}

#[tauri::command]
async fn skin_fetch_official_cmd(query: String) -> Result<ResolvedSkinDto, String> {
    let r = fetch_official(&query).await.map_err(|e| e.to_string())?;
    resolved_to_dto(r, true).await
}

#[tauri::command]
async fn skin_fetch_littleskin_cmd(name: String) -> Result<ResolvedSkinDto, String> {
    let r = fetch_littleskin_by_name(&name)
        .await
        .map_err(|e| e.to_string())?;
    resolved_to_dto(r, true).await
}

#[tauri::command]
async fn wardrobe_import_official_cmd(
    state: State<'_, AppState>,
    query: String,
    name: Option<String>,
) -> Result<WardrobeEntryDto, String> {
    let paths = state_paths(&state);
    let r = fetch_official(&query).await.map_err(|e| e.to_string())?;
    wardrobe_import_resolved(paths, &r, name.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn wardrobe_import_littleskin_cmd(
    state: State<'_, AppState>,
    player: String,
    name: Option<String>,
) -> Result<WardrobeEntryDto, String> {
    let paths = state_paths(&state);
    let r = fetch_littleskin_by_name(&player)
        .await
        .map_err(|e| e.to_string())?;
    wardrobe_import_resolved(paths, &r, name.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_instances(state: State<'_, AppState>) -> Result<InstancesStateDto, String> {
    let (store, _) = ensure_defaults(state_paths(&state)).map_err(|e| e.to_string())?;
    Ok(InstancesStateDto {
        instances: store.instances.into_iter().map(instance_list_item).collect(),
        active_id: store.active_id,
    })
}

/// Resolve one instance's display icon (pack.png / cached URL). UI may call after local file drops.
#[tauri::command]
fn resolve_instance_icon_cmd(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Option<String>, String> {
    let (store, _) = ensure_defaults(state_paths(&state)).map_err(|e| e.to_string())?;
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| "找不到这个版本".to_string())?;
    Ok(resolve_instance_icon(inst))
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
        instances: store.instances.into_iter().map(instance_list_item).collect(),
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
    /// Absolute path to `java` binary; empty string clears (auto-pick highest).
    preferred_java: Option<String>,
    memory_mb: Option<u32>,
    msa_client_id: Option<String>,
    clear_msa_client_id: Option<bool>,
    download_source: Option<String>,
    container_mode_enabled: Option<bool>,
    container_china_mirrors: Option<bool>,
    container_setup_done: Option<bool>,
    container_saves_require_propose: Option<bool>,
    container_auto_preheat: Option<bool>,
    /// Mirror profile id: daocloud | official (maps to container_china_mirrors).
    container_mirror_profile: Option<String>,
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
    if let Some(java) = patch.preferred_java {
        let t = java.trim().to_string();
        settings.preferred_java = if t.is_empty() { None } else { Some(t) };
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
    if let Some(v) = patch.container_mode_enabled {
        settings.container_mode_enabled = v;
    }
    if let Some(v) = patch.container_china_mirrors {
        settings.container_china_mirrors = v;
    }
    if let Some(v) = patch.container_setup_done {
        settings.container_setup_done = v;
    }
    if let Some(v) = patch.container_saves_require_propose {
        settings.container_saves_require_propose = v;
    }
    if let Some(v) = patch.container_auto_preheat {
        settings.container_auto_preheat = v;
    }
    if let Some(profile) = patch.container_mirror_profile.as_deref() {
        match MirrorProfile::parse(profile) {
            Some(p) => {
                settings.container_mirror_profile = p.as_str().into();
                settings.container_china_mirrors = p.prefer_china();
                // Best-effort: rewrite registries drop-in to match the pick.
                let _ = write_registries_config_for_profile(p);
            }
            None => {
                return Err(format!(
                    "未知镜像源: {profile}（daocloud / dockerproxy / onems / official）"
                ))
            }
        }
    } else if patch.container_china_mirrors.is_some() {
        // Keep profile id aligned when only the bool is patched.
        settings.container_mirror_profile = MirrorProfile::from_prefer_china(
            settings.container_china_mirrors,
        )
        .as_str()
        .into();
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
fn detect_minecraft_dirs_cmd() -> Vec<MinecraftDir> {
    detect_minecraft_dirs()
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


async fn apply_finished_playtime(paths: &Paths, procs: &ProcessManager, app: &AppHandle) {
    for f in procs.take_finished().await {
        match add_instance_playtime(paths, &f.instance_id, f.play_secs) {
            Ok(total) => {
                let _ = app.emit(
                    "game-session-ended",
                    serde_json::json!({
                        "instance_id": f.instance_id,
                        "play_secs": f.play_secs,
                        "total_play_secs": total,
                    }),
                );
            }
            Err(e) => {
                let _ = app.emit("launch-log", format!("记录游玩时长失败: {e}"));
            }
        }
    }
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

    // Watch for process exit / stop and accumulate playtime.
    let paths_watch = paths.clone();
    let procs_watch = state.procs.clone();
    let app_watch = app.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(750)).await;
            apply_finished_playtime(&paths_watch, &procs_watch, &app_watch).await;
            if !procs_watch.is_running().await {
                apply_finished_playtime(&paths_watch, &procs_watch, &app_watch).await;
                break;
            }
        }
    });

    Ok(LaunchResult {
        ok: true,
        message: status.message,
        pid: status.pid,
    })
}

#[tauri::command]
async fn stop_instance(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.procs.stop().await.map_err(|e| e.to_string())?;
    apply_finished_playtime(state_paths(&state), &state.procs, &app).await;
    Ok(())
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
) -> Result<Vec<InstalledModEntry>, String> {
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
    list_installed_mod_entries(inst).map_err(|e| e.to_string())
}

#[tauri::command]
fn mod_dep_graph_cmd(
    state: State<'_, AppState>,
    instance_id: Option<String>,
) -> Result<DepGraphReport, String> {
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
    build_dep_graph(inst).map_err(|e| e.to_string())
}

#[tauri::command]
async fn fetch_missing_mod_deps_cmd(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: Option<String>,
    only_ids: Option<Vec<String>>,
) -> Result<FetchMissingResult, String> {
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
    fetch_missing_deps(&inst, only_ids, move |p| {
        let _ = app2.emit("install-progress", &p);
    })
    .await
    .map_err(|e| e.to_string())
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
fn agent_llm_config_get() -> LlmConfigView {
    read_llm_config()
}

#[tauri::command]
fn agent_llm_config_save(
    base_url: String,
    api_key: String,
    model: String,
) -> Result<LlmStatus, String> {
    save_llm_config(&base_url, &api_key, &model).map_err(|e| e.to_string())
}

#[tauri::command]
fn agent_llm_config_clear() -> Result<LlmStatus, String> {
    clear_llm_config().map_err(|e| e.to_string())
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


/// Theme → Modrinth search query for complementary suggestions.
fn suggest_theme_query(theme: &str) -> (&'static str, &'static str) {
    match theme.trim().to_ascii_lowercase().as_str() {
        "tech" | "科技" | "工业" => ("create", "科技"),
        "magic" | "魔法" => ("botania", "魔法"),
        "performance" | "性能" | "优化" => ("sodium", "性能"),
        "qos" | "生活质量" | "便利" => ("jade", "生活质量"),
        "adventure" | "冒险" => ("origins", "冒险"),
        "building" | "建筑" => ("chisel", "建筑"),
        "shader" | "光影" => ("iris", "光影"),
        _ => ("farmers-delight", "生存"),
    }
}

fn format_mod_hits(hits: &[ModProject], limit: usize) -> (String, Vec<serde_json::Value>) {
    let mut lines = Vec::new();
    let mut items = Vec::new();
    for (i, p) in hits.iter().take(limit).enumerate() {
        let desc = p.description.chars().take(60).collect::<String>();
        lines.push(format!(
            "{}. {}（{} · {} 下载）\n   {}",
            i + 1,
            p.name,
            p.source.label_zh(),
            p.downloads,
            if desc.is_empty() { "—" } else { &desc }
        ));
        items.push(serde_json::json!({
            "id": p.id,
            "slug": p.slug,
            "name": p.name,
            "description": p.description,
            "source": p.source.as_str(),
            "downloads": p.downloads,
            "author": p.author,
        }));
    }
    (lines.join("\n"), items)
}

async fn agent_search_mod_projects(
    query: &str,
    game_version: Option<String>,
    loader: Option<easymc_core::ModLoader>,
    limit: usize,
) -> Result<Vec<ModProject>, String> {
    let limit = limit.clamp(1, 20);
    let mut q = SearchQuery {
        project_type: Default::default(),
        text: query.to_string(),
        game_version: game_version.clone(),
        loader,
        limit,
        offset: 0,
    };
    let mut hits = search_mods(ModSource::Modrinth, &q)
        .await
        .unwrap_or_default();
    // Broaden if version/loader filter returned nothing.
    if hits.is_empty() && (q.game_version.is_some() || q.loader.is_some()) {
        q.game_version = None;
        q.loader = None;
        hits = search_mods(ModSource::Modrinth, &q)
            .await
            .unwrap_or_default();
    }
    if api_key().is_some() {
        if let Ok(cf) = search_mods(ModSource::CurseForge, &q).await {
            for p in cf {
                if !hits.iter().any(|h| {
                    h.slug.eq_ignore_ascii_case(&p.slug) || h.name.eq_ignore_ascii_case(&p.name)
                }) {
                    hits.push(p);
                }
            }
        }
    }
    hits.sort_by(|a, b| b.downloads.cmp(&a.downloads));
    Ok(hits)
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
    let app_for_steps = app.clone();
    let on_step = move |ev: AgentStepEvent| {
        let _ = app_for_steps.emit("agent-step", &ev);
    };
    let mut resp = ask_with_progress(
        &state.paths,
        AskRequest {
            text,
            instance_id: instance_id.clone(),
            process_logs,
            rules_only: rules_only.unwrap_or(false),
            history: history.unwrap_or_default(),
        },
        Some(&on_step),
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
        } else if action.action == "search_mods" {
            let query = action
                .value
                .as_ref()
                .and_then(|v| v.get("query"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if query.is_empty() {
                let msg = "请告诉我要搜什么模组，例如「搜模组 sodium」。".to_string();
                resp.reply_zh = msg.clone();
                resp.action = Some(ActionResult {
                    ok: false,
                    action: "search_mods".into(),
                    message_zh: msg,
                    path: None,
                    value: Some(serde_json::json!({ "query": "" })),
                });
            } else {
                if let Ok(settings) = load_settings(&state.paths) {
                    sync_api_key_from_settings(settings.curseforge_api_key.as_deref());
                }
                let (game_version, loader) = {
                    let store = load_instances(&state.paths).ok();
                    let id = instance_id
                        .clone()
                        .filter(|s| !s.is_empty())
                        .or_else(|| store.as_ref().and_then(|s| s.active_id.clone()));
                    let inst = id.and_then(|id| {
                        store.as_ref().and_then(|s| {
                            s.instances.iter().find(|i| i.id == id).cloned()
                        })
                    });
                    (
                        inst.as_ref().map(|i| i.version.0.clone()),
                        inst.as_ref().and_then(|i| {
                            if matches!(i.loader, ModLoader::Vanilla) {
                                None
                            } else {
                                Some(i.loader)
                            }
                        }),
                    )
                };
                match agent_search_mod_projects(&query, game_version.clone(), loader, 8).await {
                    Ok(hits) if hits.is_empty() => {
                        let msg = format!(
                            "没搜到「{query}」。可以换个英文关键词，或点下方去下载页再试。"
                        );
                        resp.reply_zh = msg.clone();
                        resp.action = Some(ActionResult {
                            ok: false,
                            action: "search_mods".into(),
                            message_zh: msg,
                            path: None,
                            value: Some(serde_json::json!({
                                "query": query,
                                "tab": "mod",
                                "count": 0,
                                "game_version": game_version,
                            })),
                        });
                    }
                    Ok(hits) => {
                        let (body, items) = format_mod_hits(&hits, 8);
                        let msg = format!(
                            "找到 {} 个与「{query}」相关的模组：\n{}\n\n可以说「装 <名字>」装进当前版本，或去下载页细看。",
                            items.len(),
                            body
                        );
                        resp.reply_zh = msg.clone();
                        resp.action = Some(ActionResult {
                            ok: true,
                            action: "search_mods".into(),
                            message_zh: msg,
                            path: None,
                            value: Some(serde_json::json!({
                                "query": query,
                                "tab": "mod",
                                "count": items.len(),
                                "game_version": game_version,
                                "results": items,
                            })),
                        });
                    }
                    Err(e) => {
                        let msg = format!("搜索「{query}」失败：{e}");
                        resp.reply_zh = msg.clone();
                        resp.action = Some(ActionResult {
                            ok: false,
                            action: "search_mods".into(),
                            message_zh: msg,
                            path: None,
                            value: Some(serde_json::json!({ "query": query, "tab": "mod" })),
                        });
                    }
                }
            }
        } else if action.action == "suggest_mods" {
            let theme = action
                .value
                .as_ref()
                .and_then(|v| v.get("theme"))
                .and_then(|v| v.as_str())
                .unwrap_or("survival")
                .to_string();
            let (q, theme_zh) = suggest_theme_query(&theme);
            if let Ok(settings) = load_settings(&state.paths) {
                sync_api_key_from_settings(settings.curseforge_api_key.as_deref());
            }
            let (game_version, loader, instance_name) = {
                let store = load_instances(&state.paths).ok();
                let id = instance_id
                    .clone()
                    .filter(|s| !s.is_empty())
                    .or_else(|| store.as_ref().and_then(|s| s.active_id.clone()));
                let inst = id.and_then(|id| {
                    store.as_ref().and_then(|s| {
                        s.instances.iter().find(|i| i.id == id).cloned()
                    })
                });
                (
                    inst.as_ref().map(|i| i.version.0.clone()),
                    inst.as_ref().and_then(|i| {
                        if matches!(i.loader, ModLoader::Vanilla) {
                            None
                        } else {
                            Some(i.loader)
                        }
                    }),
                    inst.as_ref().map(|i| i.name.clone()),
                )
            };
            match agent_search_mod_projects(q, game_version.clone(), loader, 8).await {
                Ok(hits) if hits.is_empty() => {
                    let msg = format!("暂时没找到「{theme_zh}」主题的推荐模组，稍后再试或去下载页逛逛。");
                    resp.reply_zh = msg.clone();
                    resp.action = Some(ActionResult {
                        ok: false,
                        action: "suggest_mods".into(),
                        message_zh: msg,
                        path: None,
                        value: Some(serde_json::json!({
                            "theme": theme,
                            "theme_zh": theme_zh,
                            "query": q,
                            "count": 0,
                        })),
                    });
                }
                Ok(hits) => {
                    let (body, items) = format_mod_hits(&hits, 8);
                    let inst_note = instance_name
                        .as_ref()
                        .map(|n| format!("（参考当前版本「{n}」的加载器/版本过滤）"))
                        .unwrap_or_default();
                    let msg = format!(
                        "按「{theme_zh}」主题为你挑了这些热门模组{inst_note}：\n{}\n\n可以说「装 <名字>」装进当前版本。",
                        body
                    );
                    resp.reply_zh = msg.clone();
                    resp.action = Some(ActionResult {
                        ok: true,
                        action: "suggest_mods".into(),
                        message_zh: msg,
                        path: None,
                        value: Some(serde_json::json!({
                            "theme": theme,
                            "theme_zh": theme_zh,
                            "query": q,
                            "count": items.len(),
                            "game_version": game_version,
                            "instance_name": instance_name,
                            "results": items,
                        })),
                    });
                }
                Err(e) => {
                    let msg = format!("推荐「{theme_zh}」模组失败：{e}");
                    resp.reply_zh = msg.clone();
                    resp.action = Some(ActionResult {
                        ok: false,
                        action: "suggest_mods".into(),
                        message_zh: msg,
                        path: None,
                        value: Some(serde_json::json!({ "theme": theme })),
                    });
                }
            }
        }
    }
    // fetch_missing_mod_deps: download via Modrinth (host-deferred from agent)
    if let Some(action) = resp.action.clone() {
        if action.action == "fetch_missing_mod_deps" {
            let skipped = action
                .value
                .as_ref()
                .and_then(|v| v.get("skipped"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !skipped {
                let only_ids = match (
                    action
                        .value
                        .as_ref()
                        .and_then(|v| v.get("only_ids"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                .collect::<Vec<_>>()
                        }),
                    action
                        .value
                        .as_ref()
                        .and_then(|v| v.get("targets"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                .collect::<Vec<_>>()
                        }),
                ) {
                    (Some(ids), _) if !ids.is_empty() => Some(ids),
                    (_, Some(t)) if !t.is_empty() => Some(t),
                    _ => None,
                };
                let store = match load_instances(&state.paths) {
                    Ok(s) => s,
                    Err(e) => {
                        let msg = format!("读取版本列表失败：{e}");
                        resp.reply_zh = msg.clone();
                        resp.action = Some(ActionResult {
                            ok: false,
                            action: "fetch_missing_mod_deps".into(),
                            message_zh: msg,
                            path: None,
                            value: None,
                        });
                        return Ok(resp);
                    }
                };
                let id = action
                    .value
                    .as_ref()
                    .and_then(|v| v.get("instance_id"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty())
                    .or(instance_id.clone())
                    .or(store.active_id.clone());
                let Some(id) = id else {
                    let msg = "还没有选版本，没法补依赖。请先选一个版本。".to_string();
                    resp.reply_zh = msg.clone();
                    resp.action = Some(ActionResult {
                        ok: false,
                        action: "fetch_missing_mod_deps".into(),
                        message_zh: msg,
                        path: None,
                        value: None,
                    });
                    return Ok(resp);
                };
                let Some(inst) = store.instances.iter().find(|i| i.id == id).cloned() else {
                    let msg = format!("找不到版本 {id}");
                    resp.reply_zh = msg.clone();
                    resp.action = Some(ActionResult {
                        ok: false,
                        action: "fetch_missing_mod_deps".into(),
                        message_zh: msg,
                        path: None,
                        value: Some(serde_json::json!({ "instance_id": id })),
                    });
                    return Ok(resp);
                };
                if let Ok(settings) = load_settings(&state.paths) {
                    sync_api_key_from_settings(settings.curseforge_api_key.as_deref());
                }
                let app2 = app.clone();
                match fetch_missing_deps(&inst, only_ids, move |p| {
                    let _ = app2.emit("install-progress", &p);
                })
                .await
                {
                    Ok(r) => {
                        let msg = r.message_zh.clone();
                        resp.reply_zh = msg.clone();
                        resp.action = Some(ActionResult {
                            ok: r.failed.is_empty() || !r.installed.is_empty(),
                            action: "fetch_missing_mod_deps".into(),
                            message_zh: msg,
                            path: Some(
                                std::path::PathBuf::from(&inst.game_dir)
                                    .join("mods")
                                    .display()
                                    .to_string(),
                            ),
                            value: Some(serde_json::json!({
                                "instance_id": inst.id,
                                "instance_name": inst.name,
                                "attempted": r.attempted,
                                "installed": r.installed.iter().map(|i| serde_json::json!({
                                    "project_name": i.project_name,
                                    "filename": i.filename,
                                    "path": i.path,
                                })).collect::<Vec<_>>(),
                                "failed": r.failed.iter().map(|f| serde_json::json!({
                                    "mod_id": f.mod_id,
                                    "error_zh": f.error_zh,
                                    "reason": f.reason,
                                    "tried_slug": f.tried_slug,
                                })).collect::<Vec<_>>(),
                            })),
                        });
                    }
                    Err(e) => {
                        let msg = format!("补全缺失依赖失败：{e}");
                        resp.reply_zh = msg.clone();
                        resp.action = Some(ActionResult {
                            ok: false,
                            action: "fetch_missing_mod_deps".into(),
                            message_zh: msg,
                            path: None,
                            value: Some(serde_json::json!({
                                "instance_id": inst.id,
                                "instance_name": inst.name,
                            })),
                        });
                    }
                }
            }
        }
    }
    // setup_env: compose ensure_java + status already in action.message
    if let Some(action) = resp.action.clone() {
        if action.action == "setup_env" {
            let major = action
                .value
                .as_ref()
                .and_then(|v| v.get("major"))
                .and_then(|v| v.as_u64())
                .unwrap_or(21) as u32;
            let paths = state.paths.clone();
            let app2 = app.clone();
            let base_msg = action.message_zh.clone();
            let download_zh = action
                .value
                .as_ref()
                .and_then(|v| v.get("download_source_zh"))
                .and_then(|v| v.as_str())
                .unwrap_or("官方")
                .to_string();
            let memory_mb = action
                .value
                .as_ref()
                .and_then(|v| v.get("memory_mb"))
                .and_then(|v| v.as_u64())
                .unwrap_or(2048);
            match ensure_java(&paths, major, move |line| {
                let _ = app2.emit("launch-log", line);
            })
            .await
            {
                Ok(j) => {
                    let msg = format!(
                        "{base_msg}\n\nJava {major} 已就绪。\n路径：{}\n来源：{}\n下载源：{download_zh}\n内存约 {memory_mb} MB。\n下次启动游戏会用这套环境。",
                        j.path.display(),
                        j.source
                    );
                    resp.reply_zh = msg.clone();
                    resp.action = Some(ActionResult {
                        ok: true,
                        action: "setup_env".into(),
                        message_zh: msg,
                        path: Some(j.path.display().to_string()),
                        value: Some(serde_json::json!({
                            "major": j.major,
                            "path": j.path.display().to_string(),
                            "source": j.source,
                            "download_source_zh": download_zh,
                            "memory_mb": memory_mb,
                        })),
                    });
                }
                Err(e) => {
                    let msg = format!(
                        "{base_msg}\n\n准备 Java {major} 没成功：{e}\n可检查网络后再点「一键环境」。"
                    );
                    resp.reply_zh = msg.clone();
                    resp.action = Some(ActionResult {
                        ok: false,
                        action: "setup_env".into(),
                        message_zh: msg,
                        path: None,
                        value: Some(serde_json::json!({ "major": major })),
                    });
                }
            }
        } else if action.action == "build_pack" {
            let id = action
                .value
                .as_ref()
                .and_then(|v| v.get("instance_id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
                .or(instance_id.clone());
            let store = match load_instances(&state.paths) {
                Ok(s) => s,
                Err(e) => {
                    let msg = format!("读取版本列表失败：{e}");
                    resp.reply_zh = msg.clone();
                    resp.action = Some(ActionResult {
                        ok: false,
                        action: "build_pack".into(),
                        message_zh: msg,
                        path: None,
                        value: None,
                    });
                    return Ok(resp);
                }
            };
            let id = id.or(store.active_id.clone());
            let Some(id) = id else {
                let msg = "还没有选版本，没法生成整合包。请先在启动页或助手顶部选一个版本。".to_string();
                resp.reply_zh = msg.clone();
                resp.action = Some(ActionResult {
                    ok: false,
                    action: "build_pack".into(),
                    message_zh: msg,
                    path: None,
                    value: None,
                });
                return Ok(resp);
            };
            let Some(inst) = store.instances.iter().find(|i| i.id == id).cloned() else {
                let msg = format!("找不到版本 {id}");
                resp.reply_zh = msg.clone();
                resp.action = Some(ActionResult {
                    ok: false,
                    action: "build_pack".into(),
                    message_zh: msg,
                    path: None,
                    value: None,
                });
                return Ok(resp);
            };
            match export_instance(
                &state.paths,
                &inst,
                ExportOptions {
                    mode: ShareMode::Mrpack,
                    notes: Some("由助手「制作整合包」导出".into()),
                    output: None,
                    resolve_modrinth: true,
                },
            )
            .await
            {
                Ok(r) => {
                    let msg = format!(
                        "整合包已生成（Modrinth .mrpack）。\n版本：「{}」\n模组数：{}\n路径：{}\n{}",
                        inst.name, r.mod_count, r.path, r.message_zh
                    );
                    resp.reply_zh = msg.clone();
                    resp.action = Some(ActionResult {
                        ok: true,
                        action: "build_pack".into(),
                        message_zh: msg,
                        path: Some(r.path.clone()),
                        value: Some(serde_json::json!({
                            "instance_id": inst.id,
                            "instance_name": inst.name,
                            "path": r.path,
                            "mod_count": r.mod_count,
                            "bytes": r.bytes,
                            "mode": "mrpack",
                            "format": "mrpack",
                            "resolved": r.resolved,
                            "overrides": r.overrides,
                        })),
                    });
                }
                Err(e) => {
                    let msg = format!("生成整合包失败：{e}");
                    resp.reply_zh = msg.clone();
                    resp.action = Some(ActionResult {
                        ok: false,
                        action: "build_pack".into(),
                        message_zh: msg,
                        path: None,
                        value: Some(serde_json::json!({ "instance_id": inst.id })),
                    });
                }
            }
        } else if action.action == "import_mrpack" {
            let path = action
                .path
                .clone()
                .or_else(|| {
                    action
                        .value
                        .as_ref()
                        .and_then(|v| v.get("path"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_default();
            let path = path.trim().to_string();
            if path.is_empty() {
                let msg = "请带上整合包路径，例如：导入整合包 /path/to/pack.mrpack".to_string();
                resp.reply_zh = msg.clone();
                resp.action = Some(ActionResult {
                    ok: false,
                    action: "import_mrpack".into(),
                    message_zh: msg,
                    path: None,
                    value: None,
                });
            } else {
                match import_instance(
                    &state.paths,
                    std::path::Path::new(&path),
                    ImportOptions {
                        name: None,
                        apply_memory: false,
                        offline: false,
                    },
                )
                .await
                {
                    Ok(r) => {
                        let msg = format!(
                            "整合包已导入为新版本「{}」。
模组：下载 {} / 覆盖 {}；缺 {}。
{}",
                            r.instance_name,
                            r.mods_downloaded,
                            r.mods_copied,
                            r.mods_missing.len(),
                            r.message_zh
                        );
                        resp.reply_zh = msg.clone();
                        resp.action = Some(ActionResult {
                            ok: true,
                            action: "import_mrpack".into(),
                            message_zh: msg,
                            path: Some(r.path_loaded.clone()),
                            value: Some(serde_json::json!({
                                "path": r.path_loaded,
                                "instance_id": r.instance_id,
                                "instance_name": r.instance_name,
                                "mods_downloaded": r.mods_downloaded,
                                "mods_copied": r.mods_copied,
                                "mods_missing": r.mods_missing,
                            })),
                        });
                    }
                    Err(e) => {
                        let msg = format!("导入整合包失败：{e}");
                        resp.reply_zh = msg.clone();
                        resp.action = Some(ActionResult {
                            ok: false,
                            action: "import_mrpack".into(),
                            message_zh: msg,
                            path: Some(path),
                            value: None,
                        });
                    }
                }
            }
        }
    }
    resp.tool = refresh_ask_tool(resp.action.as_ref(), resp.report.as_ref());
    // Keep multi-step `tools` in sync with the host-refreshed primary card.
    if let Some(ref t) = resp.tool {
        if resp.tools.is_empty() {
            resp.tools = vec![t.clone()];
        } else if let Some(last) = resp.tools.last_mut() {
            // Prefer replacing last card when same action family, else append.
            if last.name == t.name || last.status == easymc_agent::ToolStatus::Running {
                *last = t.clone();
            } else {
                resp.tools.push(t.clone());
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
fn agent_apply_proposal(
    state: State<'_, AppState>,
    proposal: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let prop = parse_proposal(&proposal).ok_or_else(|| "无效的建议更改数据".to_string())?;
    let ar = apply_proposal(&state.paths, &prop).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "ok": ar.ok,
        "action": ar.action,
        "message": ar.message_zh,
        "message_zh": ar.message_zh,
        "value": ar.value,
    }))
}

#[tauri::command]
fn agent_undo_proposal(
    state: State<'_, AppState>,
    proposal: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let prop = parse_proposal(&proposal).ok_or_else(|| "无效的建议更改数据".to_string())?;
    let ar = undo_proposal(&state.paths, &prop).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "ok": ar.ok,
        "action": ar.action,
        "message": ar.message_zh,
        "message_zh": ar.message_zh,
        "value": ar.value,
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
        ShareMode::Mrpack
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
    /// Quick Play: "world" | "server" | "realms" | clear with clear_quick_play.
    quick_play_mode: Option<String>,
    quick_play_target: Option<String>,
    clear_quick_play: Option<bool>,
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
        if patch.clear_quick_play == Some(true) {
            inst.quick_play = None;
        } else if let Some(mode) = patch.quick_play_mode.as_deref() {
            let target = patch
                .quick_play_target
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_string();
            inst.quick_play = match mode.trim().to_ascii_lowercase().as_str() {
                "" | "none" | "off" => None,
                "world" | "singleplayer" | "save" => {
                    if target.is_empty() { None } else { Some(easymc_core::QuickPlay::World { name: target }) }
                }
                "server" | "multiplayer" => {
                    if target.is_empty() { None } else { Some(easymc_core::QuickPlay::Server { address: target }) }
                }
                "realms" | "realm" => {
                    if target.is_empty() { None } else { Some(easymc_core::QuickPlay::Realms { id: target }) }
                }
                _ => inst.quick_play.clone(),
            };
        }
    }).map_err(|e| e.to_string())
}

#[tauri::command]
fn rename_instance_cmd(state: State<'_, AppState>, instance_id: String, name: String) -> Result<Instance, String> {
    update_instance_cmd(state, instance_id, InstancePatch {
        name: Some(name), icon: None, category: None, description: None,
        memory_mb: None, clear_memory: None, java_path: None, clear_java: None,
        width: None, height: None, clear_resolution: None, jvm_args: None, clear_jvm_args: None,
        account_id: None, quick_play_mode: None, quick_play_target: None, clear_quick_play: None,
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
fn create_desktop_shortcut_cmd(state: State<'_, AppState>, instance_id: String) -> Result<serde_json::Value, String> {
    let paths = state_paths(&state);
    let store = load_instances(paths).map_err(|e| e.to_string())?;
    let accounts = load_accounts(paths).map_err(|e| e.to_string())?;
    let settings = load_settings(paths).map_err(|e| e.to_string())?;
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| format!("找不到版本: {instance_id}"))?;
    let acc_id = inst
        .account_id
        .clone()
        .or(accounts.active_id.clone())
        .ok_or_else(|| "请先添加一个账户".to_string())?;
    let acc = accounts
        .accounts
        .iter()
        .find(|a| a.id == acc_id)
        .ok_or_else(|| "账户不存在".to_string())?;
    let mem = inst.memory_mb.unwrap_or(settings.memory_mb);
    let path = create_desktop_shortcut(
        paths,
        inst,
        acc,
        &settings,
        &LaunchRequest {
            memory_mb: mem,
            extra_jvm_args: vec![],
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "path": path,
        "message_zh": format!("已创建桌面快捷方式（含应用菜单 / 开始菜单副本）\n{}", path.display())
    }))
}

#[tauri::command]
async fn upload_latest_log_cmd(
    state: State<'_, AppState>,
    instance_id: Option<String>,
) -> Result<easymc_core::MclogsUploadResult, String> {
    let paths = state_paths(&state);
    let store = load_instances(paths).map_err(|e| e.to_string())?;
    let id = instance_id
        .filter(|s| !s.trim().is_empty())
        .or(store.active_id.clone())
        .ok_or_else(|| "还没有选版本，请先在启动页选一个".to_string())?;
    let inst = store
        .instances
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| format!("找不到版本: {id}"))?;
    let game = std::path::PathBuf::from(&inst.game_dir);
    let latest = game.join("logs").join("latest.log");
    let crash_dir = game.join("crash-reports");
    let mut newest_crash: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
    if let Ok(rd) = std::fs::read_dir(&crash_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("txt") {
                continue;
            }
            if let Ok(mt) = e.metadata().and_then(|m| m.modified()) {
                if newest_crash.as_ref().map(|(t, _)| mt > *t).unwrap_or(true) {
                    newest_crash = Some((mt, p));
                }
            }
        }
    }
    let path = if latest.is_file() {
        latest
    } else if let Some((_, p)) = newest_crash {
        p
    } else {
        return Err(format!(
            "版本「{}」还没有 latest.log 或崩溃报告。先启动一次游戏再分享。",
            inst.name
        ));
    };
    let content = std::fs::read_to_string(&path).map_err(|e| format!("读日志失败: {e}"))?;
    easymc_core::upload_log_content(&content)
        .await
        .map_err(|e| e.to_string())
}



#[tauri::command]
async fn upload_process_logs_cmd(
    state: State<'_, AppState>,
) -> Result<easymc_core::MclogsUploadResult, String> {
    let lines = state.procs.logs_snapshot().await;
    if lines.is_empty() {
        return Err(
            "启动器里还没有进程日志。先启动一次游戏，或到版本设置上传 latest.log。".into(),
        );
    }
    let content = lines.join("\n");
    easymc_core::upload_log_content(&content)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_logs_cmd(state: State<'_, AppState>) -> Result<(), String> {
    state.procs.clear_logs().await;
    Ok(())
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


// —— Container mode (Podman) — Slice A–E ——

#[tauri::command]
fn container_probe_cmd() -> easymc_container::RuntimeProbe {
    probe_container_runtime()
}

#[tauri::command]
fn container_setup_plan_cmd() -> easymc_container::SetupPlan {
    install_guidance()
}

#[tauri::command]
fn container_one_click_setup_cmd(
    state: State<'_, AppState>,
    prefer_china_mirrors: Option<bool>,
) -> Result<easymc_container::SetupResult, String> {
    let paths = state_paths(&state);
    let prefer = prefer_china_mirrors.unwrap_or_else(|| {
        load_settings(paths)
            .map(|s| s.container_china_mirrors)
            .unwrap_or(true)
    });
    let result = apply_one_click_setup(prefer);
    if let Ok(mut settings) = load_settings(paths) {
        // Mark setup attempted; Ready ⇒ done.
        settings.container_setup_done = matches!(
            result.probe.status,
            easymc_container::ProbeStatus::Ready
        ) || settings.container_setup_done;
        let _ = save_settings(paths, &settings);
    }
    Ok(result)
}

#[tauri::command]
fn container_write_registries_cmd(
    state: State<'_, AppState>,
) -> Result<easymc_container::ChinaRegistriesResult, String> {
    let paths = state_paths(&state);
    let profile = load_settings(paths)
        .ok()
        .and_then(|s| MirrorProfile::parse(&s.container_mirror_profile))
        .unwrap_or_else(|| {
            load_settings(paths)
                .map(|s| MirrorProfile::from_prefer_china(s.container_china_mirrors))
                .unwrap_or(MirrorProfile::Daocloud)
        });
    write_registries_config_for_profile(profile)
}

#[tauri::command]
fn container_list_versions_cmd(
    state: State<'_, AppState>,
) -> Result<Vec<easymc_container::ContainerVersionSummary>, String> {
    list_container_versions(state_paths(&state)).map_err(|e| e.to_string())
}

#[tauri::command]
fn container_create_version_cmd(
    state: State<'_, AppState>,
    name: String,
    version: String,
    loader: Option<String>,
    loader_version: Option<String>,
) -> Result<Instance, String> {
    let paths = state_paths(&state);
    ensure_defaults(paths).map_err(|e| e.to_string())?;
    let prefer = load_settings(paths)
        .map(|s| s.container_china_mirrors)
        .unwrap_or(true);
    let loader = loader
        .as_deref()
        .and_then(ModLoader::parse_loose)
        .unwrap_or(ModLoader::Vanilla);
    create_container_version(
        paths,
        name,
        McVersion(version),
        loader,
        loader_version.filter(|s| !s.trim().is_empty()),
        prefer,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn container_attach_cmd(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Instance, String> {
    let paths = state_paths(&state);
    let prefer = load_settings(paths)
        .map(|s| s.container_china_mirrors)
        .unwrap_or(true);
    attach_container_to_instance(paths, &instance_id, prefer).map_err(|e| e.to_string())
}

#[tauri::command]
fn container_delete_version_cmd(
    state: State<'_, AppState>,
    instance_id: String,
    delete_files: Option<bool>,
) -> Result<(), String> {
    delete_container_version(state_paths(&state), &instance_id, delete_files.unwrap_or(true))
        .map_err(|e| e.to_string())
}



#[tauri::command]
fn container_provision_cmd(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<easymc_container::ProvisionResult, String> {
    let paths = state_paths(&state);
    let prefer = load_settings(paths)
        .map(|s| s.container_china_mirrors)
        .unwrap_or(true);
    provision_container_version(paths, &instance_id, prefer).map_err(|e| e.to_string())
}

#[tauri::command]
fn container_preheat_cmd(
    state: State<'_, AppState>,
    pull_image: Option<bool>,
) -> easymc_container::PreheatResult {
    let prefer = load_settings(state_paths(&state))
        .map(|s| s.container_china_mirrors)
        .unwrap_or(true);
    preheat_container_runtime(prefer, pull_image.unwrap_or(true))
}

#[tauri::command]
fn container_pull_image_cmd(
    state: State<'_, AppState>,
) -> Result<easymc_container::ImagePullResult, String> {
    let prefer = load_settings(state_paths(&state))
        .map(|s| s.container_china_mirrors)
        .unwrap_or(true);
    ensure_runtime_image(prefer)
}

#[tauri::command]
fn container_layer_state_cmd(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<easymc_container::LayerState, String> {
    layer_state_for(state_paths(&state), &instance_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn container_layer_apply_cmd(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<easymc_container::LayerOpResult, String> {
    apply_game_layer(state_paths(&state), &instance_id)
}

#[tauri::command]
fn container_layer_discard_cmd(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<easymc_container::LayerOpResult, String> {
    discard_game_layer(state_paths(&state), &instance_id)
}

#[tauri::command]
fn container_agent_workspace_cmd(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<easymc_container::AgentWorkspace, String> {
    resolve_agent_workspace(state_paths(&state), &instance_id).map_err(|e| e.to_string())
}


#[tauri::command]
fn container_preheat_status_cmd(
    state: State<'_, AppState>,
) -> easymc_container::PreheatStatus {
    let prefer = load_settings(state_paths(&state))
        .map(|s| s.container_china_mirrors)
        .unwrap_or(true);
    preheat_status(prefer)
}

#[tauri::command]
fn container_preheat_ex_cmd(
    state: State<'_, AppState>,
    pull_image: Option<bool>,
    force_pull: Option<bool>,
) -> easymc_container::PreheatResult {
    let prefer = load_settings(state_paths(&state))
        .map(|s| s.container_china_mirrors)
        .unwrap_or(true);
    preheat_container_runtime_ex(prefer, pull_image.unwrap_or(true), force_pull.unwrap_or(false))
}

#[tauri::command]
fn container_mirror_profiles_cmd() -> Vec<easymc_container::MirrorProfileInfo> {
    list_mirror_profiles()
}


#[tauri::command]
fn download_mirror_candidates_cmd() -> Vec<easymc_meta::DownloadMirrorCandidate> {
    list_download_mirror_candidates()
}

#[tauri::command]
async fn probe_download_mirrors_cmd() -> Vec<easymc_meta::DownloadMirrorProbe> {
    probe_download_mirrors().await
}

#[tauri::command]
fn registry_mirror_candidates_cmd() -> Vec<easymc_container::RegistryMirrorCandidate> {
    list_registry_mirror_candidates()
}

#[tauri::command]
async fn probe_registry_mirrors_cmd() -> Vec<easymc_container::RegistryMirrorProbe> {
    probe_registry_mirrors().await
}

#[tauri::command]
fn container_repair_cmd(
    state: State<'_, AppState>,
    prefer_china_mirrors: Option<bool>,
    reprovision: Option<bool>,
) -> easymc_container::RepairResult {
    let paths = state_paths(&state);
    let settings = load_settings(paths).ok();
    let profile = settings
        .as_ref()
        .and_then(|s| MirrorProfile::parse(&s.container_mirror_profile))
        .or_else(|| {
            prefer_china_mirrors
                .or_else(|| settings.as_ref().map(|s| s.container_china_mirrors))
                .map(MirrorProfile::from_prefer_china)
        })
        .unwrap_or(MirrorProfile::Daocloud);
    repair_container_runtime_with_profile(paths, profile, reprovision.unwrap_or(true))
}

#[tauri::command]
fn container_export_env_cmd(
    state: State<'_, AppState>,
    instance_id: String,
    pack: Option<bool>,
    notes: Option<String>,
) -> Result<easymc_container::EnvExportResult, String> {
    let mode = if pack.unwrap_or(false) {
        EnvPackMode::Pack
    } else {
        EnvPackMode::Metadata
    };
    export_env_pack(
        state_paths(&state),
        &instance_id,
        EnvExportOptions {
            mode,
            notes,
            output: None,
        },
    )
}

#[tauri::command]
fn container_import_env_cmd(
    state: State<'_, AppState>,
    path: String,
    target_instance_id: Option<String>,
    prefer_china_mirrors: Option<bool>,
) -> Result<easymc_container::EnvImportResult, String> {
    import_env_pack(
        state_paths(&state),
        std::path::Path::new(&path),
        EnvImportOptions {
            target_instance_id,
            prefer_china_mirrors,
        },
    )
}

#[tauri::command]
fn container_read_env_manifest_cmd(path: String) -> Result<easymc_container::EnvManifest, String> {
    read_env_manifest_from_path(std::path::Path::new(&path))
}


    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            paths,
            procs: Arc::new(ProcessManager::new()),
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            list_instances,
            resolve_instance_icon_cmd,
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
            detect_minecraft_dirs_cmd,
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
            mod_dep_graph_cmd,
            fetch_missing_mod_deps_cmd,
            remove_mod_cmd,
            set_mod_disabled_cmd,
            agent_llm_status,
            agent_llm_config_get,
            agent_llm_config_save,
            agent_llm_config_clear,
            agent_diagnose,
            agent_ask,
            agent_data_dir,
            agent_bump_memory,
            agent_export_diagnostics,
            agent_apply_proposal,
            agent_undo_proposal,
            export_instance_cmd,
            import_instance_cmd,
            update_instance_cmd,
            rename_instance_cmd,
            duplicate_instance_cmd,
            delete_instance_cmd,
            open_instance_folder_cmd,
            write_launch_script_cmd,
            create_desktop_shortcut_cmd,
            upload_latest_log_cmd,
            upload_process_logs_cmd,
            clear_logs_cmd,
            list_local_content_cmd,
            delete_local_content_cmd,
            search_content_cmd,
            install_content_cmd,
            install_modpack_cmd,
            container_probe_cmd,
            container_setup_plan_cmd,
            container_one_click_setup_cmd,
            container_write_registries_cmd,
            container_list_versions_cmd,
            container_create_version_cmd,
            container_attach_cmd,
            container_delete_version_cmd,
            container_provision_cmd,
            container_preheat_cmd,
            container_pull_image_cmd,
            container_layer_state_cmd,
            container_layer_apply_cmd,
            container_layer_discard_cmd,
            container_agent_workspace_cmd,
            container_preheat_status_cmd,
            container_preheat_ex_cmd,
            container_mirror_profiles_cmd,
            download_mirror_candidates_cmd,
            probe_download_mirrors_cmd,
            registry_mirror_candidates_cmd,
            probe_registry_mirrors_cmd,
            container_repair_cmd,
            container_export_env_cmd,
            container_import_env_cmd,
            container_read_env_manifest_cmd,
            wardrobe_list_cmd,
            wardrobe_set_active_cmd,
            wardrobe_upload_cmd,
            wardrobe_replace_cmd,
            wardrobe_remove_cmd,
            wardrobe_active_cmd,
            wardrobe_stage_active_cmd,
            skin_fetch_official_cmd,
            skin_fetch_littleskin_cmd,
            wardrobe_import_official_cmd,
            wardrobe_import_littleskin_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running EasyMC");
}
