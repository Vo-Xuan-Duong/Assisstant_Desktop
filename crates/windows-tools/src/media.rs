use std::mem::size_of;

use serde::Serialize;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY,
    VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK,
};

use crate::{ToolError, ToolResult};

#[derive(Debug, Clone, Serialize)]
pub struct MediaActionResult {
    pub action: &'static str,
    pub sent: bool,
}

fn send_key(key: VIRTUAL_KEY, action: &'static str) -> ToolResult<MediaActionResult> {
    let down = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                ..Default::default()
            },
        },
    };
    let up = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                dwFlags: KEYEVENTF_KEYUP,
                ..Default::default()
            },
        },
    };

    let sent = unsafe { SendInput(&[down, up], size_of::<INPUT>() as i32) };
    if sent != 2 {
        return Err(ToolError::Unsupported(format!(
            "Windows accepted {sent} of 2 media key input events"
        )));
    }

    Ok(MediaActionResult { action, sent: true })
}

pub fn play_pause() -> ToolResult<MediaActionResult> {
    send_key(VK_MEDIA_PLAY_PAUSE, "play_pause")
}

pub fn next() -> ToolResult<MediaActionResult> {
    send_key(VK_MEDIA_NEXT_TRACK, "next")
}

pub fn previous() -> ToolResult<MediaActionResult> {
    send_key(VK_MEDIA_PREV_TRACK, "previous")
}
