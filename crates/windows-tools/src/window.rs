use serde::Serialize;
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};

use crate::{apps, ToolError, ToolResult};

#[derive(Debug, Clone, Serialize)]
pub struct ActiveWindow {
    pub title: String,
    pub process_id: u32,
    pub executable: Option<String>,
}

pub fn get_active() -> ToolResult<ActiveWindow> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == Default::default() {
            return Err(ToolError::NotFound("no foreground window".into()));
        }

        let mut process_id = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));

        let mut buffer = vec![0u16; 2048];
        let length = GetWindowTextW(hwnd, &mut buffer);
        let title = if length > 0 {
            String::from_utf16_lossy(&buffer[..length as usize])
        } else {
            String::new()
        };

        let executable = apps::executable_for_process(process_id)?;

        Ok(ActiveWindow {
            title,
            process_id,
            executable,
        })
    }
}
