use std::{collections::HashSet, path::Path, sync::Arc};

use reqwest::{redirect::Policy, Client};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};
use tokio::{
    fs,
    io::AsyncWriteExt,
    sync::Mutex,
};
use uuid::Uuid;

use super::{
    resource_manifest::{manifest, ResourceInstallManifest, ResourcePackageKind},
    resource_registry::ResourceRegistry,
};

const PROGRESS_EVENT: &str = "resource:install_progress";
const PROGRESS_GRANULARITY: u64 = 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct ResourceInstallProgress {
    pub resource_id: String,
    pub stage: &'static str,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceInstallResult {
    pub resource_id: String,
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone)]
pub struct ResourceInstaller {
    client: Client,
    active: Arc<Mutex<HashSet<String>>>,
}

impl ResourceInstaller {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .redirect(Policy::limited(5))
            .user_agent("Assisstant-Desktop/0.1 resource-installer")
            .build()
            .map_err(|error| format!("cannot create resource HTTP client: {error}"))?;
        Ok(Self {
            client,
            active: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub async fn install(
        &self,
        app: &AppHandle,
        resource_id: &str,
        resources: &ResourceRegistry,
    ) -> Result<ResourceInstallResult, String> {
        let manifest = manifest(resource_id)
            .ok_or_else(|| format!("unknown resource id: {resource_id}"))?;
        if !manifest.installable {
            return Err(format!(
                "resource `{}` is not enabled for automatic installation: {}",
                manifest.id, manifest.note
            ));
        }
        if manifest.package_kind != ResourcePackageKind::SingleFile {
            return Err(format!(
                "resource `{}` requires an archive installer that is not enabled in Phase 13B",
                manifest.id
            ));
        }
        let expected_sha256 = manifest
            .sha256
            .ok_or_else(|| format!("resource `{}` has no pinned SHA-256", manifest.id))?;

        {
            let mut active = self.active.lock().await;
            if !active.insert(manifest.id.to_owned()) {
                return Err(format!("resource `{}` is already being installed", manifest.id));
            }
        }

        let result = self
            .install_single_file(app, &manifest, expected_sha256, resources)
            .await;
        self.active.lock().await.remove(manifest.id);
        result
    }

    async fn install_single_file(
        &self,
        app: &AppHandle,
        manifest: &ResourceInstallManifest,
        expected_sha256: &str,
        resources: &ResourceRegistry,
    ) -> Result<ResourceInstallResult, String> {
        let destination = match manifest.id {
            "whisper" => resources.whisper_model_path(),
            other => return Err(format!("no single-file destination is registered for `{other}`")),
        };

        if destination.exists() {
            return Err(format!(
                "destination already exists; refusing to overwrite local resource: {}",
                destination.display()
            ));
        }

        let parent = destination
            .parent()
            .ok_or_else(|| "resource destination has no parent directory".to_owned())?;
        fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("cannot create resource directory: {error}"))?;

        let file_name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("resource");
        let part = parent.join(format!(".{file_name}.{}.part", Uuid::new_v4()));

        emit_progress(
            app,
            manifest.id,
            "starting",
            0,
            manifest.expected_bytes,
            format!("Downloading verified resource from {}", manifest.source_page),
        );

        let result = self
            .download_verified(app, manifest, expected_sha256, &part)
            .await;

        let (bytes, sha256) = match result {
            Ok(value) => value,
            Err(error) => {
                let _ = fs::remove_file(&part).await;
                emit_progress(
                    app,
                    manifest.id,
                    "failed",
                    0,
                    manifest.expected_bytes,
                    error.clone(),
                );
                return Err(error);
            }
        };

        if destination.exists() {
            let _ = fs::remove_file(&part).await;
            return Err(format!(
                "destination appeared while download was running; refusing to overwrite: {}",
                destination.display()
            ));
        }

        fs::rename(&part, destination)
            .await
            .map_err(|error| {
                let path = destination.display();
                format!("verified download could not be atomically installed at {path}: {error}")
            })?;

        emit_progress(
            app,
            manifest.id,
            "installed",
            bytes,
            manifest.expected_bytes,
            "SHA-256 verified; resource installed.".into(),
        );

        Ok(ResourceInstallResult {
            resource_id: manifest.id.to_owned(),
            path: destination.display().to_string(),
            bytes,
            sha256,
        })
    }

    async fn download_verified(
        &self,
        app: &AppHandle,
        manifest: &ResourceInstallManifest,
        expected_sha256: &str,
        part: &Path,
    ) -> Result<(u64, String), String> {
        let mut response = self
            .client
            .get(manifest.source_url)
            .send()
            .await
            .map_err(|error| format!("resource download request failed: {error}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "resource server returned HTTP {}",
                response.status()
            ));
        }

        if let Some(length) = response.content_length() {
            if length != manifest.expected_bytes {
                return Err(format!(
                    "resource Content-Length mismatch: expected {}, received {length}",
                    manifest.expected_bytes
                ));
            }
        }

        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(part)
            .await
            .map_err(|error| format!("cannot create partial resource file: {error}"))?;
        let mut hasher = Sha256::new();
        let mut downloaded = 0u64;
        let mut last_progress = 0u64;

        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| format!("resource download stream failed: {error}"))?
        {
            downloaded = downloaded
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| "download byte count overflow".to_owned())?;
            if downloaded > manifest.expected_bytes {
                return Err(format!(
                    "resource exceeded pinned size: expected {}, received more than {downloaded}",
                    manifest.expected_bytes
                ));
            }

            file.write_all(&chunk)
                .await
                .map_err(|error| format!("cannot write partial resource file: {error}"))?;
            hasher.update(&chunk);

            if downloaded.saturating_sub(last_progress) >= PROGRESS_GRANULARITY
                || downloaded == manifest.expected_bytes
            {
                last_progress = downloaded;
                emit_progress(
                    app,
                    manifest.id,
                    "downloading",
                    downloaded,
                    manifest.expected_bytes,
                    "Downloading and hashing...".into(),
                );
            }
        }

        file.flush()
            .await
            .map_err(|error| format!("cannot flush partial resource file: {error}"))?;
        file.sync_all()
            .await
            .map_err(|error| format!("cannot sync partial resource file: {error}"))?;
        drop(file);

        if downloaded != manifest.expected_bytes {
            return Err(format!(
                "resource size mismatch: expected {}, downloaded {downloaded}",
                manifest.expected_bytes
            ));
        }

        let actual_sha256 = format!("{:x}", hasher.finalize());
        if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
            return Err(format!(
                "resource SHA-256 mismatch: expected {expected_sha256}, received {actual_sha256}"
            ));
        }

        emit_progress(
            app,
            manifest.id,
            "verified",
            downloaded,
            manifest.expected_bytes,
            "Pinned byte size and SHA-256 verified.".into(),
        );

        Ok((downloaded, actual_sha256))
    }
}

fn emit_progress(
    app: &AppHandle,
    resource_id: &str,
    stage: &'static str,
    downloaded_bytes: u64,
    total_bytes: u64,
    message: String,
) {
    let _ = app.emit(
        PROGRESS_EVENT,
        ResourceInstallProgress {
            resource_id: resource_id.to_owned(),
            stage,
            downloaded_bytes,
            total_bytes,
            message,
        },
    );
}
