use std::{fs, io::Write};

use antigravity_bridge::CliHealth;
use serde::Serialize;

use super::{
    permission_desktop::PermissionDesktopService,
    runtime_paths::McpBinarySource,
    wake_desktop::WakeService,
    DesktopState,
};

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

pub async fn collect(
    state: &DesktopState,
    permission: &PermissionDesktopService,
    wake: &WakeService,
) -> RuntimeReadinessReport {
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
        whisper_check(state),
        wake_check(wake),
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

    RuntimeReadinessReport { overall, checks }
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

    if let Err(error) = fs::read(&paths.mcp_config_path)
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).map_err(std::io::Error::other))
    {
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

    ReadinessCheck {
        id: "wake_word",
        label: "Wake Word",
        level: ReadinessLevel::Ready,
        detail: if status.enabled {
            format!("Wake runtime khả dụng; state={}", status.state)
        } else {
            "Wake runtime/resource sẵn sàng nhưng đang tắt theo cấu hình người dùng.".into()
        },
        path: status.model_dir,
    }
}
