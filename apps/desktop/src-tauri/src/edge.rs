use std::sync::atomic::{AtomicU64, Ordering};

use assistant_common::{AssistantEvent, AssistantState};
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindowBuilder,
};
use tracing::warn;
use windows_tools::window::{self, MonitorBounds, WindowHandle};

const EDGE_THICKNESS: u32 = 24;
const EDGE_LABELS: [&str; 4] = ["edge-top", "edge-right", "edge-bottom", "edge-left"];
static EDGE_EPOCH: AtomicU64 = AtomicU64::new(0);

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
    let epoch = next_epoch();
    if let Err(error) = show_mode(app, source_window, "activated") {
        warn!(%error, "failed to show assistant activation edge overlay");
        return;
    }

    schedule_hide(app.clone(), epoch, 900);
}

pub fn sync_assistant_event(
    app: &AppHandle,
    event: &AssistantEvent,
    source_window: Option<WindowHandle>,
) {
    let AssistantEvent::StateChanged { to, .. } = event else {
        return;
    };

    let epoch = next_epoch();
    match to {
        AssistantState::Idle => {
            // Voice turns briefly pass through Idle between the model response and
            // SAPI playback. A short grace period prevents a visible edge flicker.
            schedule_hide(app.clone(), epoch, 160);
        }
        AssistantState::Listening => show_or_warn(app, source_window, "listening"),
        AssistantState::Processing => show_or_warn(app, source_window, "processing"),
        AssistantState::Executing => show_or_warn(app, source_window, "executing"),
        AssistantState::Speaking => show_or_warn(app, source_window, "speaking"),
        AssistantState::Confirming => show_or_warn(app, source_window, "confirming"),
        AssistantState::Error => {
            show_or_warn(app, source_window, "error");
            schedule_hide(app.clone(), epoch, 1400);
        }
    }
}

fn show_or_warn(app: &AppHandle, source_window: Option<WindowHandle>, mode: &'static str) {
    if let Err(error) = show_mode(app, source_window, mode) {
        warn!(%error, %mode, "failed to update assistant edge overlay");
    }
}

fn show_mode(
    app: &AppHandle,
    source_window: Option<WindowHandle>,
    mode: &'static str,
) -> Result<(), String> {
    let bounds = resolve_monitor_bounds(app, source_window)?;
    position_windows(app, bounds)?;

    app.emit("edge:mode", EdgeModeEvent { mode })
        .map_err(|error| error.to_string())?;

    for label in EDGE_LABELS {
        if let Some(window) = app.get_webview_window(label) {
            window.show().map_err(|error| error.to_string())?;
        }
    }
    Ok(())
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

fn hide(app: &AppHandle) {
    for label in EDGE_LABELS {
        if let Some(window) = app.get_webview_window(label) {
            if let Err(error) = window.hide() {
                warn!(%error, %label, "failed to hide assistant edge overlay");
            }
        }
    }
}

fn next_epoch() -> u64 {
    EDGE_EPOCH.fetch_add(1, Ordering::AcqRel).wrapping_add(1)
}

fn schedule_hide(app: AppHandle, epoch: u64, delay_ms: u64) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        if EDGE_EPOCH.load(Ordering::Acquire) == epoch {
            hide(&app);
        }
    });
}
