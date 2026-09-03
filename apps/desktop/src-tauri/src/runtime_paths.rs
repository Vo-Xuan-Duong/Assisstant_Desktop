use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use tauri::{AppHandle, Manager};

const MCP_BINARY_NAME: &str = "assistant-mcp.exe";
const MCP_SERVER_NAME: &str = "assistant-windows";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum McpBinarySource {
    Environment,
    BundledSidecar,
    DevDebug,
    DevRelease,
    ExpectedBundled,
}

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub app_local_data: PathBuf,
    pub runtime_dir: PathBuf,
    pub context_dir: PathBuf,
    pub mcp_config_path: PathBuf,
    pub mcp_binary_path: PathBuf,
    pub mcp_binary_source: McpBinarySource,
}

#[derive(Serialize)]
struct GeneratedMcpConfig<'a> {
    #[serde(rename = "mcpServers")]
    mcp_servers: std::collections::BTreeMap<&'a str, GeneratedMcpServer>,
}

#[derive(Serialize)]
struct GeneratedMcpServer {
    command: String,
    cwd: String,
    env: std::collections::BTreeMap<String, String>,
}

impl RuntimePaths {
    pub fn prepare(app: &AppHandle) -> Result<Self, String> {
        let app_local_data = app
            .path()
            .app_local_data_dir()
            .map_err(|error| format!("cannot resolve app local data directory: {error}"))?;

        let runtime_dir = match env::var_os("ASSISTANT_RUNTIME_DIR").map(PathBuf::from) {
            Some(path) => require_absolute("ASSISTANT_RUNTIME_DIR", path)?,
            None => app_local_data.join("runtime"),
        };
        let context_dir = app_local_data.join("context");
        let mcp_config_path = runtime_dir.join(".agents").join("mcp_config.json");

        fs::create_dir_all(&runtime_dir)
            .map_err(|error| format!("cannot create runtime directory: {error}"))?;
        fs::create_dir_all(&context_dir)
            .map_err(|error| format!("cannot create context directory: {error}"))?;

        let (mcp_binary_path, mcp_binary_source) = resolve_mcp_binary(app)?;
        write_mcp_config(&mcp_config_path, &runtime_dir, &mcp_binary_path)?;

        Ok(Self {
            app_local_data,
            runtime_dir,
            context_dir,
            mcp_config_path,
            mcp_binary_path,
            mcp_binary_source,
        })
    }
}

fn resolve_mcp_binary(app: &AppHandle) -> Result<(PathBuf, McpBinarySource), String> {
    if let Some(path) = env::var_os("ASSISTANT_MCP_BINARY").map(PathBuf::from) {
        return Ok((
            require_absolute("ASSISTANT_MCP_BINARY", path)?,
            McpBinarySource::Environment,
        ));
    }

    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| format!("cannot resolve Tauri resource directory: {error}"))?;
    let bundled = resource_dir.join(MCP_BINARY_NAME);
    if bundled.is_file() {
        return Ok((bundled, McpBinarySource::BundledSidecar));
    }

    #[cfg(debug_assertions)]
    {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let debug = workspace.join("target").join("debug").join(MCP_BINARY_NAME);
        if debug.is_file() {
            return Ok((debug, McpBinarySource::DevDebug));
        }

        let release = workspace.join("target").join("release").join(MCP_BINARY_NAME);
        if release.is_file() {
            return Ok((release, McpBinarySource::DevRelease));
        }
    }

    Ok((bundled, McpBinarySource::ExpectedBundled))
}

fn require_absolute(name: &str, path: PathBuf) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(format!(
            "{name} must be an absolute path so runtime behavior never depends on the process working directory"
        ))
    }
}

fn write_mcp_config(config_path: &Path, runtime_dir: &Path, binary: &Path) -> Result<(), String> {
    let parent = config_path
        .parent()
        .ok_or_else(|| "MCP config path has no parent directory".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create MCP config directory: {error}"))?;

    let mut servers = std::collections::BTreeMap::new();
    let mut server_env = std::collections::BTreeMap::new();
    server_env.insert("RUST_LOG".to_owned(), "info".to_owned());
    servers.insert(
        MCP_SERVER_NAME,
        GeneratedMcpServer {
            command: binary.to_string_lossy().into_owned(),
            cwd: runtime_dir.to_string_lossy().into_owned(),
            env: server_env,
        },
    );

    let bytes = serde_json::to_vec_pretty(&GeneratedMcpConfig {
        mcp_servers: servers,
    })
    .map_err(|error| format!("cannot serialize generated MCP config: {error}"))?;

    fs::write(config_path, bytes)
        .map_err(|error| format!("cannot write generated MCP config: {error}"))
}
