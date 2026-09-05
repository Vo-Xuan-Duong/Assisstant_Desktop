use std::{collections::HashSet, path::{Path, PathBuf}, sync::Arc};

use reqwest::{Client, redirect::Policy};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};
use tokio::{fs, io::AsyncWriteExt, sync::Mutex};
use uuid::Uuid;

use super::{
    resource_manifest::{
        ResourceInstallManifest, ResourcePackageKind, STT_RESOURCE_ID, manifest,
    },
    resource_registry::ResourceRegistry,
};

const PROGRESS_EVENT: &str = "resource:install_progress";
const PROGRESS_GRANULARITY: u64 = 1024 * 1024;
const ZIPFORMER_REVISION: &str = "83e140db6d23fbb8480fd5fb868f74ab80e7092c";
const ZIPFORMER_REPO: &str = "csukuangfj2/sherpa-onnx-zipformer-vi-30M-int8-2026-02-09";
const TOKENS_MAX_BYTES: u64 = 128 * 1024;
const TOKENS_EXPECTED_LINES: usize = 2_000;

#[derive(Debug, Clone, Copy)]
struct BundleComponent {
    name: &'static str,
    expected_bytes: u64,
    sha256: &'static str,
}

const ZIPFORMER_COMPONENTS: [BundleComponent; 4] = [
    BundleComponent {
        name: "encoder.int8.onnx",
        expected_bytes: 27_699_063,
        sha256: "8ef5286dd427eb108055c2ddc1982aa31e544706072d5ea228729292dacade68",
    },
    BundleComponent {
        name: "decoder.onnx",
        expected_bytes: 5_165_084,
        sha256: "cf2aa385b82c9d5d40cd29c3188af52d0249b3b78f0d4b7eb84ad502d50c7e7f",
    },
    BundleComponent {
        name: "joiner.int8.onnx",
        expected_bytes: 1_033_417,
        sha256: "7311d2e17b810ecea515d79c71cc4668af8759256a06fa01d27047772320c821",
    },
    BundleComponent {
        name: "bpe.model",
        expected_bytes: 268_106,
        sha256: "002894e7a82d80ffa5e25008ec8c5496159db804005e2103de96b01b4c13d445",
    },
];

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
        let manifest =
            manifest(resource_id).ok_or_else(|| format!("unknown resource id: {resource_id}"))?;
        if !manifest.installable {
            return Err(format!(
                "resource `{}` is not enabled for automatic installation: {}",
                manifest.id, manifest.note
            ));
        }

        {
            let mut active = self.active.lock().await;
            if !active.insert(manifest.id.to_owned()) {
                return Err(format!(
                    "resource `{}` is already being installed",
                    manifest.id
                ));
            }
        }

        let result = match manifest.package_kind {
            ResourcePackageKind::MultiFile if manifest.id == STT_RESOURCE_ID => {
                self.install_zipformer_bundle(app, &manifest, resources).await
            }
            ResourcePackageKind::SingleFile => {
                let expected_sha256 = manifest
                    .sha256
                    .ok_or_else(|| format!("resource `{}` has no pinned SHA-256", manifest.id));
                match expected_sha256 {
                    Ok(expected_sha256) => {
                        self.install_single_file(app, &manifest, expected_sha256, resources)
                            .await
                    }
                    Err(error) => Err(error),
                }
            }
            _ => Err(format!(
                "resource `{}` uses package kind {:?}, which has no enabled installer",
                manifest.id, manifest.package_kind
            )),
        };

        self.active.lock().await.remove(manifest.id);
        result
    }

    async fn install_zipformer_bundle(
        &self,
        app: &AppHandle,
        manifest: &ResourceInstallManifest,
        resources: &ResourceRegistry,
    ) -> Result<ResourceInstallResult, String> {
        let destination = resources.stt_model_dir().to_path_buf();
        ensure_bundle_destination_available(&destination).await?;
        let parent = destination
            .parent()
            .ok_or_else(|| "STT model destination has no parent directory".to_owned())?;
        fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("cannot create STT model parent directory: {error}"))?;

        let dir_name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("zipformer-stt");
        let staging = parent.join(format!(".{dir_name}.{}.part", Uuid::new_v4()));
        fs::create_dir(&staging)
            .await
            .map_err(|error| format!("cannot create STT staging directory: {error}"))?;

        emit_progress(
            app,
            manifest.id,
            "starting",
            0,
            manifest.expected_bytes,
            format!(
                "Downloading Vietnamese Zipformer from immutable revision {ZIPFORMER_REVISION}"
            ),
        );

        let result = async {
            let mut total_downloaded = 0u64;
            let mut bundle_hasher = Sha256::new();

            for component in ZIPFORMER_COMPONENTS {
                let target = staging.join(component.name);
                let url = zipformer_resolve_url(component.name);
                let (bytes, actual_sha256) = self
                    .download_component(
                        app,
                        manifest,
                        &url,
                        &target,
                        component.expected_bytes,
                        component.sha256,
                        total_downloaded,
                    )
                    .await?;
                total_downloaded = total_downloaded
                    .checked_add(bytes)
                    .ok_or_else(|| "STT bundle byte count overflow".to_owned())?;
                bundle_hasher.update(component.name.as_bytes());
                bundle_hasher.update(actual_sha256.as_bytes());
            }

            if total_downloaded != manifest.expected_bytes {
                return Err(format!(
                    "STT binary bundle size mismatch: expected {}, downloaded {total_downloaded}",
                    manifest.expected_bytes
                ));
            }

            let tokens_path = staging.join("tokens.txt");
            let tokens_url = zipformer_resolve_url("tokens.txt");
            let (token_bytes, token_sha256) = self
                .download_bounded_text(&tokens_url, &tokens_path, TOKENS_MAX_BYTES)
                .await?;
            validate_zipformer_tokens(&tokens_path).await?;
            bundle_hasher.update(b"tokens.txt");
            bundle_hasher.update(token_sha256.as_bytes());

            emit_progress(
                app,
                manifest.id,
                "verified",
                manifest.expected_bytes,
                manifest.expected_bytes,
                format!(
                    "Verified four pinned model assets and validated tokens.txt ({token_bytes} bytes)."
                ),
            );

            if destination.exists() {
                return Err(format!(
                    "STT destination appeared during installation; refusing to overwrite: {}",
                    destination.display()
                ));
            }
            fs::rename(&staging, &destination)
                .await
                .map_err(|error| format!(
                    "verified STT bundle could not be atomically installed at {}: {error}",
                    destination.display()
                ))?;

            let bundle_sha256 = bundle_hasher
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            let installed_bytes = total_downloaded
                .checked_add(token_bytes)
                .ok_or_else(|| "STT installed byte count overflow".to_owned())?;

            emit_progress(
                app,
                manifest.id,
                "installed",
                manifest.expected_bytes,
                manifest.expected_bytes,
                "Vietnamese Zipformer bundle installed atomically.".into(),
            );

            Ok(ResourceInstallResult {
                resource_id: manifest.id.to_owned(),
                path: destination.display().to_string(),
                bytes: installed_bytes,
                sha256: bundle_sha256,
            })
        }
        .await;

        if result.is_err() && staging.exists() {
            let _ = fs::remove_dir_all(&staging).await;
        }
        if let Err(error) = &result {
            emit_failed(app, manifest, error.clone());
        }
        result
    }

    async fn install_single_file(
        &self,
        app: &AppHandle,
        manifest: &ResourceInstallManifest,
        expected_sha256: &str,
        _resources: &ResourceRegistry,
    ) -> Result<ResourceInstallResult, String> {
        let error = format!(
            "no single-file destination is registered for resource `{}`",
            manifest.id
        );
        emit_failed(app, manifest, error.clone());
        let _ = expected_sha256;
        Err(error)
    }

    async fn download_component(
        &self,
        app: &AppHandle,
        manifest: &ResourceInstallManifest,
        url: &str,
        destination: &Path,
        expected_bytes: u64,
        expected_sha256: &str,
        completed_before: u64,
    ) -> Result<(u64, String), String> {
        let mut response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| format!("resource download request failed for {url}: {error}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "resource server returned HTTP {} for {url}",
                response.status()
            ));
        }
        if let Some(length) = response.content_length() {
            if length != expected_bytes {
                return Err(format!(
                    "resource Content-Length mismatch for {}: expected {expected_bytes}, received {length}",
                    destination.display()
                ));
            }
        }

        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(destination)
            .await
            .map_err(|error| format!("cannot create staged model file: {error}"))?;
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
            if downloaded > expected_bytes {
                return Err(format!(
                    "resource exceeded pinned size for {}: expected {expected_bytes}, received more than {downloaded}",
                    destination.display()
                ));
            }
            file.write_all(&chunk)
                .await
                .map_err(|error| format!("cannot write staged model file: {error}"))?;
            hasher.update(&chunk);

            if downloaded.saturating_sub(last_progress) >= PROGRESS_GRANULARITY
                || downloaded == expected_bytes
            {
                last_progress = downloaded;
                emit_progress(
                    app,
                    manifest.id,
                    "downloading",
                    completed_before.saturating_add(downloaded),
                    manifest.expected_bytes,
                    format!("Downloading and hashing {}...", destination.file_name().and_then(|v| v.to_str()).unwrap_or("model file")),
                );
            }
        }

        file.flush()
            .await
            .map_err(|error| format!("cannot flush staged model file: {error}"))?;
        file.sync_all()
            .await
            .map_err(|error| format!("cannot sync staged model file: {error}"))?;
        drop(file);

        if downloaded != expected_bytes {
            return Err(format!(
                "resource size mismatch for {}: expected {expected_bytes}, downloaded {downloaded}",
                destination.display()
            ));
        }
        let actual_sha256 = hex_digest(hasher.finalize());
        if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
            return Err(format!(
                "resource SHA-256 mismatch for {}: expected {expected_sha256}, received {actual_sha256}",
                destination.display()
            ));
        }
        Ok((downloaded, actual_sha256))
    }

    async fn download_bounded_text(
        &self,
        url: &str,
        destination: &Path,
        max_bytes: u64,
    ) -> Result<(u64, String), String> {
        let mut response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| format!("tokens download request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "tokens server returned HTTP {}",
                response.status()
            ));
        }
        if response.content_length().is_some_and(|length| length > max_bytes) {
            return Err("tokens.txt exceeds the bounded download limit".into());
        }

        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(destination)
            .await
            .map_err(|error| format!("cannot create staged tokens.txt: {error}"))?;
        let mut hasher = Sha256::new();
        let mut downloaded = 0u64;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| format!("tokens download stream failed: {error}"))?
        {
            downloaded = downloaded
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| "tokens byte count overflow".to_owned())?;
            if downloaded > max_bytes {
                return Err("tokens.txt exceeded the bounded download limit".into());
            }
            file.write_all(&chunk)
                .await
                .map_err(|error| format!("cannot write staged tokens.txt: {error}"))?;
            hasher.update(&chunk);
        }
        file.flush()
            .await
            .map_err(|error| format!("cannot flush staged tokens.txt: {error}"))?;
        file.sync_all()
            .await
            .map_err(|error| format!("cannot sync staged tokens.txt: {error}"))?;
        drop(file);
        Ok((downloaded, hex_digest(hasher.finalize())))
    }
}

async fn ensure_bundle_destination_available(destination: &Path) -> Result<(), String> {
    if !destination.exists() {
        return Ok(());
    }
    if !destination.is_dir() {
        return Err(format!(
            "STT destination exists but is not a directory: {}",
            destination.display()
        ));
    }
    let mut entries = fs::read_dir(destination)
        .await
        .map_err(|error| format!("cannot inspect existing STT model directory: {error}"))?;
    if entries
        .next_entry()
        .await
        .map_err(|error| format!("cannot inspect existing STT model directory: {error}"))?
        .is_some()
    {
        return Err(format!(
            "STT model directory already contains files; refusing to overwrite: {}",
            destination.display()
        ));
    }
    fs::remove_dir(destination)
        .await
        .map_err(|error| format!("cannot remove empty STT model directory: {error}"))?;
    Ok(())
}

async fn validate_zipformer_tokens(path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(path)
        .await
        .map_err(|error| format!("cannot read downloaded tokens.txt as UTF-8: {error}"))?;
    let lines = content.lines().collect::<Vec<_>>();
    if lines.len() != TOKENS_EXPECTED_LINES {
        return Err(format!(
            "tokens.txt structure mismatch: expected {TOKENS_EXPECTED_LINES} lines, found {}",
            lines.len()
        ));
    }
    for (index, line) in lines.iter().enumerate() {
        let (_, id) = line
            .rsplit_once(' ')
            .ok_or_else(|| format!("tokens.txt line {} has no numeric id", index + 1))?;
        let parsed = id
            .parse::<usize>()
            .map_err(|_| format!("tokens.txt line {} has an invalid id", index + 1))?;
        if parsed != index {
            return Err(format!(
                "tokens.txt id sequence mismatch at line {}: expected {index}, received {parsed}",
                index + 1
            ));
        }
    }
    if lines.first().copied() != Some("<blk> 0")
        || lines.get(1).copied() != Some("<sos/eos> 1")
        || lines.get(2).copied() != Some("<unk> 2")
    {
        return Err("tokens.txt does not contain the expected Zipformer special tokens".into());
    }
    Ok(())
}

fn zipformer_resolve_url(file_name: &str) -> String {
    format!(
        "https://huggingface.co/{ZIPFORMER_REPO}/resolve/{ZIPFORMER_REVISION}/{file_name}?download=true"
    )
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn emit_failed(app: &AppHandle, manifest: &ResourceInstallManifest, message: String) {
    emit_progress(
        app,
        manifest.id,
        "failed",
        0,
        manifest.expected_bytes,
        message,
    );
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
