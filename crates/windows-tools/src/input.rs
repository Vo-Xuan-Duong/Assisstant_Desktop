use std::{collections::HashSet, mem::size_of};

use serde::Serialize;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput,
    VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT,
    VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
};

use crate::{ToolError, ToolResult};

const MAX_HOTKEY_KEYS: usize = 5;
const MAX_TYPED_UTF16_UNITS: usize = 4000;

#[derive(Debug, Clone, Serialize)]
pub struct InputActionResult {
    pub action: &'static str,
    pub event_count: usize,
}

fn key_input(key: VIRTUAL_KEY, flags: DefaultKeyFlags) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                dwFlags: flags.into(),
                ..Default::default()
            },
        },
    }
}

#[derive(Debug, Clone, Copy)]
enum DefaultKeyFlags {
    Down,
    Up,
}

impl From<DefaultKeyFlags> for windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS {
    fn from(value: DefaultKeyFlags) -> Self {
        match value {
            DefaultKeyFlags::Down => Default::default(),
            DefaultKeyFlags::Up => KEYEVENTF_KEYUP,
        }
    }
}

fn parse_key(raw: &str) -> ToolResult<VIRTUAL_KEY> {
    let key = raw.trim().to_ascii_lowercase();
    let value = match key.as_str() {
        "ctrl" | "control" => VK_CONTROL,
        "shift" => VK_SHIFT,
        "alt" => VK_MENU,
        "win" | "windows" | "super" => VK_LWIN,
        "enter" | "return" => VK_RETURN,
        "tab" => VK_TAB,
        "esc" | "escape" => VK_ESCAPE,
        "space" => VK_SPACE,
        "backspace" => VK_BACK,
        "delete" | "del" => VK_DELETE,
        "home" => VK_HOME,
        "end" => VK_END,
        "pageup" | "page_up" => VK_PRIOR,
        "pagedown" | "page_down" => VK_NEXT,
        "left" => VK_LEFT,
        "right" => VK_RIGHT,
        "up" => VK_UP,
        "down" => VK_DOWN,
        _ if key.len() == 1 => {
            let byte = key.as_bytes()[0];
            if byte.is_ascii_alphabetic() {
                VIRTUAL_KEY(byte.to_ascii_uppercase() as u16)
            } else if byte.is_ascii_digit() {
                VIRTUAL_KEY(byte as u16)
            } else {
                return Err(ToolError::InvalidArgument(format!(
                    "unsupported hotkey key `{raw}`"
                )));
            }
        }
        _ if key.starts_with('f') => {
            let number = key[1..].parse::<u16>().ok();
            match number {
                Some(number @ 1..=12) => VIRTUAL_KEY(0x70 + number - 1),
                _ => {
                    return Err(ToolError::InvalidArgument(format!(
                        "unsupported function key `{raw}`; only F1..F12 are allowed"
                    )));
                }
            }
        }
        _ => {
            return Err(ToolError::InvalidArgument(format!(
                "unsupported hotkey key `{raw}`"
            )));
        }
    };
    Ok(value)
}

pub fn send_hotkey(keys: &[String]) -> ToolResult<InputActionResult> {
    if keys.is_empty() || keys.len() > MAX_HOTKEY_KEYS {
        return Err(ToolError::InvalidArgument(format!(
            "hotkey must contain between 1 and {MAX_HOTKEY_KEYS} keys"
        )));
    }

    let mut parsed = Vec::with_capacity(keys.len());
    let mut seen = HashSet::new();
    for key in keys {
        let virtual_key = parse_key(key)?;
        if !seen.insert(virtual_key.0) {
            return Err(ToolError::InvalidArgument(format!(
                "hotkey contains duplicate key `{key}`"
            )));
        }
        parsed.push(virtual_key);
    }

    let mut inputs = Vec::with_capacity(parsed.len() * 2);
    for key in &parsed {
        inputs.push(key_input(*key, DefaultKeyFlags::Down));
    }
    for key in parsed.iter().rev() {
        inputs.push(key_input(*key, DefaultKeyFlags::Up));
    }

    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) } as usize;
    if sent != inputs.len() {
        return Err(ToolError::Unsupported(format!(
            "Windows accepted {sent} of {} keyboard input events",
            inputs.len()
        )));
    }

    Ok(InputActionResult {
        action: "send_hotkey",
        event_count: sent,
    })
}

pub fn type_text(text: &str) -> ToolResult<InputActionResult> {
    if text.is_empty() {
        return Err(ToolError::InvalidArgument("text cannot be empty".into()));
    }
    let units = text.encode_utf16().collect::<Vec<_>>();
    if units.len() > MAX_TYPED_UTF16_UNITS {
        return Err(ToolError::InvalidArgument(format!(
            "text exceeds the {MAX_TYPED_UTF16_UNITS} UTF-16 unit input limit"
        )));
    }

    let mut inputs = Vec::with_capacity(units.len() * 2);
    for unit in units {
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wScan: unit,
                    dwFlags: KEYEVENTF_UNICODE,
                    ..Default::default()
                },
            },
        });
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wScan: unit,
                    dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    ..Default::default()
                },
            },
        });
    }

    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) } as usize;
    if sent != inputs.len() {
        return Err(ToolError::Unsupported(format!(
            "Windows accepted {sent} of {} Unicode keyboard events",
            inputs.len()
        )));
    }

    Ok(InputActionResult {
        action: "type_text",
        event_count: sent,
    })
}
