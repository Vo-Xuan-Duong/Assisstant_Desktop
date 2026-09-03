use std::{
    collections::HashMap,
    env,
    fs,
    path::{Path, PathBuf},
};

use antigravity_bridge::CliHealth;
use serde::{Deserialize, Serialize};

use super::{
    permission_desktop::PermissionDesktopService,
    wake_desktop::WakeService,
    DesktopState,
};

const DEFAULT_MCP_CONFIG: &str = ".agents/mcp_config.json";
const WINDOWS_MCP_SERVER_NAME: &str = "assistant-windows";

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
}

#[derive(Debug, Deserialize)]
struct McpConfigFile {
    #[serde(rename = "mcpServers")]
    mcp_servers: HashMap<String, McpServerConfig>,
}

#[derive(Debug, Deserialize)]
struct McpServerConfig {
    command: String,
    cwd: Option<String>,
}

pub async fn collect(
    state: &DesktopState,
    permission: &PermissionDesktopService,
    wake: &WakeService,
) -> RuntimeReadinessReport {
    let mut checks = Vec::with_capacity(7);

    checks.push(antigravity_check(state).await);
    checks.push(mcp_check());
    checks.push(permission_check(permission).await);
    checks.push(context_storage_check(state));
    checks.push(ReadinessCheck {
        id: "tts",
        label: "Windows TTS",
        level: ReadinessLevel::Ready,
        detail: "Windows SAPI backend được compile vào desktop runtime.".into(),
        path: None,
    });
    checks.push(whisper_check(state));
    checks.push(wake_check(wake));

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

    RuntimeReadinessReport { overall, checks }
}

async fn antigravity_check(state: &DesktopState) -> ReadinessCheck {
    match state.client.health().await {
        CliHealth::Available { detail } => ReadinessCheck {
            id: "antigravity",
            label: "Antigravity CLI",
            level: ReadinessLevel::Ready,
            detail: detail.unwrap_or_else(|| "Antigravity CLI khả dụng.".into()),
            path: None,
        },
        CliHealth::Missing => ReadinessCheck {
            id: "antigravity",
            label: "Antigravity CLI",
            level: ReadinessLevel::Blocking,
            detail: "Không tìm thấy `agy` trong PATH; AI backend chưa thể khởi động.".into(),
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

fn mcp_check() -> ReadinessCheck {
    let current_dir = match env::current_dir() {
        Ok(path) => path,
        Err(error) => {
            return ReadinessCheck {
                id: "windows_mcp",
                label: "Windows MCP",
                level: ReadinessLevel::Blocking,
                detail: format!("Không xác định được working directory: {error}"),
                path: None,
            };
        }
    };

    let config_path = env::var_os("ASSISTANT_MCP_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_MCP_CONFIG));
    let config_path = absolutize(&current_dir, config_path);

    let bytes = match fs::read(&config_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return ReadinessCheck {
                id: "windows_mcp",
                label: "Windows MCP",
                level: ReadinessLevel::Blocking,
                detail: format!(
                    "Không đọc được MCP config. Có thể override bằng ASSISTANT_MCP_CONFIG: {error}"
                ),
                path: Some(config_path.display().to_string()),
            };
        }
    };

    let config = match serde_json::from_slice::<McpConfigFile>(&bytes) {
        Ok(config) => config,
        Err(error) => {
            return ReadinessCheck {
                id: "windows_mcp",
                label: "Windows MCP",
                level: ReadinessLevel::Blocking,
                detail: format!("MCP config không hợp lệ: {error}"),
                path: Some(config_path.display().to_string()),
            };
        }
    };

    let Some(server) = config.mcp_servers.get(WINDOWS_MCP_SERVER_NAME) else {
        return ReadinessCheck {
            id: "windows_mcp",
            label: "Windows MCP",
            level: ReadinessLevel::Blocking,
            detail: format!(
                "MCP config không có server `{WINDOWS_MCP_SERVER_NAME}`."
            ),
            path: Some(config_path.display().to_string()),
        };
    };

    let command_path = resolve_mcp_command(&current_dir, server);
    if !command_path.is_file() {
        return ReadinessCheck {
            id: "windows_mcp",
            label: "Windows MCP",
            level: ReadinessLevel::Blocking,
            detail: "MCP config đã có nhưng chưa tìm thấy `assistant-mcp.exe`. Build release binary trước khi chạy full computer-use.".into(),
            path: Some(command_path.display().to_string()),
        };
    }

    ReadinessCheck {
        id: "windows_mcp",
        label: "Windows MCP",
        level: ReadinessLevel::Ready,
        detail: format!(
            "Đã tìm thấy `{WINDOWS_MCP_SERVER_NAME}` và executable được cấu hình. Config: {}",
            config_path.display()
        ),
        path: Some(command_path.display().to_string()),
    }
}

async fn permission_check(permission: &PermissionDesktopService) -> ReadinessCheck {
    let status = permission.readiness_status().await;
    if !status.broker_bound {
        return ReadinessCheck {
            id: "permission_broker",
            label: "Permission Broker",
            level: ReadinessLevel::Blocking,
            detail: "Permission broker chưa bind; Sensitive tools phải bị coi là không khả dụng.".into(),
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
    let current_dir = match env::current_dir() {
        Ok(path) => path,
        Err(error) => {
            return ReadinessCheck {
                id: "context_storage",
                label: "Context Storage",
                level: ReadinessLevel::Blocking,
                detail: format!("Không xác định được working directory: {error}"),
                path: None,
            };
        }
    };
    let artifact_dir = absolutize(&current_dir, state.context.artifact_dir().to_path_buf());

    if let Err(error) = fs::create_dir_all(&artifact_dir) {
        return ReadinessCheck {
            id: "context_storage",
            label: "Context Storage",
            level: ReadinessLevel::Blocking,
            detail: format!("Không thể tạo context artifact directory: {error}"),
            path: Some(artifact_dir.display().to_string()),
        };
    }

    let probe = artifact_dir.join(".readiness-probe");
    if let Err(error) = fs::write(&probe, b"readiness") {
        return ReadinessCheck {
            id: "context_storage",
            label: "Context Storage",
            level: ReadinessLevel::Blocking,
            detail: format!("Context artifact directory không writable: {error}"),
            path: Some(artifact_dir.display().to_string()),
        };
    }
    let _ = fs::remove_file(&probe);

    ReadinessCheck {
        id: "context_storage",
        label: "Context Storage",
        level: ReadinessLevel::Ready,
        detail: "Context artifact directory tồn tại và writable. Hiện path vẫn phụ thuộc working directory; migration sang app-local-data nên được xử lý ở phase hardening riêng.".into(),
        path: Some(artifact_dir.display().to_string()),
    }
}

fn whisper_check(state: &DesktopState) -> ReadinessCheck {
    #[cfg(feature = "voice-whisper")]
    {
        if state.voice.model_path.is_file() {
            return ReadinessCheck {
                id: "whisper",
                label: "Local Whisper STT",
                level: ReadinessLevel::Ready,
                detail: "Feature `voice-whisper` đã bật và model file tồn tại.".into(),
                path: Some(state.voice.model_path.display().to_string()),
            };
        }

        return ReadinessCheck {
            id: "whisper",
            label: "Local Whisper STT",
            level: ReadinessLevel::OptionalMissing,
            detail: "Feature `voice-whisper` đã bật nhưng chưa có model. Text assistant và TTS vẫn dùng được.".into(),
            path: Some(state.voice.model_path.display().to_string()),
        };
    }

    #[cfg(not(feature = "voice-whisper"))]
    {
        let _ = state;
        ReadinessCheck {
            id: "whisper",
            label: "Local Whisper STT",
            level: ReadinessLevel::OptionalMissing,
            detail: "Build chưa bật feature `voice-whisper`; text assistant vẫn dùng được.".into(),
            path: None,
        }
    }
}

fn wake_check(wake: &WakeService) -> ReadinessCheck {
    let status = wake.status();
    if !status.compiled {
        return ReadinessCheck {
            id: "wake_word",
            label: "Wake Word",
            level: ReadinessLevel::OptionalMissing,
            detail: status
                .detail
                .unwrap_or_else(|| "Build chưa bật feature `wake-word`.".into()),
            path: None,
        };
    }

    if !status.available {
        return ReadinessCheck {
            id: "wake_word",
            label: "Wake Word",
            level: ReadinessLevel::OptionalMissing,
            detail: status
                .detail
                .unwrap_or_else(|| "Wake model/keywords chưa sẵn sàng.".into()),
            path: status.model_dir,
        };
    }

    let enabled_detail = if status.enabled {
        format!("Wake runtime khả dụng; state={}", status.state)
    } else {
        "Wake runtime/resource sẵn sàng nhưng đang tắt theo cấu hình người dùng.".into()
    };

    ReadinessCheck {
        id: "wake_word",
        label: "Wake Word",
        level: ReadinessLevel::Ready,
        detail: enabled_detail,
        path: status.model_dir,
    }
}

fn resolve_mcp_command(current_dir: &Path, server: &McpServerConfig) -> PathBuf {
    let command = PathBuf::from(&server.command);
    if command.is_absolute() {
        return command;
    }

    let cwd = server
        .cwd
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let cwd = absolutize(current_dir, cwd);
    cwd.join(command)
}

fn absolutize(base: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}
