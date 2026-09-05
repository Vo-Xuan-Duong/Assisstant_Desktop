use std::process::Command;

use serde::Serialize;
use windows::Win32::System::Shutdown::LockWorkStation;

use crate::ToolResult;

#[derive(Debug, Clone, Serialize)]
pub struct PowerActionResult {
    pub action: &'static str,
    pub initiated: bool,
}

fn spawn_shutdown(action: &'static str, args: &[&str]) -> ToolResult<PowerActionResult> {
    Command::new("shutdown.exe").args(args).spawn()?;
    Ok(PowerActionResult {
        action,
        initiated: true,
    })
}

pub fn lock() -> ToolResult<PowerActionResult> {
    unsafe { LockWorkStation()? };
    Ok(PowerActionResult {
        action: "lock",
        initiated: true,
    })
}

pub fn logoff() -> ToolResult<PowerActionResult> {
    spawn_shutdown("logoff", &["/l"])
}

pub fn shutdown() -> ToolResult<PowerActionResult> {
    spawn_shutdown("shutdown", &["/s", "/t", "0"])
}

pub fn restart() -> ToolResult<PowerActionResult> {
    spawn_shutdown("restart", &["/r", "/t", "0"])
}
