//! One-click share of instance configs / packs (stubs).

use easymc_core::{EasyMcError, Instance, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareBundle {
    pub format_version: u32,
    pub instance: Instance,
    pub notes: Option<String>,
}

pub fn export_bundle(instance: &Instance, notes: Option<String>) -> Result<ShareBundle> {
    Ok(ShareBundle {
        format_version: 1,
        instance: instance.clone(),
        notes,
    })
}

pub fn import_bundle(json: &str) -> Result<ShareBundle> {
    serde_json::from_str(json).map_err(|e| EasyMcError::Message(e.to_string()))
}

pub fn publish_remote(_bundle: &ShareBundle) -> Result<String> {
    Err(EasyMcError::NotImplemented("remote share publish"))
}
