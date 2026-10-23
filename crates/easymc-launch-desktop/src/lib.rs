//! Desktop launcher for Minecraft Java Edition (M0 stubs).

use async_trait::async_trait;
use easymc_core::{EasyMcError, Instance, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchRequest {
    pub instance_id: String,
    pub jvm_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchStatus {
    pub running: bool,
    pub message: String,
}

#[async_trait]
pub trait DesktopLauncher: Send + Sync {
    async fn launch(&self, instance: &Instance, req: &LaunchRequest) -> Result<LaunchStatus>;
    async fn stop(&self, instance_id: &str) -> Result<()>;
}

/// Returns a toast-friendly message; does not spawn a JVM.
pub struct StubDesktopLauncher;

#[async_trait]
impl DesktopLauncher for StubDesktopLauncher {
    async fn launch(&self, instance: &Instance, _req: &LaunchRequest) -> Result<LaunchStatus> {
        Ok(LaunchStatus {
            running: false,
            message: format!(
                "M0 stub: would launch 「{}」 ({})",
                instance.name, instance.version.0
            ),
        })
    }

    async fn stop(&self, _instance_id: &str) -> Result<()> {
        Err(EasyMcError::NotImplemented("stop process"))
    }
}
