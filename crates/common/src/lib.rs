use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantState {
    Idle,
    Listening,
    Processing,
    Executing,
    Speaking,
    Confirming,
    Error,
}

impl Default for AssistantState {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRisk {
    Safe,
    Moderate,
    Sensitive,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRequest {
    pub session_id: SessionId,
    pub text: String,
}

impl UserRequest {
    pub fn new(session_id: SessionId, text: impl Into<String>) -> Self {
        Self {
            session_id,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssistantEvent {
    StateChanged {
        from: AssistantState,
        to: AssistantState,
    },
    TextDelta {
        text: String,
    },
    ResponseCompleted {
        text: String,
    },
    ToolStarted {
        name: String,
    },
    ToolFinished {
        name: String,
        success: bool,
    },
    Error {
        code: String,
        message: String,
    },
}
