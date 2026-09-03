use std::time::Duration;

use permission_broker::{
    bind_local, BrokerError, BrokerHandle, PermissionRequest, UserDecision,
};
use tauri::{AppHandle, Emitter, State};
use tracing::warn;
use uuid::Uuid;

const CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(30);

pub struct PermissionDesktopService {
    broker: BrokerHandle,
}

impl PermissionDesktopService {
    pub fn setup(
        app: &AppHandle,
    ) -> Result<(Self, [(String, String); 2]), BrokerError> {
        let (broker, mut requests) = tauri::async_runtime::block_on(bind_local(CONFIRMATION_TIMEOUT))?;
        let environment = broker.endpoint().environment();

        let app_handle = app.clone();
        let event_broker = broker.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(request) = requests.recv().await {
                // A confirmation request must be visible even when the app is
                // currently hidden in the tray.
                super::show_main_window(&app_handle);
                let request_id = request.request_id;
                if let Err(error) = emit_request(&app_handle, &request) {
                    warn!(%error, %request_id, "failed to emit permission request to desktop UI");
                    let _ = event_broker.respond(request_id, UserDecision::Deny).await;
                }
            }
        });

        Ok((Self { broker }, environment))
    }

    async fn respond(&self, request_id: Uuid, decision: UserDecision) -> Result<(), String> {
        self.broker
            .respond(request_id, decision)
            .await
            .map_err(|error| error.to_string())
    }
}

fn emit_request(app: &AppHandle, request: &PermissionRequest) -> Result<(), String> {
    app.emit("permission:request", request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn assistant_permission_respond(
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
    permission.respond(request_id, decision).await
}
