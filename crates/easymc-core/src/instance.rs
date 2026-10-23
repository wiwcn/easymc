use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Minecraft Java Edition version id, e.g. `"1.21.1"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct McVersion(pub String);

impl McVersion {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for McVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModLoader {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

impl Default for ModLoader {
    fn default() -> Self {
        Self::Vanilla
    }
}

/// A playable instance (Java Edition).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub version: McVersion,
    #[serde(default)]
    pub loader: ModLoader,
    pub description: Option<String>,
    pub last_played: Option<DateTime<Utc>>,
    /// Absolute path to the instance game directory (saves, options…).
    pub game_dir: String,
    /// Bound account id (offline or MSA).
    pub account_id: Option<String>,
    /// Whether client jar + libraries for `version` are present.
    #[serde(default)]
    pub version_ready: bool,
    pub created_at: DateTime<Utc>,
}

impl Instance {
    pub fn new(id: impl Into<String>, name: impl Into<String>, version: McVersion, game_dir: String) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version,
            loader: ModLoader::Vanilla,
            description: None,
            last_played: None,
            game_dir,
            account_id: None,
            version_ready: false,
            created_at: Utc::now(),
        }
    }
}
