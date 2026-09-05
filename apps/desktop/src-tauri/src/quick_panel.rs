#[path = "management_ipc.rs"]
mod management_ipc;

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};
use tracing::warn;
use windows_tools::window::{self, MonitorBounds, WindowHandle};

pub const QUICK_WINDOW_LABEL: &str = "quick";

const QUICK_MAX_WIDTH: u32 = 760;
const QUICK_MIN_WIDTH: u32 = 420;
const QUICK_HEIGHT: u32 = 206;
const QUICK_SIDE_MARGIN: u32 = 28;
const QUICK_BOTTOM_MARGIN: u32 = 58;

#[derive(Debug, Clone, Serialize)]
struct QuickShownEvent {
    reason: &'static str,
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    if app.get_webview_window(QUICK_WINDOW_LABEL).is_some() {
        return Ok(());
    }

    let quick = WebviewWindowBuilder::new(
        app,
        QUICK_WINDOW_LABEL,
        WebviewUrl::App("index.html?surface=quick".into()),
    )
    .title("")
    .decorations(false)
    .transparent(true)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .closable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focusable(true)
    .focused(false)
    .shadow(false)
    .visible(false)
    .inner_size(QUICK_MAX_WIDTH as f64, QUICK_HEIGHT as f64)
    .build()?;

    // The window is interactive only inside its compact rectangle. The rest of
    // the desktop remains fully usable because this is not a fullscreen WebView.
    quick.set_ignore_cursor_events(false)?;

    // The compact overlay is the long-term graphical host, so it also owns the
    // lifecycle of the local management endpoint used by assistant.exe. Desktop
    // state and WakeService have already been managed before quick_panel::setup.
    let state = app.state::<crate::DesktopState>();
    let management = management_ipc::ManagementIpc::setup(app, &state.runtime_paths)
        .map_err(|error| tauri::Error::Io(std::io::Error::other(error)))?;
    app.manage(management);

    Ok(())
}

pub fn show(
    app: &AppHandle,
    source_window: Option<WindowHandle>,
    reason: &'static str,
) -> Result<(), String> {
    let bounds = resolve_monitor_bounds(app, source_window)?;
    position_window(app, bounds)?;

    let window = app
        .get_webview_window(QUICK_WINDOW_LABEL)
        .ok_or_else(|| "quick assistant window is unavailable".to_owned())?;

    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;

    if let Err(error) = app.emit("quick:shown", QuickShownEvent { reason }) {
        warn!(%error, "failed to emit quick assistant shown event");
    }
    Ok(())
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(QUICK_WINDOW_LABEL) {
        if let Err(error) = window.hide() {
            warn!(%error, "failed to hide quick assistant window");
        }
    }
}

pub fn is_visible(app: &AppHandle) -> bool {
    app.get_webview_window(QUICK_WINDOW_LABEL)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

fn resolve_monitor_bounds(
    app: &AppHandle,
    source_window: Option<WindowHandle>,
) -> Result<MonitorBounds, String> {
    if let Some(handle) = source_window {
        if let Ok(bounds) = window::monitor_bounds(handle) {
            return Ok(bounds);
        }
    }

    let monitor = app
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Windows did not report a primary monitor".to_owned())?;

    Ok(MonitorBounds {
        x: monitor.position().x,
        y: monitor.position().y,
        width: monitor.size().width,
        height: monitor.size().height,
    })
}

fn position_window(app: &AppHandle, bounds: MonitorBounds) -> Result<(), String> {
    let available_width = bounds
        .width
        .saturating_sub(QUICK_SIDE_MARGIN.saturating_mul(2))
        .max(1);
    let width = QUICK_MAX_WIDTH
        .min(available_width)
        .max(QUICK_MIN_WIDTH.min(available_width));
    let height = QUICK_HEIGHT.min(bounds.height).max(1);

    let x_offset = bounds.width.saturating_sub(width) / 2;
    let y_offset = bounds
        .height
        .saturating_sub(height)
        .saturating_sub(QUICK_BOTTOM_MARGIN);

    let x = bounds.x.saturating_add(x_offset as i32);
    let y = bounds.y.saturating_add(y_offset as i32);

    let window = app
        .get_webview_window(QUICK_WINDOW_LABEL)
        .ok_or_else(|| "quick assistant window is unavailable".to_owned())?;
    window
        .set_position(PhysicalPosition { x, y })
        .map_err(|error| error.to_string())?;
    window
        .set_size(PhysicalSize { width, height })
        .map_err(|error| error.to_string())?;
    Ok(())
}
