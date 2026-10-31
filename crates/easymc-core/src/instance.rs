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

impl ModLoader {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Vanilla => "原版",
            Self::Fabric => "Fabric",
            Self::Quilt => "Quilt",
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
        }
    }

    pub fn parse_loose(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "vanilla" | "原版" | "" => Some(Self::Vanilla),
            "fabric" => Some(Self::Fabric),
            "quilt" => Some(Self::Quilt),
            "forge" => Some(Self::Forge),
            "neoforge" | "neo" => Some(Self::NeoForge),
            _ => None,
        }
    }
}

impl std::fmt::Display for ModLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}


/// Quick Play target (Minecraft 1.20+ `--quickPlay*`; older clients fall back to `--server` for Server).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QuickPlay {
    /// World folder name under `saves/` (not display name).
    World { name: String },
    /// Server address, e.g. `mc.example.com` or `host:25565`.
    Server { address: String },
    /// Realms world id.
    Realms { id: String },
}

impl QuickPlay {
    pub fn mode_key(&self) -> &'static str {
        match self {
            Self::World { .. } => "world",
            Self::Server { .. } => "server",
            Self::Realms { .. } => "realms",
        }
    }

    pub fn target(&self) -> &str {
        match self {
            Self::World { name } => name,
            Self::Server { address } => address,
            Self::Realms { id } => id,
        }
    }

    pub fn label_zh(&self) -> String {
        match self {
            Self::World { name } => format!("单人世界「{name}」"),
            Self::Server { address } => format!("服务器 {address}"),
            Self::Realms { id } => format!("Realms {id}"),
        }
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
    /// Loader build id (e.g. Fabric `0.20.1`). `None` means pick stable/latest at install.
    #[serde(default)]
    pub loader_version: Option<String>,
    pub description: Option<String>,
    pub last_played: Option<DateTime<Utc>>,
    /// Absolute path to the instance game directory (saves, options…).
    pub game_dir: String,
    /// Bound account id (offline or MSA).
    pub account_id: Option<String>,
    /// Whether client jar + libraries (and loader profile if any) are present.
    #[serde(default)]
    pub version_ready: bool,
    pub created_at: DateTime<Utc>,
    /// Player-picked icon key (e.g. `grass`, `diamond`) — UI renders built-in SVG.
    #[serde(default)]
    pub icon: Option<String>,
    /// Free-form category label (生存 / 创造 / 整合包 …).
    #[serde(default)]
    pub category: Option<String>,
    /// Per-instance overrides; `None` falls back to global settings.
    #[serde(default)]
    pub memory_mb: Option<u32>,
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub jvm_args: Option<String>,
    /// Skip title screen: join a world / server / Realms on launch (CMCL / Prism Quick Play).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quick_play: Option<QuickPlay>,
    /// When set, this version runs via container mode (Podman). `None` = native.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<crate::ContainerMeta>,
}

impl Instance {
    pub fn new(id: impl Into<String>, name: impl Into<String>, version: McVersion, game_dir: String) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version,
            loader: ModLoader::Vanilla,
            loader_version: None,
            description: None,
            last_played: None,
            game_dir,
            account_id: None,
            version_ready: false,
            created_at: Utc::now(),
            icon: None,
            category: None,
            memory_mb: None,
            java_path: None,
            width: None,
            height: None,
            jvm_args: None,
            quick_play: None,
            container: None,
        }
    }
}
