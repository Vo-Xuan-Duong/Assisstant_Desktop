mod protocol;
mod session;

use assistant_common::UserRequest;
use assistant_core::{AgentBackend, CoreError};
use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::Mutex;

pub use protocol::{ResultPayload, StepUpdate, StreamEvent, Usage};
pub use session::{AntigravityConfig, AntigravitySession, TurnResult};

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("failed to start Antigravity CLI: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("Antigravity process did not expose stdin")]
    MissingStdin,
    #[error("Antigravity process did not expose stdout")]
    MissingStdout,
    #[error("prompt cannot be empty")]
    EmptyPrompt,
    #[error("Antigravity session ended unexpectedly (exit code: {0:?})")]
    SessionClosed(Option<i32>),
    #[error("Antigravity returned {status}: {message}")]
    Agent { status: String, message: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub struct AntigravityClient {
    config: AntigravityConfig,
    session: Mutex<Option<AntigravitySession>>,
}

impl AntigravityClient {
    pub fn new(config: AntigravityConfig) -> Self {
        Self {
            config,
            session: Mutex::new(None),
        }
    }

    pub async fn ask(&self, prompt: &str) -> Result<TurnResult, BridgeError> {
        let mut session = self.session.lock().await;

        if session.is_none() {
            *session = Some(AntigravitySession::spawn(&self.config).await?);
        }

        session
            .as_mut()
            .expect("session is initialized above")
            .ask(prompt)
            .await
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
