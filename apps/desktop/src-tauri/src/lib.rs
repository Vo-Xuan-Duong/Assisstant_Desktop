use std::sync::Arc;

use antigravity_bridge::{AntigravityClient, AntigravityConfig, CliHealth};
use assistant_common::{AssistantEvent, AssistantState, SessionId, UserRequest};
use assistant_core::{AssistantCore, EventSink};
use async_trait::async_trait;
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
use tracing::warn;
use tracing_subscriber::EnvFilter;

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
    session_id: RwLock<SessionId>,
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

    let session_id = state.session_id.read().await.clone();
    state
        .core
        .handle_text(UserRequest::new(session_id, prompt))
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

fn show_main_window(app: &AppHandle) {
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
            let client = Arc::new(AntigravityClient::new(AntigravityConfig::default()));
            let sink = Arc::new(TauriEventSink {
                app: app.handle().clone(),
            });
            let core = Arc::new(AssistantCore::new(Arc::clone(&client), sink));

            app.manage(DesktopState {
                client,
                core,
                session_id: RwLock::new(SessionId::new()),
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
