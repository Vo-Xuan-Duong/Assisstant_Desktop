use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tauri::State;
use windows_tools::secret::{
    is_dpapi_text, protect_text_for_current_user, unprotect_text_for_current_user,
};

use crate::DesktopState;

const SETTINGS_FILE: &str = "satellite.json";
const DEVICES_FILE: &str = "satellite-devices.json";
const REVOKED_DIR: &str = "satellite-revoked";
const REMOTE_STATE_FILE: &str = "satellite-tailscale.json";
const TOKEN_ENV: &str = "ASSISTANT_VOICE_SATELLITE_TOKEN";
const DEFAULT_BIND: &str = "0.0.0.0:8765";
const MAX_DEVICE_ID_CHARS: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SatelliteSettings {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_bind")]
    bind: String,
    #[serde(default)]
    token: Option<String>,
}

impl Default for SatelliteSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: default_bind(),
            token: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DeviceRegistry {
    #[serde(default)]
    devices: Vec<TrustedDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustedDevice {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    first_seen_unix: u64,
    #[serde(default)]
    last_seen_unix: u64,
    #[serde(default)]
    revoked: bool,
}

#[tauri::command]
pub async fn assistant_satellite_set_enabled(
    enabled: bool,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    let app_data = state.runtime_paths.app_local_data.clone();
    tokio::task::spawn_blocking(move || set_enabled(&app_data, enabled))
        .await
        .map_err(|error| format!("satellite settings worker failed: {error}"))?
}

#[tauri::command]
pub async fn assistant_satellite_set_device_revoked(
    device_id: String,
    revoked: bool,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    let app_data = state.runtime_paths.app_local_data.clone();
    tokio::task::spawn_blocking(move || set_device_revoked(&app_data, &device_id, revoked))
        .await
        .map_err(|error| format!("satellite device worker failed: {error}"))?
}

fn set_enabled(app_data: &Path, enabled: bool) -> Result<(), String> {
    let settings_dir = app_data.join("settings");
    if std::env::var_os(TOKEN_ENV).is_some() {
        return Err(format!(
            "{TOKEN_ENV} is active, so the persisted listener switch is not authoritative. Remove the environment override or manage the satellite from the CLI."
        ));
    }
    if settings_dir.join(REMOTE_STATE_FILE).is_file() {
        return Err(
            "Tailscale managed mode is active. Use `assistant satellite remote tailscale ...` so remote restore state remains authoritative."
                .to_owned(),
        );
    }

    let path = settings_dir.join(SETTINGS_FILE);
    let mut settings = load_settings(&path)?;
    if enabled {
        settings.token = Some(validated_persisted_token(settings.token.as_deref())?);
    }
    settings.enabled = enabled;
    save_settings(&path, &settings)
}

fn validated_persisted_token(raw: Option<&str>) -> Result<String, String> {
    let raw = raw.map(str::trim).filter(|value| !value.is_empty()).ok_or_else(|| {
        "Satellite has no pairing token. Pair the Android device from `assistant satellite pair --qr` first."
            .to_owned()
    })?;

    if is_dpapi_text(raw) {
        let token = unprotect_text_for_current_user(raw)
            .map_err(|error| format!("cannot decrypt the satellite pairing token: {error}"))?;
        if token.trim().len() < 16 {
            return Err(
                "Satellite pairing token is invalid; pair the device again from the CLI.".into(),
            );
        }
        return Ok(raw.to_owned());
    }

    if raw.len() < 16 {
        return Err(
            "Satellite pairing token is invalid; pair the device again from the CLI.".into(),
        );
    }
    protect_text_for_current_user(raw).map_err(|error| {
        format!("cannot migrate satellite pairing token to Windows DPAPI: {error}")
    })
}

fn set_device_revoked(app_data: &Path, raw_id: &str, revoked: bool) -> Result<(), String> {
    let device_id = validate_device_id(raw_id)?;
    let settings_dir = app_data.join("settings");
    let registry_path = settings_dir.join(DEVICES_FILE);
    let revoked_dir = settings_dir.join(REVOKED_DIR);
    let marker = revoked_dir.join(format!("{device_id}.revoked"));
    let mut registry = load_registry(&registry_path)?;
    let device = registry
        .devices
        .iter_mut()
        .find(|device| device.id == device_id)
        .ok_or_else(|| format!("unknown satellite device `{device_id}`"))?;

    if revoked {
        // Marker files are authoritative in the running satellite server. Do not
        // rewrite the registry here: an active trusted connection may be updating
        // last_seen concurrently, and the marker alone is sufficient to revoke it.
        fs::create_dir_all(&revoked_dir)
            .map_err(|error| format!("cannot create {}: {error}", revoked_dir.display()))?;
        fs::write(&marker, b"revoked\n")
            .map_err(|error| format!("cannot write {}: {error}", marker.display()))?;
        return Ok(());
    }

    // Keep the marker in place until a legacy registry-level revoke flag has
    // been cleared successfully. This makes allow fail closed on write errors.
    if device.revoked {
        device.revoked = false;
        save_registry(&registry_path, &registry)?;
    }
    if marker.is_file() {
        fs::remove_file(&marker)
            .map_err(|error| format!("cannot remove {}: {error}", marker.display()))?;
    }
    Ok(())
}

fn load_settings(path: &Path) -> Result<SatelliteSettings, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SatelliteSettings::default());
        }
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))
}

fn save_settings(path: &Path, settings: &SatelliteSettings) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("satellite settings path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("cannot serialize satellite settings: {error}"))?;
    replace_file(path, &bytes)
}

fn load_registry(path: &Path) -> Result<DeviceRegistry, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DeviceRegistry::default());
        }
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))
}

fn save_registry(path: &Path, registry: &DeviceRegistry) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(registry)
        .map_err(|error| format!("cannot serialize satellite device registry: {error}"))?;
    replace_file(path, &bytes)
}

fn replace_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let temp: PathBuf = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temp, bytes).map_err(|error| format!("cannot write {}: {error}", temp.display()))?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| {
            let _ = fs::remove_file(&temp);
            format!("cannot replace {}: {error}", path.display())
        })?;
    }
    fs::rename(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        format!("cannot promote {}: {error}", path.display())
    })
}

fn validate_device_id(value: &str) -> Result<String, String> {
    let value = value.trim();
    let chars = value.chars().count();
    if !(8..=MAX_DEVICE_ID_CHARS).contains(&chars) {
        return Err(format!(
            "device-id must contain 8..={MAX_DEVICE_ID_CHARS} characters"
        ));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
    {
        return Err("device-id contains unsupported characters".into());
    }
    Ok(value.to_owned())
}

fn default_bind() -> String {
    DEFAULT_BIND.to_owned()
}
