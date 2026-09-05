use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use assistant_common::ToolRisk;
use permission_broker::{BrokerError, BrokerHandle, PermissionRequest, UserDecision, bind_local};
use permission_engine::{
    ENV_PERMISSION_POLICY_PATH, PermissionDecision, PermissionOverrideSnapshot,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Listener, Manager, State};
use tokio::{io::AsyncWriteExt, sync::Mutex};
use tracing::warn;
use uuid::Uuid;
use windows_tools::{TOOL_CATALOG, tool_definition};

const CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RECENT_AUDIT: usize = 100;
const POLICY_GET_EVENT: &str = "permission:policy_get";
const POLICY_SET_EVENT: &str = "permission:policy_set";
const POLICY_SNAPSHOT_EVENT: &str = "permission:policy_snapshot";
const POLICY_ERROR_EVENT: &str = "permission:policy_error";

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

#[derive(Debug, Clone, Serialize)]
pub struct PermissionPolicyToolView {
    pub name: String,
    pub description: String,
    pub default_decision: PermissionDecision,
    pub override_decision: Option<PermissionDecision>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PermissionPolicyView {
    pub revision: u64,
    pub load_error: Option<String>,
    pub tools: Vec<PermissionPolicyToolView>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PermissionServiceStatus {
    pub broker_bound: bool,
    pub policy_path: String,
    pub audit_path: String,
    pub policy_load_error: Option<String>,
    pub pending_requests: usize,
}

#[derive(Debug, Deserialize)]
struct PermissionPolicySetEvent {
    tool_name: String,
    decision: Option<PermissionDecision>,
}

#[derive(Clone)]
pub struct PermissionDesktopService {
    broker: BrokerHandle,
    pending: Arc<Mutex<HashMap<Uuid, PendingPermission>>>,
    recent_audit: Arc<Mutex<VecDeque<PermissionAuditEntry>>>,
    audit_path: PathBuf,
    policy_path: PathBuf,
    policy: Arc<Mutex<PermissionOverrideSnapshot>>,
    policy_load_error: Arc<Mutex<Option<String>>>,
}

impl PermissionDesktopService {
    pub fn setup(app: &AppHandle) -> Result<(Self, Vec<(String, String)>), BrokerError> {
        let local_data = app.path().app_local_data_dir().map_err(|error| {
            BrokerError::Rejected(format!("cannot resolve app local data directory: {error}"))
        })?;
        let audit_path = local_data.join("audit").join("permissions.jsonl");
        let policy_path = local_data.join("permissions").join("policy.json");
        let (policy, policy_load_error) = load_policy_snapshot(&policy_path);

        let (broker, mut requests) =
            tauri::async_runtime::block_on(bind_local(CONFIRMATION_TIMEOUT))?;
        let mut environment = broker
            .endpoint()
            .environment()
            .into_iter()
            .collect::<Vec<_>>();
        environment.push((
            ENV_PERMISSION_POLICY_PATH.to_owned(),
            policy_path.to_string_lossy().into_owned(),
        ));

        let service = Self {
            broker,
            pending: Arc::new(Mutex::new(HashMap::new())),
            recent_audit: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_RECENT_AUDIT))),
            audit_path,
            policy_path,
            policy: Arc::new(Mutex::new(policy)),
            policy_load_error: Arc::new(Mutex::new(policy_load_error)),
        };

        let app_handle = app.clone();
        let event_service = service.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(request) = requests.recv().await {
                event_service.handle_request(&app_handle, request).await;
            }
        });

        setup_policy_events(app, &service);
        Ok((service, environment))
    }

    pub(crate) async fn readiness_status(&self) -> PermissionServiceStatus {
        PermissionServiceStatus {
            // Reaching this service means bind_local succeeded during Tauri setup.
            // The broker secret/address remains private and is intentionally not
            // exposed through diagnostics.
            broker_bound: true,
            policy_path: self.policy_path.display().to_string(),
            audit_path: self.audit_path.display().to_string(),
            policy_load_error: self.policy_load_error.lock().await.clone(),
            pending_requests: self.pending.lock().await.len(),
        }
    }

    async fn handle_request(&self, app: &AppHandle, request: PermissionRequest) {
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
            self.finish_request(app, request_id, "ui_unavailable_deny")
                .await;
            return;
        }

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

    async fn policy_view(&self) -> PermissionPolicyView {
        let policy = self.policy.lock().await.clone();
        let load_error = self.policy_load_error.lock().await.clone();
        let tools = TOOL_CATALOG
            .iter()
            .filter(|tool| tool.risk == ToolRisk::Moderate)
            .map(|tool| PermissionPolicyToolView {
                name: tool.name.to_owned(),
                description: tool.description.to_owned(),
                default_decision: PermissionDecision::Allow,
                override_decision: policy.decision_for(tool.name),
            })
            .collect();
        PermissionPolicyView {
            revision: policy.revision,
            load_error,
            tools,
        }
    }

    async fn set_policy_override(
        &self,
        tool_name: &str,
        decision: Option<PermissionDecision>,
    ) -> Result<PermissionPolicyView, String> {
        let definition = tool_definition(tool_name)
            .ok_or_else(|| format!("unknown permission tool `{tool_name}`"))?;
        if definition.risk != ToolRisk::Moderate {
            return Err(format!(
                "runtime policy overrides are restricted to Moderate tools; `{tool_name}` is {:?}",
                definition.risk
            ));
        }

        let mut next = self.policy.lock().await.clone();
        match decision {
            Some(decision) => next.set(tool_name.to_owned(), decision),
            None => next.clear(tool_name),
        }

        persist_policy_snapshot(&self.policy_path, &next).await?;
        *self.policy.lock().await = next;
        *self.policy_load_error.lock().await = None;
        Ok(self.policy_view().await)
    }
}

fn unix_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

fn load_policy_snapshot(path: &PathBuf) -> (PermissionOverrideSnapshot, Option<String>) {
    match std::fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<PermissionOverrideSnapshot>(&bytes) {
            Ok(snapshot) => (snapshot, None),
            Err(error) => (
                PermissionOverrideSnapshot::default(),
                Some(format!("runtime permission policy is malformed: {error}")),
            ),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            (PermissionOverrideSnapshot::default(), None)
        }
        Err(error) => (
            PermissionOverrideSnapshot::default(),
            Some(format!("cannot read runtime permission policy: {error}")),
        ),
    }
}

async fn persist_policy_snapshot(
    path: &PathBuf,
    snapshot: &PermissionOverrideSnapshot,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("cannot create permission policy directory: {error}"))?;
    }
    let bytes = serde_json::to_vec_pretty(snapshot)
        .map_err(|error| format!("cannot serialize permission policy: {error}"))?;
    tokio::fs::write(path, bytes)
        .await
        .map_err(|error| format!("cannot write permission policy: {error}"))
}

fn setup_policy_events(app: &AppHandle, service: &PermissionDesktopService) {
    let get_app = app.clone();
    let get_service = service.clone();
    app.listen(POLICY_GET_EVENT, move |_| {
        let app = get_app.clone();
        let service = get_service.clone();
        tauri::async_runtime::spawn(async move {
            let view = service.policy_view().await;
            let _ = app.emit(POLICY_SNAPSHOT_EVENT, view);
        });
    });

    let set_app = app.clone();
    let set_service = service.clone();
    app.listen(POLICY_SET_EVENT, move |event| {
        let request = serde_json::from_str::<PermissionPolicySetEvent>(event.payload());
        let app = set_app.clone();
        let service = set_service.clone();
        tauri::async_runtime::spawn(async move {
            match request {
                Ok(request) => match service
                    .set_policy_override(&request.tool_name, request.decision)
                    .await
                {
                    Ok(view) => {
                        let _ = app.emit(POLICY_SNAPSHOT_EVENT, view);
                    }
                    Err(error) => {
                        let _ = app.emit(POLICY_ERROR_EVENT, error);
                    }
                },
                Err(error) => {
                    let _ = app.emit(
                        POLICY_ERROR_EVENT,
                        format!("invalid permission policy event payload: {error}"),
                    );
                }
            }
        });
    });
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
    let request_id =
        Uuid::parse_str(&request_id).map_err(|_| "permission request id is invalid".to_owned())?;
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
) -> Result<Vec<PermissionAuditEntry>, String> {
    Ok(permission.recent(limit.unwrap_or(20)).await)
}
