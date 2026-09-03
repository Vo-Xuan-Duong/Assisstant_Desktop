use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use windows_tools::{
    monitor_layout,
    window::{ActiveWindow, WindowHandle},
    window_control::{self, WindowVisualState},
    window_discovery,
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
pub struct WindowListInput {
    /// Optional maximum number of visible titled top-level windows to return.
    /// Native code defaults to 80 and hard-caps the request at 200.
    pub max_windows: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WindowTargetInput {
    /// Exact HWND of the target top-level window.
    pub window_handle: i64,
    /// Process id observed for this HWND when the target was selected. The native
    /// layer rejects the action if the HWND has since been recycled to another process.
    pub expected_process_id: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WindowSetBoundsInput {
    /// Exact HWND of the target top-level window.
    pub window_handle: i64,
    /// Process id observed for this HWND when the target was selected.
    pub expected_process_id: u32,
    /// Desired left coordinate in the virtual desktop coordinate space.
    pub x: i32,
    /// Desired top coordinate in the virtual desktop coordinate space.
    pub y: i32,
    /// Desired positive width in pixels.
    pub width: i32,
    /// Desired positive height in pixels.
    pub height: i32,
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

async fn run_blocking<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| format!("Windows window worker failed: {error}"))?
}

#[tool_router(router = window_tool_router, vis = "pub(crate)")]
impl WindowsMcpServer {
    #[tool(
        name = "display_list",
        description = "List Windows monitor geometry, including full bounds, work area, and primary-monitor state. This is read-only. Prefer work_area for window placement so taskbars and desktop reserved regions are not covered."
    )]
    async fn display_list(&self) -> Result<String, String> {
        self.permissions.authorize("display_list", json!({})).await?;
        let monitors = run_blocking(|| {
            monitor_layout::list_monitors().map_err(|error| error.to_string())
        })
        .await?;
        to_json(&monitors)
    }

    #[tool(
        name = "window_list",
        description = "List visible titled top-level Windows windows with HWND, process id, executable, minimized state, and whether each window is currently foreground. The result is bounded. This is read-only and is the preferred way to discover a target before explicit window actions."
    )]
    async fn window_list(
        &self,
        Parameters(WindowListInput { max_windows }): Parameters<WindowListInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize("window_list", json!({ "max_windows": max_windows }))
            .await?;

        let max_windows = max_windows.map(|value| value as usize);
        let windows = run_blocking(move || {
            window_discovery::list_top_level(max_windows).map_err(|error| error.to_string())
        })
        .await?;
        to_json(&windows)
    }

    #[tool(
        name = "window_activate",
        description = "Restore if minimized and request foreground activation of one explicitly identified top-level Windows window. Pass the exact HWND and expected_process_id from window_list or trusted Desktop Context. The native layer rejects stale/recycled HWNDs. Windows focus-stealing rules can refuse the request; no keyboard/mouse fallback is used."
    )]
    async fn window_activate(
        &self,
        Parameters(WindowTargetInput {
            window_handle,
            expected_process_id,
        }): Parameters<WindowTargetInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "window_activate",
                json!({
                    "window_handle": window_handle,
                    "expected_process_id": expected_process_id,
                }),
            )
            .await?;

        let handle = explicit_window_handle(window_handle)?;
        let before = window_discovery::activate(handle, expected_process_id)
            .map_err(|error| error.to_string())?;
        to_json(&WindowActionResult {
            ok: true,
            action: "activate",
            window_before_action: before,
        })
    }

    #[tool(
        name = "window_set_bounds",
        description = "Move and resize one explicitly identified top-level Windows window in virtual-desktop coordinates. Pass the exact HWND and expected_process_id. Use display_list first when placing relative to a monitor and prefer its work_area. The operation preserves focus and Z-order."
    )]
    async fn window_set_bounds(
        &self,
        Parameters(WindowSetBoundsInput {
            window_handle,
            expected_process_id,
            x,
            y,
            width,
            height,
        }): Parameters<WindowSetBoundsInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "window_set_bounds",
                json!({
                    "window_handle": window_handle,
                    "expected_process_id": expected_process_id,
                    "x": x,
                    "y": y,
                    "width": width,
                    "height": height,
                }),
            )
            .await?;

        let handle = explicit_window_handle(window_handle)?;
        let before = monitor_layout::set_window_bounds(
            handle,
            expected_process_id,
            x,
            y,
            width,
            height,
        )
        .map_err(|error| error.to_string())?;
        to_json(&WindowActionResult {
            ok: true,
            action: "set_bounds",
            window_before_action: before,
        })
    }

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
        Parameters(WindowTargetInput {
            window_handle,
            expected_process_id,
        }): Parameters<WindowTargetInput>,
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
