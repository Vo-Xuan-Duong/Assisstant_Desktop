use std::{fs, io::Write, path::Path};

use antigravity_bridge::CliHealth;
use serde::{Deserialize, Serialize};

use super::{
    DesktopState,
    permission_desktop::PermissionDesktopService,
    resource_registry::{ResourceState, RuntimeResourceStatus},
    runtime_paths::McpBinarySource,
    wake_desktop::WakeService,
};

const SATELLITE_SETTINGS_FILE: &str = "satellite.json";
const SATELLITE_DEVICES_FILE: &str = "satellite-devices.json";
const SATELLITE_REVOKED_DIR: &str = "satellite-revoked";
const SATELLITE_REMOTE_STATE_FILE: &str = "satellite-tailscale.json";
const SATELLITE_DEFAULT_BIND: &str = "0.0.0.0:8765";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessLevel {
    Ready,
    OptionalMissing,
    Blocking,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadinessCheck {
    pub id: &'static str,
    pub label: &'static str,
    pub level: ReadinessLevel,
    pub detail: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeReadinessReport {
    pub overall: ReadinessLevel,
    pub checks: Vec<ReadinessCheck>,
    pub satellite: SatelliteReadinessSnapshot,
}

#[derive(Debug, Clone, Serialize)]
pub struct SatelliteReadinessSnapshot {
    pub enabled: bool,
    pub paired: bool,
    pub bind: String,
    pub credential_storage: String,
    pub environment_token_override: bool,
    pub remote_managed: bool,
    pub remote_port: Option<u16>,
    pub trusted_devices: usize,
    pub revoked_devices: usize,
    pub devices: Vec<SatelliteDeviceSnapshot>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SatelliteDeviceSnapshot {
    pub id: String,
    pub name: String,
    pub first_seen_unix: u64,
    pub last_seen_unix: u64,
    pub revoked: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct SatelliteSettingsFile {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_satellite_bind")]
    bind: String,
    #[serde(default)]
    token: Option<String>,
}

impl Default for SatelliteSettingsFile {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: default_satellite_bind(),
            token: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct SatelliteDeviceRegistryFile {
    #[serde(default)]
    devices: Vec<SatelliteDeviceFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct SatelliteDeviceFile {
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

#[derive(Debug, Clone, Deserialize)]
struct SatelliteRemoteStateFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    previous_bind: String,
    #[serde(default)]
    previous_enabled: bool,
    port: u16,
}

pub async fn collect(
    state: &DesktopState,
    permission: &PermissionDesktopService,
    wake: &WakeService,
) -> RuntimeReadinessReport {
    let satellite = satellite_snapshot(state);
    let checks = vec![
        antigravity_check(state).await,
        mcp_check(state),
        permission_check(permission).await,
        context_storage_check(state),
        ReadinessCheck {
            id: "tts",
            label: "Windows TTS",
            level: ReadinessLevel::Ready,
            detail: "Windows SAPI backend được compile vào desktop runtime.".into(),
            path: None,
        },
        satellite_check(state, &satellite),
        resource_check(state.resources.whisper_status()),
        wake_check(state.resources.wake_status(), wake),
    ];

    let overall = if checks
        .iter()
        .any(|check| check.level == ReadinessLevel::Blocking)
    {
        ReadinessLevel::Blocking
    } else if checks
        .iter()
        .any(|check| check.level == ReadinessLevel::OptionalMissing)
    {
        ReadinessLevel::OptionalMissing
    } else {
        ReadinessLevel::Ready
    };

    RuntimeReadinessReport {
        overall,
        checks,
        satellite,
    }
}

async fn antigravity_check(state: &DesktopState) -> ReadinessCheck {
    match state.client.health().await {
        CliHealth::Available { detail } => ReadinessCheck {
            id: "antigravity",
            label: "Antigravity CLI",
            level: ReadinessLevel::Ready,
            detail: detail.unwrap_or_else(|| {
                format!(
                    "Antigravity CLI khả dụng; runtime cwd={}",
                    state.runtime_paths.runtime_dir.display()
                )
            }),
            path: Some(state.runtime_paths.runtime_dir.display().to_string()),
        },
        CliHealth::Missing => ReadinessCheck {
            id: "antigravity",
            label: "Antigravity CLI",
            level: ReadinessLevel::Blocking,
            detail: "Không tìm thấy `agy` trong PATH hoặc thư mục cài đặt chuẩn; AI backend chưa thể khởi động.".into(),
            path: None,
        },
        CliHealth::Unhealthy { message } => ReadinessCheck {
            id: "antigravity",
            label: "Antigravity CLI",
            level: ReadinessLevel::Blocking,
            detail: message,
            path: None,
        },
    }
}

fn mcp_check(state: &DesktopState) -> ReadinessCheck {
    let paths = &state.runtime_paths;

    if !paths.mcp_config_path.is_file() {
        return ReadinessCheck {
            id: "windows_mcp",
            label: "Windows MCP",
            level: ReadinessLevel::Blocking,
            detail: "Runtime MCP config chưa tồn tại trong app-local-data.".into(),
            path: Some(paths.mcp_config_path.display().to_string()),
        };
    }

    if let Err(error) = fs::read(&paths.mcp_config_path).and_then(|bytes| {
        serde_json::from_slice::<serde_json::Value>(&bytes).map_err(std::io::Error::other)
    }) {
        return ReadinessCheck {
            id: "windows_mcp",
            label: "Windows MCP",
            level: ReadinessLevel::Blocking,
            detail: format!("Generated MCP config không đọc/parse được: {error}"),
            path: Some(paths.mcp_config_path.display().to_string()),
        };
    }

    if !paths.mcp_binary_path.is_file() {
        return ReadinessCheck {
            id: "windows_mcp",
            label: "Windows MCP",
            level: ReadinessLevel::Blocking,
            detail: format!(
                "Không tìm thấy assistant-mcp sidecar tại runtime path (source={}).",
                source_name(paths.mcp_binary_source)
            ),
            path: Some(paths.mcp_binary_path.display().to_string()),
        };
    }

    ReadinessCheck {
        id: "windows_mcp",
        label: "Windows MCP",
        level: ReadinessLevel::Ready,
        detail: format!(
            "MCP config được sinh trong app-local-data; assistant-mcp source={}",
            source_name(paths.mcp_binary_source)
        ),
        path: Some(paths.mcp_binary_path.display().to_string()),
    }
}

fn source_name(source: McpBinarySource) -> &'static str {
    match source {
        McpBinarySource::Environment => "environment_override",
        McpBinarySource::BundledSidecar => "bundled_sidecar",
        McpBinarySource::DevDebug => "dev_debug",
        McpBinarySource::DevRelease => "dev_release",
        McpBinarySource::ExpectedBundled => "expected_bundled_missing",
    }
}

async fn permission_check(permission: &PermissionDesktopService) -> ReadinessCheck {
    let status = permission.readiness_status().await;
    if !status.broker_bound {
        return ReadinessCheck {
            id: "permission_broker",
            label: "Permission Broker",
            level: ReadinessLevel::Blocking,
            detail: "Permission broker chưa bind; Sensitive tools phải bị coi là không khả dụng."
                .into(),
            path: None,
        };
    }

    if let Some(error) = status.policy_load_error {
        return ReadinessCheck {
            id: "permission_broker",
            label: "Permission Broker",
            level: ReadinessLevel::Blocking,
            detail: format!("Broker đã bind nhưng runtime policy có lỗi: {error}"),
            path: Some(status.policy_path),
        };
    }

    ReadinessCheck {
        id: "permission_broker",
        label: "Permission Broker",
        level: ReadinessLevel::Ready,
        detail: format!(
            "Broker loopback đã bind; {} confirmation request đang chờ. Audit: {}",
            status.pending_requests, status.audit_path
        ),
        path: Some(status.policy_path),
    }
}

fn context_storage_check(state: &DesktopState) -> ReadinessCheck {
    let artifact_dir = state.context.artifact_dir();

    if let Err(error) = fs::create_dir_all(artifact_dir) {
        return ReadinessCheck {
            id: "context_storage",
            label: "Context Storage",
            level: ReadinessLevel::Blocking,
            detail: format!("Không thể tạo context artifact directory: {error}"),
            path: Some(artifact_dir.display().to_string()),
        };
    }

    let probe = artifact_dir.join(format!(".readiness-probe-{}", std::process::id()));
    let mut probe_file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(file) => file,
        Err(error) => {
            return ReadinessCheck {
                id: "context_storage",
                label: "Context Storage",
                level: ReadinessLevel::Blocking,
                detail: format!("Context artifact directory không writable: {error}"),
                path: Some(artifact_dir.display().to_string()),
            };
        }
    };

    if let Err(error) = probe_file.write_all(b"readiness") {
        drop(probe_file);
        let _ = fs::remove_file(&probe);
        return ReadinessCheck {
            id: "context_storage",
            label: "Context Storage",
            level: ReadinessLevel::Blocking,
            detail: format!("Không thể ghi context readiness probe: {error}"),
            path: Some(artifact_dir.display().to_string()),
        };
    }
    drop(probe_file);
    let _ = fs::remove_file(&probe);

    ReadinessCheck {
        id: "context_storage",
        label: "Context Storage",
        level: ReadinessLevel::Ready,
        detail: "Context artifacts dùng app-local-data và directory hiện writable.".into(),
        path: Some(artifact_dir.display().to_string()),
    }
}

fn satellite_snapshot(state: &DesktopState) -> SatelliteReadinessSnapshot {
    let settings_dir = state.runtime_paths.app_local_data.join("settings");
    let settings_path = settings_dir.join(SATELLITE_SETTINGS_FILE);
    let devices_path = settings_dir.join(SATELLITE_DEVICES_FILE);
    let revoked_dir = settings_dir.join(SATELLITE_REVOKED_DIR);
    let remote_path = settings_dir.join(SATELLITE_REMOTE_STATE_FILE);
    let mut warnings = Vec::new();

    let settings = read_json_or_default::<SatelliteSettingsFile>(&settings_path, &mut warnings);
    let registry =
        read_json_or_default::<SatelliteDeviceRegistryFile>(&devices_path, &mut warnings);
    let remote = read_optional_json::<SatelliteRemoteStateFile>(&remote_path, &mut warnings);

    let mut devices = registry
        .devices
        .into_iter()
        .map(|device| {
            let marker_revoked = revoked_dir.join(format!("{}.revoked", device.id)).is_file();
            SatelliteDeviceSnapshot {
                id: device.id,
                name: device.name,
                first_seen_unix: device.first_seen_unix,
                last_seen_unix: device.last_seen_unix,
                revoked: device.revoked || marker_revoked,
            }
        })
        .collect::<Vec<_>>();
    devices.sort_by(|left, right| right.last_seen_unix.cmp(&left.last_seen_unix));

    let revoked_devices = devices.iter().filter(|device| device.revoked).count();
    let trusted_devices = devices.len().saturating_sub(revoked_devices);
    let paired = settings
        .token
        .as_deref()
        .map(str::trim)
        .is_some_and(|token| token.len() >= 16);
    let credential_storage = match settings.token.as_deref().map(str::trim) {
        None | Some("") => "unpaired",
        Some(token) if token.starts_with("dpapi:") => "dpapi-current-user",
        Some(_) => "legacy-plaintext",
    }
    .to_owned();

    let environment_token_override = std::env::var_os("ASSISTANT_VOICE_SATELLITE_TOKEN").is_some();
    if environment_token_override {
        warnings.push(
            "ASSISTANT_VOICE_SATELLITE_TOKEN đang override token persisted; UI không hiển thị giá trị secret."
                .to_owned(),
        );
    }
    if credential_storage == "legacy-plaintext" {
        warnings.push(
            "Pairing token persisted vẫn ở dạng plaintext legacy; chạy một lệnh satellite mutation để migrate sang DPAPI."
                .to_owned(),
        );
    }

    let remote_managed = remote.is_some();
    let remote_port = remote.as_ref().map(|state| state.port);
    if let Some(remote) = &remote {
        let expected_loopback = format!("127.0.0.1:{}", remote.port);
        if settings.bind != expected_loopback {
            warnings.push(format!(
                "Tailscale managed state đang tồn tại nhưng backend bind là `{}` thay vì `{expected_loopback}`.",
                settings.bind
            ));
        }
        if remote.version != 1 {
            warnings.push(format!(
                "Tailscale remote state version {} chưa được UI này nhận diện đầy đủ.",
                remote.version
            ));
        }
        let _ = (&remote.previous_bind, remote.previous_enabled);
    }

    SatelliteReadinessSnapshot {
        enabled: settings.enabled,
        paired,
        bind: settings.bind,
        credential_storage,
        environment_token_override,
        remote_managed,
        remote_port,
        trusted_devices,
        revoked_devices,
        devices,
        warnings,
    }
}

fn satellite_check(state: &DesktopState, snapshot: &SatelliteReadinessSnapshot) -> ReadinessCheck {
    let settings_path = state
        .runtime_paths
        .app_local_data
        .join("settings")
        .join(SATELLITE_SETTINGS_FILE);

    let level = if snapshot.enabled && snapshot.paired && snapshot.warnings.is_empty() {
        ReadinessLevel::Ready
    } else {
        ReadinessLevel::OptionalMissing
    };

    let mode = if snapshot.remote_managed {
        "tailscale"
    } else {
        "lan/local"
    };
    let detail = if !snapshot.enabled {
        "Android Voice Satellite đang tắt; desktop text/Quick và fallback voice vẫn hoạt động."
            .to_owned()
    } else if !snapshot.paired {
        "Android Voice Satellite đang bật nhưng chưa có pairing token hợp lệ.".to_owned()
    } else if let Some(warning) = snapshot.warnings.first() {
        format!("Satellite hoạt động ở mode={mode}, nhưng cần kiểm tra: {warning}")
    } else {
        format!(
            "Satellite sẵn sàng; mode={mode}, bind={}, trusted={}, revoked={}, credential={}",
            snapshot.bind,
            snapshot.trusted_devices,
            snapshot.revoked_devices,
            snapshot.credential_storage
        )
    };

    ReadinessCheck {
        id: "android_voice_satellite",
        label: "Android Voice Satellite",
        level,
        detail,
        path: Some(settings_path.display().to_string()),
    }
}

fn read_json_or_default<T>(path: &Path, warnings: &mut Vec<String>) -> T
where
    T: for<'de> Deserialize<'de> + Default,
{
    if !path.is_file() {
        return T::default();
    }
    match fs::read(path)
        .map_err(|error| error.to_string())
        .and_then(|bytes| serde_json::from_slice::<T>(&bytes).map_err(|error| error.to_string()))
    {
        Ok(value) => value,
        Err(error) => {
            warnings.push(format!("Không thể đọc {}: {error}", path.display()));
            T::default()
        }
    }
}

fn read_optional_json<T>(path: &Path, warnings: &mut Vec<String>) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    if !path.is_file() {
        return None;
    }
    match fs::read(path)
        .map_err(|error| error.to_string())
        .and_then(|bytes| serde_json::from_slice::<T>(&bytes).map_err(|error| error.to_string()))
    {
        Ok(value) => Some(value),
        Err(error) => {
            warnings.push(format!("Không thể đọc {}: {error}", path.display()));
            None
        }
    }
}

fn default_satellite_bind() -> String {
    SATELLITE_DEFAULT_BIND.to_owned()
}

fn resource_check(resource: RuntimeResourceStatus) -> ReadinessCheck {
    ReadinessCheck {
        id: resource.id,
        label: resource.label,
        level: match resource.state {
            ResourceState::Ready => ReadinessLevel::Ready,
            ResourceState::Missing | ResourceState::Incomplete | ResourceState::NotCompiled => {
                ReadinessLevel::OptionalMissing
            }
        },
        detail: resource.detail,
        path: Some(resource.root_path),
    }
}

fn wake_check(resource: RuntimeResourceStatus, wake: &WakeService) -> ReadinessCheck {
    if resource.state != ResourceState::Ready {
        return resource_check(resource);
    }

    let status = wake.status();
    if !status.available {
        return ReadinessCheck {
            id: "wake_word",
            label: "Wake Word",
            level: ReadinessLevel::OptionalMissing,
            detail: status.detail.unwrap_or_else(|| {
                "Wake resources đầy đủ nhưng detector runtime chưa khởi tạo được.".into()
            }),
            path: status.model_dir.or(Some(resource.root_path)),
        };
    }

    ReadinessCheck {
        id: "wake_word",
        label: "Wake Word",
        level: ReadinessLevel::Ready,
        detail: if status.enabled {
            format!("Wake runtime khả dụng; state={}", status.state)
        } else {
            "Wake runtime/resource sẵn sàng nhưng đang tắt theo cấu hình người dùng.".into()
        },
        path: status.model_dir.or(Some(resource.root_path)),
    }
}
