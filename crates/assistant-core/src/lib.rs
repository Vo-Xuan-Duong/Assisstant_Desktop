use std::sync::Arc;

use assistant_common::{AssistantEvent, AssistantState, UserRequest};
use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{debug, warn};

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid assistant state transition: {from:?} -> {to:?}")]
    InvalidTransition {
        from: AssistantState,
        to: AssistantState,
    },
    #[error("agent backend failed: {0}")]
    Backend(String),
}

#[async_trait]
pub trait AgentBackend: Send + Sync {
    async fn complete(&self, request: &UserRequest) -> Result<String, CoreError>;
}

#[async_trait]
pub trait EventSink: Send + Sync {
    async fn publish(&self, event: AssistantEvent);
}

pub struct NoopEventSink;

#[async_trait]
impl EventSink for NoopEventSink {
    async fn publish(&self, _event: AssistantEvent) {}
}

#[derive(Debug, Default)]
pub struct StateMachine {
    state: AssistantState,
}

impl StateMachine {
    pub fn state(&self) -> AssistantState {
        self.state
    }

    pub fn transition(&mut self, to: AssistantState) -> Result<AssistantEvent, CoreError> {
        let from = self.state;
        if from == to {
            return Ok(AssistantEvent::StateChanged { from, to });
        }

        if !is_valid_transition(from, to) {
            return Err(CoreError::InvalidTransition { from, to });
        }

        self.state = to;
        Ok(AssistantEvent::StateChanged { from, to })
    }
}

fn is_valid_transition(from: AssistantState, to: AssistantState) -> bool {
    use AssistantState::*;

    matches!(
        (from, to),
        (Idle, Listening)
            | (Idle, Processing)
            | (Idle, Speaking)
            | (Idle, Error)
            | (Listening, Processing)
            | (Listening, Idle)
            | (Listening, Error)
            | (Processing, Executing)
            | (Processing, Confirming)
            | (Processing, Speaking)
            | (Processing, Idle)
            | (Processing, Error)
            | (Executing, Processing)
            | (Executing, Speaking)
            | (Executing, Confirming)
            | (Executing, Idle)
            | (Executing, Error)
            | (Confirming, Processing)
            | (Confirming, Executing)
            | (Confirming, Idle)
            | (Confirming, Error)
            | (Speaking, Listening)
            | (Speaking, Idle)
            | (Speaking, Error)
            | (Error, Idle)
    )
}

pub struct AssistantCore<B, S>
where
    B: AgentBackend,
    S: EventSink,
{
    backend: Arc<B>,
    events: Arc<S>,
    state: Mutex<StateMachine>,
    request_gate: Mutex<()>,
}

impl<B, S> AssistantCore<B, S>
where
    B: AgentBackend,
    S: EventSink,
{
    pub fn new(backend: Arc<B>, events: Arc<S>) -> Self {
        Self {
            backend,
            events,
            state: Mutex::new(StateMachine::default()),
            request_gate: Mutex::new(()),
        }
    }

    pub async fn state(&self) -> AssistantState {
        self.state.lock().await.state()
    }

    pub async fn begin_listening(&self) -> Result<(), CoreError> {
        self.change_state(AssistantState::Listening).await
    }

    pub async fn cancel_listening(&self) -> Result<(), CoreError> {
        if self.state().await == AssistantState::Listening {
            self.change_state(AssistantState::Idle).await?;
        }
        Ok(())
    }

    pub async fn begin_speaking(&self) -> Result<(), CoreError> {
        self.change_state(AssistantState::Speaking).await
    }

    pub async fn finish_speaking(&self) -> Result<(), CoreError> {
        if self.state().await == AssistantState::Speaking {
            self.change_state(AssistantState::Idle).await?;
        }
        Ok(())
    }

    /// Mark an in-flight backend request as waiting for an explicit user
    /// confirmation. This is intentionally a narrow lifecycle API rather than
    /// exposing arbitrary state transitions to desktop integrations.
    pub async fn begin_confirming(&self) -> Result<(), CoreError> {
        match self.state().await {
            AssistantState::Processing | AssistantState::Executing => {
                self.change_state(AssistantState::Confirming).await
            }
            AssistantState::Confirming => Ok(()),
            from => Err(CoreError::InvalidTransition {
                from,
                to: AssistantState::Confirming,
            }),
        }
    }

    /// Resume backend processing after the pending confirmation has been
    /// resolved. Both Allow and Deny return control to the same in-flight agent
    /// turn; the MCP result tells the model which decision occurred.
    pub async fn finish_confirming(&self) -> Result<(), CoreError> {
        if self.state().await == AssistantState::Confirming {
            self.change_state(AssistantState::Processing).await?;
        }
        Ok(())
    }

    pub async fn handle_text(&self, request: UserRequest) -> Result<String, CoreError> {
        // Keep the first implementation single-flight. Voice interruption and
        // concurrent tool execution are introduced deliberately in later phases.
        let _request_guard = self.request_gate.lock().await;
        self.change_state(AssistantState::Processing).await?;

        match self.backend.complete(&request).await {
            Ok(response) => {
                self.events
                    .publish(AssistantEvent::ResponseCompleted {
                        text: response.clone(),
                    })
                    .await;
                // A broker timeout can resolve after the desktop event loop has
                // lost its UI consumer. Normalize a lingering Confirming state
                // before ending the completed request.
                if self.state().await == AssistantState::Confirming {
                    self.change_state(AssistantState::Processing).await?;
                }
                self.change_state(AssistantState::Idle).await?;
                Ok(response)
            }
            Err(error) => {
                warn!(%error, "agent backend request failed");
                self.change_state(AssistantState::Error).await?;
                self.events
                    .publish(AssistantEvent::Error {
                        code: "backend_error".into(),
                        message: error.to_string(),
                    })
                    .await;
                Err(error)
            }
        }
    }

    pub async fn recover(&self) -> Result<(), CoreError> {
        if self.state().await == AssistantState::Error {
            self.change_state(AssistantState::Idle).await?;
        }
        Ok(())
    }

    async fn change_state(&self, to: AssistantState) -> Result<(), CoreError> {
        let event = {
            let mut state = self.state.lock().await;
            state.transition(to)?
        };

        debug!(?to, "assistant state changed");
        self.events.publish(event).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_machine_accepts_expected_text_flow() {
        let mut state = StateMachine::default();
        state.transition(AssistantState::Processing).unwrap();
        state.transition(AssistantState::Idle).unwrap();
        assert_eq!(state.state(), AssistantState::Idle);
    }

    #[test]
    fn state_machine_accepts_confirmation_round_trip() {
        let mut state = StateMachine::default();
        state.transition(AssistantState::Processing).unwrap();
        state.transition(AssistantState::Confirming).unwrap();
        state.transition(AssistantState::Processing).unwrap();
        state.transition(AssistantState::Idle).unwrap();
        assert_eq!(state.state(), AssistantState::Idle);
    }

    #[test]
    fn state_machine_accepts_voice_output_after_completed_text_turn() {
        let mut state = StateMachine::default();
        state.transition(AssistantState::Processing).unwrap();
        state.transition(AssistantState::Idle).unwrap();
        state.transition(AssistantState::Speaking).unwrap();
        state.transition(AssistantState::Idle).unwrap();
        assert_eq!(state.state(), AssistantState::Idle);
    }

    #[test]
    fn state_machine_rejects_invalid_transition() {
        let mut state = StateMachine::default();
        let result = state.transition(AssistantState::Executing);
        assert!(matches!(result, Err(CoreError::InvalidTransition { .. })));
    }
}
