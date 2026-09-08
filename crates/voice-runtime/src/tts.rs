use std::{
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread,
};

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::oneshot;
use windows::{
    Win32::{
        Media::Speech::{
            ISpeechVoice, SVSFIsXML, SVSFPurgeBeforeSpeak, SVSFlagsAsync, SpVoice,
            SpeechVoiceSpeakFlags,
        },
        System::Com::{
            CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
        },
    },
    core::{BSTR, Error as WindowsError},
};

const SAPI_LANG_EN_US: &str = "409";
const SAPI_LANG_VI_VN: &str = "42A";

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
}

impl Default for WindowsSapiTts {
    fn default() -> Self {
        Self::new(TtsConfig::default())
    }
}

impl WindowsSapiTts {
    pub fn new(config: TtsConfig) -> Self {
        Self {
            runtime: Arc::new(SapiRuntime::spawn(config)),
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
        self.runtime
            .speak(text.to_owned(), resolve_language(language, text))
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

    async fn speak(&self, text: String, language: TtsLanguage) -> Result<(), TtsError> {
        let commands = self.commands.as_ref().ok_or_else(|| {
            TtsError::Worker("Windows SAPI worker thread could not be started".into())
        })?;
        let (result_tx, result_rx) = oneshot::channel();
        commands
            .send(SapiCommand::Speak {
                text,
                language,
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
                result,
            } => {
                if !run_speech(&voice, &receiver, text, language, result) {
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

fn run_speech(
    voice: &ISpeechVoice,
    receiver: &Receiver<SapiCommand>,
    text: String,
    language: TtsLanguage,
    result: oneshot::Sender<Result<(), TtsError>>,
) -> bool {
    let markup = language_markup(&text, language);
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
}
