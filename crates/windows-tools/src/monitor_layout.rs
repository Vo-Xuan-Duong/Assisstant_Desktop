use std::mem::size_of;

use serde::Serialize;
use windows::{
    core::{BOOL, Error as WindowsError},
    Win32::{
        Foundation::{LPARAM, RECT},
        Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO},
        UI::WindowsAndMessaging::{
            SetWindowPos, MONITORINFOF_PRIMARY, SWP_NOACTIVATE, SWP_NOZORDER,
        },
    },
};

use crate::{
    window::{self, ActiveWindow, WindowHandle},
    ToolError, ToolResult,
};

const HARD_MAX_MONITORS: usize = 32;
const MAX_WINDOW_DIMENSION: i32 = 32_768;
const MAX_ABS_WINDOW_COORDINATE: i64 = 100_000;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct DesktopRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl From<RECT> for DesktopRect {
    fn from(value: RECT) -> Self {
        Self {
            left: value.left,
            top: value.top,
            right: value.right,
            bottom: value.bottom,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MonitorDescriptor {
    pub monitor_handle: isize,
    pub bounds: DesktopRect,
    pub work_area: DesktopRect,
    pub primary: bool,
}

/// List physical/logical monitor rectangles exposed by Win32.
///
/// Both full monitor bounds and the work area are returned because assistant
/// placement should normally prefer the work area to avoid taskbars/docks.
pub fn list_monitors() -> ToolResult<Vec<MonitorDescriptor>> {
    let mut monitors = Vec::<MonitorDescriptor>::new();
    let ok = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(enum_monitor_callback),
            LPARAM((&mut monitors as *mut Vec<MonitorDescriptor>) as isize),
        )
    };

    if !ok.as_bool() {
        return Err(ToolError::Windows(WindowsError::from_thread()));
    }

    Ok(monitors)
}

/// Move and resize one explicitly identified top-level window.
///
/// The operation intentionally keeps both focus and Z-order unchanged. Call
/// `window_activate` separately when foreground activation is actually desired.
pub fn set_window_bounds(
    handle: WindowHandle,
    expected_process_id: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> ToolResult<ActiveWindow> {
    validate_bounds(x, y, width, height)?;

    if expected_process_id == 0 {
        return Err(ToolError::InvalidArgument(
            "expected_process_id must not be zero".into(),
        ));
    }

    let before = window::get(handle)?;
    if before.process_id != expected_process_id {
        return Err(ToolError::NotFound(format!(
            "window target is stale: expected process {expected_process_id}, current process {}",
            before.process_id
        )));
    }

    unsafe {
        SetWindowPos(
            handle.hwnd(),
            None,
            x,
            y,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )?;
    }

    Ok(before)
}

fn validate_bounds(x: i32, y: i32, width: i32, height: i32) -> ToolResult<()> {
    if width <= 0 || height <= 0 {
        return Err(ToolError::InvalidArgument(
            "window width and height must be positive".into(),
        ));
    }
    if width > MAX_WINDOW_DIMENSION || height > MAX_WINDOW_DIMENSION {
        return Err(ToolError::InvalidArgument(format!(
            "window dimensions must not exceed {MAX_WINDOW_DIMENSION} pixels"
        )));
    }

    let x = i64::from(x);
    let y = i64::from(y);
    if x.abs() > MAX_ABS_WINDOW_COORDINATE || y.abs() > MAX_ABS_WINDOW_COORDINATE {
        return Err(ToolError::InvalidArgument(format!(
            "window coordinates must stay within ±{MAX_ABS_WINDOW_COORDINATE} pixels"
        )));
    }
    Ok(())
}

unsafe extern "system" fn enum_monitor_callback(
    monitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let monitors = unsafe { &mut *(lparam.0 as *mut Vec<MonitorDescriptor>) };
    if monitors.len() >= HARD_MAX_MONITORS {
        return BOOL(1);
    }

    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        monitors.push(MonitorDescriptor {
            monitor_handle: monitor.0 as isize,
            bounds: info.rcMonitor.into(),
            work_area: info.rcWork.into(),
            primary: (info.dwFlags & MONITORINFOF_PRIMARY) != 0,
        });
    }

    BOOL(1)
}
