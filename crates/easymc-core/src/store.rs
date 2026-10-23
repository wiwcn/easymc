use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Account, EasyMcError, Instance, Paths, Result, create_offline_account};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub msa_client_id: Option<String>,
    pub preferred_java: Option<String>,
    pub memory_mb: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            msa_client_id: None,
            preferred_java: None,
            memory_mb: 2048,
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

/// Ensure default offline account + a starter vanilla instance exist.
pub fn ensure_defaults(paths: &Paths) -> Result<(InstanceStore, AccountStore)> {
    paths.ensure()?;
    let mut accounts = load_accounts(paths)?;
    if accounts.accounts.is_empty() {
        let acc = create_offline_account("EasyPlayer");
        accounts.active_id = Some(acc.id.clone());
        accounts.accounts.push(acc);
        save_accounts(paths, &accounts)?;
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
        inst.description = Some("默认实例 · 安装版本后即可启动".into());
        inst.account_id = accounts.active_id.clone();
        instances.active_id = Some(id);
        instances.instances.push(inst);
        save_instances(paths, &instances)?;
    } else if instances.active_id.is_none() {
        instances.active_id = instances.instances.first().map(|i| i.id.clone());
        save_instances(paths, &instances)?;
    }

    if !paths.settings_file().exists() {
        save_settings(paths, &Settings::default())?;
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
