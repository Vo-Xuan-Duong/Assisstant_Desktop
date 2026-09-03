use std::{
    fs,
    path::PathBuf,
    sync::Mutex,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::resource_registry::ResourceRegistry;

#[cfg(feature = "wake-word")]
use tokio::sync::{broadcast, Mutex as AsyncMutex};
#[cfg(feature = "wake-word")]
use voice_runtime::{
    sherpa_wake::SherpaWakeWordDetector,
    wake::SherpaWakeConfig,
    wake_runtime::{
        spawn_wake_runtime, WakeRuntimeConfig, WakeRuntimeEvent, WakeRuntimeHandle,
        WakeRuntimeState,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WakePreferences {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    phrase: Option<String>,
}

#[derive(Debug, Clone)]
struct WakeSettingsStore {
    path: PathBuf,
}

impl WakeSettingsStore {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn load(&self) -> Result<WakePreferences, String> {
        if !self.path.is_file() {
            return Ok(WakePreferences::default());
        }
        let bytes = fs::read(&self.path)
            .map_err(|error| format!("cannot read wake settings: {error}"))?;
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("cannot parse wake settings: {error}"))
    }

    fn save(&self, preferences: &WakePreferences) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "wake settings path has no parent directory".to_owned())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create wake settings directory: {error}"))?;

        let bytes = serde_json::to_vec_pretty(preferences)
            .map_err(|error| format!("cannot serialize wake settings: {error}"))?;
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("wake.json");
        let temporary = parent.join(format!(".{file_name}.{}.part", Uuid::new_v4()));
        let backup = parent.join(format!(".{file_name}.{}.bak", Uuid::new_v4()));

        fs::write(&temporary, bytes)
            .map_err(|error| format!("cannot write temporary wake settings: {error}"))?;

        let had_existing = self.path.exists();
        if had_existing {
            if let Err(error) = fs::rename(&self.path, &backup) {
                let _ = fs::remove_file(&temporary);
                return Err(format!("cannot stage previous wake settings: {error}"));
            }
        }

        if let Err(error) = fs::rename(&temporary, &self.path) {
            if had_existing {
                let _ = fs::rename(&backup, &self.path);
            }
            let _ = fs::remove_file(&temporary);
            return Err(format!("cannot install wake settings: {error}"));
        }

        if had_existing {
            let _ = fs::remove_file(&backup);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WakeStatus {
    pub compiled: bool,
    pub available: bool,
    pub enabled: bool,
    pub state: String,
    pub model_dir: Option<String>,
    pub keywords_path: Option<String>,
    pub phrase: Option<String>,
    pub detail: Option<String>,
}

pub struct WakeService {
    #[cfg(feature = "wake-word")]
    handle: Mutex<Option<WakeRuntimeHandle>>,
    #[cfg(feature = "wake-word")]
    events: broadcast::Sender<WakeRuntimeEvent>,
    #[cfg(feature = "wake-word")]
    reload_gate: AsyncMutex<()>,
    model_dir: Option<PathBuf>,
    keywords_path: Option<PathBuf>,
    settings: WakeSettingsStore,
    preferences: Mutex<WakePreferences>,
    detail: Mutex<Option<String>>,
}

impl WakeService {
    pub fn setup(resources: &ResourceRegistry) -> Self {
        let settings = WakeSettingsStore::new(resources.wake_settings_path());
        let (preferences, settings_error) = match settings.load() {
            Ok(value) => (value, None),
            Err(error) => (WakePreferences::default(), Some(error)),
        };

        #[cfg(feature = "wake-word")]
        {
            return Self::setup_sherpa(resources, settings, preferences, settings_error);
        }

        #[cfg(not(feature = "wake-word"))]
        {
            let detail = settings_error.unwrap_or_else(|| {
                "Bản build hiện tại chưa bật feature `wake-word`.".into()
            });
            Self {
                model_dir: Some(resources.wake_model_dir().to_path_buf()),
                keywords_path: Some(resources.wake_keywords_path().to_path_buf()),
                settings,
                preferences: Mutex::new(preferences),
                detail: Mutex::new(Some(detail)),
            }
        }
    }

    #[cfg(feature = "wake-word")]
    fn setup_sherpa(
        resources: &ResourceRegistry,
        settings: WakeSettingsStore,
        preferences: WakePreferences,
        settings_error: Option<String>,
    ) -> Self {
        let model_dir = resources.wake_model_dir().to_path_buf();
        let keywords_path = resources.wake_keywords_path().to_path_buf();
        let (events, _) = broadcast::channel(32);
        let enabled_on_start = env_bool("ASSISTANT_WAKE_ENABLED").unwrap_or(preferences.enabled);
        let config = SherpaWakeConfig::gigaspeech_int8(&model_dir, &keywords_path);

        let (handle, detector_error) = match SherpaWakeWordDetector::load(config) {
            Ok(detector) => {
                let handle = spawn_wake_runtime(
                    Box::new(detector),
                    WakeRuntimeConfig {
                        enabled_on_start,
                        ..WakeRuntimeConfig::default()
                    },
                );
                relay_runtime_events(handle.subscribe(), events.clone());
                (Some(handle), None)
            }
            Err(error) => (None, Some(error.to_string())),
        };

        let detail = match (settings_error, detector_error) {
            (Some(settings), Some(detector)) => Some(format!("{settings}; {detector}")),
            (Some(settings), None) => Some(settings),
            (None, Some(detector)) => Some(detector),
            (None, None) => None,
        };

        Self {
            handle: Mutex::new(handle),
            events,
            reload_gate: AsyncMutex::new(()),
            model_dir: Some(model_dir),
            keywords_path: Some(keywords_path),
            settings,
            preferences: Mutex::new(preferences),
            detail: Mutex::new(detail),
        }
    }

    pub fn status(&self) -> WakeStatus {
        let phrase = self
            .preferences
            .lock()
            .ok()
            .and_then(|value| value.phrase.clone());

        #[cfg(feature = "wake-word")]
        {
            if let Some(handle) = self.current_handle() {
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
                    phrase,
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
                phrase,
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
            phrase,
            detail: self.detail.lock().ok().and_then(|value| value.clone()),
        }
    }

    pub async fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        #[cfg(feature = "wake-word")]
        {
            let handle = self.current_handle().ok_or_else(|| self.unavailable_message())?;
            handle.set_enabled(enabled).await?;
            let persist = self.update_preferences(|preferences| preferences.enabled = enabled);
            self.record_persistence_result(persist);
            return Ok(());
        }

        #[cfg(not(feature = "wake-word"))]
        {
            let _ = enabled;
            Err(self.unavailable_message())
        }
    }

    #[cfg(feature = "wake-word")]
    pub async fn reload_or_start(
        &self,
        detector: SherpaWakeWordDetector,
        phrase: String,
    ) -> Result<(), String> {
        let _gate = self.reload_gate.lock().await;
        if let Some(handle) = self.current_handle() {
            handle.reload(Box::new(detector)).await?;
        } else {
            let preferences = self
                .preferences
                .lock()
                .map(|value| value.clone())
                .unwrap_or_default();
            let enabled_on_start =
                env_bool("ASSISTANT_WAKE_ENABLED").unwrap_or(preferences.enabled);
            let handle = spawn_wake_runtime(
                Box::new(detector),
                WakeRuntimeConfig {
                    enabled_on_start,
                    ..WakeRuntimeConfig::default()
                },
            );
            relay_runtime_events(handle.subscribe(), self.events.clone());
            let mut slot = self
                .handle
                .lock()
                .map_err(|_| "wake runtime handle lock is poisoned".to_owned())?;
            *slot = Some(handle);
        }

        let persist = self.update_preferences(|preferences| preferences.phrase = Some(phrase));
        self.record_persistence_result(persist);
        Ok(())
    }

    pub async fn suspend(&self) {
        #[cfg(feature = "wake-word")]
        if let Some(handle) = self.current_handle() {
            let mut state = handle.subscribe_state();
            if handle.suspend().await.is_err() {
                return;
            }

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
        if let Some(handle) = self.current_handle() {
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(delay).await;
                let _ = handle.resume().await;
            });
        }

        #[cfg(not(feature = "wake-word"))]
        let _ = delay;
    }

    #[cfg(feature = "wake-word")]
    pub fn subscribe(&self) -> Option<broadcast::Receiver<WakeRuntimeEvent>> {
        Some(self.events.subscribe())
    }

    #[cfg(feature = "wake-word")]
    fn current_handle(&self) -> Option<WakeRuntimeHandle> {
        self.handle
            .lock()
            .ok()
            .and_then(|handle| handle.clone())
    }

    fn update_preferences(
        &self,
        update: impl FnOnce(&mut WakePreferences),
    ) -> Result<(), String> {
        let next = {
            let mut preferences = self
                .preferences
                .lock()
                .map_err(|_| "wake preferences lock is poisoned".to_owned())?;
            update(&mut preferences);
            preferences.clone()
        };
        self.settings.save(&next)
    }

    fn record_persistence_result(&self, result: Result<(), String>) {
        if let Ok(mut detail) = self.detail.lock() {
            *detail = result
                .err()
                .map(|error| format!("Wake runtime updated, but settings persistence failed: {error}"));
        }
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
fn relay_runtime_events(
    mut source: broadcast::Receiver<WakeRuntimeEvent>,
    destination: broadcast::Sender<WakeRuntimeEvent>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            match source.recv().await {
                Ok(event) => {
                    let _ = destination.send(event);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
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
