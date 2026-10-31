use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Account, EasyMcError, Instance, McVersion, ModLoader, Paths, Result, create_offline_account};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstanceStore {
    pub instances: Vec<Instance>,
    pub active_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountStore {
    pub accounts: Vec<Account>,
    pub active_id: Option<String>,
}

/// Launcher UX mode chosen at first-run (or later in Settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppMode {
    /// Agent / diagnose assistant is more prominent.
    Agent,
    /// Classic launcher layout; Agent still reachable but less primary.
    #[default]
    Standard,
}

impl AppMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Standard => "standard",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "agent" => Some(Self::Agent),
            "standard" => Some(Self::Standard),
            _ => None,
        }
    }
}

/// Preferred login path from onboarding / settings (`microsoft` | `offline` | `skin_station`).
pub type PreferredLogin = String;


/// Preferred Minecraft file download source (version list + install).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DownloadSource {
    /// Mojang / official Fabric / Forge / NeoForge hosts.
    #[default]
    Official,
    /// BMCLAPI mirror (`bmclapi2.bangbang93.com`) — China-friendly accelerate.
    Bmclapi,
}

impl DownloadSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Official => "official",
            Self::Bmclapi => "bmclapi",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "official" | "mojang" | "default" => Some(Self::Official),
            "bmclapi" | "bmcl" | "bmclapi2" => Some(Self::Bmclapi),
            _ => None,
        }
    }

    /// Player-facing Chinese label.
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Official => "官方",
            Self::Bmclapi => "BMCLAPI 加速",
        }
    }
}

fn default_true() -> bool { true }

fn default_mirror_profile() -> String { "daocloud".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub msa_client_id: Option<String>,
    pub preferred_java: Option<String>,
    pub memory_mb: u32,
    /// CurseForge API key (player-pasted). Env `EASYMC_CURSEFORGE_API_KEY` overrides when set.
    /// Stored only under the local data dir — never commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curseforge_api_key: Option<String>,
    /// First-run mode: `agent` | `standard`.
    #[serde(default)]
    pub mode: AppMode,
    /// When false, UI shows the first-run onboarding gate (mode + ≥1 account).
    #[serde(default)]
    pub onboarding_complete: bool,
    /// Last chosen login style: microsoft / offline / skin_station.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_login: Option<PreferredLogin>,
    /// Game file download source: official or BMCLAPI mirror.
    #[serde(default)]
    pub download_source: DownloadSource,
    /// Player opted into container mode (after Agent onboarding ask).
    #[serde(default)]
    pub container_mode_enabled: bool,
    /// Prefer China registry mirrors when pulling container images.
    #[serde(default = "default_true")]
    pub container_china_mirrors: bool,
    /// Selected container registry mirror id: daocloud | dockerproxy | onems | official.
    /// Kept in sync with `container_china_mirrors` (official → false; else → true).
    #[serde(default = "default_mirror_profile")]
    pub container_mirror_profile: String,
    /// One-click container setup finished (or skipped) during onboarding.
    #[serde(default)]
    pub container_setup_done: bool,
    /// Container mode: mutating `saves/` requires Agent propose/confirm (default true).
    #[serde(default = "default_true")]
    pub container_saves_require_propose: bool,
    /// Auto-preheat container runtime when container mode is enabled (Slice E).
    #[serde(default = "default_true")]
    pub container_auto_preheat: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            msa_client_id: None,
            preferred_java: None,
            memory_mb: 2048,
            curseforge_api_key: None,
            mode: AppMode::Standard,
            onboarding_complete: false,
            preferred_login: None,
            download_source: DownloadSource::Official,
            container_mode_enabled: false,
            container_china_mirrors: true,
            container_mirror_profile: default_mirror_profile(),
            container_setup_done: false,
            container_saves_require_propose: true,
            container_auto_preheat: true,
        }
    }
}

fn read_json<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> Result<T> {
    if !path.exists() {
        return Ok(T::default());
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(T::default());
    }
    Ok(serde_json::from_str(&raw)?)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(value)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, raw)?;
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn load_instances(paths: &Paths) -> Result<InstanceStore> {
    read_json(&paths.instances_file())
}

pub fn save_instances(paths: &Paths, store: &InstanceStore) -> Result<()> {
    write_json(&paths.instances_file(), store)
}

pub fn load_accounts(paths: &Paths) -> Result<AccountStore> {
    read_json(&paths.accounts_file())
}

pub fn save_accounts(paths: &Paths, store: &AccountStore) -> Result<()> {
    write_json(&paths.accounts_file(), store)
}

pub fn load_settings(paths: &Paths) -> Result<Settings> {
    read_json(&paths.settings_file())
}

pub fn save_settings(paths: &Paths, settings: &Settings) -> Result<()> {
    write_json(&paths.settings_file(), settings)
}

/// Ensure starter vanilla instance exists; seed offline account only after onboarding
/// (or for upgrades that already have accounts). First-run leaves accounts empty so the
/// UI gate can collect mode + login.
pub fn ensure_defaults(paths: &Paths) -> Result<(InstanceStore, AccountStore)> {
    paths.ensure()?;

    let mut settings = if paths.settings_file().exists() {
        load_settings(paths)?
    } else {
        let s = Settings::default();
        save_settings(paths, &s)?;
        s
    };

    let mut accounts = load_accounts(paths)?;

    // Upgrade path: installs that already have accounts skip the first-run gate.
    if !settings.onboarding_complete && !accounts.accounts.is_empty() {
        settings.onboarding_complete = true;
        save_settings(paths, &settings)?;
    }

    if accounts.accounts.is_empty() {
        // Do not auto-create during first-run onboarding; after onboarding, keep a fallback.
        if settings.onboarding_complete {
            let acc = create_offline_account("EasyPlayer")?;
            accounts.active_id = Some(acc.id.clone());
            accounts.accounts.push(acc);
            save_accounts(paths, &accounts)?;
        }
    } else if accounts.active_id.is_none() {
        accounts.active_id = accounts.accounts.first().map(|a| a.id.clone());
        save_accounts(paths, &accounts)?;
    }

    let mut instances = load_instances(paths)?;
    if instances.instances.is_empty() {
        let id = "default".to_string();
        let game_dir = paths.instance_game_dir(&id);
        fs::create_dir_all(&game_dir)?;
        let mut inst = Instance::new(
            id.clone(),
            "Survival World",
            crate::McVersion("1.21.1".into()),
            game_dir.to_string_lossy().into_owned(),
        );
        inst.description = Some("准备好了就开始；需要正版服时再登录微软账户".into());
        inst.account_id = accounts.active_id.clone();
        instances.active_id = Some(id);
        instances.instances.push(inst);
        save_instances(paths, &instances)?;
    } else if instances.active_id.is_none() {
        instances.active_id = instances.instances.first().map(|i| i.id.clone());
        save_instances(paths, &instances)?;
    }

    Ok((instances, accounts))
}

pub fn upsert_instance(paths: &Paths, instance: Instance) -> Result<InstanceStore> {
    let mut store = load_instances(paths)?;
    if let Some(slot) = store.instances.iter_mut().find(|i| i.id == instance.id) {
        *slot = instance;
    } else {
        store.instances.push(instance);
    }
    save_instances(paths, &store)?;
    Ok(store)
}

pub fn get_instance<'a>(store: &'a InstanceStore, id: &str) -> Result<&'a Instance> {
    store
        .instances
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| EasyMcError::Missing(format!("instance {id}")))
}

pub fn get_account<'a>(store: &'a AccountStore, id: &str) -> Result<&'a Account> {
    store
        .accounts
        .iter()
        .find(|a| a.id == id)
        .ok_or_else(|| EasyMcError::Missing(format!("account {id}")))
}

/// Slugify a display name into a filesystem-safe instance id fragment.
fn slugify_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else if c.is_whitespace() || c == '-' || c == '_' {
                '-'
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "inst".into()
    } else {
        s.chars().take(32).collect()
    }
}

/// Options when creating a named instance (no download).
#[derive(Debug, Clone)]
pub struct CreateInstanceOpts {
    pub name: String,
    pub version: McVersion,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub version_ready: bool,
}

impl CreateInstanceOpts {
    pub fn vanilla(name: impl Into<String>, version: McVersion, version_ready: bool) -> Self {
        Self {
            name: name.into(),
            version,
            loader: ModLoader::Vanilla,
            loader_version: None,
            version_ready,
        }
    }
}

/// Create a new named instance bound to `version` (+ optional mod loader). Does not download.
pub fn create_named_instance(
    paths: &Paths,
    name: impl Into<String>,
    version: McVersion,
    version_ready: bool,
) -> Result<Instance> {
    create_named_instance_with(
        paths,
        CreateInstanceOpts::vanilla(name, version, version_ready),
    )
}

pub fn create_named_instance_with(paths: &Paths, opts: CreateInstanceOpts) -> Result<Instance> {
    paths.ensure()?;
    let name = if opts.name.trim().is_empty() {
        let loader_bit = match opts.loader {
            ModLoader::Vanilla => String::new(),
            other => format!(" ({})", other.label_zh()),
        };
        format!("Instance {}{loader_bit}", opts.version)
    } else {
        opts.name.trim().to_string()
    };
    let slug = slugify_name(&name);
    let id = format!(
        "{}-{}",
        slug,
        &uuid::Uuid::new_v4().to_string()[..8]
    );
    let game_dir = paths.instance_game_dir(&id);
    fs::create_dir_all(&game_dir)?;
    let accounts = load_accounts(paths)?;
    let mut inst = Instance::new(
        id.clone(),
        name,
        opts.version,
        game_dir.to_string_lossy().into_owned(),
    );
    inst.loader = opts.loader;
    inst.loader_version = opts.loader_version.filter(|s| !s.trim().is_empty());
    inst.account_id = accounts.active_id;
    inst.version_ready = opts.version_ready;
    inst.description = Some(match opts.loader {
        ModLoader::Vanilla => "用户创建的实例".into(),
        other => format!("用户创建 · {}", other.label_zh()),
    });

    let mut store = load_instances(paths)?;
    store.active_id = Some(id);
    store.instances.push(inst.clone());
    save_instances(paths, &store)?;
    Ok(inst)
}

/// Mark `id` as the active / default instance.
pub fn set_active_instance(paths: &Paths, id: &str) -> Result<InstanceStore> {
    let mut store = load_instances(paths)?;
    get_instance(&store, id)?;
    store.active_id = Some(id.to_string());
    save_instances(paths, &store)?;
    Ok(store)
}


/// Patch mutable player-facing fields on an instance.
pub fn update_instance_fields(
    paths: &Paths,
    id: &str,
    mut patch: impl FnMut(&mut Instance),
) -> Result<Instance> {
    let mut store = load_instances(paths)?;
    let inst = store
        .instances
        .iter_mut()
        .find(|i| i.id == id)
        .ok_or_else(|| EasyMcError::Missing(format!("instance {id}")))?;
    patch(inst);
    let out = inst.clone();
    save_instances(paths, &store)?;
    Ok(out)
}

pub fn remove_instance(paths: &Paths, id: &str, delete_files: bool) -> Result<()> {
    let mut store = load_instances(paths)?;
    let Some(pos) = store.instances.iter().position(|i| i.id == id) else {
        return Err(EasyMcError::Missing(format!("instance {id}")));
    };
    let removed = store.instances.remove(pos);
    if store.active_id.as_deref() == Some(id) {
        store.active_id = store.instances.first().map(|i| i.id.clone());
    }
    save_instances(paths, &store)?;
    if delete_files {
        let dir = PathBuf::from(&removed.game_dir);
        // game_dir is usually .../instances/<id>/.minecraft — remove the instance folder.
        let root = dir.parent().filter(|p| p.file_name().is_some()).unwrap_or(dir.as_path());
        if root.exists() {
            let _ = fs::remove_dir_all(root);
        }
    }
    Ok(())
}

pub fn duplicate_instance(paths: &Paths, id: &str, new_name: Option<&str>) -> Result<Instance> {
    let store = load_instances(paths)?;
    let src = get_instance(&store, id)?.clone();
    let name = new_name
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}（副本）", src.name));
    let mut copy = create_named_instance_with(
        paths,
        CreateInstanceOpts {
            name,
            version: src.version.clone(),
            loader: src.loader,
            loader_version: src.loader_version.clone(),
            version_ready: src.version_ready,
        },
    )?;
    copy.icon = src.icon.clone();
    copy.category = src.category.clone();
    copy.memory_mb = src.memory_mb;
    copy.java_path = src.java_path.clone();
    copy.width = src.width;
    copy.height = src.height;
    copy.jvm_args = src.jvm_args.clone();
    copy.quick_play = src.quick_play.clone();
    // Container identity is per-version — duplicate stays native until re-attached.
    copy.container = None;
    copy.description = Some(format!("由「{}」复制", src.name));
    // Best-effort copy of mods / resourcepacks / shaderpacks / config (not saves — those are huge).
    for sub in ["mods", "resourcepacks", "shaderpacks", "config", "datapacks"] {
        let from = PathBuf::from(&src.game_dir).join(sub);
        let to = PathBuf::from(&copy.game_dir).join(sub);
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        }
    }
    upsert_instance(paths, copy.clone())?;
    Ok(copy)
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for ent in fs::read_dir(src)? {
        let ent = ent?;
        let t = dst.join(ent.file_name());
        if ent.file_type()?.is_dir() {
            copy_dir_recursive(&ent.path(), &t)?;
        } else {
            fs::copy(ent.path(), t)?;
        }
    }
    Ok(())
}
