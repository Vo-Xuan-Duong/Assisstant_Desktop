use std::{path::PathBuf, sync::Mutex, time::Duration};

use serde::Serialize;

use super::resource_registry::ResourceRegistry;

#[cfg(feature = "wake-word")]
use voice_runtime::{
    sherpa_wake::SherpaWakeWordDetector,
    wake::SherpaWakeConfig,
    wake_runtime::{
        spawn_wake_runtime, WakeRuntimeConfig, WakeRuntimeEvent, WakeRuntimeHandle,
        WakeRuntimeState,
    },
};

#[derive(Debug, Clone, Serialize)]
pub struct WakeStatus {
    pub compiled: bool,
    pub available: bool,
    pub enabled: bool,
    pub state: String,
    pub model_dir: Option<String>,
    pub keywords_path: Option<String>,
    pub detail: Option<String>,
}

pub struct WakeService {
    #[cfg(feature = "wake-word")]
    handle: Option<WakeRuntimeHandle>,
    model_dir: Option<PathBuf>,
    keywords_path: Option<PathBuf>,
    detail: Mutex<Option<String>>,
}

impl WakeService {
    pub fn setup(resources: &ResourceRegistry) -> Self {
        #[cfg(feature = "wake-word")]
        {
            return Self::setup_sherpa(resources);
        }

        #[cfg(not(feature = "wake-word"))]
        {
            Self {
                model_dir: Some(resources.wake_model_dir().to_path_buf()),
                keywords_path: Some(resources.wake_keywords_path().to_path_buf()),
                detail: Mutex::new(Some(
                    "Bản build hiện tại chưa bật feature `wake-word`.".into(),
                )),
            }
        }
    }

    #[cfg(feature = "wake-word")]
    fn setup_sherpa(resources: &ResourceRegistry) -> Self {
        let model_dir = resources.wake_model_dir().to_path_buf();
        let keywords_path = resources.wake_keywords_path().to_path_buf();
        let config = SherpaWakeConfig::gigaspeech_int8(&model_dir, &keywords_path);

        let detector = match SherpaWakeWordDetector::load(config) {
            Ok(detector) => detector,
            Err(error) => {
                return Self {
                    handle: None,
                    model_dir: Some(model_dir),
                    keywords_path: Some(keywords_path),
                    detail: Mutex::new(Some(error.to_string())),
                };
            }
        };

        let enabled_on_start = env_bool("ASSISTANT_WAKE_ENABLED").unwrap_or(false);
        let handle = spawn_wake_runtime(
            Box::new(detector),
            WakeRuntimeConfig {
                enabled_on_start,
                ..WakeRuntimeConfig::default()
            },
        );

        Self {
            handle: Some(handle),
            model_dir: Some(model_dir),
            keywords_path: Some(keywords_path),
            detail: Mutex::new(None),
        }
    }

    pub fn status(&self) -> WakeStatus {
        #[cfg(feature = "wake-word")]
        {
            if let Some(handle) = &self.handle {
                let state = handle.state();
                return WakeStatus {
                    compiled: true,
                    available: true,
                    enabled: !matches!(
                        state,
                        WakeRuntimeState::Disabled | WakeRuntimeState::Stopped
                    ),
                    state: state_name(state).into(),
                    model_dir: self.model_dir.as_ref().map(|path| path.display().to_string()),
                    keywords_path: self
                        .keywords_path
                        .as_ref()
                        .map(|path| path.display().to_string()),
                    detail: self.detail.lock().ok().and_then(|value| value.clone()),
                };
            }

            return WakeStatus {
                compiled: true,
                available: false,
                enabled: false,
                state: "unavailable".into(),
                model_dir: self.model_dir.as_ref().map(|path| path.display().to_string()),
                keywords_path: self
                    .keywords_path
                    .as_ref()
                    .map(|path| path.display().to_string()),
                detail: self.detail.lock().ok().and_then(|value| value.clone()),
            };
        }

        #[cfg(not(feature = "wake-word"))]
        WakeStatus {
            compiled: false,
            available: false,
            enabled: false,
            state: "not_compiled".into(),
            model_dir: self.model_dir.as_ref().map(|path| path.display().to_string()),
            keywords_path: self
                .keywords_path
                .as_ref()
                .map(|path| path.display().to_string()),
            detail: self.detail.lock().ok().and_then(|value| value.clone()),
        }
    }

    pub async fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        #[cfg(feature = "wake-word")]
        {
            let handle = self
                .handle
                .as_ref()
                .ok_or_else(|| self.unavailable_message())?;
            return handle.set_enabled(enabled).await;
        }

        #[cfg(not(feature = "wake-word"))]
        {
            let _ = enabled;
            Err(self.unavailable_message())
        }
    }

    pub async fn suspend(&self) {
        #[cfg(feature = "wake-word")]
        if let Some(handle) = &self.handle {
            let mut state = handle.subscribe_state();
            if handle.suspend().await.is_err() {
                return;
            }

            // Sending the command is not enough: wait until the worker has
            // dropped its CPAL stream before a full voice turn opens a new one.
            let wait = async {
                loop {
                    if matches!(
                        *state.borrow_and_update(),
                        WakeRuntimeState::Suspended
                            | WakeRuntimeState::Disabled
                            | WakeRuntimeState::Stopped
                    ) {
                        break;
                    }
                    if state.changed().await.is_err() {
                        break;
                    }
                }
            };
            let _ = tokio::time::timeout(Duration::from_secs(2), wait).await;
        }
    }

    pub fn resume_after(&self, delay: Duration) {
        #[cfg(feature = "wake-word")]
        if let Some(handle) = self.handle.clone() {
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(delay).await;
                let _ = handle.resume().await;
            });
        }

        #[cfg(not(feature = "wake-word"))]
        let _ = delay;
    }

    #[cfg(feature = "wake-word")]
    pub fn subscribe(&self) -> Option<tokio::sync::broadcast::Receiver<WakeRuntimeEvent>> {
        self.handle.as_ref().map(WakeRuntimeHandle::subscribe)
    }

    fn unavailable_message(&self) -> String {
        self.detail
            .lock()
            .ok()
            .and_then(|value| value.clone())
            .unwrap_or_else(|| "Wake-word runtime hiện không khả dụng.".into())
    }
}

#[cfg(feature = "wake-word")]
fn state_name(state: WakeRuntimeState) -> &'static str {
    match state {
        WakeRuntimeState::Disabled => "disabled",
        WakeRuntimeState::Starting => "starting",
        WakeRuntimeState::Listening => "listening",
        WakeRuntimeState::Suspended => "suspended",
        WakeRuntimeState::Cooldown => "cooldown",
        WakeRuntimeState::Error => "error",
        WakeRuntimeState::Stopped => "stopped",
    }
}

fn env_bool(name: &str) -> Option<bool> {
    let value = std::env::var(name).ok()?;
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}
