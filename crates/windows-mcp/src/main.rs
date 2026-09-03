mod server;

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::server::WindowsMcpServer;

#[tokio::main]
async fn main() -> Result<()> {
    // MCP owns stdout. Keep all diagnostics on stderr so protocol frames are never polluted.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    info!("starting Assisstant Desktop Windows MCP server");

    let service = WindowsMcpServer::default().serve(stdio()).await?;
    let reason = service.waiting().await?;

    info!(?reason, "Windows MCP server stopped");
    Ok(())
}
