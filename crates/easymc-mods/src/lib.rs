//! Pluggable mod store providers. Modrinth + CurseForge stubs for M0.

use async_trait::async_trait;
use easymc_core::{EasyMcError, McVersion, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModSource {
    Modrinth,
    CurseForge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModProject {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: ModSource,
    pub downloads: u64,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub game_version: Option<McVersion>,
    pub limit: usize,
}

/// Trait every mod marketplace must implement.
#[async_trait]
pub trait ModProvider: Send + Sync {
    fn source(&self) -> ModSource;
    async fn search(&self, query: &SearchQuery) -> Result<Vec<ModProject>>;
    async fn get_project(&self, id: &str) -> Result<ModProject>;
}

pub struct ModrinthProvider;

#[async_trait]
impl ModProvider for ModrinthProvider {
    fn source(&self) -> ModSource {
        ModSource::Modrinth
    }

    async fn search(&self, _query: &SearchQuery) -> Result<Vec<ModProject>> {
        Ok(vec![ModProject {
            id: "stub-seedling".into(),
            name: "Seedling".into(),
            description: "Core library for lightweight mods (stub)".into(),
            source: ModSource::Modrinth,
            downloads: 125_000,
            categories: vec!["library".into()],
        }])
    }

    async fn get_project(&self, id: &str) -> Result<ModProject> {
        Ok(ModProject {
            id: id.into(),
            name: id.into(),
            description: "Modrinth stub project".into(),
            source: ModSource::Modrinth,
            downloads: 0,
            categories: vec![],
        })
    }
}

pub struct CurseForgeProvider;

#[async_trait]
impl ModProvider for CurseForgeProvider {
    fn source(&self) -> ModSource {
        ModSource::CurseForge
    }

    async fn search(&self, _query: &SearchQuery) -> Result<Vec<ModProject>> {
        Ok(vec![ModProject {
            id: "stub-feather".into(),
            name: "FeatherFly".into(),
            description: "Utility stub from CurseForge".into(),
            source: ModSource::CurseForge,
            downloads: 89_000,
            categories: vec!["utility".into()],
        }])
    }

    async fn get_project(&self, id: &str) -> Result<ModProject> {
        Ok(ModProject {
            id: id.into(),
            name: id.into(),
            description: "CurseForge stub project".into(),
            source: ModSource::CurseForge,
            downloads: 0,
            categories: vec![],
        })
    }
}

pub async fn install_mod(_project: &ModProject) -> Result<()> {
    Err(EasyMcError::NotImplemented("mod download/install"))
}
