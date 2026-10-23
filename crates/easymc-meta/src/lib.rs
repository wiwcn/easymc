//! Version / metadata provider stubs (no real download yet).

use async_trait::async_trait;
use easymc_core::{EasyMcError, McVersion, Result};

/// Fetches Java Edition version manifests (stub).
#[async_trait]
pub trait MetaProvider: Send + Sync {
    async fn list_versions(&self) -> Result<Vec<McVersion>>;
    async fn resolve(&self, version: &McVersion) -> Result<VersionMeta>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VersionMeta {
    pub id: McVersion,
    pub release_type: String,
}

/// Placeholder that returns a fixed list — no network in M0.
pub struct StubMetaProvider;

#[async_trait]
impl MetaProvider for StubMetaProvider {
    async fn list_versions(&self) -> Result<Vec<McVersion>> {
        Ok(vec![
            McVersion("1.21.1".into()),
            McVersion("1.20.1".into()),
        ])
    }

    async fn resolve(&self, version: &McVersion) -> Result<VersionMeta> {
        Ok(VersionMeta {
            id: version.clone(),
            release_type: "release".into(),
        })
    }
}

/// Real Mojang / Prism-meta clients land in M1.
pub async fn download_client(_version: &McVersion) -> Result<()> {
    Err(EasyMcError::NotImplemented("client download"))
}
