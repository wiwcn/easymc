//! EasyMC core domain types. Java Edition only (M0 stubs).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Minecraft Java Edition version id, e.g. `"1.21.1"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct McVersion(pub String);

/// Loader / mod platform for an instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModLoader {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

/// A playable instance (world / pack) stub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub version: McVersion,
    pub loader: ModLoader,
    pub description: Option<String>,
    pub last_played: Option<String>,
}

#[derive(Debug, Error)]
pub enum EasyMcError {
    #[error("not implemented in M0: {0}")]
    NotImplemented(&'static str),
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, EasyMcError>;

/// Sample instances for UI / smoke tests (no disk I/O).
pub fn sample_instances() -> Vec<Instance> {
    vec![
        Instance {
            id: "survival".into(),
            name: "Survival World".into(),
            version: McVersion("1.21.1".into()),
            loader: ModLoader::Fabric,
            description: Some("Your go-to survival experience.".into()),
            last_played: Some("Today, 4:32 PM".into()),
        },
        Instance {
            id: "creative".into(),
            name: "Creative Build".into(),
            version: McVersion("1.21.1".into()),
            loader: ModLoader::Vanilla,
            description: None,
            last_played: None,
        },
        Instance {
            id: "modded".into(),
            name: "Modded Adventures".into(),
            version: McVersion("1.21.1".into()),
            loader: ModLoader::Fabric,
            description: None,
            last_played: None,
        },
        Instance {
            id: "test".into(),
            name: "Test World".into(),
            version: McVersion("1.21.1".into()),
            loader: ModLoader::Vanilla,
            description: None,
            last_played: None,
        },
    ]
}
