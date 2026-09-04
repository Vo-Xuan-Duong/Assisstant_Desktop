use serde::Serialize;
use windows::{
    core::BOOL,
    Win32::{
        Foundation::{HWND, LPARAM},
        UI::WindowsAndMessaging::{
            EnumWindows, GetForegroundWindow, IsIconic, IsWindowVisible, SetForegroundWindow,
            ShowWindow, SW_RESTORE,
        },
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
/// The returned payload is bounded so one MCP call cannot return an unbounded
/// desktop window set. Enumeration itself is allowed to complete successfully
/// even after the payload cap is reached because returning FALSE from the Win32
/// callback would make `EnumWindows` report a failed enumeration.
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

    // Keep the callback successful after reaching the payload cap. EnumWindows
    // interprets FALSE as early termination/failure, while we only want to stop
    // collecting additional records.
    if state.windows.len() >= state.max_windows {
        return BOOL(1);
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

    BOOL(1)
}
