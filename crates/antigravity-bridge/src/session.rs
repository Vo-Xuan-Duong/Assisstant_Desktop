use std::{path::PathBuf, process::Stdio};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
};
use tracing::{debug, warn};

use crate::{
    protocol::{ResultPayload, StreamEvent, UserInputEvent},
    BridgeError,
};

#[derive(Debug, Clone)]
pub struct AntigravityConfig {
    pub binary: String,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub effort: Option<String>,
    pub working_directory: Option<PathBuf>,
}

impl Default for AntigravityConfig {
    fn default() -> Self {
        Self {
            binary: "agy".into(),
            model: None,
            agent: None,
            effort: None,
            working_directory: None,
        }
    }
}

impl AntigravityConfig {
    fn command(&self) -> Command {
        let mut command = Command::new(&self.binary);
        command
            .arg("--input-format")
            .arg("stream-json")
            .arg("--output-format")
            .arg("stream-json")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
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
}

impl AntigravitySession {
    pub async fn spawn(config: &AntigravityConfig) -> Result<Self, BridgeError> {
        let mut child = config.command().spawn().map_err(BridgeError::Spawn)?;
        let stdin = child.stdin.take().ok_or(BridgeError::MissingStdin)?;
        let stdout = child.stdout.take().ok_or(BridgeError::MissingStdout)?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            conversation_id: None,
        })
    }

    pub fn conversation_id(&self) -> Option<&str> {
        self.conversation_id.as_deref()
    }

    pub async fn ask(&mut self, prompt: &str) -> Result<TurnResult, BridgeError> {
        if prompt.trim().is_empty() {
            return Err(BridgeError::EmptyPrompt);
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
                    warn!(%error, raw = %line, "ignoring malformed Antigravity stdout event");
                    continue;
                }
            };

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
                            message: result
                                .error
                                .clone()
                                .unwrap_or_else(|| "Antigravity returned an unsuccessful result".into()),
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
        Err(BridgeError::SessionClosed(status.and_then(|status| status.code())))
    }

    pub async fn shutdown(mut self) -> Result<(), BridgeError> {
        self.stdin.shutdown().await?;
        let _ = self.child.wait().await?;
        Ok(())
    }
}
