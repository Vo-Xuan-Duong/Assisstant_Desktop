use serde::Serialize;
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::WindowsAndMessaging::{HWND_BROADCAST, PostMessageW, SC_MONITORPOWER, WM_SYSCOMMAND},
};

use crate::ToolResult;

#[derive(Debug, Clone, Serialize)]
pub struct DisplayPowerResult {
    pub action: &'static str,
    pub initiated: bool,
}

pub fn turn_off() -> ToolResult<DisplayPowerResult> {
    unsafe {
        PostMessageW(
            Some(HWND_BROADCAST),
            WM_SYSCOMMAND,
            WPARAM(SC_MONITORPOWER as usize),
            LPARAM(2),
        )?;
    }
    Ok(DisplayPowerResult {
        action: "turn_off",
        initiated: true,
    })
}
