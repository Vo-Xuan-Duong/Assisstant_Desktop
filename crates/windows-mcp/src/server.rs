mod system_tools;
mod ui_tools;

use rmcp::{
    handler::server::router::tool::ToolRouter,
    tool_handler,
    ServerHandler,
};
use serde::Serialize;
use windows_tools::ToolError;

use crate::permissions::McpPermissionGateway;

#[derive(Clone)]
pub struct WindowsMcpServer {
    pub(crate) permissions: McpPermissionGateway,
    tool_router: ToolRouter<Self>,
}

impl Default for WindowsMcpServer {
    fn default() -> Self {
        Self {
            permissions: McpPermissionGateway::default(),
            tool_router: Self::system_tool_router() + Self::ui_tool_router(),
        }
    }
}

pub(crate) fn to_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|error| format!("failed to serialize tool result: {error}"))
}

pub(crate) fn tool_error(error: ToolError) -> String {
    error.to_string()
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for WindowsMcpServer {}
