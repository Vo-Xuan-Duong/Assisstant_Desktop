#[path = "satellite_devices.rs"]
mod satellite_devices;

use std::{
    collections::{HashSet, VecDeque},
    fs,
    sync::LazyLock,
    time::Duration,
};

use assistant_common::{AssistantState, UserRequest};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex as AsyncMutex,
};
use tracing::{debug, info, warn};
use voice_runtime::tts::{TextToSpeech, TtsLanguage};

use crate::{DesktopState, WakeService};

const PROTOCOL_VERSION: u32 = 1;
const DEFAULT_BIND: &str = "0.0.0.0:8765";
const TOKEN_ENV: &str = "ASSISTANT_VOICE_SATELLITE_TOKEN";
const BIND_ENV: &str = "ASSISTANT_VOICE_SATELLITE_BIND";
const SETTINGS_FILE: &str = "satellite.json";
const VOICE_TRANSCRIPT_EVENT: &str = "voice:transcript";
const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const MAX_HTTP_HEADER_BYTES: usize = 16 * 1024;
const MAX_FRAME_BYTES: usize = 64 * 1024;
const SETTINGS_POLL_INTERVAL: Duration = Duration::from_secs(1);
const DEVICE_TRUST_POLL_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RECENT_COMMAND_IDS: usize = 128;
const MAX_COMMAND_ID_CHARS: usize = 128;

static COMMAND_GATE: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));
static RECENT_COMMAND_IDS: LazyLock<AsyncMutex<RecentCommandIds>> =
    LazyLock::new(|| AsyncMutex::new(RecentCommandIds::default()));

#[derive(Default)]
struct RecentCommandIds {
    order: VecDeque<String>,
    ids: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SatelliteConfig {
    bind: String,
    token: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PersistedSatelliteSettings {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_bind")]
    bind: String,
    #[serde(default)]
    token: Option<String>,
}

fn default_bind() -> String {
    DEFAULT_BIND.to_owned()
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

impl ResponseLanguage {
    fn tts_language(self) -> TtsLanguage {
        match self {
            Self::Vi => TtsLanguage::Vietnamese,
            Self::En => TtsLanguage::English,
            Self::Auto => TtsLanguage::Auto,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Hello {
        token: String,
        device_id: String,
        device_name: Option<String>,
        protocol: u32,
    },
    Command {
        id: String,
        text: String,
        #[serde(default)]
        response_language: ResponseLanguage,
    },
    Cancel {
        #[serde(default)]
        id: Option<String>,
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
    Cancelled {
        id: Option<&'a str>,
        accepted: bool,
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

enum IncomingFrame {
    Text(String),
    Ping(Vec<u8>),
    Pong,
    Close(Vec<u8>),
}

struct WebSocketConnection {
    stream: TcpStream,
    buffered: Vec<u8>,
}

impl WebSocketConnection {
    async fn accept(mut stream: TcpStream) -> Result<Self, String> {
        let mut request = Vec::with_capacity(2048);
        let header_end = loop {
            if let Some(end) = find_header_end(&request) {
                break end;
            }
            if request.len() >= MAX_HTTP_HEADER_BYTES {
                return Err("WebSocket upgrade header is too large".to_owned());
            }

            let mut chunk = [0_u8; 1024];
            let read = stream
                .read(&mut chunk)
                .await
                .map_err(|error| format!("WebSocket upgrade read failed: {error}"))?;
            if read == 0 {
                return Err("client disconnected during WebSocket upgrade".to_owned());
            }
            request.extend_from_slice(&chunk[..read]);
        };

        let header = std::str::from_utf8(&request[..header_end])
            .map_err(|_| "WebSocket upgrade header is not valid UTF-8".to_owned())?;
        let mut lines = header.split("\r\n");
        let request_line = lines.next().unwrap_or_default();
        if !request_line.starts_with("GET ") || !request_line.ends_with(" HTTP/1.1") {
            return Err("WebSocket upgrade requires HTTP/1.1 GET".to_owned());
        }

        let mut websocket_key = None::<String>;
        let mut upgrade_ok = false;
        let mut connection_ok = false;
        let mut version_ok = false;
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let name = name.trim();
            let value = value.trim();
            if name.eq_ignore_ascii_case("Sec-WebSocket-Key") {
                websocket_key = Some(value.to_owned());
            } else if name.eq_ignore_ascii_case("Sec-WebSocket-Version") {
                version_ok = value == "13";
            } else if name.eq_ignore_ascii_case("Upgrade") {
                upgrade_ok = value.eq_ignore_ascii_case("websocket");
            } else if name.eq_ignore_ascii_case("Connection") {
                connection_ok = value
                    .split(',')
                    .any(|token| token.trim().eq_ignore_ascii_case("upgrade"));
            }
        }

        if !(upgrade_ok && connection_ok && version_ok) {
            return Err("invalid WebSocket upgrade headers".to_owned());
        }
        let key = websocket_key.ok_or_else(|| "WebSocket key is missing".to_owned())?;
        let accept_key = websocket_accept_key(&key);
        let response = format!(
            "HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: {accept_key}\r\n\r\n"
        );
        stream
            .write_all(response.as_bytes())
            .await
            .map_err(|error| format!("WebSocket upgrade response failed: {error}"))?;

        Ok(Self {
            stream,
            buffered: request[header_end..].to_vec(),
        })
    }

    async fn read_frame(&mut self) -> Result<IncomingFrame, String> {
        let mut header = [0_u8; 2];
        self.read_exact(&mut header).await?;

        let fin = header[0] & 0x80 != 0;
        let opcode = header[0] & 0x0f;
        let masked = header[1] & 0x80 != 0;
        if !fin {
            return Err("fragmented WebSocket frames are not supported by protocol v1".to_owned());
        }
        if !masked {
            return Err("client WebSocket frames must be masked".to_owned());
        }

        let mut payload_len = u64::from(header[1] & 0x7f);
        if payload_len == 126 {
            let mut extended = [0_u8; 2];
            self.read_exact(&mut extended).await?;
            payload_len = u64::from(u16::from_be_bytes(extended));
        } else if payload_len == 127 {
            let mut extended = [0_u8; 8];
            self.read_exact(&mut extended).await?;
            payload_len = u64::from_be_bytes(extended);
        }
        if payload_len > MAX_FRAME_BYTES as u64 {
            return Err(format!(
                "WebSocket frame exceeds {MAX_FRAME_BYTES} byte satellite limit"
            ));
        }

        let mut mask = [0_u8; 4];
        self.read_exact(&mut mask).await?;
        let mut payload = vec![0_u8; payload_len as usize];
        self.read_exact(&mut payload).await?;
        for (index, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[index % 4];
        }

        match opcode {
            0x1 => String::from_utf8(payload)
                .map(IncomingFrame::Text)
                .map_err(|_| "WebSocket text frame is not valid UTF-8".to_owned()),
            0x8 => Ok(IncomingFrame::Close(payload)),
            0x9 => Ok(IncomingFrame::Ping(payload)),
            0xA => Ok(IncomingFrame::Pong),
            _ => Err(format!("unsupported WebSocket opcode 0x{opcode:x}")),
        }
    }

    async fn send_text(&mut self, text: &str) -> Result<(), String> {
        self.send_frame(0x1, text.as_bytes()).await
    }

    async fn send_pong(&mut self, payload: &[u8]) -> Result<(), String> {
        self.send_frame(0xA, payload).await
    }

    async fn send_close(&mut self, payload: &[u8]) -> Result<(), String> {
        self.send_frame(0x8, payload).await
    }

    async fn send_frame(&mut self, opcode: u8, payload: &[u8]) -> Result<(), String> {
        if payload.len() > MAX_FRAME_BYTES {
            return Err("server WebSocket payload exceeds satellite limit".to_owned());
        }

        let mut frame = Vec::with_capacity(payload.len() + 10);
        frame.push(0x80 | opcode);
        match payload.len() {
            len if len <= 125 => frame.push(len as u8),
            len if len <= u16::MAX as usize => {
                frame.push(126);
                frame.extend_from_slice(&(len as u16).to_be_bytes());
            }
            len => {
                frame.push(127);
                frame.extend_from_slice(&(len as u64).to_be_bytes());
            }
        }
        frame.extend_from_slice(payload);
        self.stream
            .write_all(&frame)
            .await
            .map_err(|error| format!("WebSocket send failed: {error}"))
    }

    async fn read_exact(&mut self, target: &mut [u8]) -> Result<(), String> {
        let from_buffer = target.len().min(self.buffered.len());
        if from_buffer > 0 {
            target[..from_buffer].copy_from_slice(&self.buffered[..from_buffer]);
            self.buffered.drain(..from_buffer);
        }
        if from_buffer < target.len() {
            self.stream
                .read_exact(&mut target[from_buffer..])
                .await
                .map_err(|error| format!("WebSocket receive failed: {error}"))?;
        }
        Ok(())
    }
}

pub fn setup(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        supervise_server(app).await;
    });
}

async fn supervise_server(app: AppHandle) {
    let mut active_config: Option<SatelliteConfig> = None;
    let mut server_task: Option<tauri::async_runtime::JoinHandle<()>> = None;
    let mut last_config_error: Option<String> = None;

    loop {
        let desired_config = match config_from_app(&app) {
            Ok(config) => {
                last_config_error = None;
                config
            }
            Err(error) => {
                if last_config_error.as_deref() != Some(error.as_str()) {
                    warn!(%error, "Android voice satellite configuration was rejected");
                    last_config_error = Some(error);
                }
                None
            }
        };

        let server_finished = server_task
            .as_ref()
            .is_some_and(|task| task.inner().is_finished());
        if desired_config != active_config || server_finished {
            if let Some(task) = server_task.take() {
                task.abort();
            }

            active_config = desired_config.clone();
            server_task = desired_config.map(|config| {
                let server_app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = run_server(server_app, config).await {
                        warn!(%error, "Android voice satellite server stopped");
                    }
                })
            });

            if active_config.is_none() {
                info!("Android voice satellite listener is disabled");
            }
        }

        tokio::time::sleep(SETTINGS_POLL_INTERVAL).await;
    }
}

fn config_from_app(app: &AppHandle) -> Result<Option<SatelliteConfig>, String> {
    if let Ok(token) = std::env::var(TOKEN_ENV) {
        let token = token.trim().to_owned();
        if token.len() < 16 {
            return Err(format!("{TOKEN_ENV} must contain at least 16 characters"));
        }
        let bind = std::env::var(BIND_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(default_bind);
        return Ok(Some(SatelliteConfig { bind, token }));
    }

    let state = app.state::<DesktopState>();
    let settings_path = state
        .runtime_paths
        .app_local_data
        .join("settings")
        .join(SETTINGS_FILE);
    let bytes = match fs::read(&settings_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "cannot read satellite settings {}: {error}",
                settings_path.display()
            ));
        }
    };
    let settings: PersistedSatelliteSettings = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "cannot parse satellite settings {}: {error}",
            settings_path.display()
        )
    })?;
    if !settings.enabled {
        return Ok(None);
    }
    let bind = settings.bind.trim();
    if bind.is_empty() {
        return Err("satellite bind address cannot be empty".into());
    }
    let token = settings
        .token
        .map(|value| value.trim().to_owned())
        .filter(|value| value.len() >= 16)
        .ok_or_else(|| "satellite is enabled but has no valid pairing token".to_owned())?;

    Ok(Some(SatelliteConfig {
        bind: bind.to_owned(),
        token,
    }))
}

async fn run_server(app: AppHandle, config: SatelliteConfig) -> Result<(), String> {
    let listener = TcpListener::bind(&config.bind)
        .await
        .map_err(|error| format!("cannot bind voice satellite server to {}: {error}", config.bind))?;
    let mut connections = tokio::task::JoinSet::new();

    info!(
        bind = %config.bind,
        "Android voice satellite WebSocket server is listening"
    );

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, peer) = accepted
                    .map_err(|error| format!("voice satellite accept failed: {error}"))?;
                let connection_app = app.clone();
                let expected_token = config.token.clone();
                connections.spawn(async move {
                    if let Err(error) = handle_connection(connection_app, stream, expected_token).await {
                        debug!(%peer, %error, "voice satellite connection closed");
                    }
                });
            }
            Some(result) = connections.join_next(), if !connections.is_empty() => {
                if let Err(error) = result {
                    debug!(%error, "voice satellite connection task ended unexpectedly");
                }
            }
        }
    }
}

async fn handle_connection(
    app: AppHandle,
    stream: TcpStream,
    expected_token: String,
) -> Result<(), String> {
    let mut socket = WebSocketConnection::accept(stream).await?;
    let device_id = match authenticate(&app, &mut socket, &expected_token).await {
        Ok(device_id) => device_id,
        Err(error) => {
            let _ = send_json(
                &mut socket,
                &ServerMessage::Error {
                    id: None,
                    code: "authentication_failed",
                    message: "Pairing or trusted-device authentication was rejected by the desktop.",
                },
            )
            .await;
            return Err(error);
        }
    };

    send_json(
        &mut socket,
        &ServerMessage::Ready {
            protocol: PROTOCOL_VERSION,
            response_language: ResponseLanguage::Vi,
        },
    )
    .await?;

    loop {
        let frame = tokio::select! {
            frame = socket.read_frame() => frame?,
            _ = tokio::time::sleep(DEVICE_TRUST_POLL_INTERVAL) => {
                if satellite_devices::is_device_revoked(&app, &device_id)? {
                    let _ = send_json(
                        &mut socket,
                        &ServerMessage::Error {
                            id: None,
                            code: "device_revoked",
                            message: "This Android satellite device has been revoked on the desktop.",
                        },
                    )
                    .await;
                    return Err(format!("satellite device `{device_id}` was revoked"));
                }
                continue;
            }
        };

        match frame {
            IncomingFrame::Text(text) => {
                let message = match serde_json::from_str::<ClientMessage>(&text) {
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
                        if !is_valid_command_id(&id) {
                            send_json(
                                &mut socket,
                                &ServerMessage::Error {
                                    id: None,
                                    code: "invalid_command_id",
                                    message: "command id is empty, too long, or contains unsupported characters",
                                },
                            )
                            .await?;
                            continue;
                        }
                        handle_command(&app, &mut socket, &id, &text, response_language).await?;
                    }
                    ClientMessage::Cancel { id } => {
                        let accepted = cancel_active_interaction(&app).await;
                        send_json(
                            &mut socket,
                            &ServerMessage::Cancelled {
                                id: id.as_deref(),
                                accepted,
                            },
                        )
                        .await?;
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
            IncomingFrame::Ping(payload) => socket.send_pong(&payload).await?,
            IncomingFrame::Pong => {}
            IncomingFrame::Close(payload) => {
                let _ = socket.send_close(&payload).await;
                break;
            }
        }
    }

    Ok(())
}

async fn authenticate(
    app: &AppHandle,
    socket: &mut WebSocketConnection,
    expected_token: &str,
) -> Result<String, String> {
    let first = tokio::time::timeout(Duration::from_secs(5), socket.read_frame())
        .await
        .map_err(|_| "voice satellite authentication timed out".to_owned())??;

    let IncomingFrame::Text(text) = first else {
        return Err("voice satellite first message must be a JSON hello message".to_owned());
    };

    let hello = serde_json::from_str::<ClientMessage>(&text)
        .map_err(|error| format!("invalid voice satellite hello: {error}"))?;
    let ClientMessage::Hello {
        token,
        device_id,
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

    let (device_id, device_name) = satellite_devices::record_authenticated_device(
        app,
        &device_id,
        device_name.as_deref(),
    )?;
    info!(
        device_id = %device_id,
        device = %device_name,
        "Android voice satellite authenticated"
    );
    Ok(device_id)
}

async fn handle_command(
    app: &AppHandle,
    socket: &mut WebSocketConnection,
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
    let phase = state.core.state().await;
    if phase != AssistantState::Idle && phase != AssistantState::Error {
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

    if !remember_command_id(id).await {
        send_json(
            socket,
            &ServerMessage::Error {
                id: Some(id),
                code: "duplicate_command",
                message: "this command id was already accepted; the desktop will not execute it twice",
            },
        )
        .await?;
        return Ok(());
    }

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
            let cancelled = is_cancellation_error(&error);
            send_json(
                socket,
                &ServerMessage::Error {
                    id: Some(id),
                    code: if cancelled { "cancelled" } else { "assistant_error" },
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

    match speak_response(app, &response, response_language.tts_language()).await {
        Ok(()) => {
            send_json(
                socket,
                &ServerMessage::Response {
                    id,
                    text: &response,
                    tts_error: None,
                },
            )
            .await
        }
        Err(error) if is_cancellation_error(&error) => {
            send_json(
                socket,
                &ServerMessage::Response {
                    id,
                    text: &response,
                    tts_error: None,
                },
            )
            .await?;
            send_json(
                socket,
                &ServerMessage::Cancelled {
                    id: Some(id),
                    accepted: true,
                },
            )
            .await
        }
        Err(error) => {
            send_json(
                socket,
                &ServerMessage::Response {
                    id,
                    text: &response,
                    tts_error: Some(&error),
                },
            )
            .await
        }
    }
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

async fn speak_response(
    app: &AppHandle,
    text: &str,
    language: TtsLanguage,
) -> Result<(), String> {
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

    let speak_result = state
        .tts
        .speak_with_language(text, language)
        .await
        .map_err(|error| error.to_string());
    let finish_result = state
        .core
        .finish_speaking()
        .await
        .map_err(|error| error.to_string());
    wake.resume_after(Duration::from_millis(900));

    speak_result?;
    finish_result
}

async fn cancel_active_interaction(app: &AppHandle) -> bool {
    let state = app.state::<DesktopState>();
    let phase = state.core.state().await;
    match phase {
        AssistantState::Listening => {
            let generation = voice_runtime::cancellation::cancel_current();
            match state.core.cancel_listening().await {
                Ok(()) => {
                    debug!(generation, "cancelled listening from Android satellite control channel");
                    true
                }
                Err(error) => {
                    warn!(%error, generation, "failed to cancel listening from Android satellite");
                    false
                }
            }
        }
        AssistantState::Speaking => {
            let accepted = state.tts.cancel();
            if accepted {
                debug!("requested SAPI cancellation from Android satellite");
            }
            accepted
        }
        AssistantState::Processing => {
            if state.client.cancel_active_turn() {
                debug!("requested Antigravity cancellation from Android satellite");
                return true;
            }
            let core = state.core.clone();
            let client = state.client.clone();
            drop(state);
            tokio::time::sleep(Duration::from_millis(20)).await;
            if core.state().await == AssistantState::Processing && client.cancel_active_turn() {
                debug!("requested delayed Antigravity cancellation from Android satellite");
                true
            } else {
                false
            }
        }
        phase => {
            debug!(?phase, "ignored Android satellite cancel in non-cancellable phase");
            false
        }
    }
}

fn is_cancellation_error(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    normalized.contains("cancelled") || normalized.contains("canceled")
}

fn is_valid_command_id(id: &str) -> bool {
    let id = id.trim();
    !id.is_empty()
        && id.len() <= MAX_COMMAND_ID_CHARS
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

async fn remember_command_id(id: &str) -> bool {
    let mut recent = RECENT_COMMAND_IDS.lock().await;
    if recent.ids.contains(id) {
        return false;
    }
    recent.ids.insert(id.to_owned());
    recent.order.push_back(id.to_owned());
    while recent.order.len() > MAX_RECENT_COMMAND_IDS {
        if let Some(expired) = recent.order.pop_front() {
            recent.ids.remove(&expired);
        }
    }
    true
}

async fn send_json<T: Serialize>(
    socket: &mut WebSocketConnection,
    message: &T,
) -> Result<(), String> {
    let json = serde_json::to_string(message)
        .map_err(|error| format!("cannot encode voice satellite response: {error}"))?;
    socket.send_text(&json).await
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
}

fn websocket_accept_key(key: &str) -> String {
    let mut input = Vec::with_capacity(key.len() + WEBSOCKET_GUID.len());
    input.extend_from_slice(key.trim().as_bytes());
    input.extend_from_slice(WEBSOCKET_GUID.as_bytes());
    base64_encode(&sha1_digest(&input))
}

fn sha1_digest(input: &[u8]) -> [u8; 20] {
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    let mut h0 = 0x67452301_u32;
    let mut h1 = 0xEFCDAB89_u32;
    let mut h2 = 0x98BADCFE_u32;
    let mut h3 = 0x10325476_u32;
    let mut h4 = 0xC3D2E1F0_u32;

    for chunk in padded.chunks_exact(64) {
        let mut words = [0_u32; 80];
        for (index, word) in words.iter_mut().take(16).enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }
        for index in 16..80 {
            words[index] = (words[index - 3]
                ^ words[index - 8]
                ^ words[index - 14]
                ^ words[index - 16])
                .rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (index, word) in words.iter().enumerate() {
            let (function, constant) = match index {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(function)
                .wrapping_add(e)
                .wrapping_add(constant)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut output = [0_u8; 20];
    for (index, value) in [h0, h1, h2, h3, h4].iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&value.to_be_bytes());
    }
    output
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let a = chunk[0];
        let b = chunk.get(1).copied().unwrap_or(0);
        let c = chunk.get(2).copied().unwrap_or(0);
        output.push(TABLE[(a >> 2) as usize] as char);
        output.push(TABLE[(((a & 0x03) << 4) | (b >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[(((b & 0x0f) << 2) | (c >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(c & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
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
    fn websocket_accept_matches_rfc_6455_example() {
        assert_eq!(
            websocket_accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

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

    #[test]
    fn response_language_maps_to_tts_hint() {
        assert_eq!(ResponseLanguage::Vi.tts_language(), TtsLanguage::Vietnamese);
        assert_eq!(ResponseLanguage::En.tts_language(), TtsLanguage::English);
        assert_eq!(ResponseLanguage::Auto.tts_language(), TtsLanguage::Auto);
    }

    #[test]
    fn command_id_validation_is_bounded() {
        assert!(is_valid_command_id("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!is_valid_command_id(""));
        assert!(!is_valid_command_id("bad id"));
        assert!(!is_valid_command_id(&"a".repeat(MAX_COMMAND_ID_CHARS + 1)));
    }

    #[test]
    fn cancellation_error_detection_accepts_both_spellings() {
        assert!(is_cancellation_error("text-to-speech request was cancelled"));
        assert!(is_cancellation_error("request canceled"));
        assert!(!is_cancellation_error("device unavailable"));
    }
}
