//! Optional container-runtime metadata on an [`crate::Instance`].
//! Native path = `container: None` forever.

use serde::{Deserialize, Serialize};

pub const CONTAINER_RUNTIME_PODMAN: &str = "podman";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerVolume {
    pub host: String,
    pub container: String,
    #[serde(default = "default_mode")]
    pub mode: String,
}

fn default_mode() -> String {
    "rw".into()
}

impl ContainerVolume {
    pub fn rw(host: impl Into<String>, container: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            container: container.into(),
            mode: "rw".into(),
        }
    }

    pub fn ro(host: impl Into<String>, container: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            container: container.into(),
            mode: "ro".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerMeta {
    pub runtime: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default)]
    pub volumes: Vec<ContainerVolume>,
    #[serde(default)]
    pub provisioned: bool,
}

impl ContainerMeta {
    pub fn new_podman(image: impl Into<String>) -> Self {
        Self {
            runtime: CONTAINER_RUNTIME_PODMAN.into(),
            container_id: None,
            image: Some(image.into()),
            volumes: Vec::new(),
            provisioned: false,
        }
    }

    pub fn is_podman(&self) -> bool {
        self.runtime.eq_ignore_ascii_case(CONTAINER_RUNTIME_PODMAN)
    }
}
