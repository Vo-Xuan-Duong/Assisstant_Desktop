use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::json;
use windows_tools::{display_control, files, input, power, process};

use super::{to_json, tool_error, WindowsMcpServer};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ProcessTerminateInput {
    /// Exact process id discovered from apps_list/window_list.
    pub process_id: u32,
    /// Executable name observed for that process id, used to reject stale/reused ids.
    pub expected_executable: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FilePathInput {
    /// Explicit absolute Windows filesystem path.
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FileListInput {
    /// Explicit absolute directory path.
    pub directory: String,
    /// Optional bounded result count. Native code defaults to 100 and caps at 500.
    pub max_entries: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FileTransferInput {
    /// Explicit absolute source path.
    pub source: String,
    /// Explicit absolute destination path. Existing destinations are rejected.
    pub destination: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct HotkeyInput {
    /// Ordered shortcut keys, for example ["ctrl", "shift", "s"]. Maximum 5 keys.
    pub keys: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TypeTextInput {
    /// Unicode text to type into the currently focused control. Native code bounds the size.
    pub text: String,
}

async fn run_blocking<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| format!("Windows system-control worker failed: {error}"))?
}

#[tool_router(router = system_control_tool_router, vis = "pub(crate)")]
impl WindowsMcpServer {
    #[tool(
        name = "system_lock",
        description = "Lock the current interactive Windows workstation. This changes the session state and requires authorization."
    )]
    async fn system_lock(&self) -> Result<String, String> {
        self.permissions.authorize("system_lock", json!({})).await?;
        let value = run_blocking(|| power::lock().map_err(tool_error)).await?;
        to_json(&value)
    }

    #[tool(
        name = "system_logoff",
        description = "Log off the current Windows user session immediately. Sensitive action; unsaved work may be lost and desktop confirmation is required."
    )]
    async fn system_logoff(&self) -> Result<String, String> {
        self.permissions.authorize("system_logoff", json!({})).await?;
        let value = run_blocking(|| power::logoff().map_err(tool_error)).await?;
        to_json(&value)
    }

    #[tool(
        name = "system_shutdown",
        description = "Request immediate Windows shutdown. Sensitive action; always requires desktop confirmation."
    )]
    async fn system_shutdown(&self) -> Result<String, String> {
        self.permissions.authorize("system_shutdown", json!({})).await?;
        let value = run_blocking(|| power::shutdown().map_err(tool_error)).await?;
        to_json(&value)
    }

    #[tool(
        name = "system_restart",
        description = "Request immediate Windows restart. Sensitive action; always requires desktop confirmation."
    )]
    async fn system_restart(&self) -> Result<String, String> {
        self.permissions.authorize("system_restart", json!({})).await?;
        let value = run_blocking(|| power::restart().map_err(tool_error)).await?;
        to_json(&value)
    }

    #[tool(
        name = "display_turn_off",
        description = "Ask Windows to power off attached displays. A later user/input event normally wakes them again."
    )]
    async fn display_turn_off(&self) -> Result<String, String> {
        self.permissions.authorize("display_turn_off", json!({})).await?;
        let value = run_blocking(|| display_control::turn_off().map_err(tool_error)).await?;
        to_json(&value)
    }

    #[tool(
        name = "process_terminate",
        description = "Terminate one explicit process. Supply both process_id and expected_executable from recent discovery; native code rejects stale/reused ids and the assistant process itself. Sensitive action requires desktop confirmation."
    )]
    async fn process_terminate(
        &self,
        Parameters(ProcessTerminateInput {
            process_id,
            expected_executable,
        }): Parameters<ProcessTerminateInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "process_terminate",
                json!({
                    "process_id": process_id,
                    "expected_executable": &expected_executable,
                }),
            )
            .await?;
        let value = run_blocking(move || {
            process::terminate(process_id, &expected_executable).map_err(tool_error)
        })
        .await?;
        to_json(&value)
    }

    #[tool(
        name = "file_info",
        description = "Read metadata for one explicit absolute Windows path. Does not read file contents."
    )]
    async fn file_info(
        &self,
        Parameters(FilePathInput { path }): Parameters<FilePathInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize("file_info", json!({ "path": &path }))
            .await?;
        let value = run_blocking(move || files::info(&path).map_err(tool_error)).await?;
        to_json(&value)
    }

    #[tool(
        name = "file_list",
        description = "List a bounded number of entries from one explicit absolute Windows directory. Does not read file contents."
    )]
    async fn file_list(
        &self,
        Parameters(FileListInput {
            directory,
            max_entries,
        }): Parameters<FileListInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "file_list",
                json!({ "directory": &directory, "max_entries": max_entries }),
            )
            .await?;
        let max_entries = max_entries.map(|value| value as usize);
        let value = run_blocking(move || files::list(&directory, max_entries).map_err(tool_error)).await?;
        to_json(&value)
    }

    #[tool(
        name = "file_create_directory",
        description = "Create a new directory tree at one explicit absolute Windows path. Existing paths are rejected. Sensitive filesystem mutation requires desktop confirmation."
    )]
    async fn file_create_directory(
        &self,
        Parameters(FilePathInput { path }): Parameters<FilePathInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize("file_create_directory", json!({ "path": &path }))
            .await?;
        let value = run_blocking(move || files::create_directory(&path).map_err(tool_error)).await?;
        to_json(&value)
    }

    #[tool(
        name = "file_copy",
        description = "Copy one regular file between explicit absolute Windows paths. Directory/symlink copy and overwrite are intentionally disabled. Sensitive mutation requires desktop confirmation."
    )]
    async fn file_copy(
        &self,
        Parameters(FileTransferInput {
            source,
            destination,
        }): Parameters<FileTransferInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "file_copy",
                json!({ "source": &source, "destination": &destination }),
            )
            .await?;
        let value = run_blocking(move || files::copy_file(&source, &destination).map_err(tool_error)).await?;
        to_json(&value)
    }

    #[tool(
        name = "file_move",
        description = "Move or rename one explicit regular file/directory to a new absolute Windows path. Symlinks/junctions and overwrite are disabled. Sensitive mutation requires desktop confirmation."
    )]
    async fn file_move(
        &self,
        Parameters(FileTransferInput {
            source,
            destination,
        }): Parameters<FileTransferInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize(
                "file_move",
                json!({ "source": &source, "destination": &destination }),
            )
            .await?;
        let value = run_blocking(move || files::move_path(&source, &destination).map_err(tool_error)).await?;
        to_json(&value)
    }

    #[tool(
        name = "file_delete",
        description = "Delete one explicit regular file or empty directory. Recursive deletion, filesystem roots, symlinks and junctions are not exposed. Sensitive mutation requires desktop confirmation."
    )]
    async fn file_delete(
        &self,
        Parameters(FilePathInput { path }): Parameters<FilePathInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize("file_delete", json!({ "path": &path }))
            .await?;
        let value = run_blocking(move || files::delete_path(&path).map_err(tool_error)).await?;
        to_json(&value)
    }

    #[tool(
        name = "input_send_hotkey",
        description = "Send a bounded keyboard shortcut to the currently focused Windows application. Supports named modifiers/navigation keys, A-Z, 0-9 and F1-F12. Sensitive action requires desktop confirmation."
    )]
    async fn input_send_hotkey(
        &self,
        Parameters(HotkeyInput { keys }): Parameters<HotkeyInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize("input_send_hotkey", json!({ "keys": &keys }))
            .await?;
        let value = run_blocking(move || input::send_hotkey(&keys).map_err(tool_error)).await?;
        to_json(&value)
    }

    #[tool(
        name = "input_type_text",
        description = "Type bounded Unicode text into the currently focused Windows control through SendInput. Prefer semantic UI Automation set_value when available. Sensitive action requires desktop confirmation."
    )]
    async fn input_type_text(
        &self,
        Parameters(TypeTextInput { text }): Parameters<TypeTextInput>,
    ) -> Result<String, String> {
        self.permissions
            .authorize("input_type_text", json!({ "text": &text }))
            .await?;
        let value = run_blocking(move || input::type_text(&text).map_err(tool_error)).await?;
        to_json(&value)
    }
}
