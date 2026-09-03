use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::json;
use windows_tools::{apps, audio, clipboard, media, system, window};

use super::{to_json, tool_error, WindowsMcpServer};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetVolumeInput {
    /// Desired master output volume in the inclusive range 0..=100.
    pub percent: f32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetMuteInput {
    /// True to mute the default output device, false to unmute it.
    pub muted: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct OpenAppInput {
    /// Windows Shell target such as `chrome`, `notepad`, a file path, or an https URI.
    pub target: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ClipboardWriteInput {
    /// Unicode text that will replace the current clipboard content.
    pub text: String,
}

#[tool_router(router = system_tool_router, vis = "pub(crate)")]
impl WindowsMcpServer {
    #[tool(
        name = "audio_get_volume",
        description = "Read the current master volume percentage and mute state of the default Windows output device. This is read-only."
    )]
    async fn audio_get_volume(&self) -> Result<String, String> {
        self.permissions.authorize("audio_get_volume", json!({})).await?;
        audio::get_state()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "audio_set_volume",
        description = "Set the master volume percentage of the default Windows output device. The percent must be between 0 and 100."
    )]
    async fn audio_set_volume(
        &self,
        Parameters(SetVolumeInput { percent }): Parameters<SetVolumeInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize("audio_set_volume", json!({ "percent": percent }))
            .await?;
        audio::set_volume(percent)
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "audio_set_mute",
        description = "Mute or unmute the default Windows output device."
    )]
    async fn audio_set_mute(
        &self,
        Parameters(SetMuteInput { muted }): Parameters<SetMuteInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize("audio_set_mute", json!({ "muted": muted }))
            .await?;
        audio::set_mute(muted)
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "apps_open",
        description = "Open an application, document, file path, or URI through the Windows Shell. Do not use this as an arbitrary shell-command executor."
    )]
    async fn apps_open(
        &self,
        Parameters(OpenAppInput { target }): Parameters<OpenAppInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize("apps_open", json!({ "target": &target }))
            .await?;
        apps::open(&target)
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "apps_list",
        description = "List currently running Windows process executables and process ids. This is read-only and does not terminate or modify processes."
    )]
    async fn apps_list(&self) -> Result<String, String> {
        self.permissions.authorize("apps_list", json!({})).await?;
        apps::list_running()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "window_get_active",
        description = "Read the title and process id of the current Windows foreground window. This is read-only."
    )]
    async fn window_get_active(&self) -> Result<String, String> {
        self.permissions
            .authorize("window_get_active", json!({}))
            .await?;
        window::get_active()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "system_get_info",
        description = "Read basic Windows machine information including logical CPU count and physical-memory usage. This is read-only."
    )]
    async fn system_get_info(&self) -> Result<String, String> {
        self.permissions
            .authorize("system_get_info", json!({}))
            .await?;
        system::get_info()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "media_play_pause",
        description = "Send the Windows media play/pause key to the active media session."
    )]
    async fn media_play_pause(&self) -> Result<String, String> {
        self.permissions
            .authorize("media_play_pause", json!({}))
            .await?;
        media::play_pause()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "media_next",
        description = "Send the Windows next-track media key to the active media session."
    )]
    async fn media_next(&self) -> Result<String, String> {
        self.permissions.authorize("media_next", json!({})).await?;
        media::next()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "media_previous",
        description = "Send the Windows previous-track media key to the active media session."
    )]
    async fn media_previous(&self) -> Result<String, String> {
        self.permissions
            .authorize("media_previous", json!({}))
            .await?;
        media::previous()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "clipboard_read_text",
        description = "Read Unicode text from the Windows clipboard. Clipboard content can contain sensitive information, so call this only when it is relevant to the user's request."
    )]
    async fn clipboard_read_text(&self) -> Result<String, String> {
        self.permissions
            .authorize("clipboard_read_text", json!({}))
            .await?;
        clipboard::read_text()
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }

    #[tool(
        name = "clipboard_write_text",
        description = "Replace the Windows clipboard contents with the supplied Unicode text."
    )]
    async fn clipboard_write_text(
        &self,
        Parameters(ClipboardWriteInput { text }): Parameters<ClipboardWriteInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize("clipboard_write_text", json!({ "text": &text }))
            .await?;
        clipboard::write_text(&text)
            .map_err(tool_error)
            .and_then(|value| to_json(&value))
    }
}
