mod edge;
mod permission_desktop;
mod readiness;
mod resource_registry;
mod runtime_paths;
mod wake_desktop;

use std::sync::{Arc, Mutex};

#[cfg(feature = "voice-whisper")]
use std::{path::PathBuf, time::Duration};

use antigravity_bridge::{AntigravityClient, AntigravityConfig, CliHealth};
use assistant_common::{AssistantEvent, AssistantState, SessionId, UserRequest};
use assistant_core::{AssistantCore, EventSink};
use async_trait::async_trait;
use context_engine::{ContextConfig, ContextEngine};
use permission_desktop::PermissionDesktopService;
use resource_registry::{ResourceRegistry, ResourceState, RuntimeResourceSnapshot};
use runtime_paths::RuntimePaths;
use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WindowEvent,
};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
};
use tokio::sync::RwLock;
#[cfg(feature = "voice-whisper")]
use tokio::sync::Mutex as AsyncMutex;
use tracing::{debug, warn};
use tracing_subscriber::EnvFilter;
use voice_runtime::tts::{TextToSpeech, WindowsSapiTts};
#[cfg(feature = "voice-whisper")]
use voice_runtime::{
    stt::SpeechRecognizer,
    vad::{Utterance, UtteranceSegmenter, VadEvent},
    whisper::{WhisperConfig, WhisperRecognizer},
    MicrophoneConfig, MicrophoneStream,
};
#[cfg(feature = "wake-word")]
use voice_runtime::wake_runtime::WakeRuntimeEvent;
use wake_desktop::{WakeService, WakeStatus};
use windows_tools::window::{self, WindowHandle};

#[derive(Clone)]
struct TauriEventSink {
    app: AppHandle,
}

#[async_trait]
impl EventSink for TauriEventSink {
    async fn publish(&self, event: AssistantEvent) {
        if let Err(error) = self.app.emit("assistant:event", event) {
            warn!(%error, "failed to emit assistant event to desktop UI");
        }
    }
}

type DesktopCore = AssistantCore<AntigravityClient, TauriEventSink>;

#[cfg(feature = "voice-whisper")]
struct WhisperVoiceState {
    model_path: PathBuf,
    recognizer: AsyncMutex<Option<WhisperRecognizer>>,
    turn_gate: AsyncMutex<()>,
}

struct DesktopState {
    client: Arc<AntigravityClient>,
    core: Arc<DesktopCore>,
    context: ContextEngine,
    runtime_paths: RuntimePaths,
    resources: ResourceRegistry,
    tts: WindowsSapiTts,
    session_id: RwLock<SessionId>,
    source_window: Mutex<Option<WindowHandle>>,
    #[cfg(feature = "voice-whisper")]
    voice: WhisperVoiceState,
}

#[derive(Debug, Serialize)]
struct RuntimeHealth {
    state: &'static str,
    detail: Option<String>,
    conversation_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct VoiceCapabilities {
    tts_available: bool,
    whisper_compiled: bool,
    model_path: Option<String>,
    model_available: bool,
}

#[derive(Debug, Serialize)]
struct VoiceTurnResult {
    transcript: String,
    response: String,
    tts_error: Option<String>,
}

#[tauri::command]
async fn assistant_health(state: State<'_, DesktopState>) -> RuntimeHealth {
    let conversation_id = state.client.conversation_id().await;
    match state.client.health().await {
        CliHealth::Available { detail } => RuntimeHealth {
            state: "available",
            detail,
            conversation_id,
        },
        CliHealth::Missing => RuntimeHealth {
            state: "missing",
            detail: Some("Không tìm thấy lệnh `agy` trong PATH.".into()),
            conversation_id,
        },
        CliHealth::Unhealthy { message } => RuntimeHealth {
            state: "unhealthy",
            detail: Some(message),
            conversation_id,
        },
    }
}

#[tauri::command]
async fn assistant_readiness(
    state: State<'_, DesktopState>,
    permission: State<'_, PermissionDesktopService>,
    wake: State<'_, WakeService>,
) -> readiness::RuntimeReadinessReport {
    readiness::collect(state.inner(), permission.inner(), wake.inner()).await
}

#[tauri::command]
async fn assistant_resources(state: State<'_, DesktopState>) -> RuntimeResourceSnapshot {
    state.resources.snapshot()
}

#[tauri::command]
async fn assistant_wake_status(wake: State<'_, WakeService>) -> WakeStatus {
    wake.status()
}

#[tauri::command]
async fn assistant_wake_set_enabled(
    enabled: bool,
    wake: State<'_, WakeService>,
) -> Result<WakeStatus, String> {
    wake.set_enabled(enabled).await?;
    tokio::task::yield_now().await;
    Ok(wake.status())
}

async fn complete_prompt(prompt: &str, state: &DesktopState) -> Result<String, String> {
    if state.core.state().await == AssistantState::Error {
        state.core.recover().await.map_err(|error| error.to_string())?;
    }

    let source_window = state
        .source_window
        .lock()
        .map(|guard| *guard)
        .unwrap_or(None);
    let context = state.context.collect_for_window(prompt, source_window).await;
    for warning in &context.warnings {
        warn!(%warning, "desktop context source was unavailable");
    }

    let enriched_prompt = match context.prompt_block() {
        Some(block) => {
            debug!(
                active_window = context.active_window.is_some(),
                clipboard = context.clipboard_text.is_some(),
                screen = context.screen.is_some(),
                "adding local desktop context to Antigravity request"
            );
            format!("{block}\n\n<user_request>\n{prompt}\n</user_request>")
        }
        None => prompt.to_owned(),
    };

    let session_id = state.session_id.read().await.clone();
    state
        .core
        .handle_text(UserRequest::new(session_id, enriched_prompt))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn assistant_submit(text: String, state: State<'_, DesktopState>) -> Result<String, String> {
    let prompt = text.trim();
    if prompt.is_empty() {
        return Err("Yêu cầu không được để trống.".into());
    }
    complete_prompt(prompt, state.inner()).await
}

#[tauri::command]
async fn assistant_voice_capabilities(state: State<'_, DesktopState>) -> VoiceCapabilities {
    let whisper = state.resources.whisper_status();
    VoiceCapabilities {
        tts_available: true,
        whisper_compiled: whisper.compiled,
        model_path: whisper.files.first().map(|file| file.path.clone()),
        model_available: whisper.state == ResourceState::Ready,
    }
}

#[tauri::command]
async fn assistant_speak(
    text: String,
    state: State<'_, DesktopState>,
    wake: State<'_, WakeService>,
) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Không có nội dung để đọc.".into());
    }
    if state.core.state().await != AssistantState::Idle {
        return Err("Assistant đang bận với một tác vụ khác.".into());
    }

    wake.suspend().await;
    let begin = state
        .core
        .begin_speaking()
        .await
        .map_err(|error| error.to_string());
    if let Err(error) = begin {
        wake.resume_after(std::time::Duration::from_millis(900));
        return Err(error);
    }

    let speak_result = state.tts.speak(text).await.map_err(|error| error.to_string());
    let finish_result = state
        .core
        .finish_speaking()
        .await
        .map_err(|error| error.to_string());
    wake.resume_after(std::time::Duration::from_millis(900));

    speak_result?;
    finish_result
}

#[tauri::command]
async fn assistant_voice_turn(
    app: AppHandle,
    state: State<'_, DesktopState>,
    wake: State<'_, WakeService>,
) -> Result<VoiceTurnResult, String> {
    #[cfg(not(feature = "voice-whisper"))]
    {
        let _ = (app, state, wake);
        Err(
            "Bản build hiện tại chưa bật feature `voice-whisper`. Text/TTS vẫn hoạt động bình thường."
                .into(),
        )
    }

    #[cfg(feature = "voice-whisper")]
    {
        if state.core.state().await == AssistantState::Error {
            state.core.recover().await.map_err(|error| error.to_string())?;
        }
        if state.core.state().await != AssistantState::Idle {
            return Err("Assistant đang bận với một tác vụ khác.".into());
        }
        if !state.voice.model_path.is_file() {
            return Err(format!(
                "Chưa có Whisper model tại {}. Có thể override bằng ASSISTANT_WHISPER_MODEL.",
                state.voice.model_path.display()
            ));
        }

        wake.suspend().await;
        let result = run_voice_turn_inner(&app, state.inner()).await;
        wake.resume_after(std::time::Duration::from_millis(900));
        result
    }
}

#[cfg(feature = "voice-whisper")]
async fn run_voice_turn_inner(
    app: &AppHandle,
    state: &DesktopState,
) -> Result<VoiceTurnResult, String> {
    let _turn = state.voice.turn_gate.lock().await;

    state
        .core
        .begin_listening()
        .await
        .map_err(|error| error.to_string())?;

    let utterance = match capture_one_utterance(app).await {
        Ok(utterance) => utterance,
        Err(error) => return fail_listening(state, error).await,
    };

    let recognizer = match get_or_load_recognizer(state).await {
        Ok(recognizer) => recognizer,
        Err(error) => return fail_listening(state, error).await,
    };

    let transcript = match recognizer.transcribe(utterance).await {
        Ok(transcript) if !transcript.is_empty() => transcript,
        Ok(_) => {
            return fail_listening(state, "Không nhận diện được nội dung giọng nói.".into()).await;
        }
        Err(error) => return fail_listening(state, error.to_string()).await,
    };

    let response = complete_prompt(&transcript.text, state).await?;

    state
        .core
        .begin_speaking()
        .await
        .map_err(|error| error.to_string())?;
    let tts_error = state.tts.speak(&response).await.err().map(|error| error.to_string());
    if let Err(error) = state.core.finish_speaking().await {
        warn!(%error, "failed to finish voice speaking state");
    }

    Ok(VoiceTurnResult {
        transcript: transcript.text,
        response,
        tts_error,
    })
}

#[cfg(feature = "voice-whisper")]
async fn fail_listening<T>(state: &DesktopState, message: String) -> Result<T, String> {
    if let Err(error) = state.core.cancel_listening().await {
        warn!(%error, "failed to cancel listening state after voice failure");
    }
    Err(message)
}

#[cfg(feature = "voice-whisper")]
async fn get_or_load_recognizer(state: &DesktopState) -> Result<WhisperRecognizer, String> {
    let mut slot = state.voice.recognizer.lock().await;
    if let Some(recognizer) = slot.as_ref() {
        return Ok(recognizer.clone());
    }

    let mut config = WhisperConfig::new(state.voice.model_path.clone());
    config.language = Some("vi".into());
    let recognizer = WhisperRecognizer::load(config).map_err(|error| error.to_string())?;
    *slot = Some(recognizer.clone());
    Ok(recognizer)
}

#[cfg(feature = "voice-whisper")]
async fn capture_one_utterance(app: &AppHandle) -> Result<Utterance, String> {
    let mut microphone = MicrophoneStream::open_default(MicrophoneConfig::default())
        .map_err(|error| error.to_string())?;
    let mut segmenter = UtteranceSegmenter::default();

    let recording = async {
        while let Some(chunk) = microphone.next_chunk().await {
            let _ = app.emit("voice:level", chunk.level);
            match segmenter.push(chunk) {
                VadEvent::UtteranceReady(utterance) => return Ok(utterance),
                VadEvent::DiscardedShortUtterance => {
                    debug!("discarded short voice utterance");
                }
                VadEvent::Idle | VadEvent::SpeechStarted | VadEvent::SpeechContinues => {}
            }
        }

        Err("Microphone stream đã kết thúc trước khi nhận được câu nói.".to_owned())
    };

    tokio::time::timeout(Duration::from_secs(25), recording)
        .await
        .map_err(|_| "Không phát hiện câu nói hoàn chỉnh trong 25 giây.".to_owned())?
}

#[tauri::command]
async fn assistant_restart(state: State<'_, DesktopState>) -> Result<(), String> {
    state.client.restart().await.map_err(|error| error.to_string())?;
    state.core.recover().await.map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
async fn assistant_reset(state: State<'_, DesktopState>) -> Result<(), String> {
    state.client.reset().await;
    *state.session_id.write().await = SessionId::new();
    state.core.recover().await.map_err(|error| error.to_string())?;
    Ok(())
}

fn current_external_window() -> Option<WindowHandle> {
    let handle = window::get_active_handle().ok()?;
    let info = window::get(handle).ok()?;
    (info.process_id != std::process::id()).then_some(handle)
}

fn remember_source_window(app: &AppHandle) {
    let Some(handle) = current_external_window() else {
        return;
    };

    let state = app.state::<DesktopState>();
    if let Ok(mut source) = state.source_window.lock() {
        *source = Some(handle);
    }
}

fn source_window(app: &AppHandle) -> Option<WindowHandle> {
    app.state::<DesktopState>()
        .source_window
        .lock()
        .map(|source| *source)
        .unwrap_or(None)
}

fn show_main_window(app: &AppHandle) {
    remember_source_window(app);
    edge::activate(app, source_window(app));

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn hide_main_window(app: &AppHandle) {
    edge::hide(app);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[cfg(feature = "wake-word")]
fn setup_wake_events(app: &AppHandle) {
    let wake = app.state::<WakeService>();
    let Some(mut events) = wake.subscribe() else {
        return;
    };
    let app = app.clone();

    tauri::async_runtime::spawn(async move {
        loop {
            match events.recv().await {
                Ok(event) => {
                    let _ = app.emit("wake:event", &event);
                    if matches!(event, WakeRuntimeEvent::Detected { .. }) {
                        show_main_window(&app);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!(skipped, "desktop wake event receiver lagged");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

#[cfg(not(feature = "wake-word"))]
fn setup_wake_events(_app: &AppHandle) {}

fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Mở Assistant", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Ẩn cửa sổ", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Thoát", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &quit])?;

    TrayIconBuilder::new()
        .tooltip("Assisstant Desktop")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "hide" => hide_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn setup_shortcut(app: &mut tauri::App) -> tauri::Result<()> {
    let activation = Shortcut::new(Some(Modifiers::ALT), Code::Space);
    let handler_shortcut = activation;

    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, shortcut, event| {
                if shortcut == &handler_shortcut && event.state() == ShortcutState::Pressed {
                    show_main_window(app);
                }
            })
            .build(),
    )?;

    app.global_shortcut().register(activation)?;
    Ok(())
}

pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();

    tauri::Builder::default()
        .setup(|app| {
            let initial_source_window = current_external_window();
            let runtime_paths = RuntimePaths::prepare(app.handle()).map_err(std::io::Error::other)?;
            let resources = ResourceRegistry::resolve(&runtime_paths).map_err(std::io::Error::other)?;
            let wake_service = WakeService::setup(&resources);
            let (permission_service, broker_environment) =
                PermissionDesktopService::setup(app.handle())?;
            let mut antigravity_config = AntigravityConfig::default();
            antigravity_config.working_directory = Some(runtime_paths.runtime_dir.clone());
            antigravity_config.set_environment(
                "ASSISTANT_MCP_CONFIG",
                runtime_paths.mcp_config_path.to_string_lossy().into_owned(),
            );
            for (key, value) in broker_environment {
                antigravity_config.set_environment(key, value);
            }
            let client = Arc::new(AntigravityClient::new(antigravity_config));
            let sink = Arc::new(TauriEventSink {
                app: app.handle().clone(),
            });
            let core = Arc::new(AssistantCore::new(Arc::clone(&client), sink));
            let context = ContextEngine::new(ContextConfig {
                artifact_dir: runtime_paths.context_dir.clone(),
                ..ContextConfig::default()
            });

            #[cfg(feature = "voice-whisper")]
            let model_path = resources.whisper_model_path().to_path_buf();

            app.manage(DesktopState {
                client,
                core,
                context,
                runtime_paths,
                resources,
                tts: WindowsSapiTts::default(),
                session_id: RwLock::new(SessionId::new()),
                source_window: Mutex::new(initial_source_window),
                #[cfg(feature = "voice-whisper")]
                voice: WhisperVoiceState {
                    model_path,
                    recognizer: AsyncMutex::new(None),
                    turn_gate: AsyncMutex::new(()),
                },
            });
            app.manage(permission_service);
            app.manage(wake_service);

            edge::setup(app.handle())?;
            setup_wake_events(app.handle());
            setup_shortcut(app)?;
            setup_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    edge::hide(window.app_handle());
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            assistant_health,
            assistant_readiness,
            assistant_resources,
            assistant_wake_status,
            assistant_wake_set_enabled,
            assistant_submit,
            assistant_voice_capabilities,
            assistant_voice_turn,
            assistant_speak,
            assistant_restart,
            assistant_reset,
            permission_desktop::assistant_permission_respond
        ])
        .run(tauri::generate_context!())
        .expect("error while running Assisstant Desktop");
}
