use std::sync::OnceLock;

use tauri::{AppHandle, State};

use super::{
    resource_installer::{ResourceInstallResult, ResourceInstaller},
    resource_manifest::{manifests, ResourceInstallManifest},
    DesktopState,
};

static INSTALLER: OnceLock<Result<ResourceInstaller, String>> = OnceLock::new();

#[tauri::command]
pub async fn assistant_resource_catalog() -> Vec<ResourceInstallManifest> {
    manifests()
}

#[tauri::command]
pub async fn assistant_resource_install(
    app: AppHandle,
    resource_id: String,
    state: State<'_, DesktopState>,
) -> Result<ResourceInstallResult, String> {
    installer()?
        .install(&app, resource_id.trim(), &state.resources)
        .await
}

fn installer() -> Result<&'static ResourceInstaller, String> {
    match INSTALLER.get_or_init(ResourceInstaller::new) {
        Ok(installer) => Ok(installer),
        Err(error) => Err(error.clone()),
    }
}
