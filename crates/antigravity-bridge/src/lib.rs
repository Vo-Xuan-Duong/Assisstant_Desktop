mod health;
mod protocol;
mod session;

use assistant_common::UserRequest;
use assistant_core::{AgentBackend, CoreError};
use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::{broadcast, Mutex};

pub use health::{probe_cli, BridgeFailureKind, CliHealth};
pub use protocol::{ResultPayload, StepUpdate, StreamEvent, Usage};
pub use session::{AntigravityConfig, AntigravitySession, TurnResult};

use crate::health::classify_message;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("failed to start Antigravity CLI: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("Antigravity process did not expose stdin")]
    MissingStdin,
    #[error("Antigravity process did not expose stdout")]
    MissingStdout,
    #[error("Antigravity process did not expose stderr")]
    MissingStderr,
    #[error("prompt cannot be empty")]
    EmptyPrompt,
    #[error("Antigravity session ended unexpectedly (exit code: {code:?}): {diagnostics:?}")]
    SessionClosed {
        code: Option<i32>,
        diagnostics: Vec<String>,
    },
    #[error("Antigravity returned {status}: {message}")]
    Agent { status: String, message: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl BridgeError {
    pub fn kind(&self) -> BridgeFailureKind {
        match self {
            Self::EmptyPrompt => BridgeFailureKind::InvalidInput,
            Self::Json(_) => BridgeFailureKind::Protocol,
            Self::Spawn(_) | Self::Io(_) => BridgeFailureKind::Transport,
            Self::MissingStdin | Self::MissingStdout | Self::MissingStderr => {
                BridgeFailureKind::Process
            }
            Self::SessionClosed { diagnostics, .. } => {
                let classified = classify_message(&diagnostics.join("\n"));
                if classified == BridgeFailureKind::Unknown {
                    BridgeFailureKind::Process
                } else {
                    classified
                }
            }
            Self::Agent { message, .. } => classify_message(message),
        }
    }

    pub fn invalidates_session(&self) -> bool {
        matches!(
            self.kind(),
            BridgeFailureKind::Transport | BridgeFailureKind::Process | BridgeFailureKind::Protocol
        )
    }
}

pub struct AntigravityClient {
    config: AntigravityConfig,
    session: Mutex<Option<AntigravitySession>>,
    events: broadcast::Sender<StreamEvent>,
}

impl AntigravityClient {
    pub fn new(config: AntigravityConfig) -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            config,
            session: Mutex::new(None),
            events,
        }
    }

    pub async fn health(&self) -> CliHealth {
        probe_cli(&self.config).await
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StreamEvent> {
        self.events.subscribe()
    }

    pub async fn start(&self) -> Result<(), BridgeError> {
        let mut session = self.session.lock().await;
        if session.is_none() {
            *session = Some(
                AntigravitySession::spawn_with_events(&self.config, Some(self.events.clone())).await?,
            );
        }
        Ok(())
    }

    pub async fn ask(&self, prompt: &str) -> Result<TurnResult, BridgeError> {
        let mut session = self.session.lock().await;

        if session.is_none() {
            *session = Some(
                AntigravitySession::spawn_with_events(&self.config, Some(self.events.clone())).await?,
            );
        }

        let result = session
            .as_mut()
            .expect("session is initialized above")
            .ask(prompt)
            .await;

        if result
            .as_ref()
            .err()
            .is_some_and(BridgeError::invalidates_session)
        {
            if let Some(failed) = session.take() {
                let _ = failed.shutdown().await;
            }
        }

        result
    }

    pub async fn conversation_id(&self) -> Option<String> {
        self.session
            .lock()
            .await
            .as_ref()
            .and_then(AntigravitySession::conversation_id)
            .map(ToOwned::to_owned)
    }

    pub async fn reset(&self) {
        let mut session = self.session.lock().await;
        if let Some(active) = session.take() {
            let _ = active.shutdown().await;
        }
    }

    pub async fn restart(&self) -> Result<(), BridgeError> {
        self.reset().await;
        self.start().await
    }
}

#[async_trait]
impl AgentBackend for AntigravityClient {
    async fn complete(&self, request: &UserRequest) -> Result<String, CoreError> {
        self.ask(&request.text)
            .await
            .map(|turn| turn.response)
            .map_err(|error| CoreError::Backend(error.to_string()))
    }
}
