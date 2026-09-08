use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread,
};

use async_trait::async_trait;
use serde::Serialize;
use thiserror::Error;
use tokio::sync::oneshot;
use tracing::warn;
use windows::{
    Win32::{
        Media::Speech::{
            ISpeechObjectToken, ISpeechVoice, SVSFIsXML, SVSFPurgeBeforeSpeak, SVSFlagsAsync,
            SpVoice, SpeechVoiceSpeakFlags,
        },
        System::Com::{
            CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
        },
    },
    core::{BSTR, Error as WindowsError},
};

const SAPI_LANG_EN_US: &str = "409";
const SAPI_LANG_VI_VN: &str = "42A";
const APP_IDENTIFIER: &str = "com.voduong.assisstantdesktop";
const TTS_SETTINGS_ENV: &str = "ASSISTANT_TTS_SETTINGS_PATH";
const APP_DATA_ENV: &str = "ASSISTANT_APP_DATA";
const TTS_SETTINGS_FILE: &str = "tts.conf";
const TTS_SETTINGS_VERSION: &str = "1";

#[derive(Debug, Error)]
pub enum TtsError {
    #[error("text-to-speech input is empty")]
    EmptyText,
    #[error("text-to-speech request was cancelled")]
    Cancelled,
    #[error("text-to-speech backend is already speaking")]
    Busy,
    #[error("text-to-speech backend failed: {0}")]
    Backend(String),
    #[error("text-to-speech worker failed: {0}")]
    Worker(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TtsLanguage {
    Auto,
    Vietnamese,
    English,
}

impl Default for TtsLanguage {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TtsConfig {
    /// Windows SAPI speaking rate. Values are clamped to -10..=10.
    pub rate: i32,
    /// Windows SAPI output volume. Values are clamped to 0..=100.
    pub volume: i32,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            rate: 0,
            volume: 100,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TtsVoicePreferences {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vietnamese_voice_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub english_voice_id: Option<String>,
}

impl TtsVoicePreferences {
    fn preferred_id(&self, language: TtsLanguage) -> Option<&str> {
        match language {
            TtsLanguage::Vietnamese => self.vietnamese_voice_id.as_deref(),
            TtsLanguage::English => self.english_voice_id.as_deref(),
            TtsLanguage::Auto => None,
        }
        .map(str::trim)
        .filter(|value| !value.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SapiVoiceInfo {
    pub id: String,
    pub name: String,
    pub language: Option<String>,
}

pub fn default_voice_preferences_path() -> Option<PathBuf> {
    if let Some(path) = env::var_os(TTS_SETTINGS_ENV).map(PathBuf::from) {
        if path.is_absolute() {
            return Some(path);
        }
    }

    if let Some(root) = env::var_os(APP_DATA_ENV).map(PathBuf::from) {
        if root.is_absolute() {
            return Some(root.join("settings").join(TTS_SETTINGS_FILE));
        }
    }

    env::var_os("LOCALAPPDATA").map(PathBuf::from).map(|root| {
        root.join(APP_IDENTIFIER)
            .join("settings")
            .join(TTS_SETTINGS_FILE)
    })
}

pub fn load_voice_preferences(path: &Path) -> Result<TtsVoicePreferences, TtsError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(TtsVoicePreferences::default());
        }
        Err(error) => {
            return Err(TtsError::Backend(format!(
                "cannot read TTS settings {}: {error}",
                path.display()
            )));
        }
    };

    let mut preferences = TtsVoicePreferences::default();
    let mut saw_version = false;
    let mut saw_vi = false;
    let mut saw_en = false;
    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(TtsError::Backend(format!(
                "invalid TTS settings {} line {}: expected key=value",
                path.display(),
                index + 1
            )));
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "version" => {
                if saw_version || value != TTS_SETTINGS_VERSION {
                    return Err(TtsError::Backend(format!(
                        "unsupported or duplicate TTS settings version in {}",
                        path.display()
                    )));
                }
                saw_version = true;
            }
            "vietnamese_voice_id" => {
                if saw_vi {
                    return Err(TtsError::Backend(format!(
                        "duplicate vietnamese_voice_id in {}",
                        path.display()
                    )));
                }
                saw_vi = true;
                preferences.vietnamese_voice_id = normalize_saved_voice_id(value, path)?;
            }
            "english_voice_id" => {
                if saw_en {
                    return Err(TtsError::Backend(format!(
                        "duplicate english_voice_id in {}",
                        path.display()
                    )));
                }
                saw_en = true;
                preferences.english_voice_id = normalize_saved_voice_id(value, path)?;
            }
            other => {
                return Err(TtsError::Backend(format!(
                    "unknown TTS settings key `{other}` in {}",
                    path.display()
                )));
            }
        }
    }

    if !saw_version {
        return Err(TtsError::Backend(format!(
            "TTS settings {} are missing version=1",
            path.display()
        )));
    }
    Ok(preferences)
}

pub fn save_voice_preferences(
    path: &Path,
    preferences: &TtsVoicePreferences,
) -> Result<(), TtsError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TtsError::Backend(format!(
                "cannot create TTS settings directory {}: {error}",
                parent.display()
            ))
        })?;
    }

    let vi = validate_voice_id_for_save(preferences.vietnamese_voice_id.as_deref())?;
    let en = validate_voice_id_for_save(preferences.english_voice_id.as_deref())?;
    let payload = format!(
        "version={TTS_SETTINGS_VERSION}\nvietnamese_voice_id={vi}\nenglish_voice_id={en}\n"
    );
    let temporary = path.with_extension("conf.tmp");
    fs::write(&temporary, payload).map_err(|error| {
        TtsError::Backend(format!(
            "cannot write temporary TTS settings {}: {error}",
            temporary.display()
        ))
    })?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| {
            let _ = fs::remove_file(&temporary);
            TtsError::Backend(format!(
                "cannot replace TTS settings {}: {error}",
                path.display()
            ))
        })?;
    }
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        TtsError::Backend(format!(
            "cannot commit TTS settings {}: {error}",
            path.display()
        ))
    })
}

fn normalize_saved_voice_id(value: &str, path: &Path) -> Result<Option<String>, TtsError> {
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().any(|ch| ch.is_control()) {
        return Err(TtsError::Backend(format!(
            "TTS voice id in {} contains control characters",
            path.display()
        )));
    }
    Ok(Some(value.to_owned()))
}

fn validate_voice_id_for_save(value: Option<&str>) -> Result<&str, TtsError> {
    let value = value.map(str::trim).unwrap_or("");
    if value.chars().any(|ch| ch.is_control()) {
        return Err(TtsError::Backend(
            "TTS voice token id contains control characters".into(),
        ));
    }
    Ok(value)
}

pub fn enumerate_windows_sapi_voices() -> Result<Vec<SapiVoiceInfo>, TtsError> {
    let initialize = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if initialize.is_err() {
        return Err(TtsError::Backend(
            WindowsError::from_hresult(initialize).to_string(),
        ));
    }
    let _com = ComGuard;

    let voice: ISpeechVoice = unsafe { CoCreateInstance(&SpVoice, None, CLSCTX_ALL) }
        .map_err(|error| TtsError::Backend(error.to_string()))?;
    enumerate_voice_tokens(&voice)
}

fn enumerate_voice_tokens(voice: &ISpeechVoice) -> Result<Vec<SapiVoiceInfo>, TtsError> {
    let empty = BSTR::from("");
    let voices = unsafe { voice.GetVoices(&empty, &empty) }
        .map_err(|error| TtsError::Backend(error.to_string()))?;
    let count = unsafe { voices.Count() }
        .map_err(|error| TtsError::Backend(error.to_string()))?
        .max(0);
    let language_attribute = BSTR::from("Language");
    let mut result = Vec::with_capacity(count as usize);

    for index in 0..count {
        let token = match unsafe { voices.Item(index) } {
            Ok(token) => token,
            Err(error) => {
                warn!(index, %error, "failed to inspect one installed SAPI voice token");
                continue;
            }
        };
        let id = match unsafe { token.Id() } {
            Ok(value) => value.to_string(),
            Err(error) => {
                warn!(index, %error, "installed SAPI voice token has no readable id");
                continue;
            }
        };
        let name = unsafe { token.GetDescription(0) }
            .map(|value| value.to_string())
            .unwrap_or_else(|_| id.clone());
        let language = unsafe { token.GetAttribute(&language_attribute) }
            .ok()
            .map(|value| value.to_string())
            .filter(|value| !value.trim().is_empty());
        result.push(SapiVoiceInfo { id, name, language });
    }

    Ok(result)
}

#[async_trait]
pub trait TextToSpeech: Send + Sync {
    async fn speak(&self, text: &str) -> Result<(), TtsError>;

    /// Speaks with an explicit response-language hint when the backend supports it.
    /// The default keeps backwards compatibility for non-language-aware backends.
    async fn speak_with_language(
        &self,
        text: &str,
        _language: TtsLanguage,
    ) -> Result<(), TtsError> {
        self.speak(text).await
    }

    /// Requests cancellation of the currently active speech operation.
    /// Implementations return false when no cancellation command can be sent.
    fn cancel(&self) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct WindowsSapiTts {
    runtime: Arc<SapiRuntime>,
    preferences_path: Option<PathBuf>,
}

impl Default for WindowsSapiTts {
    fn default() -> Self {
        Self::new(TtsConfig::default())
    }
}

impl WindowsSapiTts {
    pub fn new(config: TtsConfig) -> Self {
        Self::new_with_preferences_path(config, default_voice_preferences_path())
    }

    pub fn new_with_preferences_path(config: TtsConfig, preferences_path: Option<PathBuf>) -> Self {
        Self {
            runtime: Arc::new(SapiRuntime::spawn(config)),
            preferences_path,
        }
    }

    pub async fn speak_with_language(
        &self,
        text: &str,
        language: TtsLanguage,
    ) -> Result<(), TtsError> {
        if text.trim().is_empty() {
            return Err(TtsError::EmptyText);
        }
        let resolved_language = resolve_language(language, text);
        let preferences = match self.preferences_path.as_deref() {
            Some(path) => match load_voice_preferences(path) {
                Ok(preferences) => preferences,
                Err(error) => {
                    warn!(%error, "ignoring invalid TTS voice preferences for this utterance");
                    TtsVoicePreferences::default()
                }
            },
            None => TtsVoicePreferences::default(),
        };
        self.runtime
            .speak(text.to_owned(), resolved_language, preferences)
            .await
    }

    /// Sends an out-of-band purge request to the dedicated SAPI worker.
    /// The worker owns the COM voice for its full lifetime, so cancellation does
    /// not depend on aborting a Tokio `spawn_blocking` task.
    pub fn cancel(&self) -> bool {
        self.runtime.cancel()
    }
}

#[async_trait]
impl TextToSpeech for WindowsSapiTts {
    async fn speak(&self, text: &str) -> Result<(), TtsError> {
        WindowsSapiTts::speak_with_language(self, text, TtsLanguage::Auto).await
    }

    async fn speak_with_language(
        &self,
        text: &str,
        language: TtsLanguage,
    ) -> Result<(), TtsError> {
        WindowsSapiTts::speak_with_language(self, text, language).await
    }

    fn cancel(&self) -> bool {
        WindowsSapiTts::cancel(self)
    }
}

enum SapiCommand {
    Speak {
        text: String,
        language: TtsLanguage,
        preferences: TtsVoicePreferences,
        result: oneshot::Sender<Result<(), TtsError>>,
    },
    Cancel,
}

struct SapiRuntime {
    commands: Option<Sender<SapiCommand>>,
}

impl SapiRuntime {
    fn spawn(config: TtsConfig) -> Self {
        let (commands, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("assistant-sapi-tts".into())
            .spawn(move || sapi_worker(config, receiver));

        Self {
            commands: worker.ok().map(|_| commands),
        }
    }

    async fn speak(
        &self,
        text: String,
        language: TtsLanguage,
        preferences: TtsVoicePreferences,
    ) -> Result<(), TtsError> {
        let commands = self.commands.as_ref().ok_or_else(|| {
            TtsError::Worker("Windows SAPI worker thread could not be started".into())
        })?;
        let (result_tx, result_rx) = oneshot::channel();
        commands
            .send(SapiCommand::Speak {
                text,
                language,
                preferences,
                result: result_tx,
            })
            .map_err(|_| TtsError::Worker("Windows SAPI worker is unavailable".into()))?;
        result_rx
            .await
            .map_err(|_| TtsError::Worker("Windows SAPI worker stopped unexpectedly".into()))?
    }

    fn cancel(&self) -> bool {
        self.commands
            .as_ref()
            .is_some_and(|commands| commands.send(SapiCommand::Cancel).is_ok())
    }
}

fn sapi_worker(config: TtsConfig, receiver: Receiver<SapiCommand>) {
    let initialize = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if initialize.is_err() {
        let message = WindowsError::from_hresult(initialize).to_string();
        reject_worker_commands(receiver, message);
        return;
    }
    let _com = ComGuard;

    let voice: ISpeechVoice = match unsafe { CoCreateInstance(&SpVoice, None, CLSCTX_ALL) } {
        Ok(voice) => voice,
        Err(error) => {
            reject_worker_commands(receiver, error.to_string());
            return;
        }
    };
    let default_voice = unsafe { voice.Voice() }.ok();

    let setup = unsafe {
        voice
            .SetRate(config.rate.clamp(-10, 10))
            .and_then(|_| voice.SetVolume(config.volume.clamp(0, 100)))
    };
    if let Err(error) = setup {
        reject_worker_commands(receiver, error.to_string());
        return;
    }

    while let Ok(command) = receiver.recv() {
        match command {
            SapiCommand::Speak {
                text,
                language,
                preferences,
                result,
            } => {
                let preferred_voice_active =
                    apply_preferred_voice(&voice, default_voice.as_ref(), language, &preferences);
                if !run_speech(
                    &voice,
                    &receiver,
                    text,
                    language,
                    preferred_voice_active,
                    result,
                ) {
                    break;
                }
            }
            SapiCommand::Cancel => {
                // Idle cancellation is intentionally harmless. A cancel command
                // may arrive just after the previous stream completed.
            }
        }
    }
}

fn apply_preferred_voice(
    voice: &ISpeechVoice,
    default_voice: Option<&ISpeechObjectToken>,
    language: TtsLanguage,
    preferences: &TtsVoicePreferences,
) -> bool {
    let Some(preferred_id) = preferences.preferred_id(language) else {
        if let Some(default_voice) = default_voice {
            let _ = unsafe { voice.putref_Voice(default_voice) };
        }
        return false;
    };

    match find_voice_token(voice, preferred_id) {
        Ok(Some(token)) => match unsafe { voice.putref_Voice(&token) } {
            Ok(()) => true,
            Err(error) => {
                warn!(%error, %preferred_id, "failed to activate preferred SAPI voice; using locale fallback");
                if let Some(default_voice) = default_voice {
                    let _ = unsafe { voice.putref_Voice(default_voice) };
                }
                false
            }
        },
        Ok(None) => {
            warn!(%preferred_id, "preferred SAPI voice is no longer installed; using locale fallback");
            if let Some(default_voice) = default_voice {
                let _ = unsafe { voice.putref_Voice(default_voice) };
            }
            false
        }
        Err(error) => {
            warn!(%error, %preferred_id, "could not enumerate SAPI voices; using locale fallback");
            if let Some(default_voice) = default_voice {
                let _ = unsafe { voice.putref_Voice(default_voice) };
            }
            false
        }
    }
}

fn find_voice_token(
    voice: &ISpeechVoice,
    preferred_id: &str,
) -> Result<Option<ISpeechObjectToken>, TtsError> {
    let empty = BSTR::from("");
    let voices = unsafe { voice.GetVoices(&empty, &empty) }
        .map_err(|error| TtsError::Backend(error.to_string()))?;
    let count = unsafe { voices.Count() }
        .map_err(|error| TtsError::Backend(error.to_string()))?
        .max(0);
    for index in 0..count {
        let token = unsafe { voices.Item(index) }
            .map_err(|error| TtsError::Backend(error.to_string()))?;
        let id = unsafe { token.Id() }
            .map_err(|error| TtsError::Backend(error.to_string()))?
            .to_string();
        if id == preferred_id {
            return Ok(Some(token));
        }
    }
    Ok(None)
}

fn run_speech(
    voice: &ISpeechVoice,
    receiver: &Receiver<SapiCommand>,
    text: String,
    language: TtsLanguage,
    preferred_voice_active: bool,
    result: oneshot::Sender<Result<(), TtsError>>,
) -> bool {
    // SAPI's <lang> tag is a voice-selection tag. When a stable voice token was
    // explicitly selected above, using <lang> here could replace that voice.
    // Keep XML escaping for safety but only use locale voice selection in the
    // automatic/fallback path.
    let markup = if preferred_voice_active {
        escape_sapi_xml(&text)
    } else {
        language_markup(&text, language)
    };
    let text = BSTR::from(markup);
    let speak_flags = SpeechVoiceSpeakFlags(
        SVSFlagsAsync.0 | SVSFPurgeBeforeSpeak.0 | SVSFIsXML.0,
    );
    if let Err(error) = unsafe { voice.Speak(&text, speak_flags) } {
        let _ = result.send(Err(TtsError::Backend(error.to_string())));
        return true;
    }

    loop {
        match unsafe { voice.WaitUntilDone(40) } {
            Ok(done) if done.as_bool() => {
                let _ = result.send(Ok(()));
                return true;
            }
            Ok(_) => {}
            Err(error) => {
                let _ = result.send(Err(TtsError::Backend(error.to_string())));
                return true;
            }
        }

        loop {
            match receiver.try_recv() {
                Ok(SapiCommand::Cancel) => {
                    let purge = purge_voice(voice);
                    let outcome = match purge {
                        Ok(()) => Err(TtsError::Cancelled),
                        Err(error) => Err(error),
                    };
                    let _ = result.send(outcome);
                    return true;
                }
                Ok(SapiCommand::Speak { result, .. }) => {
                    let _ = result.send(Err(TtsError::Busy));
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    let _ = purge_voice(voice);
                    let _ = result.send(Err(TtsError::Cancelled));
                    return false;
                }
            }
        }
    }
}

fn resolve_language(language: TtsLanguage, text: &str) -> TtsLanguage {
    match language {
        TtsLanguage::Auto if looks_vietnamese(text) => TtsLanguage::Vietnamese,
        TtsLanguage::Auto => TtsLanguage::English,
        language => language,
    }
}

fn language_markup(text: &str, language: TtsLanguage) -> String {
    let lang_id = match resolve_language(language, text) {
        TtsLanguage::Vietnamese => SAPI_LANG_VI_VN,
        TtsLanguage::English | TtsLanguage::Auto => SAPI_LANG_EN_US,
    };
    format!(
        "<lang langid=\"{lang_id}\">{}</lang>",
        escape_sapi_xml(text)
    )
}

fn escape_sapi_xml(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn looks_vietnamese(text: &str) -> bool {
    const VIETNAMESE_MARKERS: &str = concat!(
        "ăâđêôơưĂÂĐÊÔƠƯ",
        "áàảãạấầẩẫậắằẳẵặéèẻẽẹếềểễệ",
        "íìỉĩịóòỏõọốồổỗộớờởỡợúùủũụứừửữự",
        "ýỳỷỹỵÁÀẢÃẠẤẦẨẪẬẮẰẲẴẶÉÈẺẼẸẾỀỂỄỆ",
        "ÍÌỈĨỊÓÒỎÕỌỐỒỔỖỘỚỜỞỠỢÚÙỦŨỤỨỪỬỮỰÝỲỶỸỴ"
    );
    text.chars().any(|ch| VIETNAMESE_MARKERS.contains(ch))
}

fn purge_voice(voice: &ISpeechVoice) -> Result<(), TtsError> {
    let empty = BSTR::from("");
    let flags = SpeechVoiceSpeakFlags(SVSFlagsAsync.0 | SVSFPurgeBeforeSpeak.0);
    match unsafe { voice.Speak(&empty, flags) } {
        Ok(_) => Ok(()),
        Err(purge_error) => {
            // Some SAPI voices/audio drivers can reject a purge while the output
            // device is busy. `Skip` is the documented sentence-level escape
            // hatch, so use it as a bounded fallback on the same COM worker.
            let sentence = BSTR::from("Sentence");
            unsafe { voice.Skip(&sentence, i32::MAX) }
                .map(|_| ())
                .map_err(|skip_error| {
                    TtsError::Backend(format!(
                        "SAPI purge failed: {purge_error}; sentence skip fallback failed: {skip_error}"
                    ))
                })
        }
    }
}

fn reject_worker_commands(receiver: Receiver<SapiCommand>, message: String) {
    while let Ok(command) = receiver.recv() {
        if let SapiCommand::Speak { result, .. } = command {
            let _ = result.send(Err(TtsError::Backend(message.clone())));
        }
    }
}

struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_language_detects_vietnamese_diacritics() {
        assert_eq!(
            resolve_language(TtsLanguage::Auto, "Được, mình đã mở ứng dụng."),
            TtsLanguage::Vietnamese
        );
        assert_eq!(
            resolve_language(TtsLanguage::Auto, "Sure, I opened the app."),
            TtsLanguage::English
        );
    }

    #[test]
    fn language_markup_uses_windows_sapi_langids() {
        assert!(language_markup("Xin chào", TtsLanguage::Vietnamese).starts_with(
            "<lang langid=\"42A\">"
        ));
        assert!(language_markup("Hello", TtsLanguage::English).starts_with(
            "<lang langid=\"409\">"
        ));
    }

    #[test]
    fn sapi_xml_text_is_escaped() {
        assert_eq!(
            escape_sapi_xml("A & B < C > D \"x\" 'y'"),
            "A &amp; B &lt; C &gt; D &quot;x&quot; &apos;y&apos;"
        );
    }

    #[test]
    fn preference_lookup_is_language_specific() {
        let preferences = TtsVoicePreferences {
            vietnamese_voice_id: Some("vi-id".into()),
            english_voice_id: Some("en-id".into()),
        };
        assert_eq!(preferences.preferred_id(TtsLanguage::Vietnamese), Some("vi-id"));
        assert_eq!(preferences.preferred_id(TtsLanguage::English), Some("en-id"));
        assert_eq!(preferences.preferred_id(TtsLanguage::Auto), None);
    }

    #[test]
    fn settings_parser_is_strict_and_versioned() {
        let unique = format!("assistant-tts-test-{}", std::process::id());
        let path = std::env::temp_dir().join(unique).with_extension("conf");
        fs::write(
            &path,
            "version=1\nvietnamese_voice_id=vi-id\nenglish_voice_id=en-id\n",
        )
        .unwrap();
        let loaded = load_voice_preferences(&path).unwrap();
        assert_eq!(loaded.vietnamese_voice_id.as_deref(), Some("vi-id"));
        assert_eq!(loaded.english_voice_id.as_deref(), Some("en-id"));
        let _ = fs::remove_file(path);
    }
}
