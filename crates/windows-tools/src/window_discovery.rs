use serde::Serialize;
use windows::Win32::{
    Foundation::{BOOL, HWND, LPARAM},
    UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, IsIconic, IsWindowVisible, SetForegroundWindow,
        ShowWindow, SW_RESTORE,
    },
};

use crate::{
    window::{self, ActiveWindow, WindowHandle},
    ToolError, ToolResult,
};

const DEFAULT_MAX_WINDOWS: usize = 80;
const HARD_MAX_WINDOWS: usize = 200;

#[derive(Debug, Clone, Serialize)]
pub struct TopLevelWindow {
    pub window_handle: isize,
    pub title: String,
    pub process_id: u32,
    pub executable: Option<String>,
    pub minimized: bool,
    pub foreground: bool,
}

struct EnumerationState {
    windows: Vec<TopLevelWindow>,
    max_windows: usize,
    foreground: HWND,
}

/// Enumerate visible top-level application windows with non-empty titles.
///
/// Enumeration is bounded so one MCP call cannot return an unbounded desktop
/// window set. The returned HWND + process_id pair can be reused by the explicit
/// window mutation tools, which revalidate ownership immediately before acting.
pub fn list_top_level(max_windows: Option<usize>) -> ToolResult<Vec<TopLevelWindow>> {
    let max_windows = max_windows
        .unwrap_or(DEFAULT_MAX_WINDOWS)
        .clamp(1, HARD_MAX_WINDOWS);

    let mut state = EnumerationState {
        windows: Vec::with_capacity(max_windows.min(32)),
        max_windows,
        foreground: unsafe { GetForegroundWindow() },
    };

    unsafe {
        EnumWindows(
            Some(enum_window_callback),
            LPARAM((&mut state as *mut EnumerationState) as isize),
        )?;
    }

    Ok(state.windows)
}

/// Restore (when minimized) and request foreground activation of one explicit
/// HWND/process pair.
///
/// Windows can legitimately refuse `SetForegroundWindow` because foreground
/// activation is governed by OS focus-stealing rules. That refusal is surfaced
/// to the caller rather than falling back to synthetic keyboard/mouse input.
pub fn activate(
    handle: WindowHandle,
    expected_process_id: u32,
) -> ToolResult<ActiveWindow> {
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

    let hwnd = handle.hwnd();
    if unsafe { IsIconic(hwnd) }.as_bool() {
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
    }

    if !unsafe { SetForegroundWindow(hwnd) }.as_bool() {
        return Err(ToolError::Unsupported(
            "Windows refused foreground activation for this window".into(),
        ));
    }

    Ok(before)
}

unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let state = unsafe { &mut *(lparam.0 as *mut EnumerationState) };
    if state.windows.len() >= state.max_windows {
        return BOOL(0);
    }

    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return BOOL(1);
    }

    let handle = WindowHandle(hwnd.0 as isize);
    let Ok(metadata) = window::get(handle) else {
        return BOOL(1);
    };

    if metadata.title.trim().is_empty() {
        return BOOL(1);
    }

    state.windows.push(TopLevelWindow {
        window_handle: handle.0,
        title: metadata.title,
        process_id: metadata.process_id,
        executable: metadata.executable,
        minimized: unsafe { IsIconic(hwnd) }.as_bool(),
        foreground: hwnd == state.foreground,
    });

    if state.windows.len() >= state.max_windows {
        BOOL(0)
    } else {
        BOOL(1)
    }
}
