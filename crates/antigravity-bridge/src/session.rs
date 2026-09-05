use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    path::PathBuf,
    process::Stdio,
    sync::Arc,
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::{Mutex, broadcast},
    task::JoinHandle,
};
use tracing::{debug, warn};

use crate::{
    BridgeError,
    protocol::{ResultPayload, StreamEvent, UserInputEvent},
};

const MAX_DIAGNOSTIC_LINES: usize = 32;

#[derive(Clone)]
pub struct AntigravityConfig {
    pub binary: String,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub effort: Option<String>,
    pub working_directory: Option<PathBuf>,
    /// Ephemeral environment passed only to the Antigravity process tree. This
    /// is used for local runtime integration such as the permission broker and
    /// must never be logged with values.
    pub environment: BTreeMap<String, String>,
}

impl fmt::Debug for AntigravityConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let environment_keys: Vec<&str> = self.environment.keys().map(String::as_str).collect();
        formatter
            .debug_struct("AntigravityConfig")
            .field("binary", &self.binary)
            .field("model", &self.model)
            .field("agent", &self.agent)
            .field("effort", &self.effort)
            .field("working_directory", &self.working_directory)
            .field("environment_keys", &environment_keys)
            .finish()
    }
}

impl Default for AntigravityConfig {
    fn default() -> Self {
        Self {
            binary: default_binary(),
            model: std::env::var("ASSISTANT_ANTIGRAVITY_MODEL")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            agent: std::env::var("ASSISTANT_ANTIGRAVITY_AGENT")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            effort: std::env::var("ASSISTANT_ANTIGRAVITY_EFFORT")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            working_directory: None,
            environment: BTreeMap::new(),
        }
    }
}

fn default_binary() -> String {
    if let Ok(binary) = std::env::var("ASSISTANT_ANTIGRAVITY_BIN") {
        if !binary.trim().is_empty() {
            return binary;
        }
    }
    #[cfg(windows)]
    if let Some(local_data) = std::env::var_os("LOCALAPPDATA") {
        let installed = PathBuf::from(local_data).join("agy/bin/agy.exe");
        if installed.is_file() {
            return installed.to_string_lossy().into_owned();
        }
    }
    #[cfg(windows)]
    if let Some(user_profile) = std::env::var_os("USERPROFILE") {
        let installed = PathBuf::from(user_profile).join(".gemini/bin/agy.exe");
        if installed.is_file() {
            return installed.to_string_lossy().into_owned();
        }
    }
    "agy".into()
}

impl AntigravityConfig {
    pub fn set_environment(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.environment.insert(key.into(), value.into());
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.binary);
        command
            .arg("--input-format")
            .arg("stream-json")
            .arg("--output-format")
            .arg("stream-json")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if let Some(model) = &self.model {
            command.arg("--model").arg(model);
        }
        if let Some(agent) = &self.agent {
            command.arg("--agent").arg(agent);
        }
        if let Some(effort) = &self.effort {
            command.arg("--effort").arg(effort);
        }
        if let Some(cwd) = &self.working_directory {
            command.current_dir(cwd);
        }
        if !self.environment.is_empty() {
            command.envs(&self.environment);
        }

        command
    }
}

#[derive(Debug, Clone)]
pub struct TurnResult {
    pub conversation_id: Option<String>,
    pub response: String,
    pub raw: ResultPayload,
}

pub struct AntigravitySession {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    conversation_id: Option<String>,
    diagnostics: Arc<Mutex<VecDeque<String>>>,
    events: Option<broadcast::Sender<StreamEvent>>,
    stderr_task: JoinHandle<()>,
}

impl AntigravitySession {
    pub async fn spawn(config: &AntigravityConfig) -> Result<Self, BridgeError> {
        Self::spawn_with_events(config, None).await
    }

    pub async fn spawn_with_events(
        config: &AntigravityConfig,
        events: Option<broadcast::Sender<StreamEvent>>,
    ) -> Result<Self, BridgeError> {
        let mut child = config.command().spawn().map_err(BridgeError::Spawn)?;
        let stdin = child.stdin.take().ok_or(BridgeError::MissingStdin)?;
        let stdout = child.stdout.take().ok_or(BridgeError::MissingStdout)?;
        let stderr = child.stderr.take().ok_or(BridgeError::MissingStderr)?;

        let diagnostics = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_DIAGNOSTIC_LINES)));
        let diagnostics_writer = Arc::clone(&diagnostics);
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        debug!(diagnostic = %line, "Antigravity diagnostic");
                        let mut buffer = diagnostics_writer.lock().await;
                        if buffer.len() == MAX_DIAGNOSTIC_LINES {
                            buffer.pop_front();
                        }
                        buffer.push_back(line);
                    }
                    Ok(None) => break,
                    Err(error) => {
                        warn!(%error, "failed reading Antigravity stderr");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            conversation_id: None,
            diagnostics,
            events,
            stderr_task,
        })
    }

    pub fn conversation_id(&self) -> Option<&str> {
        self.conversation_id.as_deref()
    }

    pub async fn diagnostics(&self) -> Vec<String> {
        self.diagnostics.lock().await.iter().cloned().collect()
    }

    pub async fn ask(&mut self, prompt: &str) -> Result<TurnResult, BridgeError> {
        if prompt.trim().is_empty() {
            return Err(BridgeError::EmptyPrompt);
        }

        if let Some(status) = self.child.try_wait()? {
            return Err(self.session_closed(status.code()).await);
        }

        let line = serde_json::to_string(&UserInputEvent::new(prompt))?;
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;

        while let Some(line) = self.stdout.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }

            let event: StreamEvent = match serde_json::from_str(&line) {
                Ok(event) => event,
                Err(error) => {
                    warn!(%error, "ignoring malformed Antigravity stdout event");
                    continue;
                }
            };

            if let Some(events) = &self.events {
                let _ = events.send(event.clone());
            }

            match event {
                StreamEvent::Init {
                    conversation_id, ..
                } => {
                    if conversation_id.is_some() {
                        self.conversation_id = conversation_id;
                    }
                }
                StreamEvent::StepUpdate { step_update } => {
                    if let Some(id) = step_update.conversation_id {
                        self.conversation_id = Some(id);
                    }
                    if let Some(delta) = step_update.text_delta {
                        debug!(text_delta = %delta, "Antigravity response delta");
                    }
                }
                StreamEvent::Result { result } => {
                    if let Some(id) = &result.conversation_id {
                        self.conversation_id = Some(id.clone());
                    }

                    if result.status != "SUCCESS" {
                        return Err(BridgeError::Agent {
                            status: result.status.clone(),
                            message: result.error.clone().unwrap_or_else(|| {
                                "Antigravity returned an unsuccessful result".into()
                            }),
                        });
                    }

                    return Ok(TurnResult {
                        conversation_id: result.conversation_id.clone(),
                        response: result.response.clone(),
                        raw: result,
                    });
                }
                StreamEvent::Unknown => {
                    debug!("ignoring unknown Antigravity stream event");
                }
            }
        }

        let status = self.child.try_wait()?;
        Err(self
            .session_closed(status.and_then(|status| status.code()))
            .await)
    }

    pub async fn shutdown(mut self) -> Result<(), BridgeError> {
        let _ = self.stdin.shutdown().await;
        let wait_result = self.child.wait().await;
        self.stderr_task.abort();
        wait_result?;
        Ok(())
    }

    async fn session_closed(&self, code: Option<i32>) -> BridgeError {
        BridgeError::SessionClosed {
            code,
            diagnostics: self.diagnostics().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_binary_returns_usable_path_or_command() {
        let binary = default_binary();
        assert!(!binary.trim().is_empty());
    }
}
