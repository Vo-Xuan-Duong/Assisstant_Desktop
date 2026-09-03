use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use windows_tools::{
    virtualized,
    window::{self, ActiveWindow, WindowHandle},
};

use super::{to_json, tool_error, WindowsMcpServer};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VirtualizedItemInput {
    /// Exact root HWND used by the recent UI Automation inspection.
    pub window_handle: i64,
    /// Child-index path of the candidate element from the recent inspection.
    pub path: Vec<u32>,
}

#[derive(Debug, Serialize)]
struct VirtualizedItemStatusResult {
    window: ActiveWindow,
    path: Vec<u32>,
    supported: bool,
}

#[derive(Debug, Serialize)]
struct VirtualizedItemActionResult {
    ok: bool,
    action: &'static str,
    window_before_action: ActiveWindow,
    path: Vec<u32>,
    reinspection_required: bool,
}

fn explicit_window_handle(raw: i64) -> Result<WindowHandle, String> {
    let raw = isize::try_from(raw)
        .map_err(|_| "window_handle is outside the native Windows handle range".to_owned())?;
    if raw == 0 {
        return Err("window_handle must not be zero".to_owned());
    }
    Ok(WindowHandle(raw))
}

async fn run_blocking<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| format!("Windows UI Automation worker failed: {error}"))?
}

#[tool_router(router = virtualized_tool_router, vis = "pub(crate)")]
impl WindowsMcpServer {
    #[tool(
        name = "ui_virtualized_item_status",
        description = "Check whether one explicitly inspected Windows UI Automation element exposes VirtualizedItemPattern. This is read-only. Use the exact root_window_handle and path from a recent inspection."
    )]
    async fn ui_virtualized_item_status(
        &self,
        Parameters(VirtualizedItemInput {
            window_handle,
            path,
        }): Parameters<VirtualizedItemInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "ui_virtualized_item_status",
                json!({ "window_handle": window_handle, "path": &path }),
            )
            .await?;
        let handle = explicit_window_handle(window_handle)?;
        let result_path = path.clone();
        let result = run_blocking(move || {
            let window = window::get(handle).map_err(tool_error)?;
            let status = virtualized::status(handle, &path).map_err(tool_error)?;
            Ok(VirtualizedItemStatusResult {
                window,
                path: result_path,
                supported: status.supported,
            })
        })
        .await?;
        to_json(&result)
    }

    #[tool(
        name = "ui_realize",
        description = "Materialize an explicitly inspected virtualized Windows UI Automation element using VirtualizedItemPattern::Realize. Check ui_virtualized_item_status first. This does not focus, select, invoke, click or type. Re-inspect after success because realization can change the accessibility tree and invalidate the old path."
    )]
    async fn ui_realize(
        &self,
        Parameters(VirtualizedItemInput {
            window_handle,
            path,
        }): Parameters<VirtualizedItemInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "ui_realize",
                json!({ "window_handle": window_handle, "path": &path }),
            )
            .await?;
        let handle = explicit_window_handle(window_handle)?;
        let result_path = path.clone();
        let result = run_blocking(move || {
            let window_before_action = window::get(handle).map_err(tool_error)?;
            virtualized::realize(handle, &path).map_err(tool_error)?;
            Ok(VirtualizedItemActionResult {
                ok: true,
                action: "realize",
                window_before_action,
                path: result_path,
                reinspection_required: true,
            })
        })
        .await?;
        to_json(&result)
    }
}
