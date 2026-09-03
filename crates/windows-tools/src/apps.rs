use serde::Serialize;
use windows::{
    Win32::{UI::Shell::ShellExecuteW, UI::WindowsAndMessaging::SW_SHOWNORMAL},
    core::{HSTRING, PCWSTR, w},
};

use crate::{ToolError, ToolResult};

#[derive(Debug, Clone, Serialize)]
pub struct OpenResult {
    pub target: String,
    pub launched: bool,
}

pub fn open(target: &str) -> ToolResult<OpenResult> {
    let target = target.trim();
    if target.is_empty() {
        return Err(ToolError::InvalidArgument("target cannot be empty".into()));
    }

    let wide_target = HSTRING::from(target);
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &wide_target,
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
