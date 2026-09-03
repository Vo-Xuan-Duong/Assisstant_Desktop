pub mod apps;
pub mod audio;
pub mod clipboard;
pub mod error;
pub mod media;
pub mod system;
pub mod window;

use assistant_common::ToolRisk;

pub use error::{ToolError, ToolResult};

#[derive(Debug, Clone, Copy)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub risk: ToolRisk,
    pub description: &'static str,
}

pub const TOOL_CATALOG: &[ToolDefinition] = &[
    ToolDefinition {
        name: "audio.get_volume",
        risk: ToolRisk::Safe,
        description: "Read the default Windows output volume and mute state.",
    },
    ToolDefinition {
        name: "audio.set_volume",
        risk: ToolRisk::Moderate,
        description: "Set the default Windows output volume to a percentage.",
    },
    ToolDefinition {
        name: "audio.set_mute",
        risk: ToolRisk::Moderate,
        description: "Mute or unmute the default Windows output device.",
    },
    ToolDefinition {
        name: "apps.open",
        risk: ToolRisk::Moderate,
        description: "Open an application, document, URI, or shell target through Windows.",
    },
    ToolDefinition {
        name: "window.get_active",
        risk: ToolRisk::Safe,
        description: "Read metadata about the current foreground window.",
    },
    ToolDefinition {
        name: "system.get_info",
        risk: ToolRisk::Safe,
        description: "Read basic machine and memory information.",
    },
    ToolDefinition {
        name: "media.play_pause",
        risk: ToolRisk::Moderate,
        description: "Send the Windows media play/pause key.",
    },
    ToolDefinition {
        name: "media.next",
        risk: ToolRisk::Moderate,
        description: "Send the Windows next-track media key.",
    },
    ToolDefinition {
        name: "media.previous",
        risk: ToolRisk::Moderate,
        description: "Send the Windows previous-track media key.",
    },
    ToolDefinition {
        name: "clipboard.read_text",
        risk: ToolRisk::Moderate,
        description: "Read Unicode text from the Windows clipboard. Clipboard data may be sensitive.",
    },
    ToolDefinition {
        name: "clipboard.write_text",
        risk: ToolRisk::Moderate,
        description: "Replace Windows clipboard content with Unicode text.",
    },
];

pub fn tool_definition(name: &str) -> Option<&'static ToolDefinition> {
    TOOL_CATALOG.iter().find(|tool| tool.name == name)
}
