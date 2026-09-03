use std::sync::OnceLock;

#[cfg(feature = "wake-word")]
use std::{fs, io::Write};

#[cfg(feature = "wake-word")]
use sha2::{Digest, Sha256};
use tauri::{AppHandle, State};
#[cfg(feature = "wake-word")]
use uuid::Uuid;
#[cfg(feature = "wake-word")]
use voice_runtime::wake_keywords::prepare_gigaspeech_keyword;

use super::{
    resource_installer::{ResourceInstallResult, ResourceInstaller},
    resource_manifest::{manifests, ResourceInstallManifest},
    DesktopState,
};

const WAKE_KEYWORDS_ACTION_ID: &str = "wake_keywords";
static INSTALLER: OnceLock<Result<ResourceInstaller, String>> = OnceLock::new();

#[tauri::command]
pub async fn assistant_resource_catalog() -> Vec<ResourceInstallManifest> {
    manifests()
}

#[tauri::command]
pub async fn assistant_resource_install(
    app: AppHandle,
    resource_id: String,
    phrase: Option<String>,
    state: State<'_, DesktopState>,
) -> Result<ResourceInstallResult, String> {
    let resource_id = resource_id.trim();
    if resource_id == WAKE_KEYWORDS_ACTION_ID {
        return prepare_wake_keywords(phrase, state.inner()).await;
    }

    installer()?
        .install(&app, resource_id, &state.resources)
        .await
}

#[cfg(feature = "wake-word")]
async fn prepare_wake_keywords(
    phrase: Option<String>,
    state: &DesktopState,
) -> Result<ResourceInstallResult, String> {
    let phrase = phrase
        .ok_or_else(|| "wake keyword preparation requires a phrase".to_owned())?;
    let bpe_model = state.resources.wake_bpe_model_path().to_path_buf();
    let tokens = state.resources.wake_tokens_path();
    let destination = state.resources.wake_keywords_path().to_path_buf();

    tokio::task::spawn_blocking(move || {
        if destination.exists() {
            return Err(format!(
                "keywords file already exists; refusing to overwrite it in Phase 13C: {}",
                destination.display()
            ));
        }

        let prepared = prepare_gigaspeech_keyword(&bpe_model, &tokens, &phrase)
            .map_err(|error| error.to_string())?;
        let parent = destination
            .parent()
            .ok_or_else(|| "wake keywords destination has no parent directory".to_owned())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create wake keyword directory: {error}"))?;

        let file_name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("keywords.txt");
        let temporary = parent.join(format!(".{file_name}.{}.part", Uuid::new_v4()));
        let content = format!("{}\n", prepared.keyword_line);

        let write_result = (|| -> Result<(), String> {
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)
                .map_err(|error| format!("cannot create temporary keywords file: {error}"))?;
            file.write_all(content.as_bytes())
                .map_err(|error| format!("cannot write temporary keywords file: {error}"))?;
            file.flush()
                .map_err(|error| format!("cannot flush temporary keywords file: {error}"))?;
            file.sync_all()
                .map_err(|error| format!("cannot sync temporary keywords file: {error}"))?;
            drop(file);

            if destination.exists() {
                return Err(format!(
                    "keywords destination appeared while preparing; refusing to overwrite: {}",
                    destination.display()
                ));
            }
            fs::rename(&temporary, &destination)
                .map_err(|error| format!("cannot atomically install keywords file: {error}"))?;
            Ok(())
        })();

        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }

        let sha256 = format!("{:x}", Sha256::digest(content.as_bytes()));
        Ok(ResourceInstallResult {
            resource_id: WAKE_KEYWORDS_ACTION_ID.to_owned(),
            path: destination.display().to_string(),
            bytes: content.len() as u64,
            sha256,
        })
    })
    .await
    .map_err(|error| format!("wake keyword preparation worker failed: {error}"))?
}

#[cfg(not(feature = "wake-word"))]
async fn prepare_wake_keywords(
    _phrase: Option<String>,
    _state: &DesktopState,
) -> Result<ResourceInstallResult, String> {
    Err("Bản build hiện tại chưa bật feature `wake-word`; không thể tokenize wake phrase local.".into())
}

fn installer() -> Result<&'static ResourceInstaller, String> {
    match INSTALLER.get_or_init(ResourceInstaller::new) {
        Ok(installer) => Ok(installer),
        Err(error) => Err(error.clone()),
    }
}
