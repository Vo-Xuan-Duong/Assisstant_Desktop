use std::sync::{Arc, Mutex};

use antigravity_bridge::{AntigravityClient, AntigravityConfig, CliHealth};
use assistant_common::{AssistantEvent, AssistantState, SessionId, UserRequest};
use assistant_core::{AssistantCore, EventSink};
use async_trait::async_trait;
use context_engine::ContextEngine;
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
use tracing::{debug, warn};
use tracing_subscriber::EnvFilter;
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

struct DesktopState {
    client: Arc<AntigravityClient>,
    core: Arc<DesktopCore>,
    context: ContextEngine,
    session_id: RwLock<SessionId>,
    source_window: Mutex<Option<WindowHandle>>,
}

#[derive(Debug, Serialize)]
struct RuntimeHealth {
    state: &'static str,
    detail: Option<String>,
    conversation_id: Option<String>,
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
async fn assistant_submit(text: String, state: State<'_, DesktopState>) -> Result<String, String> {
    let prompt = text.trim();
    if prompt.is_empty() {
        return Err("Yêu cầu không được để trống.".into());
    }

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

fn show_main_window(app: &AppHandle) {
    // Preserve the application the user was looking at before our UI steals focus.
    // Only its handle is stored here; no pixels or clipboard data are collected.
    remember_source_window(app);

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn hide_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

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
            // During setup the webview has not yet necessarily taken foreground focus,
            // so this also gives the first interaction a useful source window.
            let initial_source_window = current_external_window();
            let client = Arc::new(AntigravityClient::new(AntigravityConfig::default()));
            let sink = Arc::new(TauriEventSink {
                app: app.handle().clone(),
            });
            let core = Arc::new(AssistantCore::new(Arc::clone(&client), sink));

            app.manage(DesktopState {
                client,
                core,
                context: ContextEngine::default(),
                session_id: RwLock::new(SessionId::new()),
                source_window: Mutex::new(initial_source_window),
            });

            setup_shortcut(app)?;
            setup_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            assistant_health,
            assistant_submit,
            assistant_restart,
            assistant_reset
        ])
        .run(tauri::generate_context!())
        .expect("error while running Assisstant Desktop");
}
