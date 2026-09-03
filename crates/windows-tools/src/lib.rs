pub mod apps;
pub mod audio;
pub mod automation;
pub mod clipboard;
pub mod error;
pub mod media;
pub mod screen;
pub mod system;
pub mod window;

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
        name: "window_get_active",
        risk: ToolRisk::Safe,
        description: "Read metadata about the current foreground window.",
    },
    ToolDefinition {
        name: "system_get_info",
        risk: ToolRisk::Safe,
        description: "Read basic machine and memory information.",
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
}
