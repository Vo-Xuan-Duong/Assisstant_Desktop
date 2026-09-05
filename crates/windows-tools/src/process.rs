use serde::Serialize;
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess},
};

use crate::{ToolError, ToolResult, apps};

#[derive(Debug, Clone, Serialize)]
pub struct ProcessTerminateResult {
    pub process_id: u32,
    pub executable: String,
    pub terminated: bool,
}

struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub fn terminate(process_id: u32, expected_executable: &str) -> ToolResult<ProcessTerminateResult> {
    if process_id == 0 {
        return Err(ToolError::InvalidArgument(
            "process_id must be a non-zero Windows process id".into(),
        ));
    }
    if process_id == std::process::id() {
        return Err(ToolError::Unsupported(
            "the assistant process cannot terminate itself".into(),
        ));
    }

    let expected = expected_executable.trim();
    if expected.is_empty() {
        return Err(ToolError::InvalidArgument(
            "expected_executable is required to guard against stale process ids".into(),
        ));
    }

    let actual = apps::executable_for_process(process_id)?
        .ok_or_else(|| ToolError::NotFound(format!("process {process_id} no longer exists")))?;
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(ToolError::Unsupported(format!(
            "process {process_id} is now `{actual}`, expected `{expected}`; refusing stale target"
        )));
    }

    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, false, process_id)? };
    let _handle = HandleGuard(handle);
    unsafe { TerminateProcess(handle, 1)? };

    Ok(ProcessTerminateResult {
        process_id,
        executable: actual,
        terminated: true,
    })
}
