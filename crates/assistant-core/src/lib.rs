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
    #[error("local tool failed: {0}")]
    LocalTool(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalSafeIntent {
    GetVolume,
    ListRunningApps,
    GetActiveWindow,
    GetSystemInfo,
}

impl LocalSafeIntent {
    pub const fn tool_name(self) -> &'static str {
        match self {
            Self::GetVolume => "audio_get_volume",
            Self::ListRunningApps => "apps_list",
            Self::GetActiveWindow => "window_get_active",
            Self::GetSystemInfo => "system_get_info",
        }
    }
}

/// Match a deliberately small set of read-only commands that do not require
/// model reasoning. Mutating or ambiguous requests intentionally return None
/// so they continue through Antigravity + MCP + the normal permission path.
pub fn match_local_safe_intent(text: &str) -> Option<LocalSafeIntent> {
    let normalized = text.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }

    let asks_volume = contains_any(&normalized, &["âm lượng", "am luong", "volume"]);
    let volume_is_query = contains_any(
        &normalized,
        &[
            "bao nhiêu",
            "bao nhieu",
            "hiện tại",
            "hien tai",
            "current",
            "what is",
            "mức nào",
            "muc nao",
        ],
    );
    let volume_is_mutation = contains_any(
        &normalized,
        &[
            "đặt ",
            "dat ",
            "set ",
            "tăng",
            "tang",
            "giảm",
            "giam",
            "mute",
            "unmute",
            "tắt tiếng",
            "tat tieng",
            "bật tiếng",
            "bat tieng",
        ],
    );
    if asks_volume && volume_is_query && !volume_is_mutation {
        return Some(LocalSafeIntent::GetVolume);
    }

    let asks_active_window = contains_any(
        &normalized,
        &[
            "cửa sổ active",
            "cua so active",
            "cửa sổ đang dùng",
            "cua so dang dung",
            "active window",
            "foreground window",
            "ứng dụng đang active",
            "ung dung dang active",
            "app đang active",
            "app dang active",
        ],
    );
    if asks_active_window {
        return Some(LocalSafeIntent::GetActiveWindow);
    }

    let asks_running_apps = contains_any(
        &normalized,
        &[
            "ứng dụng đang chạy",
            "ứng dụng nào đang chạy",
            "ung dung nao dang chay",
            "ung dung dang chay",
            "app đang chạy",
            "app dang chay",
            "running apps",
            "running applications",
            "process đang chạy",
            "process dang chay",
            "tiến trình đang chạy",
            "tien trinh dang chay",
        ],
    );
    if asks_running_apps {
        return Some(LocalSafeIntent::ListRunningApps);
    }

    let asks_system_info = contains_any(
        &normalized,
        &[
            "máy đang dùng bao nhiêu ram",
            "may dang dung bao nhieu ram",
            "ram hiện tại",
            "ram hien tai",
            "memory usage",
            "memory hiện tại",
            "memory hien tai",
            "thông tin máy",
            "thong tin may",
            "system info",
            "system information",
        ],
    );
    if asks_system_info {
        return Some(LocalSafeIntent::GetSystemInfo);
    }

    None
}

fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| text.contains(pattern))
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

    pub async fn handle_local_safe_tool<F>(
        &self,
        tool_name: &'static str,
        operation: F,
    ) -> Result<String, CoreError>
    where
        F: FnOnce() -> Result<String, CoreError>,
    {
        let _request_guard = self.request_gate.lock().await;
        self.change_state(AssistantState::Processing).await?;
        self.events
            .publish(AssistantEvent::ToolStarted {
                name: tool_name.to_owned(),
            })
            .await;
        self.change_state(AssistantState::Executing).await?;

        match operation() {
            Ok(response) => {
                self.events
                    .publish(AssistantEvent::ToolFinished {
                        name: tool_name.to_owned(),
                        success: true,
                    })
                    .await;
                self.events
                    .publish(AssistantEvent::ResponseCompleted {
                        text: response.clone(),
                    })
                    .await;
                self.change_state(AssistantState::Idle).await?;
                Ok(response)
            }
            Err(error) => {
                warn!(%error, %tool_name, "local safe tool failed");
                self.events
                    .publish(AssistantEvent::ToolFinished {
                        name: tool_name.to_owned(),
                        success: false,
                    })
                    .await;
                self.change_state(AssistantState::Error).await?;
                self.events
                    .publish(AssistantEvent::Error {
                        code: "local_tool_error".into(),
                        message: error.to_string(),
                    })
                    .await;
                Err(error)
            }
        }
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

    #[test]
    fn safe_intent_matches_vietnamese_volume_query() {
        assert_eq!(
            match_local_safe_intent("Âm lượng hiện tại bao nhiêu?"),
            Some(LocalSafeIntent::GetVolume)
        );
    }

    #[test]
    fn safe_intent_matches_running_apps_and_active_window() {
        assert_eq!(
            match_local_safe_intent("Ứng dụng nào đang chạy?"),
            Some(LocalSafeIntent::ListRunningApps)
        );
        assert_eq!(
            match_local_safe_intent("Cửa sổ active hiện tại là gì?"),
            Some(LocalSafeIntent::GetActiveWindow)
        );
    }

    #[test]
    fn safe_intent_matches_system_memory_query() {
        assert_eq!(
            match_local_safe_intent("Máy đang dùng bao nhiêu RAM?"),
            Some(LocalSafeIntent::GetSystemInfo)
        );
    }

    #[test]
    fn mutating_volume_request_does_not_use_safe_fast_path() {
        assert_eq!(match_local_safe_intent("Đặt âm lượng xuống 30%"), None);
    }

    #[test]
    fn unrelated_prompt_falls_back_to_agent() {
        assert_eq!(match_local_safe_intent("Giải thích Rust ownership"), None);
    }
}
