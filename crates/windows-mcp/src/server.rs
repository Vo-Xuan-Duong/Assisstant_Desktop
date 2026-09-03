use rmcp::{
    handler::server::wrapper::Parameters,
    schemars,
    tool,
    tool_router,
};
use serde::{Deserialize, Serialize};

use windows_tools::{
    apps, audio, automation, clipboard, media, system, window,
    automation::{UiInspectOptions, UiTreeSnapshot},
    window::{ActiveWindow, WindowHandle},
    ToolError,
};

#[derive(Debug, Clone, Default)]
pub struct WindowsMcpServer;

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
    /// Optional HWND returned by a previous Windows/window tool. When omitted,
    /// inspect the current foreground window.
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

#[derive(Debug, Serialize)]
struct UiInspectResult {
    window: ActiveWindow,
    tree: UiTreeSnapshot,
}

#[derive(Debug, Serialize)]
struct UiActionResult {
    ok: bool,
    action: &'static str,
    window: ActiveWindow,
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
    handle: WindowHandle,
    path: Vec<u32>,
    action: &'static str,
) -> Result<UiActionResult, String> {
    let window = window::get(handle).map_err(tool_error)?;
    Ok(UiActionResult {
        ok: true,
        action,
        window,
        path,
    })
}

#[tool_router(server_handler)]
impl WindowsMcpServer {
    #[tool(
        name = "audio_get_volume",
        description = "Read the current master volume percentage and mute state of the default Windows output device. This is read-only."
    )]
    fn audio_get_volume(&self) -> Result<String, String> {
        audio::get_state().map_err(tool_error).and_then(|value| to_json(&value))
    }

    #[tool(
        name = "audio_set_volume",
        description = "Set the master volume percentage of the default Windows output device. The percent must be between 0 and 100."
    )]
    fn audio_set_volume(
        &self,
        Parameters(SetVolumeInput { percent }): Parameters<SetVolumeInput>,
    ) -> Result<String, String> {
        audio::set_volume(percent)
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "audio_set_mute",
        description = "Mute or unmute the default Windows output device."
    )]
    fn audio_set_mute(
        &self,
        Parameters(SetMuteInput { muted }): Parameters<SetMuteInput>,
    ) -> Result<String, String> {
        audio::set_mute(muted)
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "apps_open",
        description = "Open an application, document, file path, or URI through the Windows Shell. Do not use this as an arbitrary shell-command executor."
    )]
    fn apps_open(
        &self,
        Parameters(OpenAppInput { target }): Parameters<OpenAppInput>,
    ) -> Result<String, String> {
        apps::open(&target)
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "apps_list",
        description = "List currently running Windows process executables and process ids. This is read-only and does not terminate or modify processes."
    )]
    fn apps_list(&self) -> Result<String, String> {
        apps::list_running()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "window_get_active",
        description = "Read the title and process id of the current Windows foreground window. This is read-only."
    )]
    fn window_get_active(&self) -> Result<String, String> {
        window::get_active()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "system_get_info",
        description = "Read basic Windows machine information including logical CPU count and physical-memory usage. This is read-only."
    )]
    fn system_get_info(&self) -> Result<String, String> {
        system::get_info()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "media_play_pause",
        description = "Send the Windows media play/pause key to the active media session."
    )]
    fn media_play_pause(&self) -> Result<String, String> {
        media::play_pause()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "media_next",
        description = "Send the Windows next-track media key to the active media session."
    )]
    fn media_next(&self) -> Result<String, String> {
        media::next()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "media_previous",
        description = "Send the Windows previous-track media key to the active media session."
    )]
    fn media_previous(&self) -> Result<String, String> {
        media::previous()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "clipboard_read_text",
        description = "Read Unicode text from the Windows clipboard. Clipboard content can contain sensitive information, so call this only when it is relevant to the user's request."
    )]
    fn clipboard_read_text(&self) -> Result<String, String> {
        clipboard::read_text()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "clipboard_write_text",
        description = "Replace the Windows clipboard contents with the supplied Unicode text."
    )]
    fn clipboard_write_text(
        &self,
        Parameters(ClipboardWriteInput { text }): Parameters<ClipboardWriteInput>,
    ) -> Result<String, String> {
        clipboard::write_text(&text)
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "ui_inspect",
        description = "Inspect the Windows UI Automation Control View for a window. If window_handle is omitted, inspect the current foreground window. Returns structural accessibility metadata and a root_window_handle plus child-index paths. It deliberately does not read editable field values. Use this before ui_focus/ui_invoke/ui_set_value, and inspect again if a path becomes stale."
    )]
    async fn ui_inspect(
        &self,
        Parameters(UiInspectInput {
            window_handle,
            max_depth,
            max_nodes,
        }): Parameters<UiInspectInput>,
    ) -> Result<String, String> {
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
        let handle = explicit_window_handle(window_handle)?;
        let result_path = path.clone();
        let result = run_blocking(move || {
            automation::focus(handle, &path).map_err(tool_error)?;
            action_result(handle, result_path, "focus")
        })
        .await?;
        to_json(&result)
    }

    #[tool(
        name = "ui_invoke",
        description = "Invoke an element from a recent ui_inspect snapshot using Windows UI Automation InvokePattern. Use only when the inspected element reports supports_invoke=true. This does not synthesize a mouse click. Pass the exact inspected root_window_handle and path; inspect again if stale."
    )]
    async fn ui_invoke(
        &self,
        Parameters(UiElementActionInput {
            window_handle,
            path,
        }): Parameters<UiElementActionInput>,
    ) -> Result<String, String> {
        let handle = explicit_window_handle(window_handle)?;
        let result_path = path.clone();
        let result = run_blocking(move || {
            automation::invoke(handle, &path).map_err(tool_error)?;
            action_result(handle, result_path, "invoke")
        })
        .await?;
        to_json(&result)
    }

    #[tool(
        name = "ui_set_value",
        description = "Set text/value on an element from a recent ui_inspect snapshot using a writable Windows UI Automation ValuePattern. Use only when supports_value=true and the user request requires editing that control. Never infer passwords or secrets. Pass the exact inspected root_window_handle and path; inspect again if stale."
    )]
    async fn ui_set_value(
        &self,
        Parameters(UiSetValueInput {
            window_handle,
            path,
            value,
        }): Parameters<UiSetValueInput>,
    ) -> Result<String, String> {
        let handle = explicit_window_handle(window_handle)?;
        let result_path = path.clone();
        let result = run_blocking(move || {
            automation::set_value(handle, &path, &value).map_err(tool_error)?;
            action_result(handle, result_path, "set_value")
        })
        .await?;
        to_json(&result)
    }
}
