use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindowBuilder,
};
use tracing::warn;
use windows_tools::window::{self, MonitorBounds, WindowHandle};

const EDGE_THICKNESS: u32 = 24;
const EDGE_LABELS: [&str; 4] = ["edge-top", "edge-right", "edge-bottom", "edge-left"];

#[derive(Debug, Clone, Serialize)]
struct EdgeModeEvent {
    mode: &'static str,
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    for (label, edge) in [
        ("edge-top", "top"),
        ("edge-right", "right"),
        ("edge-bottom", "bottom"),
        ("edge-left", "left"),
    ] {
        if app.get_webview_window(label).is_some() {
            continue;
        }

        let route = format!("index.html?surface=edge&edge={edge}");
        let overlay = WebviewWindowBuilder::new(app, label, WebviewUrl::App(route.into()))
            .title("")
            .decorations(false)
            .transparent(true)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .closable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focusable(false)
            .focused(false)
            .shadow(false)
            .visible(false)
            .inner_size(1.0, 1.0)
            .build()?;

        // The glow must never interfere with the application underneath it.
        overlay.set_ignore_cursor_events(true)?;
    }

    Ok(())
}

pub fn activate(app: &AppHandle, source_window: Option<WindowHandle>) {
    if let Err(error) = show(app, source_window) {
        warn!(%error, "failed to show assistant edge overlay");
        return;
    }

    if let Err(error) = app.emit("edge:mode", EdgeModeEvent { mode: "activated" }) {
        warn!(%error, "failed to emit assistant edge activation mode");
    }
}

fn show(app: &AppHandle, source_window: Option<WindowHandle>) -> Result<(), String> {
    let bounds = resolve_monitor_bounds(app, source_window)?;
    position_windows(app, bounds)?;

    for label in EDGE_LABELS {
        if let Some(window) = app.get_webview_window(label) {
            window.show().map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

pub fn hide(app: &AppHandle) {
    for label in EDGE_LABELS {
        if let Some(window) = app.get_webview_window(label) {
            if let Err(error) = window.hide() {
                warn!(%error, %label, "failed to hide assistant edge overlay");
            }
        }
    }
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

fn position_windows(app: &AppHandle, bounds: MonitorBounds) -> Result<(), String> {
    let thickness = EDGE_THICKNESS.min(bounds.width).min(bounds.height).max(1);
    let right_x = bounds
        .x
        .saturating_add(bounds.width.saturating_sub(thickness) as i32);
    let bottom_y = bounds
        .y
        .saturating_add(bounds.height.saturating_sub(thickness) as i32);

    set_geometry(
        app,
        "edge-top",
        bounds.x,
        bounds.y,
        bounds.width,
        thickness,
    )?;
    set_geometry(
        app,
        "edge-right",
        right_x,
        bounds.y,
        thickness,
        bounds.height,
    )?;
    set_geometry(
        app,
        "edge-bottom",
        bounds.x,
        bottom_y,
        bounds.width,
        thickness,
    )?;
    set_geometry(
        app,
        "edge-left",
        bounds.x,
        bounds.y,
        thickness,
        bounds.height,
    )?;
    Ok(())
}

fn set_geometry(
    app: &AppHandle,
    label: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("edge window `{label}` is unavailable"))?;

    window
        .set_position(PhysicalPosition { x, y })
        .map_err(|error| error.to_string())?;
    window
        .set_size(PhysicalSize { width, height })
        .map_err(|error| error.to_string())?;
    Ok(())
}
