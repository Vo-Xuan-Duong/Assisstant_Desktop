pub mod apps;
pub mod audio;
pub mod automation;
pub mod clipboard;
pub mod display_control;
pub mod error;
pub mod files;
pub mod input;
pub mod media;
pub mod monitor_layout;
pub mod power;
pub mod process;
pub mod screen;
pub mod system;
pub mod virtualized;
pub mod window;
pub mod window_control;
pub mod window_discovery;

use assistant_common::ToolRisk;

pub use error::{ToolError, ToolResult};

#[derive(Debug, Clone, Copy)]
pub struct ToolDefinition {
    /// Exact public MCP tool name used by Antigravity.
    pub name: &'static str,
    pub risk: ToolRisk,
    pub description: &'static str,
}

pub const TOOL_CATALOG: &[ToolDefinition] = &[
    ToolDefinition {
        name: "audio_get_volume",
        risk: ToolRisk::Safe,
        description: "Read the default Windows output volume and mute state.",
    },
    ToolDefinition {
        name: "audio_set_volume",
        risk: ToolRisk::Moderate,
        description: "Set the default Windows output volume to a percentage.",
    },
    ToolDefinition {
        name: "audio_set_mute",
        risk: ToolRisk::Moderate,
        description: "Mute or unmute the default Windows output device.",
    },
    ToolDefinition {
        name: "apps_open",
        risk: ToolRisk::Moderate,
        description: "Open an application, document, URI, or shell target through Windows.",
    },
    ToolDefinition {
        name: "apps_list",
        risk: ToolRisk::Safe,
        description: "List running process executables and process ids.",
    },
    ToolDefinition {
        name: "display_list",
        risk: ToolRisk::Safe,
        description: "List monitor bounds and work areas exposed by Windows.",
    },
    ToolDefinition {
        name: "display_turn_off",
        risk: ToolRisk::Moderate,
        description: "Request that Windows power off attached displays until the next user/input wake event.",
    },
    ToolDefinition {
        name: "window_get_active",
        risk: ToolRisk::Safe,
        description: "Read metadata about the current foreground window.",
    },
    ToolDefinition {
        name: "window_list",
        risk: ToolRisk::Safe,
        description: "List visible titled top-level Windows windows with HWND, process id and state metadata.",
    },
    ToolDefinition {
        name: "window_activate",
        risk: ToolRisk::Moderate,
        description: "Restore if needed and request foreground activation of an explicitly identified top-level Windows window.",
    },
    ToolDefinition {
        name: "window_set_bounds",
        risk: ToolRisk::Moderate,
        description: "Move and resize an explicitly identified top-level Windows window while preserving focus and Z-order.",
    },
    ToolDefinition {
        name: "window_set_state",
        risk: ToolRisk::Moderate,
        description: "Minimize, maximize, or restore an explicitly identified top-level Windows window.",
    },
    ToolDefinition {
        name: "window_close",
        risk: ToolRisk::Sensitive,
        description: "Request graceful close of an explicitly identified top-level window through WM_CLOSE.",
    },
    ToolDefinition {
        name: "system_get_info",
        risk: ToolRisk::Safe,
        description: "Read basic machine and memory information.",
    },
    ToolDefinition {
        name: "system_lock",
        risk: ToolRisk::Moderate,
        description: "Lock the current interactive Windows workstation.",
    },
    ToolDefinition {
        name: "system_logoff",
        risk: ToolRisk::Sensitive,
        description: "Log off the current Windows user session.",
    },
    ToolDefinition {
        name: "system_shutdown",
        risk: ToolRisk::Sensitive,
        description: "Request an immediate Windows shutdown.",
    },
    ToolDefinition {
        name: "system_restart",
        risk: ToolRisk::Sensitive,
        description: "Request an immediate Windows restart.",
    },
    ToolDefinition {
        name: "process_terminate",
        risk: ToolRisk::Sensitive,
        description: "Terminate one explicit process id after verifying its expected executable name.",
    },
    ToolDefinition {
        name: "media_play_pause",
        risk: ToolRisk::Moderate,
        description: "Send the Windows media play/pause key.",
    },
    ToolDefinition {
        name: "media_next",
        risk: ToolRisk::Moderate,
        description: "Send the Windows next-track media key.",
    },
    ToolDefinition {
        name: "media_previous",
        risk: ToolRisk::Moderate,
        description: "Send the Windows previous-track media key.",
    },
    ToolDefinition {
        name: "input_send_hotkey",
        risk: ToolRisk::Sensitive,
        description: "Send one bounded explicit keyboard shortcut to the active Windows application.",
    },
    ToolDefinition {
        name: "input_type_text",
        risk: ToolRisk::Sensitive,
        description: "Type bounded Unicode text into the currently focused Windows control using SendInput.",
    },
    ToolDefinition {
        name: "clipboard_read_text",
        risk: ToolRisk::Moderate,
        description: "Read Unicode text from the Windows clipboard. Clipboard data may be sensitive.",
    },
    ToolDefinition {
        name: "clipboard_write_text",
        risk: ToolRisk::Moderate,
        description: "Replace Windows clipboard content with Unicode text.",
    },
    ToolDefinition {
        name: "file_info",
        risk: ToolRisk::Moderate,
        description: "Read metadata for one explicit absolute filesystem path.",
    },
    ToolDefinition {
        name: "file_list",
        risk: ToolRisk::Moderate,
        description: "List a bounded number of entries from one explicit absolute directory path.",
    },
    ToolDefinition {
        name: "file_create_directory",
        risk: ToolRisk::Sensitive,
        description: "Create a new directory tree at an explicit absolute path; existing paths are rejected.",
    },
    ToolDefinition {
        name: "file_copy",
        risk: ToolRisk::Sensitive,
        description: "Copy one regular file to a new explicit absolute path without overwrite.",
    },
    ToolDefinition {
        name: "file_move",
        risk: ToolRisk::Sensitive,
        description: "Move or rename one explicit file/directory to a new absolute path without overwrite.",
    },
    ToolDefinition {
        name: "file_delete",
        risk: ToolRisk::Sensitive,
        description: "Delete one explicit regular file or empty directory; recursive deletion is not exposed.",
    },
    ToolDefinition {
        name: "ui_inspect",
        risk: ToolRisk::Safe,
        description: "Inspect structural Windows UI Automation metadata without reading editable values.",
    },
    ToolDefinition {
        name: "ui_focus",
        risk: ToolRisk::Moderate,
        description: "Move keyboard focus to an explicitly inspected Windows UI Automation element.",
    },
    ToolDefinition {
        name: "ui_invoke",
        risk: ToolRisk::Sensitive,
        description: "Invoke an explicitly inspected Windows UI Automation control. The semantic action can be consequential.",
    },
    ToolDefinition {
        name: "ui_set_value",
        risk: ToolRisk::Sensitive,
        description: "Write text/value into an explicitly inspected Windows UI Automation control.",
    },
    ToolDefinition {
        name: "ui_set_range_value",
        risk: ToolRisk::Sensitive,
        description: "Set a bounded numeric value on an explicitly inspected Windows UI Automation RangeValue control.",
    },
    ToolDefinition {
        name: "ui_toggle",
        risk: ToolRisk::Sensitive,
        description: "Toggle an explicitly inspected UI Automation checkbox or switch. The semantic setting change can be consequential.",
    },
    ToolDefinition {
        name: "ui_select",
        risk: ToolRisk::Sensitive,
        description: "Select an explicitly inspected UI Automation item. Selection can alter application state or workflow choices.",
    },
    ToolDefinition {
        name: "ui_set_expanded",
        risk: ToolRisk::Moderate,
        description: "Expand or collapse an explicitly inspected UI Automation control.",
    },
    ToolDefinition {
        name: "ui_scroll",
        risk: ToolRisk::Moderate,
        description: "Scroll an explicitly inspected UI Automation container by a bounded relative amount.",
    },
    ToolDefinition {
        name: "ui_scroll_into_view",
        risk: ToolRisk::Moderate,
        description: "Ask an explicitly inspected UI Automation ScrollItem element to move into its owning viewport.",
    },
    ToolDefinition {
        name: "ui_virtualized_item_status",
        risk: ToolRisk::Safe,
        description: "Check whether an explicitly inspected UI Automation element exposes VirtualizedItemPattern.",
    },
    ToolDefinition {
        name: "ui_realize",
        risk: ToolRisk::Moderate,
        description: "Ask an explicitly inspected UI Automation VirtualizedItem element to materialize through its provider.",
    },
];

pub fn tool_definition(name: &str) -> Option<&'static ToolDefinition> {
    TOOL_CATALOG.iter().find(|tool| tool.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_tool_names_are_unique() {
        for (index, tool) in TOOL_CATALOG.iter().enumerate() {
            assert!(
                !TOOL_CATALOG[..index]
                    .iter()
                    .any(|existing| existing.name == tool.name),
                "duplicate public tool name: {}",
                tool.name
            );
        }
    }

    #[test]
    fn catalogue_lookup_uses_public_mcp_names() {
        let tool = tool_definition("audio_set_volume").expect("tool must exist");
        assert_eq!(tool.risk, ToolRisk::Moderate);
    }

    #[test]
    fn system_control_mutations_are_not_safe() {
        for name in [
            "system_shutdown",
            "system_restart",
            "process_terminate",
            "file_delete",
            "input_type_text",
        ] {
            let tool = tool_definition(name).expect("tool must exist");
            assert_ne!(tool.risk, ToolRisk::Safe, "{name} must never be Safe");
        }
    }
}
