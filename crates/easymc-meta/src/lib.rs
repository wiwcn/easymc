//! Mojang piston-meta client + installer for Java Edition.

mod rules;
mod types;
mod install;
mod download;
mod fabric;
mod forge;

pub use install::*;
pub use download::{download_file, FetchResult};
pub use types::*;
pub use rules::{rules_allow, current_os_name, is_native_library};
pub use fabric::*;
pub use forge::{
    forge_profile_id, install_forge, install_neoforge, install_forge_family,
    list_forge_versions, list_neoforge_versions, neoforge_profile_id,
};

use async_trait::async_trait;
use easymc_core::{EasyMcError, McVersion, Result};

pub const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[async_trait]
pub trait MetaProvider: Send + Sync {
    async fn list_versions(&self) -> Result<Vec<VersionSummary>>;
    async fn resolve(&self, version: &McVersion) -> Result<VersionInfo>;
}

pub struct PistonMetaProvider {
    client: reqwest::Client,
}

impl Default for PistonMetaProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl PistonMetaProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!("EasyMC/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client");
        Self { client }
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }
}

#[async_trait]
impl MetaProvider for PistonMetaProvider {
    async fn list_versions(&self) -> Result<Vec<VersionSummary>> {
        let manifest: VersionManifest = self
            .client
            .get(VERSION_MANIFEST_URL)
            .send()
            .await
            .map_err(|e| EasyMcError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| EasyMcError::Http(e.to_string()))?
            .json()
            .await
            .map_err(|e| EasyMcError::Http(e.to_string()))?;
        Ok(manifest.versions)
    }

    async fn resolve(&self, version: &McVersion) -> Result<VersionInfo> {
        let list = self.list_versions().await?;
        let summary = list
            .into_iter()
            .find(|v| v.id == version.0)
            .ok_or_else(|| EasyMcError::Missing(format!("version {}", version.0)))?;
        let info: VersionInfo = self
            .client
            .get(&summary.url)
            .send()
            .await
            .map_err(|e| EasyMcError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| EasyMcError::Http(e.to_string()))?
            .json()
            .await
            .map_err(|e| EasyMcError::Http(e.to_string()))?;
        Ok(info)
    }
}

/// Kept for tests / offline demos.
pub struct StubMetaProvider;

#[async_trait]
impl MetaProvider for StubMetaProvider {
    async fn list_versions(&self) -> Result<Vec<VersionSummary>> {
        Ok(vec![
            VersionSummary {
                id: "1.21.1".into(),
                version_type: "release".into(),
                url: String::new(),
                sha1: None,
            },
            VersionSummary {
                id: "1.20.1".into(),
                version_type: "release".into(),
                url: String::new(),
                sha1: None,
            },
        ])
    }

    async fn resolve(&self, version: &McVersion) -> Result<VersionInfo> {
        Err(EasyMcError::msg(format!(
            "StubMetaProvider cannot resolve {version}; use PistonMetaProvider"
        )))
    }
}
