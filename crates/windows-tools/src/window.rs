use std::{ffi::c_void, mem::size_of};

use serde::Serialize;
use windows::{
    core::Error as WindowsError,
    Win32::{
        Foundation::HWND,
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        },
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId},
    },
};

use crate::{apps, ToolError, ToolResult};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct WindowHandle(pub isize);

impl WindowHandle {
    pub(crate) fn hwnd(self) -> HWND {
        HWND(self.0 as *mut c_void)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ActiveWindow {
    pub title: String,
    pub process_id: u32,
    pub executable: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct MonitorBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub fn get_active_handle() -> ToolResult<WindowHandle> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return Err(ToolError::NotFound("no foreground window".into()));
    }
    Ok(WindowHandle(hwnd.0 as isize))
}

pub fn get_active() -> ToolResult<ActiveWindow> {
    get(get_active_handle()?)
}

pub fn get(handle: WindowHandle) -> ToolResult<ActiveWindow> {
    unsafe {
        let hwnd = handle.hwnd();
        if hwnd.0.is_null() {
            return Err(ToolError::NotFound("window handle is empty".into()));
        }

        let mut process_id = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        if process_id == 0 {
            return Err(ToolError::NotFound("window process no longer exists".into()));
        }

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

pub fn monitor_bounds(handle: WindowHandle) -> ToolResult<MonitorBounds> {
    unsafe {
        let hwnd = handle.hwnd();
        if hwnd.0.is_null() {
            return Err(ToolError::NotFound("window handle is empty".into()));
        }

        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.0.is_null() {
            return Err(ToolError::NotFound(
                "Windows could not resolve a monitor for the window".into(),
            ));
        }

        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return Err(ToolError::Windows(WindowsError::from_win32()));
        }

        let width = info.rcMonitor.right - info.rcMonitor.left;
        let height = info.rcMonitor.bottom - info.rcMonitor.top;
        if width <= 0 || height <= 0 {
            return Err(ToolError::Unsupported(
                "resolved monitor has invalid dimensions".into(),
            ));
        }

        Ok(MonitorBounds {
            x: info.rcMonitor.left,
            y: info.rcMonitor.top,
            width: width as u32,
            height: height as u32,
        })
    }
}
