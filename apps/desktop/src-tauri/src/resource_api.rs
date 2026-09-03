use std::sync::OnceLock;

#[cfg(feature = "wake-word")]
use std::{fs, io::Write, path::PathBuf};

#[cfg(feature = "wake-word")]
use sha2::{Digest, Sha256};
use tauri::{AppHandle, State};
#[cfg(feature = "wake-word")]
use uuid::Uuid;
#[cfg(feature = "wake-word")]
use voice_runtime::{
    sherpa_wake::SherpaWakeWordDetector,
    wake::SherpaWakeConfig,
    wake_keywords::prepare_gigaspeech_keyword,
};

use super::{
    resource_installer::{ResourceInstallResult, ResourceInstaller},
    resource_manifest::{manifests, ResourceInstallManifest},
    wake_desktop::WakeService,
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
    wake: State<'_, WakeService>,
) -> Result<ResourceInstallResult, String> {
    let resource_id = resource_id.trim();
    if resource_id == WAKE_KEYWORDS_ACTION_ID {
        return prepare_wake_keywords(phrase, state.inner(), wake.inner()).await;
    }

    installer()?
        .install(&app, resource_id, &state.resources)
        .await
}

#[cfg(feature = "wake-word")]
struct PreparedWakeReplacement {
    detector: SherpaWakeWordDetector,
    phrase: String,
    destination: PathBuf,
    backup: Option<PathBuf>,
    content: Vec<u8>,
}

#[cfg(feature = "wake-word")]
async fn prepare_wake_keywords(
    phrase: Option<String>,
    state: &DesktopState,
    wake: &WakeService,
) -> Result<ResourceInstallResult, String> {
    let phrase = phrase
        .ok_or_else(|| "wake keyword preparation requires a phrase".to_owned())?;
    let model_dir = state.resources.wake_model_dir().to_path_buf();
    let bpe_model = state.resources.wake_bpe_model_path().to_path_buf();
    let tokens = state.resources.wake_tokens_path();
    let destination = state.resources.wake_keywords_path().to_path_buf();

    let replacement = tokio::task::spawn_blocking(move || {
        prepare_wake_replacement(model_dir, bpe_model, tokens, destination, phrase)
    })
    .await
    .map_err(|error| format!("wake keyword preparation worker failed: {error}"))??;

    let PreparedWakeReplacement {
        detector,
        phrase,
        destination,
        backup,
        content,
    } = replacement;

    if let Err(error) = wake.reload_or_start(detector, phrase).await {
        let destination_for_rollback = destination.clone();
        let rollback = tokio::task::spawn_blocking(move || {
            rollback_keyword_swap(&destination_for_rollback, backup.as_deref())
        })
        .await
        .map_err(|join| format!("wake keyword rollback worker failed: {join}"))?;
        if let Err(rollback_error) = rollback {
            return Err(format!(
                "wake detector reload failed: {error}; keyword rollback also failed: {rollback_error}"
            ));
        }
        return Err(format!("wake detector reload failed; previous keywords restored: {error}"));
    }

    if let Some(backup) = backup {
        let _ = fs::remove_file(backup);
    }

    let sha256 = format!("{:x}", Sha256::digest(&content));
    Ok(ResourceInstallResult {
        resource_id: WAKE_KEYWORDS_ACTION_ID.to_owned(),
        path: destination.display().to_string(),
        bytes: content.len() as u64,
        sha256,
    })
}

#[cfg(feature = "wake-word")]
fn prepare_wake_replacement(
    model_dir: PathBuf,
    bpe_model: PathBuf,
    tokens: PathBuf,
    destination: PathBuf,
    phrase: String,
) -> Result<PreparedWakeReplacement, String> {
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
    let backup_path = parent.join(format!(".{file_name}.{}.bak", Uuid::new_v4()));
    let content = format!("{}\n", prepared.keyword_line).into_bytes();

    let result = (|| -> Result<PreparedWakeReplacement, String> {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| format!("cannot create temporary keywords file: {error}"))?;
        file.write_all(&content)
            .map_err(|error| format!("cannot write temporary keywords file: {error}"))?;
        file.flush()
            .map_err(|error| format!("cannot flush temporary keywords file: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("cannot sync temporary keywords file: {error}"))?;
        drop(file);

        // Load the complete native detector before touching the currently active
        // keyword file. This validates the generated keyword against the actual
        // model/token configuration, not just SentencePiece syntax.
        let detector = SherpaWakeWordDetector::load(SherpaWakeConfig::gigaspeech_int8(
            &model_dir,
            &temporary,
        ))
        .map_err(|error| format!("new wake detector could not load generated keywords: {error}"))?;

        let backup = if destination.exists() {
            fs::rename(&destination, &backup_path)
                .map_err(|error| format!("cannot stage previous keywords file: {error}"))?;
            Some(backup_path.clone())
        } else {
            None
        };

        if let Err(error) = fs::rename(&temporary, &destination) {
            if let Some(backup) = backup.as_deref() {
                let _ = fs::rename(backup, &destination);
            }
            return Err(format!("cannot install generated keywords file: {error}"));
        }

        Ok(PreparedWakeReplacement {
            detector,
            phrase: prepared.phrase,
            destination,
            backup,
            content,
        })
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(feature = "wake-word")]
fn rollback_keyword_swap(destination: &std::path::Path, backup: Option<&std::path::Path>) -> Result<(), String> {
    if destination.exists() {
        fs::remove_file(destination)
            .map_err(|error| format!("cannot remove failed replacement keywords: {error}"))?;
    }
    if let Some(backup) = backup {
        fs::rename(backup, destination)
            .map_err(|error| format!("cannot restore previous keywords file: {error}"))?;
    }
    Ok(())
}

#[cfg(not(feature = "wake-word"))]
async fn prepare_wake_keywords(
    _phrase: Option<String>,
    _state: &DesktopState,
    _wake: &WakeService,
) -> Result<ResourceInstallResult, String> {
    Err("Bản build hiện tại chưa bật feature `wake-word`; không thể tokenize wake phrase local.".into())
}

fn installer() -> Result<&'static ResourceInstaller, String> {
    match INSTALLER.get_or_init(ResourceInstaller::new) {
        Ok(installer) => Ok(installer),
        Err(error) => Err(error.clone()),
    }
}
