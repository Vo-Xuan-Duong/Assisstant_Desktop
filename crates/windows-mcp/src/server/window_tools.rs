use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use windows_tools::{
    window::{ActiveWindow, WindowHandle},
    window_control::{self, WindowVisualState},
};

use super::{to_json, WindowsMcpServer};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WindowStateInput {
    Minimize,
    Maximize,
    Restore,
}

impl From<WindowStateInput> for WindowVisualState {
    fn from(value: WindowStateInput) -> Self {
        match value {
            WindowStateInput::Minimize => WindowVisualState::Minimize,
            WindowStateInput::Maximize => WindowVisualState::Maximize,
            WindowStateInput::Restore => WindowVisualState::Restore,
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WindowSetStateInput {
    /// Exact HWND of the target top-level window.
    pub window_handle: i64,
    /// Process id observed for this HWND when the target was selected. The native
    /// layer rejects the action if the HWND has since been recycled to another process.
    pub expected_process_id: u32,
    /// Desired semantic visual state.
    pub state: WindowStateInput,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WindowCloseInput {
    /// Exact HWND of the target top-level window.
    pub window_handle: i64,
    /// Process id observed for this HWND when the target was selected.
    pub expected_process_id: u32,
}

#[derive(Debug, Serialize)]
struct WindowActionResult {
    ok: bool,
    action: &'static str,
    window_before_action: ActiveWindow,
}

fn explicit_window_handle(raw: i64) -> Result<WindowHandle, String> {
    let raw = isize::try_from(raw)
        .map_err(|_| "window_handle is outside the native Windows handle range".to_owned())?;
    if raw == 0 {
        return Err("window_handle must not be zero".to_owned());
    }
    Ok(WindowHandle(raw))
}

#[tool_router(router = window_tool_router, vis = "pub(crate)")]
impl WindowsMcpServer {
    #[tool(
        name = "window_set_state",
        description = "Minimize, maximize, or restore one explicitly identified top-level Windows window. Pass both the exact HWND and expected_process_id from the selected target so native validation can reject a recycled/stale HWND."
    )]
    async fn window_set_state(
        &self,
        Parameters(WindowSetStateInput {
            window_handle,
            expected_process_id,
            state,
        }): Parameters<WindowSetStateInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "window_set_state",
                json!({
                    "window_handle": window_handle,
                    "expected_process_id": expected_process_id,
                    "state": state,
                }),
            )
            .await?;

        let handle = explicit_window_handle(window_handle)?;
        let native_state = WindowVisualState::from(state);
        let before = window_control::set_visual_state(handle, expected_process_id, native_state)
            .map_err(|error| error.to_string())?;
        let action = match native_state {
            WindowVisualState::Minimize => "minimize",
            WindowVisualState::Maximize => "maximize",
            WindowVisualState::Restore => "restore",
        };
        to_json(&WindowActionResult {
            ok: true,
            action,
            window_before_action: before,
        })
    }

    #[tool(
        name = "window_close",
        description = "Request graceful close of one explicitly identified top-level Windows window by posting WM_CLOSE. This does not force-terminate the process; applications may show unsaved-changes UI or cancel closing. Pass the exact HWND and expected_process_id. Sensitive action requires desktop confirmation."
    )]
    async fn window_close(
        &self,
        Parameters(WindowCloseInput {
            window_handle,
            expected_process_id,
        }): Parameters<WindowCloseInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "window_close",
                json!({
                    "window_handle": window_handle,
                    "expected_process_id": expected_process_id,
                }),
            )
            .await?;

        let handle = explicit_window_handle(window_handle)?;
        let before = window_control::request_close(handle, expected_process_id)
            .map_err(|error| error.to_string())?;
        to_json(&WindowActionResult {
            ok: true,
            action: "close_requested",
            window_before_action: before,
        })
    }
}
