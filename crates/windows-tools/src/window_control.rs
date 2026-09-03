use serde::{Deserialize, Serialize};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::WindowsAndMessaging::{
        PostMessageW, ShowWindow, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, WM_CLOSE,
    },
};

use crate::{
    window::{self, ActiveWindow, WindowHandle},
    ToolError, ToolResult,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowVisualState {
    Minimize,
    Maximize,
    Restore,
}

/// Change one explicitly identified top-level window's visual state.
///
/// `expected_process_id` protects against HWND reuse: the action is rejected if
/// the current owner of the handle no longer matches the process observed by the
/// caller when it selected the target.
pub fn set_visual_state(
    handle: WindowHandle,
    expected_process_id: u32,
    state: WindowVisualState,
) -> ToolResult<ActiveWindow> {
    let before = validate_target(handle, expected_process_id)?;
    let command = match state {
        WindowVisualState::Minimize => SW_MINIMIZE,
        WindowVisualState::Maximize => SW_MAXIMIZE,
        WindowVisualState::Restore => SW_RESTORE,
    };

    // ShowWindow's BOOL reports whether the window was previously visible; it is
    // not a conventional success/failure result, so intentionally ignore it.
    unsafe {
        let _ = ShowWindow(handle.hwnd(), command);
    }
    Ok(before)
}

/// Ask a top-level window to close using the standard WM_CLOSE contract.
///
/// This is intentionally not a force-terminate primitive. The target application
/// remains free to show an unsaved-changes dialog, cancel closing, or perform its
/// normal shutdown handling.
pub fn request_close(
    handle: WindowHandle,
    expected_process_id: u32,
) -> ToolResult<ActiveWindow> {
    let before = validate_target(handle, expected_process_id)?;
    unsafe {
        PostMessageW(Some(handle.hwnd()), WM_CLOSE, WPARAM(0), LPARAM(0))?;
    }
    Ok(before)
}

fn validate_target(handle: WindowHandle, expected_process_id: u32) -> ToolResult<ActiveWindow> {
    if expected_process_id == 0 {
        return Err(ToolError::InvalidArgument(
            "expected_process_id must not be zero".into(),
        ));
    }

    let current = window::get(handle)?;
    if current.process_id != expected_process_id {
        return Err(ToolError::NotFound(format!(
            "window target is stale: expected process {expected_process_id}, current process {}",
            current.process_id
        )));
    }
    Ok(current)
}
