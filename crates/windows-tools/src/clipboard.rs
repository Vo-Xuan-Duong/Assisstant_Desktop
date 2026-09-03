use std::{ptr, slice};

use serde::Serialize;
use windows::Win32::{
    Foundation::{GlobalFree, HANDLE, HGLOBAL},
    System::{
        DataExchange::{CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData},
        Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
        Ole::CF_UNICODETEXT,
    },
};

use crate::{ToolError, ToolResult};

struct ClipboardGuard;

impl ClipboardGuard {
    fn open() -> ToolResult<Self> {
        unsafe { OpenClipboard(None)? };
        Ok(Self)
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ClipboardText {
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClipboardWriteResult {
    pub characters: usize,
}

pub fn read_text() -> ToolResult<ClipboardText> {
    let _clipboard = ClipboardGuard::open()?;

    unsafe {
        let handle = GetClipboardData(CF_UNICODETEXT.0 as u32)?;
        let memory = HGLOBAL(handle.0);
        let pointer = GlobalLock(memory) as *const u16;
        if pointer.is_null() {
            return Err(ToolError::Unsupported(
                "clipboard Unicode text could not be locked".into(),
            ));
        }

        let mut length = 0usize;
        while *pointer.add(length) != 0 {
            length += 1;
        }

        let text = String::from_utf16_lossy(slice::from_raw_parts(pointer, length));
        let _ = GlobalUnlock(memory);

        Ok(ClipboardText { text })
    }
}

pub fn write_text(text: &str) -> ToolResult<ClipboardWriteResult> {
    let encoded: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = encoded.len() * size_of::<u16>();

    let _clipboard = ClipboardGuard::open()?;

    unsafe {
        EmptyClipboard()?;

        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes)?;
        let destination = GlobalLock(memory) as *mut u16;
        if destination.is_null() {
            let _ = GlobalFree(Some(memory));
            return Err(ToolError::Unsupported(
                "allocated clipboard memory could not be locked".into(),
            ));
        }

        ptr::copy_nonoverlapping(encoded.as_ptr(), destination, encoded.len());
        let _ = GlobalUnlock(memory);

        let transfer = SetClipboardData(
            CF_UNICODETEXT.0 as u32,
            Some(HANDLE(memory.0)),
        );

        match transfer {
            Ok(_) => Ok(ClipboardWriteResult {
                characters: text.chars().count(),
            }),
            Err(error) => {
                let _ = GlobalFree(Some(memory));
                Err(ToolError::Windows(error))
            }
        }
    }
}

const fn size_of<T>() -> usize {
    std::mem::size_of::<T>()
}
