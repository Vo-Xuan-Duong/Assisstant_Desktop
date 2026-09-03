use std::mem::size_of;

use serde::Serialize;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

use crate::ToolResult;

#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub computer_name: Option<String>,
    pub logical_cpus: usize,
    pub memory_total_bytes: u64,
    pub memory_available_bytes: u64,
    pub memory_load_percent: u32,
}

pub fn get_info() -> ToolResult<SystemInfo> {
    let mut memory = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };

    unsafe { GlobalMemoryStatusEx(&mut memory)? };

    Ok(SystemInfo {
        computer_name: std::env::var("COMPUTERNAME").ok(),
        logical_cpus: std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1),
        memory_total_bytes: memory.ullTotalPhys,
        memory_available_bytes: memory.ullAvailPhys,
        memory_load_percent: memory.dwMemoryLoad,
    })
}
