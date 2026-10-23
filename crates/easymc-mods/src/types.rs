use easymc_core::ModLoader;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModSource {
    Modrinth,
    CurseForge,
}

impl ModSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Modrinth => "modrinth",
            Self::CurseForge => "curseforge",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Modrinth => "Modrinth",
            Self::CurseForge => "CurseForge",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "modrinth" | "mr" => Some(Self::Modrinth),
            "curseforge" | "cf" | "curse" => Some(Self::CurseForge),
            _ => None,
        }
    }
}

impl std::fmt::Display for ModSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModProject {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub source: ModSource,
    pub downloads: u64,
    pub author: Option<String>,
    pub icon_url: Option<String>,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModVersion {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub downloads: u64,
    pub date_published: Option<String>,
    pub files: Vec<ModFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModFile {
    pub filename: String,
    pub url: String,
    pub size: Option<u64>,
    pub primary: bool,
    pub sha1: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub text: String,
    pub game_version: Option<String>,
    pub loader: Option<ModLoader>,
    pub limit: usize,
    pub offset: usize,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: String::new(),
            game_version: None,
            loader: None,
            limit: 20,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModInstallResult {
    pub source: ModSource,
    pub project_id: String,
    pub project_name: String,
    pub version_id: String,
    pub version_number: String,
    pub filename: String,
    pub path: String,
    pub skipped: bool,
}

/// Map instance loader to marketplace loader filter strings.
pub fn loader_filter_name(loader: ModLoader) -> Option<&'static str> {
    match loader {
        ModLoader::Fabric => Some("fabric"),
        ModLoader::Quilt => Some("quilt"),
        ModLoader::Forge => Some("forge"),
        ModLoader::NeoForge => Some("neoforge"),
        ModLoader::Vanilla => None,
    }
}
