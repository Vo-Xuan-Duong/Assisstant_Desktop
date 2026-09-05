use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use antigravity_bridge::CliHealth;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
    time::timeout,
};
use tracing::{debug, warn};
use uuid::Uuid;

use super::{
    DesktopState, hide_quick_window, quick_panel,
    resource_registry::RuntimeResourceSnapshot, runtime_paths::RuntimePaths, show_quick_window,
    wake_desktop::WakeService,
};

pub const MANAGEMENT_PROTOCOL_VERSION: u32 = 1;
pub const MANAGEMENT_ENDPOINT_FILE: &str = "management.json";
const MAX_REQUEST_BYTES: usize = 64 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(3);
const WRITE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagementEndpoint {
    pub version: u32,
    pub host: String,
    pub port: u16,
    pub secret: String,
    pub pid: u32,
}

#[derive(Debug, Deserialize)]
struct ManagementRequest {
    version: u32,
    secret: String,
    command: String,
    #[serde(default)]
    payload: Value,
}

#[derive(Debug, Serialize)]
struct ManagementResponse {
    version: u32,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl ManagementResponse {
    fn success(result: Value) -> Self {
        Self {
            version: MANAGEMENT_PROTOCOL_VERSION,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    fn failure(error: impl Into<String>) -> Self {
        Self {
            version: MANAGEMENT_PROTOCOL_VERSION,
            ok: false,
            result: None,
            error: Some(error.into()),
        }
    }
}

pub struct ManagementIpc {
    endpoint_path: PathBuf,
    secret: String,
    task: JoinHandle<()>,
}

impl ManagementIpc {
    pub fn setup(app: &AppHandle, paths: &RuntimePaths) -> Result<Self, String> {
        let endpoint_path = paths.runtime_dir.join(MANAGEMENT_ENDPOINT_FILE);
        let secret = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let listener = tauri::async_runtime::block_on(TcpListener::bind(("127.0.0.1", 0)))
            .map_err(|error| format!("cannot bind management IPC to loopback: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("cannot read management IPC address: {error}"))?;
        if !address.ip().is_loopback() {
            return Err("management IPC refused a non-loopback listener".into());
        }

        let endpoint = ManagementEndpoint {
            version: MANAGEMENT_PROTOCOL_VERSION,
            host: "127.0.0.1".into(),
            port: address.port(),
            secret: secret.clone(),
            pid: std::process::id(),
        };
        write_endpoint_atomic(&endpoint_path, &endpoint)?;

        let task_app = app.clone();
        let task_secret = secret.clone();
        let task = tauri::async_runtime::spawn(async move {
            run_server(task_app, listener, task_secret).await;
        });

        debug!(port = endpoint.port, "management IPC listening on loopback");
        Ok(Self {
            endpoint_path,
            secret,
            task,
        })
    }
}

impl Drop for ManagementIpc {
    fn drop(&mut self) {
        self.task.abort();
        remove_endpoint_if_owned(&self.endpoint_path, &self.secret);
    }
}

async fn run_server(app: AppHandle, listener: TcpListener, secret: String) {
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(value) => value,
            Err(error) => {
                warn!(%error, "management IPC accept failed");
                break;
            }
        };
        if !peer.ip().is_loopback() {
            warn!(%peer, "management IPC rejected non-loopback peer");
            continue;
        }

        // Requests are deliberately handled one at a time. Management commands
        // are low-volume, and serial execution avoids an accidental flood of
        // concurrent runtime mutations from local scripts.
        if let Err(error) = handle_connection(&app, stream, &secret).await {
            warn!(%error, "management IPC request failed");
        }
    }
}

async fn handle_connection(
    app: &AppHandle,
    mut stream: TcpStream,
    expected_secret: &str,
) -> Result<(), String> {
    let bytes = read_bounded_line(&mut stream).await?;
    let request: ManagementRequest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid management request JSON: {error}"))?;

    let response = if request.version != MANAGEMENT_PROTOCOL_VERSION {
        ManagementResponse::failure(format!(
            "unsupported management protocol version {}; expected {}",
            request.version, MANAGEMENT_PROTOCOL_VERSION
        ))
    } else if !secret_matches(expected_secret, &request.secret) {
        ManagementResponse::failure("management authentication failed")
    } else {
        match dispatch(app, request.command.trim(), request.payload).await {
            Ok(result) => ManagementResponse::success(result),
            Err(error) => ManagementResponse::failure(error),
        }
    };

    let mut encoded = serde_json::to_vec(&response)
        .map_err(|error| format!("cannot serialize management response: {error}"))?;
    encoded.push(b'\n');
    timeout(WRITE_TIMEOUT, stream.write_all(&encoded))
        .await
        .map_err(|_| "management response timed out".to_owned())?
        .map_err(|error| format!("cannot write management response: {error}"))?;
    let _ = stream.shutdown().await;
    Ok(())
}

async fn read_bounded_line(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut request = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];

    loop {
        let read = timeout(READ_TIMEOUT, stream.read(&mut chunk))
            .await
            .map_err(|_| "management request timed out".to_owned())?
            .map_err(|error| format!("cannot read management request: {error}"))?;
        if read == 0 {
            break;
        }

        let slice = &chunk[..read];
        if let Some(newline) = slice.iter().position(|byte| *byte == b'\n') {
            request.extend_from_slice(&slice[..newline]);
            break;
        }
        request.extend_from_slice(slice);
        if request.len() > MAX_REQUEST_BYTES {
            return Err(format!(
                "management request exceeds {MAX_REQUEST_BYTES} bytes"
            ));
        }
    }

    if request.is_empty() {
        return Err("management request is empty".into());
    }
    if request.len() > MAX_REQUEST_BYTES {
        return Err(format!(
            "management request exceeds {MAX_REQUEST_BYTES} bytes"
        ));
    }
    Ok(request)
}

async fn dispatch(app: &AppHandle, command: &str, payload: Value) -> Result<Value, String> {
    match command {
        "runtime.ping" => Ok(json!({
            "pid": std::process::id(),
            "protocol": MANAGEMENT_PROTOCOL_VERSION,
        })),
        "runtime.status" => runtime_status(app).await,
        "runtime.restart_agent" => {
            let state = app.state::<DesktopState>();
            state
                .client
                .restart()
                .await
                .map_err(|error| format!("cannot restart Antigravity runtime: {error}"))?;
            Ok(json!({ "restarted": true }))
        }
        "overlay.show" => {
            show_quick_window(app, "cli");
            Ok(json!({ "visible": true }))
        }
        "overlay.hide" => {
            hide_quick_window(app);
            Ok(json!({ "visible": false }))
        }
        "ai.get" => ai_get(app).await,
        "ai.set" => ai_set(app, payload).await,
        "wake.get" => {
            let wake = app.state::<WakeService>();
            serde_json::to_value(wake.status())
                .map_err(|error| format!("cannot serialize wake status: {error}"))
        }
        "wake.set_enabled" => wake_set_enabled(app, payload).await,
        "resources.list" => {
            let state = app.state::<DesktopState>();
            serialize_resources(state.resources.snapshot())
        }
        _ => Err(format!("unknown management command `{command}`")),
    }
}

async fn runtime_status(app: &AppHandle) -> Result<Value, String> {
    let state = app.state::<DesktopState>();
    let health = match state.client.health().await {
        CliHealth::Available { detail } => json!({
            "state": "available",
            "detail": detail,
        }),
        CliHealth::Missing => json!({
            "state": "missing",
            "detail": "Antigravity CLI was not found",
        }),
        CliHealth::Unhealthy { message } => json!({
            "state": "unhealthy",
            "detail": message,
        }),
    };
    let config = state.client.get_config_snapshot().await;
    let conversation_id = state.client.conversation_id().await;
    let wake = app.state::<WakeService>().status();
    let resources = serde_json::to_value(state.resources.snapshot())
        .map_err(|error| format!("cannot serialize resources: {error}"))?;

    Ok(json!({
        "pid": std::process::id(),
        "protocol": MANAGEMENT_PROTOCOL_VERSION,
        "antigravity": health,
        "conversation_id": conversation_id,
        "model": config.model,
        "effort": config.effort,
        "quick_visible": quick_panel::is_visible(app),
        "wake": wake,
        "resources": resources,
    }))
}

async fn ai_get(app: &AppHandle) -> Result<Value, String> {
    let state = app.state::<DesktopState>();
    let persisted = state.antigravity_store.load().unwrap_or_default();
    let runtime = state.client.get_config_snapshot().await;
    Ok(json!({
        "model": persisted.model.or(runtime.model),
        "effort": persisted.effort.or(runtime.effort),
        "binary": runtime.binary,
    }))
}

#[derive(Debug, Deserialize)]
struct AiSetPayload {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    effort: Option<String>,
}

async fn ai_set(app: &AppHandle, payload: Value) -> Result<Value, String> {
    let payload: AiSetPayload = serde_json::from_value(payload)
        .map_err(|error| format!("invalid ai.set payload: {error}"))?;
    let model = normalize_optional(payload.model);
    let effort = normalize_optional(payload.effort);
    let settings = super::antigravity_settings::AntigravitySettings {
        model: model.clone(),
        effort: effort.clone(),
    };

    let state = app.state::<DesktopState>();
    state.antigravity_store.save(&settings)?;
    state.client.update_model_config(model, effort).await;
    ai_get(app).await
}

#[derive(Debug, Deserialize)]
struct WakeEnabledPayload {
    enabled: bool,
}

async fn wake_set_enabled(app: &AppHandle, payload: Value) -> Result<Value, String> {
    let payload: WakeEnabledPayload = serde_json::from_value(payload)
        .map_err(|error| format!("invalid wake.set_enabled payload: {error}"))?;
    let wake = app.state::<WakeService>();
    wake.set_enabled(payload.enabled).await?;
    serde_json::to_value(wake.status())
        .map_err(|error| format!("cannot serialize wake status: {error}"))
}

fn serialize_resources(snapshot: RuntimeResourceSnapshot) -> Result<Value, String> {
    serde_json::to_value(snapshot)
        .map_err(|error| format!("cannot serialize resource snapshot: {error}"))
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_owned();
        if value.is_empty() || value.eq_ignore_ascii_case("default") {
            None
        } else {
            Some(value)
        }
    })
}

fn secret_matches(expected: &str, supplied: &str) -> bool {
    if expected.len() != supplied.len() {
        return false;
    }
    expected
        .as_bytes()
        .iter()
        .zip(supplied.as_bytes())
        .fold(0u8, |difference, (left, right)| difference | (left ^ right))
        == 0
}

fn write_endpoint_atomic(path: &Path, endpoint: &ManagementEndpoint) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "management endpoint path has no parent directory".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create management endpoint directory: {error}"))?;
    let bytes = serde_json::to_vec_pretty(endpoint)
        .map_err(|error| format!("cannot serialize management endpoint: {error}"))?;
    let temporary = parent.join(format!(".management.{}.part", Uuid::new_v4()));
    let backup = parent.join(format!(".management.{}.bak", Uuid::new_v4()));
    fs::write(&temporary, bytes)
        .map_err(|error| format!("cannot write temporary management endpoint: {error}"))?;

    let had_existing = path.exists();
    if had_existing {
        if let Err(error) = fs::rename(path, &backup) {
            let _ = fs::remove_file(&temporary);
            return Err(format!("cannot stage previous management endpoint: {error}"));
        }
    }
    if let Err(error) = fs::rename(&temporary, path) {
        if had_existing {
            let _ = fs::rename(&backup, path);
        }
        let _ = fs::remove_file(&temporary);
        return Err(format!("cannot install management endpoint: {error}"));
    }
    if had_existing {
        let _ = fs::remove_file(&backup);
    }
    Ok(())
}

fn remove_endpoint_if_owned(path: &Path, secret: &str) {
    let owned = fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ManagementEndpoint>(&bytes).ok())
        .is_some_and(|endpoint| secret_matches(secret, &endpoint.secret));
    if owned {
        let _ = fs::remove_file(path);
    }
}
