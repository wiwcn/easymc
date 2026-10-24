//! EasyMC share bundle format (v1).

use serde::{Deserialize, Serialize};

pub const FORMAT_ID: &str = "easymc.share";
pub const FORMAT_VERSION: u32 = 1;
pub const MANIFEST_NAME: &str = "easymc.share.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareMode {
    /// JSON only — small; import resolves mods via Modrinth/CF ids when present.
    Metadata,
    /// Zip with manifest + mods/*.jar.
    Pack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JvmSpec {
    #[serde(default = "default_memory")]
    pub memory_mb: u32,
}

fn default_memory() -> u32 {
    2048
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceSpec {
    pub name: String,
    pub version: String,
    #[serde(default = "default_loader")]
    pub loader: String,
    #[serde(default)]
    pub loader_version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

fn default_loader() -> String {
    "vanilla".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModEntry {
    pub filename: String,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub sha1: Option<String>,
    /// `modrinth` | `curseforge`
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub version_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareManifest {
    pub format: String,
    pub format_version: u32,
    pub mode: ShareMode,
    pub exported_at: String,
    #[serde(default)]
    pub notes: Option<String>,
    pub instance: InstanceSpec,
    pub jvm: JvmSpec,
    #[serde(default)]
    pub mods: Vec<ModEntry>,
}

impl ShareManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.format != FORMAT_ID {
            return Err(format!("未知格式: {}", self.format));
        }
        if self.format_version == 0 || self.format_version > FORMAT_VERSION {
            return Err(format!("不支持的 format_version: {}", self.format_version));
        }
        if self.instance.name.trim().is_empty() {
            return Err("instance.name 为空".into());
        }
        if self.instance.version.trim().is_empty() {
            return Err("instance.version 为空".into());
        }
        Ok(())
    }
}
