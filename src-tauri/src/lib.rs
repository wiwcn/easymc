use easymc_core::{sample_instances, Instance};
use easymc_launch_desktop::{DesktopLauncher, LaunchRequest, LaunchStatus, StubDesktopLauncher};
use serde::Serialize;

#[derive(Serialize)]
struct LaunchResult {
    ok: bool,
    message: String,
}

#[tauri::command]
fn list_instances() -> Vec<Instance> {
    sample_instances()
}

#[tauri::command]
async fn launch_instance(instance_id: String) -> Result<LaunchResult, String> {
    let instances = sample_instances();
    let instance = instances
        .into_iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| format!("unknown instance: {instance_id}"))?;

    let launcher = StubDesktopLauncher;
    let status: LaunchStatus = launcher
        .launch(
            &instance,
            &LaunchRequest {
                instance_id: instance.id.clone(),
                jvm_args: vec![],
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(LaunchResult {
        ok: true,
        message: status.message,
    })
}

#[tauri::command]
fn app_info() -> serde_json::Value {
    serde_json::json!({
        "name": "EasyMC",
        "version": env!("CARGO_PKG_VERSION"),
        "edition": "Java",
        "milestone": "M0",
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            list_instances,
            launch_instance,
            app_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running EasyMC");
}
