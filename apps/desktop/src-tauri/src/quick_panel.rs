#[path = "management_ipc.rs"]
mod management_ipc;

use std::time::Duration;

use assistant_common::AssistantState;
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Listener, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindowBuilder,
};
use tracing::{debug, warn};
use windows_tools::window::{self, MonitorBounds, WindowHandle};

pub const QUICK_WINDOW_LABEL: &str = "quick";

const QUICK_MAX_WIDTH: u32 = 760;
const QUICK_MIN_WIDTH: u32 = 420;
const QUICK_DEFAULT_HEIGHT: u32 = 206;
const QUICK_MAX_HEIGHT: u32 = 380;
const QUICK_SIDE_MARGIN: u32 = 28;
const QUICK_BOTTOM_MARGIN: u32 = 18;
const QUICK_RESIZE_EVENT: &str = "quick:resize_request";
const QUICK_CANCEL_EVENT: &str = "quick:cancel_request";

#[derive(Debug, Clone, Serialize)]
struct QuickShownEvent {
    reason: &'static str,
}

#[derive(Debug, Deserialize)]
struct QuickResizeRequest {
    height: u32,
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
    .inner_size(QUICK_MAX_WIDTH as f64, QUICK_DEFAULT_HEIGHT as f64)
    .build()?;

    // The window is interactive only inside its compact rectangle. The rest of
    // the desktop remains fully usable because this is not a fullscreen WebView.
    quick.set_ignore_cursor_events(false)?;

    // Frontend content may request more vertical room, but native code remains
    // authoritative for clamping and work-area-aware positioning. This avoids
    // granting the WebView direct window mutation permissions.
    let resize_app = app.clone();
    app.listen(QUICK_RESIZE_EVENT, move |event| {
        let request = serde_json::from_str::<QuickResizeRequest>(event.payload());
        match request {
            Ok(request) => {
                if let Err(error) = resize_window(&resize_app, request.height) {
                    warn!(%error, "failed to resize quick assistant window");
                }
            }
            Err(error) => {
                warn!(%error, "ignored malformed quick assistant resize request");
            }
        }
    });

    // Cancellation is routed by the Assistant lifecycle phase. Processing keeps
    // the existing out-of-band Antigravity child cancellation, while Speaking
    // sends a purge request to the dedicated Windows SAPI worker. Executing and
    // Confirming remain non-cancellable because an external side effect may
    // already be in progress and cannot be safely rolled back from this UI.
    let cancel_app = app.clone();
    app.listen(QUICK_CANCEL_EVENT, move |_| {
        let state = cancel_app.state::<crate::DesktopState>();
        let client = state.client.clone();
        let core = state.core.clone();
        let tts = state.tts.clone();

        tauri::async_runtime::spawn(async move {
            match core.state().await {
                AssistantState::Speaking => {
                    if tts.cancel() {
                        debug!("requested cancellation of active Windows SAPI speech");
                    } else {
                        debug!("ignored Quick speech cancellation because SAPI worker is unavailable");
                    }
                }
                AssistantState::Processing => {
                    if client.cancel_active_turn() {
                        debug!("requested cancellation of active Antigravity turn");
                        return;
                    }

                    // Processing is published immediately before
                    // AgentBackend::complete enters AntigravityClient::ask(). A
                    // human can click Stop inside that tiny registration gap.
                    // Retry once after 20 ms while the phase is still Processing.
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    if core.state().await == AssistantState::Processing
                        && client.cancel_active_turn()
                    {
                        debug!("requested cancellation after Antigravity turn registration");
                    } else {
                        debug!("ignored Quick cancel request because no cancellable Antigravity turn was active");
                    }
                }
                phase => {
                    debug!(?phase, "ignored Quick cancel request in non-cancellable phase");
                }
            }
        });
    });

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
    let bounds = resolve_work_area(app, source_window)?;
    position_window(app, bounds, QUICK_DEFAULT_HEIGHT)?;

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

fn resize_window(app: &AppHandle, requested_height: u32) -> Result<(), String> {
    let bounds = resolve_work_area(app, crate::source_window(app))?;
    position_window(app, bounds, requested_height)
}

fn resolve_work_area(
    app: &AppHandle,
    source_window: Option<WindowHandle>,
) -> Result<MonitorBounds, String> {
    if let Some(handle) = source_window {
        if let Ok(bounds) = window::monitor_work_area(handle) {
            return Ok(bounds);
        }
    }

    if let Ok(bounds) = window::primary_monitor_work_area() {
        return Ok(bounds);
    }

    // Keep a Tauri fallback so Quick can still surface if Win32 monitor work-area
    // lookup is temporarily unavailable. This fallback may include the taskbar.
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

fn position_window(
    app: &AppHandle,
    bounds: MonitorBounds,
    requested_height: u32,
) -> Result<(), String> {
    let available_width = bounds
        .width
        .saturating_sub(QUICK_SIDE_MARGIN.saturating_mul(2))
        .max(1);
    let width = QUICK_MAX_WIDTH
        .min(available_width)
        .max(QUICK_MIN_WIDTH.min(available_width));

    let max_available_height = bounds
        .height
        .saturating_sub(QUICK_BOTTOM_MARGIN)
        .max(1);
    let max_height = QUICK_MAX_HEIGHT.min(max_available_height);
    let min_height = QUICK_DEFAULT_HEIGHT.min(max_height);
    let height = requested_height.max(min_height).min(max_height);

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
