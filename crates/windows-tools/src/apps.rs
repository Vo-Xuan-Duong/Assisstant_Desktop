use std::mem::size_of;

use serde::Serialize;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    },
    core::{PCWSTR, w},
};

use crate::{ToolError, ToolResult};

#[derive(Debug, Clone, Serialize)]
pub struct OpenResult {
    pub target: String,
    pub launched: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunningApp {
    pub process_id: u32,
    pub executable: String,
}

struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub fn open(target: &str) -> ToolResult<OpenResult> {
    let target = target.trim();
    if target.is_empty() {
        return Err(ToolError::InvalidArgument("target cannot be empty".into()));
    }

    let wide_target: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide_target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };

    // ShellExecute returns values greater than 32 on success and an error code otherwise.
    let code = result.0 as isize;
    if code <= 32 {
        return Err(ToolError::Unsupported(format!(
            "Windows could not open target `{target}` (ShellExecute code {code})"
        )));
    }

    Ok(OpenResult {
        target: target.to_owned(),
        launched: true,
    })
}

pub fn list_running() -> ToolResult<Vec<RunningApp>> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)? };
    let _snapshot = HandleGuard(snapshot);

    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };

    unsafe { Process32FirstW(snapshot, &mut entry)? };

    let mut apps = Vec::new();
    loop {
        let end = entry
            .szExeFile
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(entry.szExeFile.len());
        let executable = String::from_utf16_lossy(&entry.szExeFile[..end]);

        apps.push(RunningApp {
            process_id: entry.th32ProcessID,
            executable,
        });

        if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
            break;
        }
    }

    apps.sort_unstable_by(|left, right| {
        left.executable
            .to_ascii_lowercase()
            .cmp(&right.executable.to_ascii_lowercase())
            .then(left.process_id.cmp(&right.process_id))
    });

    Ok(apps)
}
