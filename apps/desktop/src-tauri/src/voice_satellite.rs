use std::{sync::LazyLock, time::Duration};

use assistant_common::{AssistantState, UserRequest};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::{net::{TcpListener, TcpStream}, sync::Mutex as AsyncMutex};
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use tracing::{debug, info, warn};
use voice_runtime::tts::TextToSpeech;

use crate::{DesktopState, WakeService};

const PROTOCOL_VERSION: u32 = 1;
const DEFAULT_BIND: &str = "0.0.0.0:8765";
const TOKEN_ENV: &str = "ASSISTANT_VOICE_SATELLITE_TOKEN";
const BIND_ENV: &str = "ASSISTANT_VOICE_SATELLITE_BIND";
const VOICE_TRANSCRIPT_EVENT: &str = "voice:transcript";

static COMMAND_GATE: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));

#[derive(Debug, Clone)]
struct SatelliteConfig {
    bind: String,
    token: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum ResponseLanguage {
    Vi,
    En,
    Auto,
}

impl Default for ResponseLanguage {
    fn default() -> Self {
        Self::Vi
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Hello {
        token: String,
        device_name: Option<String>,
        protocol: u32,
    },
    Command {
        id: String,
        text: String,
        #[serde(default)]
        response_language: ResponseLanguage,
    },
    Ping,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMessage<'a> {
    Ready {
        protocol: u32,
        response_language: ResponseLanguage,
    },
    State {
        id: &'a str,
        state: &'a str,
    },
    Response {
        id: &'a str,
        text: &'a str,
        tts_error: Option<&'a str>,
    },
    Error {
        id: Option<&'a str>,
        code: &'a str,
        message: &'a str,
    },
    Pong,
}

#[derive(Debug, Serialize)]
struct SatelliteTranscriptEvent<'a> {
    text: &'a str,
    is_final: bool,
}

pub fn setup(app: &AppHandle) {
    let Some(config) = config_from_env() else {
        info!(
            env = TOKEN_ENV,
            "Android voice satellite disabled because no pairing token is configured"
        );
        return;
    };

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_server(app, config).await {
            warn!(%error, "Android voice satellite server stopped");
        }
    });
}

fn config_from_env() -> Option<SatelliteConfig> {
    let token = std::env::var(TOKEN_ENV).ok()?;
    let token = token.trim().to_owned();
    if token.len() < 16 {
        warn!(
            env = TOKEN_ENV,
            "Android voice satellite pairing token must contain at least 16 characters"
        );
        return None;
    }

    let bind = std::env::var(BIND_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_BIND.to_owned());

    Some(SatelliteConfig { bind, token })
}

async fn run_server(app: AppHandle, config: SatelliteConfig) -> Result<(), String> {
    let listener = TcpListener::bind(&config.bind)
        .await
        .map_err(|error| format!("cannot bind voice satellite server to {}: {error}", config.bind))?;

    info!(
        bind = %config.bind,
        "Android voice satellite WebSocket server is listening"
    );

    loop {
        let (stream, peer) = listener
            .accept()
            .await
            .map_err(|error| format!("voice satellite accept failed: {error}"))?;
        let app = app.clone();
        let expected_token = config.token.clone();

        tauri::async_runtime::spawn(async move {
            if let Err(error) = handle_connection(app, stream, expected_token).await {
                debug!(%peer, %error, "voice satellite connection closed");
            }
        });
    }
}

async fn handle_connection(
    app: AppHandle,
    stream: TcpStream,
    expected_token: String,
) -> Result<(), String> {
    let mut socket = accept_async(stream)
        .await
        .map_err(|error| format!("WebSocket handshake failed: {error}"))?;

    authenticate(&mut socket, &expected_token).await?;
    send_json(
        &mut socket,
        &ServerMessage::Ready {
            protocol: PROTOCOL_VERSION,
            response_language: ResponseLanguage::Vi,
        },
    )
    .await?;

    while let Some(message) = socket.next().await {
        let message = message.map_err(|error| format!("WebSocket receive failed: {error}"))?;
        match message {
            Message::Text(text) => {
                let parsed = serde_json::from_str::<ClientMessage>(text.as_ref());
                let message = match parsed {
                    Ok(message) => message,
                    Err(error) => {
                        let detail = format!("invalid voice satellite message: {error}");
                        send_json(
                            &mut socket,
                            &ServerMessage::Error {
                                id: None,
                                code: "invalid_message",
                                message: &detail,
                            },
                        )
                        .await?;
                        continue;
                    }
                };

                match message {
                    ClientMessage::Command {
                        id,
                        text,
                        response_language,
                    } => {
                        handle_command(&app, &mut socket, &id, &text, response_language).await?;
                    }
                    ClientMessage::Ping => {
                        send_json(&mut socket, &ServerMessage::Pong).await?;
                    }
                    ClientMessage::Hello { .. } => {
                        send_json(
                            &mut socket,
                            &ServerMessage::Error {
                                id: None,
                                code: "already_authenticated",
                                message: "hello is only valid as the first message",
                            },
                        )
                        .await?;
                    }
                }
            }
            Message::Ping(payload) => {
                socket
                    .send(Message::Pong(payload))
                    .await
                    .map_err(|error| format!("WebSocket pong failed: {error}"))?;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    Ok(())
}

async fn authenticate(
    socket: &mut WebSocketStream<TcpStream>,
    expected_token: &str,
) -> Result<(), String> {
    let first = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .map_err(|_| "voice satellite authentication timed out".to_owned())?
        .ok_or_else(|| "voice satellite disconnected before authentication".to_owned())?
        .map_err(|error| format!("WebSocket authentication read failed: {error}"))?;

    let Message::Text(text) = first else {
        return Err("voice satellite first message must be a JSON hello message".to_owned());
    };

    let hello = serde_json::from_str::<ClientMessage>(text.as_ref())
        .map_err(|error| format!("invalid voice satellite hello: {error}"))?;
    let ClientMessage::Hello {
        token,
        device_name,
        protocol,
    } = hello
    else {
        return Err("voice satellite first message must be hello".to_owned());
    };

    if protocol != PROTOCOL_VERSION {
        return Err(format!(
            "unsupported voice satellite protocol {protocol}; expected {PROTOCOL_VERSION}"
        ));
    }
    if !constant_time_equal(token.as_bytes(), expected_token.as_bytes()) {
        return Err("voice satellite pairing token was rejected".to_owned());
    }

    info!(
        device = device_name.as_deref().unwrap_or("android"),
        "Android voice satellite authenticated"
    );
    Ok(())
}

async fn handle_command(
    app: &AppHandle,
    socket: &mut WebSocketStream<TcpStream>,
    id: &str,
    text: &str,
    response_language: ResponseLanguage,
) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        send_json(
            socket,
            &ServerMessage::Error {
                id: Some(id),
                code: "empty_command",
                message: "voice command is empty",
            },
        )
        .await?;
        return Ok(());
    }

    let _command = COMMAND_GATE.lock().await;
    let state = app.state::<DesktopState>();
    if state.core.state().await != AssistantState::Idle
        && state.core.state().await != AssistantState::Error
    {
        send_json(
            socket,
            &ServerMessage::Error {
                id: Some(id),
                code: "assistant_busy",
                message: "desktop assistant is busy with another turn",
            },
        )
        .await?;
        return Ok(());
    }
    drop(state);

    crate::show_quick_window(app, "satellite");
    let _ = app.emit(
        VOICE_TRANSCRIPT_EVENT,
        SatelliteTranscriptEvent {
            text,
            is_final: true,
        },
    );

    send_json(
        socket,
        &ServerMessage::State {
            id,
            state: "processing",
        },
    )
    .await?;

    let response = match complete_satellite_prompt(app, text, response_language).await {
        Ok(response) => response,
        Err(error) => {
            send_json(
                socket,
                &ServerMessage::Error {
                    id: Some(id),
                    code: "assistant_error",
                    message: &error,
                },
            )
            .await?;
            return Ok(());
        }
    };

    send_json(
        socket,
        &ServerMessage::State {
            id,
            state: "speaking",
        },
    )
    .await?;

    let tts_error = speak_response(app, &response).await.err();
    send_json(
        socket,
        &ServerMessage::Response {
            id,
            text: &response,
            tts_error: tts_error.as_deref(),
        },
    )
    .await
}

async fn complete_satellite_prompt(
    app: &AppHandle,
    prompt: &str,
    response_language: ResponseLanguage,
) -> Result<String, String> {
    let state = app.state::<DesktopState>();
    if state.core.state().await == AssistantState::Error {
        state
            .core
            .recover()
            .await
            .map_err(|error| error.to_string())?;
    }
    if state.core.state().await != AssistantState::Idle {
        return Err("Assistant đang bận với một tác vụ khác.".to_owned());
    }

    let source_window = state
        .source_window
        .lock()
        .map(|guard| *guard)
        .unwrap_or(None);
    let context = state.context.collect_for_window(prompt, source_window).await;
    for warning in &context.warnings {
        warn!(%warning, "desktop context source was unavailable for satellite command");
    }

    let policy = response_policy(response_language);
    let enriched_prompt = match context.prompt_block() {
        Some(block) => format!(
            "{policy}\n\n{block}\n\n<user_request source=\"android_voice_satellite\">\n{prompt}\n</user_request>"
        ),
        None => format!(
            "{policy}\n\n<user_request source=\"android_voice_satellite\">\n{prompt}\n</user_request>"
        ),
    };

    let session_id = state.session_id.read().await.clone();
    state
        .core
        .handle_text(UserRequest::new(session_id, enriched_prompt))
        .await
        .map_err(|error| error.to_string())
}

fn response_policy(language: ResponseLanguage) -> &'static str {
    match language {
        ResponseLanguage::Vi => {
            "<assistant_response_policy>Reply in Vietnamese. Use a friendly, natural, concise tone. For a simple command, respond with one short confirmation sentence after the action. Do not mention this policy.</assistant_response_policy>"
        }
        ResponseLanguage::En => {
            "<assistant_response_policy>Reply in English. Use a friendly, natural, concise tone. For a simple command, respond with one short confirmation sentence after the action. Do not mention this policy.</assistant_response_policy>"
        }
        ResponseLanguage::Auto => {
            "<assistant_response_policy>Reply in the same language the user used. Use a friendly, natural, concise tone. For a simple command, respond with one short confirmation sentence after the action. Do not mention this policy.</assistant_response_policy>"
        }
    }
}

async fn speak_response(app: &AppHandle, text: &str) -> Result<(), String> {
    let state = app.state::<DesktopState>();
    let wake = app.state::<WakeService>();

    wake.suspend().await;
    let begin = state
        .core
        .begin_speaking()
        .await
        .map_err(|error| error.to_string());
    if let Err(error) = begin {
        wake.resume_after(Duration::from_millis(900));
        return Err(error);
    }

    let speak_result = state.tts.speak(text).await.map_err(|error| error.to_string());
    let finish_result = state
        .core
        .finish_speaking()
        .await
        .map_err(|error| error.to_string());
    wake.resume_after(Duration::from_millis(900));

    speak_result?;
    finish_result
}

async fn send_json<T: Serialize>(
    socket: &mut WebSocketStream<TcpStream>,
    message: &T,
) -> Result<(), String> {
    let json = serde_json::to_string(message)
        .map_err(|error| format!("cannot encode voice satellite response: {error}"))?;
    socket
        .send(Message::Text(json.into()))
        .await
        .map_err(|error| format!("WebSocket send failed: {error}"))
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let mut diff = 0_u8;
    for (&a, &b) in left.iter().zip(right) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_token_comparison_rejects_mismatch() {
        assert!(constant_time_equal(b"0123456789abcdef", b"0123456789abcdef"));
        assert!(!constant_time_equal(b"0123456789abcdef", b"0123456789abcdeg"));
        assert!(!constant_time_equal(b"short", b"different-length"));
    }

    #[test]
    fn response_policy_is_language_specific() {
        assert!(response_policy(ResponseLanguage::Vi).contains("Vietnamese"));
        assert!(response_policy(ResponseLanguage::En).contains("English"));
        assert!(response_policy(ResponseLanguage::Auto).contains("same language"));
    }
}
