use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::DesktopState;

const DEVICES_FILE: &str = "satellite-devices.json";
const REVOKED_DIR: &str = "satellite-revoked";
const MAX_DEVICE_ID_CHARS: usize = 128;
const MAX_DEVICE_NAME_CHARS: usize = 96;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DeviceRegistry {
    #[serde(default)]
    devices: Vec<TrustedDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustedDevice {
    id: String,
    name: String,
    first_seen_unix: u64,
    last_seen_unix: u64,
    #[serde(default)]
    revoked: bool,
}

pub fn record_authenticated_device(
    app: &AppHandle,
    raw_id: &str,
    raw_name: Option<&str>,
) -> Result<(String, String), String> {
    let id = normalize_device_id(raw_id)?;
    if revocation_marker_path(app, &id).is_file() {
        return Err("this Android satellite device has been revoked".to_owned());
    }

    let name = normalize_device_name(raw_name);
    let path = registry_path(app);
    let mut registry = load_registry(&path)?;
    let now = unix_now();

    if let Some(device) = registry.devices.iter_mut().find(|device| device.id == id) {
        // Keep the registry flag for backwards compatibility and diagnostics.
        // The dedicated marker file is authoritative for new revocations because
        // it cannot be overwritten by a concurrent last-seen registry update.
        if device.revoked {
            return Err("this Android satellite device has been revoked".to_owned());
        }
        device.name = name.clone();
        device.last_seen_unix = now;
    } else {
        registry.devices.push(TrustedDevice {
            id: id.clone(),
            name: name.clone(),
            first_seen_unix: now,
            last_seen_unix: now,
            revoked: false,
        });
    }

    registry
        .devices
        .sort_by(|left, right| left.id.cmp(&right.id));
    save_registry(&path, &registry)?;
    Ok((id, name))
}

pub fn is_device_revoked(app: &AppHandle, raw_id: &str) -> Result<bool, String> {
    let id = normalize_device_id(raw_id)?;
    if revocation_marker_path(app, &id).is_file() {
        return Ok(true);
    }

    let path = registry_path(app);
    let registry = load_registry(&path)?;
    Ok(registry
        .devices
        .iter()
        .find(|device| device.id == id)
        .is_some_and(|device| device.revoked))
}

fn settings_dir(app: &AppHandle) -> PathBuf {
    let state = app.state::<DesktopState>();
    state.runtime_paths.app_local_data.join("settings")
}

fn registry_path(app: &AppHandle) -> PathBuf {
    settings_dir(app).join(DEVICES_FILE)
}

fn revocation_marker_path(app: &AppHandle, device_id: &str) -> PathBuf {
    settings_dir(app)
        .join(REVOKED_DIR)
        .join(format!("{device_id}.revoked"))
}

fn normalize_device_id(raw: &str) -> Result<String, String> {
    let id = raw.trim();
    let chars = id.chars().count();
    if !(8..=MAX_DEVICE_ID_CHARS).contains(&chars) {
        return Err(format!(
            "satellite device_id must contain 8..={MAX_DEVICE_ID_CHARS} characters"
        ));
    }
    if !id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
    {
        return Err("satellite device_id contains unsupported characters".to_owned());
    }
    Ok(id.to_owned())
}

fn normalize_device_name(raw: Option<&str>) -> String {
    let name = raw.unwrap_or("Android device").trim();
    let name = if name.is_empty() {
        "Android device"
    } else {
        name
    };
    let sanitized = name
        .chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_DEVICE_NAME_CHARS)
        .collect::<String>();
    if sanitized.trim().is_empty() {
        "Android device".to_owned()
    } else {
        sanitized
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn load_registry(path: &Path) -> Result<DeviceRegistry, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DeviceRegistry::default());
        }
        Err(error) => {
            return Err(format!(
                "cannot read satellite device registry {}: {error}",
                path.display()
            ));
        }
    };

    serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "cannot parse satellite device registry {}: {error}",
            path.display()
        )
    })
}

fn save_registry(path: &Path, registry: &DeviceRegistry) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "satellite device registry path has no parent: {}",
            path.display()
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;

    let bytes = serde_json::to_vec_pretty(registry)
        .map_err(|error| format!("cannot serialize satellite device registry: {error}"))?;
    let temp = path.with_extension(format!("json.tmp-{}", std::process::id()));
    fs::write(&temp, bytes).map_err(|error| format!("cannot write {}: {error}", temp.display()))?;
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("cannot replace {}: {error}", path.display()))?;
    }
    fs::rename(&temp, path)
        .map_err(|error| format!("cannot promote {}: {error}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_id_validation_accepts_uuid_shape() {
        assert!(normalize_device_id("2f765145-e6fd-4bb6-ae19-e1da2df383b9").is_ok());
    }

    #[test]
    fn device_id_validation_rejects_path_like_values() {
        assert!(normalize_device_id("../../device").is_err());
    }

    #[test]
    fn device_name_is_bounded_and_sanitized() {
        let long = format!("phone\n{}", "a".repeat(MAX_DEVICE_NAME_CHARS + 20));
        let normalized = normalize_device_name(Some(&long));
        assert!(normalized.chars().count() <= MAX_DEVICE_NAME_CHARS);
        assert!(!normalized.contains('\n'));
    }
}
