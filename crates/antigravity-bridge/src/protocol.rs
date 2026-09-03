use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct UserInputEvent<'a> {
    pub event: &'static str,
    pub message: UserMessage<'a>,
}

impl<'a> UserInputEvent<'a> {
    pub fn new(content: &'a str) -> Self {
        Self {
            event: "user",
            message: UserMessage { content },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct UserMessage<'a> {
    pub content: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub thinking_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InitEvent {
    pub conversation_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StepUpdate {
    pub conversation_id: Option<String>,
    pub step_index: Option<u64>,
    pub state: Option<String>,
    pub step_type: Option<String>,
    pub text_delta: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResultPayload {
    pub conversation_id: Option<String>,
    pub status: String,
    #[serde(default)]
    pub response: String,
    pub error: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum StreamEvent {
    Init {
        conversation_id: Option<String>,
        #[serde(default)]
        init: serde_json::Value,
    },
    StepUpdate {
        step_update: StepUpdate,
    },
    Result {
        result: ResultPayload,
    },
    #[serde(other)]
    Unknown,
}
