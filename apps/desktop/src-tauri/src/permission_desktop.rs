use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use permission_broker::{
    bind_local, BrokerError, BrokerHandle, PermissionRequest, UserDecision,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{
    io::AsyncWriteExt,
    sync::Mutex,
};
use tracing::warn;
use uuid::Uuid;

const CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RECENT_AUDIT: usize = 100;

#[derive(Debug)]
struct PendingPermission {
    request_id: Uuid,
    tool_name: String,
    risk: String,
    started_at: Instant,
    timestamp_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PermissionAuditEntry {
    pub request_id: String,
    pub tool_name: String,
    pub risk: String,
    pub decision: String,
    pub timestamp_unix_ms: u64,
    pub duration_ms: u64,
}

#[derive(Clone)]
pub struct PermissionDesktopService {
    broker: BrokerHandle,
    pending: Arc<Mutex<HashMap<Uuid, PendingPermission>>>,
    recent_audit: Arc<Mutex<VecDeque<PermissionAuditEntry>>>,
    audit_path: PathBuf,
}

impl PermissionDesktopService {
    pub fn setup(
        app: &AppHandle,
        audit_path: PathBuf,
    ) -> Result<(Self, [(String, String); 2]), BrokerError> {
        let (broker, mut requests) = tauri::async_runtime::block_on(bind_local(CONFIRMATION_TIMEOUT))?;
        let environment = broker.endpoint().environment();
        let service = Self {
            broker,
            pending: Arc::new(Mutex::new(HashMap::new())),
            recent_audit: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_RECENT_AUDIT))),
            audit_path,
        };

        let app_handle = app.clone();
        let event_service = service.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(request) = requests.recv().await {
                event_service.handle_request(&app_handle, request).await;
            }
        });

        Ok((service, environment))
    }

    async fn handle_request(&self, app: &AppHandle, request: PermissionRequest) {
        // A confirmation request must be visible even when the app is hidden in
        // the tray. show_main_window also preserves the external source HWND.
        super::show_main_window(app);

        let request_id = request.request_id;
        let state = app.state::<super::DesktopState>();
        if let Err(error) = state.core.begin_confirming().await {
            warn!(%error, %request_id, "cannot enter confirming state; denying tool request");
            let _ = self.broker.respond(request_id, UserDecision::Deny).await;
            self.record_untracked(&request, "state_rejected_deny").await;
            return;
        }

        self.pending.lock().await.insert(
            request_id,
            PendingPermission {
                request_id,
                tool_name: request.tool_name.clone(),
                risk: format!("{:?}", request.risk).to_lowercase(),
                started_at: Instant::now(),
                timestamp_unix_ms: unix_ms_now(),
            },
        );

        if let Err(error) = emit_request(app, &request) {
            warn!(%error, %request_id, "failed to emit permission request to desktop UI");
            let _ = self.broker.respond(request_id, UserDecision::Deny).await;
            self.finish_request(app, request_id, "ui_unavailable_deny").await;
            return;
        }

        // Broker timeout is fail-closed. Mirror the timeout in the desktop
        // lifecycle so Confirming cannot linger if the UI disappears.
        let timeout_service = self.clone();
        let timeout_app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(CONFIRMATION_TIMEOUT).await;
            timeout_service
                .finish_request(&timeout_app, request_id, "timeout_deny")
                .await;
        });
    }

    async fn respond(
        &self,
        app: &AppHandle,
        request_id: Uuid,
        decision: UserDecision,
    ) -> Result<(), String> {
        let broker_result = self
            .broker
            .respond(request_id, decision)
            .await
            .map_err(|error| error.to_string());

        let audit_decision = match (decision, broker_result.is_ok()) {
            (UserDecision::AllowOnce, true) => "allow_once",
            (UserDecision::Deny, true) => "deny",
            (_, false) => "stale_or_failed_deny",
        };
        self.finish_request(app, request_id, audit_decision).await;
        broker_result
    }

    async fn finish_request(&self, app: &AppHandle, request_id: Uuid, decision: &str) {
        let (entry, pending_empty) = {
            let mut pending = self.pending.lock().await;
            let Some(item) = pending.remove(&request_id) else {
                return;
            };
            let duration_ms = item.started_at.elapsed().as_millis().min(u64::MAX as u128) as u64;
            (
                PermissionAuditEntry {
                    request_id: item.request_id.to_string(),
                    tool_name: item.tool_name,
                    risk: item.risk,
                    decision: decision.to_owned(),
                    timestamp_unix_ms: item.timestamp_unix_ms,
                    duration_ms,
                },
                pending.is_empty(),
            )
        };

        self.append_audit(entry).await;

        // If multiple tool confirmations are ever outstanding, stay Confirming
        // until the final pending request has resolved.
        if pending_empty {
            let state = app.state::<super::DesktopState>();
            if let Err(error) = state.core.finish_confirming().await {
                warn!(%error, "failed to leave confirming state after permission resolution");
            }
        }
    }

    async fn record_untracked(&self, request: &PermissionRequest, decision: &str) {
        self.append_audit(PermissionAuditEntry {
            request_id: request.request_id.to_string(),
            tool_name: request.tool_name.clone(),
            risk: format!("{:?}", request.risk).to_lowercase(),
            decision: decision.to_owned(),
            timestamp_unix_ms: unix_ms_now(),
            duration_ms: 0,
        })
        .await;
    }

    async fn append_audit(&self, entry: PermissionAuditEntry) {
        {
            let mut recent = self.recent_audit.lock().await;
            if recent.len() == MAX_RECENT_AUDIT {
                recent.pop_front();
            }
            recent.push_back(entry.clone());
        }

        if let Some(parent) = self.audit_path.parent() {
            if let Err(error) = tokio::fs::create_dir_all(parent).await {
                warn!(%error, "failed to create permission audit directory");
                return;
            }
        }

        let mut file = match tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.audit_path)
            .await
        {
            Ok(file) => file,
            Err(error) => {
                warn!(%error, "failed to open permission audit log");
                return;
            }
        };

        let mut line = match serde_json::to_vec(&entry) {
            Ok(line) => line,
            Err(error) => {
                warn!(%error, "failed to serialize permission audit entry");
                return;
            }
        };
        line.push(b'\n');
        if let Err(error) = file.write_all(&line).await {
            warn!(%error, "failed to append permission audit entry");
        }
    }

    async fn recent(&self, limit: usize) -> Vec<PermissionAuditEntry> {
        let recent = self.recent_audit.lock().await;
        let limit = limit.clamp(1, MAX_RECENT_AUDIT);
        recent.iter().rev().take(limit).cloned().collect()
    }
}

fn unix_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

fn emit_request(app: &AppHandle, request: &PermissionRequest) -> Result<(), String> {
    app.emit("permission:request", request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn assistant_permission_respond(
    app: AppHandle,
    request_id: String,
    allow: bool,
    permission: State<'_, PermissionDesktopService>,
) -> Result<(), String> {
    let request_id = Uuid::parse_str(&request_id)
        .map_err(|_| "permission request id is invalid".to_owned())?;
    let decision = if allow {
        UserDecision::AllowOnce
    } else {
        UserDecision::Deny
    };
    permission.respond(&app, request_id, decision).await
}

#[tauri::command]
pub async fn assistant_permission_audit(
    limit: Option<usize>,
    permission: State<'_, PermissionDesktopService>,
) -> Vec<PermissionAuditEntry> {
    permission.recent(limit.unwrap_or(20)).await
}
