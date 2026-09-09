use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use voice_runtime::tts::{
    SapiVoiceInfo, TtsVoicePreferences, default_voice_preferences_path,
    enumerate_windows_sapi_voices, load_voice_preferences, save_voice_preferences,
};

#[derive(Debug)]
struct TtsSettingsService {
    path: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct TtsSettingsView {
    pub vietnamese_voice_id: Option<String>,
    pub english_voice_id: Option<String>,
    pub voices: Vec<SapiVoiceInfo>,
}

#[derive(Debug, Deserialize)]
pub struct SetTtsVoicePayload {
    pub language: String,
    pub voice_id: Option<String>,
}

impl TtsSettingsService {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn snapshot(&self) -> Result<TtsSettingsView, String> {
        let preferences = load_voice_preferences(&self.path).map_err(|error| error.to_string())?;
        let voices = enumerate_windows_sapi_voices().map_err(|error| error.to_string())?;
        Ok(view(preferences, voices))
    }

    fn set_voice(&self, payload: SetTtsVoicePayload) -> Result<TtsSettingsView, String> {
        let language = normalize_language(&payload.language)?;
        let requested_voice_id = payload
            .voice_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let installed = enumerate_windows_sapi_voices().map_err(|error| error.to_string())?;
        let selected_voice_id = match requested_voice_id {
            Some(id) => {
                let voice = installed
                    .iter()
                    .find(|voice| voice.id == id)
                    .ok_or_else(|| {
                        "The selected SAPI voice is not installed on this Windows user.".to_owned()
                    })?;
                Some(voice.id.clone())
            }
            None => None,
        };

        let mut preferences =
            load_voice_preferences(&self.path).map_err(|error| error.to_string())?;
        match language {
            "vi" => preferences.vietnamese_voice_id = selected_voice_id,
            "en" => preferences.english_voice_id = selected_voice_id,
            _ => unreachable!(),
        }

        save_voice_preferences(&self.path, &preferences).map_err(|error| error.to_string())?;
        Ok(view(preferences, installed))
    }
}

#[tauri::command]
pub fn assistant_tts_settings() -> Result<TtsSettingsView, String> {
    service()?.snapshot()
}

#[tauri::command]
pub fn assistant_tts_set_voice(
    payload: SetTtsVoicePayload,
) -> Result<TtsSettingsView, String> {
    service()?.set_voice(payload)
}

fn service() -> Result<TtsSettingsService, String> {
    let path = default_voice_preferences_path().ok_or_else(|| {
        "Cannot resolve the Windows TTS settings path. Set ASSISTANT_TTS_SETTINGS_PATH to an absolute path."
            .to_owned()
    })?;
    Ok(TtsSettingsService::new(path))
}

fn view(preferences: TtsVoicePreferences, voices: Vec<SapiVoiceInfo>) -> TtsSettingsView {
    TtsSettingsView {
        vietnamese_voice_id: preferences.vietnamese_voice_id,
        english_voice_id: preferences.english_voice_id,
        voices,
    }
}

fn normalize_language(language: &str) -> Result<&'static str, String> {
    match language.trim().to_ascii_lowercase().as_str() {
        "vi" | "vietnamese" => Ok("vi"),
        "en" | "english" => Ok("en"),
        _ => Err("TTS language must be `vi` or `en`.".to_owned()),
    }
}
