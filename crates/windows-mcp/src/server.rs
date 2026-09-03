use rmcp::{
    handler::server::wrapper::Parameters,
    schemars,
    tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::permissions::McpPermissionGateway;
use windows_tools::{
    apps, audio, automation, clipboard, media, system, window,
    automation::{UiInspectOptions, UiScrollAmount, UiTreeSnapshot},
    window::{ActiveWindow, WindowHandle},
    ToolError,
};

#[derive(Clone, Default)]
pub struct WindowsMcpServer {
    permissions: McpPermissionGateway,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetVolumeInput {
    /// Desired master output volume in the inclusive range 0..=100.
    pub percent: f32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetMuteInput {
    /// True to mute the default output device, false to unmute it.
    pub muted: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct OpenAppInput {
    /// Windows Shell target such as `chrome`, `notepad`, a file path, or an https URI.
    pub target: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ClipboardWriteInput {
    /// Unicode text that will replace the current clipboard content.
    pub text: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UiInspectInput {
    /// Optional HWND supplied by Desktop Context or a previous inspection. When
    /// omitted, inspect the current foreground window.
    pub window_handle: Option<i64>,
    /// Maximum UI Automation tree depth. Defaults to 4 and is hard-capped natively.
    pub max_depth: Option<u32>,
    /// Maximum number of returned UI elements. Defaults to 160 and is hard-capped natively.
    pub max_nodes: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UiElementActionInput {
    /// Exact root HWND returned by `ui_inspect`. Actions require an explicit
    /// inspected window handle so a foreground-window change cannot retarget them.
    pub window_handle: i64,
    /// Child-index path returned by the most recent `ui_inspect` snapshot.
    pub path: Vec<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UiSetValueInput {
    /// Exact root HWND returned by `ui_inspect`.
    pub window_handle: i64,
    /// Child-index path returned by the most recent `ui_inspect` snapshot.
    pub path: Vec<u32>,
    /// Text/value to assign to a writable UI Automation ValuePattern element.
    pub value: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UiSetExpandedInput {
    /// Exact root HWND returned by `ui_inspect`.
    pub window_handle: i64,
    /// Child-index path returned by the most recent `ui_inspect` snapshot.
    pub path: Vec<u32>,
    /// True to expand, false to collapse.
    pub expanded: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UiScrollAmountInput {
    LargeDecrement,
    SmallDecrement,
    None,
    LargeIncrement,
    SmallIncrement,
}

impl From<UiScrollAmountInput> for UiScrollAmount {
    fn from(value: UiScrollAmountInput) -> Self {
        match value {
            UiScrollAmountInput::LargeDecrement => UiScrollAmount::LargeDecrement,
            UiScrollAmountInput::SmallDecrement => UiScrollAmount::SmallDecrement,
            UiScrollAmountInput::None => UiScrollAmount::None,
            UiScrollAmountInput::LargeIncrement => UiScrollAmount::LargeIncrement,
            UiScrollAmountInput::SmallIncrement => UiScrollAmount::SmallIncrement,
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UiScrollInput {
    /// Exact root HWND returned by `ui_inspect`.
    pub window_handle: i64,
    /// Path of an element whose snapshot contains a non-null `scroll` capability.
    pub path: Vec<u32>,
    /// Horizontal relative scroll amount. Use `none` when only vertical scrolling is required.
    pub horizontal: UiScrollAmountInput,
    /// Vertical relative scroll amount. Use `none` when only horizontal scrolling is required.
    pub vertical: UiScrollAmountInput,
}

#[derive(Debug, Serialize)]
struct UiInspectResult {
    window: ActiveWindow,
    tree: UiTreeSnapshot,
}

#[derive(Debug, Serialize)]
struct UiActionResult {
    ok: bool,
    action: &'static str,
    /// Snapshot of the target root before the action. Invoke can legitimately
    /// close or replace its window, so post-action HWND lookup is not required.
    window_before_action: ActiveWindow,
    path: Vec<u32>,
}

fn to_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("failed to serialize tool result: {error}"))
}

fn tool_error(error: ToolError) -> String {
    error.to_string()
}

fn explicit_window_handle(raw: i64) -> Result<WindowHandle, String> {
    let raw = isize::try_from(raw)
        .map_err(|_| "window_handle is outside the native Windows handle range".to_owned())?;
    if raw == 0 {
        return Err("window_handle must not be zero".to_owned());
    }
    Ok(WindowHandle(raw))
}

fn inspect_window_handle(raw: Option<i64>) -> Result<WindowHandle, String> {
    match raw {
        Some(raw) => explicit_window_handle(raw),
        None => window::get_active_handle().map_err(tool_error),
    }
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

fn action_result(
    window_before_action: ActiveWindow,
    path: Vec<u32>,
    action: &'static str,
) -> UiActionResult {
    UiActionResult {
        ok: true,
        action,
        window_before_action,
        path,
    }
}

#[tool_router(server_handler)]
impl WindowsMcpServer {
    #[tool(
        name = "audio_get_volume",
        description = "Read the current master volume percentage and mute state of the default Windows output device. This is read-only."
    )]
    async fn audio_get_volume(&self) -> Result<String, String> {
        self.permissions.authorize("audio_get_volume", json!({})).await?;
        audio::get_state().map_err(tool_error).and_then(|value| to_json(&value))
    }

    #[tool(
        name = "audio_set_volume",
        description = "Set the master volume percentage of the default Windows output device. The percent must be between 0 and 100."
    )]
    async fn audio_set_volume(
        &self,
        Parameters(SetVolumeInput { percent }): Parameters<SetVolumeInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize("audio_set_volume", json!({ "percent": percent }))
            .await?;
        audio::set_volume(percent)
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "audio_set_mute",
        description = "Mute or unmute the default Windows output device."
    )]
    async fn audio_set_mute(
        &self,
        Parameters(SetMuteInput { muted }): Parameters<SetMuteInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize("audio_set_mute", json!({ "muted": muted }))
            .await?;
        audio::set_mute(muted)
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "apps_open",
        description = "Open an application, document, file path, or URI through the Windows Shell. Do not use this as an arbitrary shell-command executor."
    )]
    async fn apps_open(
        &self,
        Parameters(OpenAppInput { target }): Parameters<OpenAppInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize("apps_open", json!({ "target": &target }))
            .await?;
        apps::open(&target)
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "apps_list",
        description = "List currently running Windows process executables and process ids. This is read-only and does not terminate or modify processes."
    )]
    async fn apps_list(&self) -> Result<String, String> {
        self.permissions.authorize("apps_list", json!({})).await?;
        apps::list_running()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "window_get_active",
        description = "Read the title and process id of the current Windows foreground window. This is read-only."
    )]
    async fn window_get_active(&self) -> Result<String, String> {
        self.permissions
            .authorize("window_get_active", json!({}))
            .await?;
        window::get_active()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "system_get_info",
        description = "Read basic Windows machine information including logical CPU count and physical-memory usage. This is read-only."
    )]
    async fn system_get_info(&self) -> Result<String, String> {
        self.permissions
            .authorize("system_get_info", json!({}))
            .await?;
        system::get_info()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "media_play_pause",
        description = "Send the Windows media play/pause key to the active media session."
    )]
    async fn media_play_pause(&self) -> Result<String, String> {
        self.permissions
            .authorize("media_play_pause", json!({}))
            .await?;
        media::play_pause()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "media_next",
        description = "Send the Windows next-track media key to the active media session."
    )]
    async fn media_next(&self) -> Result<String, String> {
        self.permissions.authorize("media_next", json!({})).await?;
        media::next()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "media_previous",
        description = "Send the Windows previous-track media key to the active media session."
    )]
    async fn media_previous(&self) -> Result<String, String> {
        self.permissions
            .authorize("media_previous", json!({}))
            .await?;
        media::previous()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "clipboard_read_text",
        description = "Read Unicode text from the Windows clipboard. Clipboard content can contain sensitive information, so call this only when it is relevant to the user's request."
    )]
    async fn clipboard_read_text(&self) -> Result<String, String> {
        self.permissions
            .authorize("clipboard_read_text", json!({}))
            .await?;
        clipboard::read_text()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "clipboard_write_text",
        description = "Replace the Windows clipboard contents with the supplied Unicode text."
    )]
    async fn clipboard_write_text(
        &self,
        Parameters(ClipboardWriteInput { text }): Parameters<ClipboardWriteInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize("clipboard_write_text", json!({ "text": &text }))
            .await?;
        clipboard::write_text(&text)
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "ui_inspect",
        description = "Inspect the Windows UI Automation Control View for a window. When Desktop Context provides active_window_handle for the user's referenced app, pass it explicitly; only omit window_handle when the actual foreground window is intentionally the target. Returns structural metadata, root_window_handle, child-index paths, and available pattern state (toggle/selection/expand/scroll) without reading editable field values. Use this before any UI action and inspect again if a path becomes stale."
    )]
    async fn ui_inspect(
        &self,
        Parameters(UiInspectInput {
            window_handle,
            max_depth,
            max_nodes,
        }): Parameters<UiInspectInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "ui_inspect",
                json!({
                    "window_handle": window_handle,
                    "max_depth": max_depth,
                    "max_nodes": max_nodes,
                }),
            )
            .await?;
        let handle = inspect_window_handle(window_handle)?;
        let options = UiInspectOptions {
            max_depth: max_depth.unwrap_or(4),
            max_nodes: max_nodes.unwrap_or(160) as usize,
        };

        let result = run_blocking(move || {
            let window = window::get(handle).map_err(tool_error)?;
            let tree = automation::inspect(handle, options).map_err(tool_error)?;
            Ok(UiInspectResult { window, tree })
        })
        .await?;

        to_json(&result)
    }

    #[tool(
        name = "ui_focus",
        description = "Focus an element from a recent ui_inspect snapshot. Pass the exact root_window_handle returned by ui_inspect and the element path. The operation fails if the path is stale; inspect again instead of guessing."
    )]
    async fn ui_focus(
        &self,
        Parameters(UiElementActionInput {
            window_handle,
            path,
        }): Parameters<UiElementActionInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "ui_focus",
                json!({ "window_handle": window_handle, "path": &path }),
            )
            .await?;
        let handle = explicit_window_handle(window_handle)?;
        let result_path = path.clone();
        let result = run_blocking(move || {
            let window_before_action = window::get(handle).map_err(tool_error)?;
            automation::focus(handle, &path).map_err(tool_error)?;
            Ok(action_result(window_before_action, result_path, "focus"))
        })
        .await?;
        to_json(&result)
    }

    #[tool(
        name = "ui_invoke",
        description = "Invoke an element from a recent ui_inspect snapshot using Windows UI Automation InvokePattern. Use only when supports_invoke=true. This does not synthesize a mouse click. Pass the exact inspected root_window_handle and path; inspect again if stale. Sensitive actions require desktop confirmation."
    )]
    async fn ui_invoke(
        &self,
        Parameters(UiElementActionInput {
            window_handle,
            path,
        }): Parameters<UiElementActionInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "ui_invoke",
                json!({ "window_handle": window_handle, "path": &path }),
            )
            .await?;
        let handle = explicit_window_handle(window_handle)?;
        let result_path = path.clone();
        let result = run_blocking(move || {
            let window_before_action = window::get(handle).map_err(tool_error)?;
            automation::invoke(handle, &path).map_err(tool_error)?;
            Ok(action_result(window_before_action, result_path, "invoke"))
        })
        .await?;
        to_json(&result)
    }

    #[tool(
        name = "ui_set_value",
        description = "Set text/value on an element from a recent ui_inspect snapshot using a writable Windows UI Automation ValuePattern. Use only when supports_value=true and the user request requires editing that control. Never infer passwords or secrets. Pass the exact inspected root_window_handle and path; inspect again if stale. Sensitive actions require desktop confirmation."
    )]
    async fn ui_set_value(
        &self,
        Parameters(UiSetValueInput {
            window_handle,
            path,
            value,
        }): Parameters<UiSetValueInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "ui_set_value",
                json!({
                    "window_handle": window_handle,
                    "path": &path,
                    "value": &value,
                }),
            )
            .await?;
        let handle = explicit_window_handle(window_handle)?;
        let result_path = path.clone();
        let result = run_blocking(move || {
            let window_before_action = window::get(handle).map_err(tool_error)?;
            automation::set_value(handle, &path, &value).map_err(tool_error)?;
            Ok(action_result(window_before_action, result_path, "set_value"))
        })
        .await?;
        to_json(&result)
    }

    #[tool(
        name = "ui_toggle",
        description = "Toggle an element that exposes UI Automation TogglePattern, such as many checkboxes or switches. Inspect first and use the returned toggle_state to avoid unnecessary or incorrect toggles. Sensitive actions require desktop confirmation."
    )]
    async fn ui_toggle(
        &self,
        Parameters(UiElementActionInput {
            window_handle,
            path,
        }): Parameters<UiElementActionInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "ui_toggle",
                json!({ "window_handle": window_handle, "path": &path }),
            )
            .await?;
        let handle = explicit_window_handle(window_handle)?;
        let result_path = path.clone();
        let result = run_blocking(move || {
            let window_before_action = window::get(handle).map_err(tool_error)?;
            automation::toggle(handle, &path).map_err(tool_error)?;
            Ok(action_result(window_before_action, result_path, "toggle"))
        })
        .await?;
        to_json(&result)
    }

    #[tool(
        name = "ui_select",
        description = "Select one element that exposes UI Automation SelectionItemPattern, such as many list, tab, menu, or radio items. Inspect first and prefer an element whose is_selected state is false. Sensitive actions require desktop confirmation."
    )]
    async fn ui_select(
        &self,
        Parameters(UiElementActionInput {
            window_handle,
            path,
        }): Parameters<UiElementActionInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "ui_select",
                json!({ "window_handle": window_handle, "path": &path }),
            )
            .await?;
        let handle = explicit_window_handle(window_handle)?;
        let result_path = path.clone();
        let result = run_blocking(move || {
            let window_before_action = window::get(handle).map_err(tool_error)?;
            automation::select(handle, &path).map_err(tool_error)?;
            Ok(action_result(window_before_action, result_path, "select"))
        })
        .await?;
        to_json(&result)
    }

    #[tool(
        name = "ui_set_expanded",
        description = "Expand or collapse an element that exposes UI Automation ExpandCollapsePattern. Inspect first and compare expand_collapse_state before changing it. expanded=true expands; false collapses."
    )]
    async fn ui_set_expanded(
        &self,
        Parameters(UiSetExpandedInput {
            window_handle,
            path,
            expanded,
        }): Parameters<UiSetExpandedInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "ui_set_expanded",
                json!({
                    "window_handle": window_handle,
                    "path": &path,
                    "expanded": expanded,
                }),
            )
            .await?;
        let handle = explicit_window_handle(window_handle)?;
        let result_path = path.clone();
        let action = if expanded { "expand" } else { "collapse" };
        let result = run_blocking(move || {
            let window_before_action = window::get(handle).map_err(tool_error)?;
            automation::set_expanded(handle, &path, expanded).map_err(tool_error)?;
            Ok(action_result(window_before_action, result_path, action))
        })
        .await?;
        to_json(&result)
    }

    #[tool(
        name = "ui_scroll",
        description = "Scroll an element that exposes UI Automation ScrollPattern by discrete horizontal/vertical amounts. Inspect first and use the scroll object to determine which axes are scrollable. Use `none` for an unchanged axis; at least one axis must request a non-none amount."
    )]
    async fn ui_scroll(
        &self,
        Parameters(UiScrollInput {
            window_handle,
            path,
            horizontal,
            vertical,
        }): Parameters<UiScrollInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "ui_scroll",
                json!({
                    "window_handle": window_handle,
                    "path": &path,
                    "horizontal": horizontal,
                    "vertical": vertical,
                }),
            )
            .await?;
        let handle = explicit_window_handle(window_handle)?;
        let result_path = path.clone();
        let horizontal = UiScrollAmount::from(horizontal);
        let vertical = UiScrollAmount::from(vertical);
        let result = run_blocking(move || {
            let window_before_action = window::get(handle).map_err(tool_error)?;
            automation::scroll(handle, &path, horizontal, vertical).map_err(tool_error)?;
            Ok(action_result(window_before_action, result_path, "scroll"))
        })
        .await?;
        to_json(&result)
    }
}
