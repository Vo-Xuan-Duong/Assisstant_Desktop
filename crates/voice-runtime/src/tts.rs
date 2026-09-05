use async_trait::async_trait;
use thiserror::Error;
use windows::{
    Win32::{
        Media::Speech::{ISpeechVoice, SpVoice, SpeechVoiceSpeakFlags},
        System::Com::{
            CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
        },
    },
    core::{BSTR, Error as WindowsError},
};

#[derive(Debug, Error)]
pub enum TtsError {
    #[error("text-to-speech input is empty")]
    EmptyText,
    #[error("text-to-speech backend failed: {0}")]
    Backend(String),
    #[error("text-to-speech worker failed: {0}")]
    Worker(String),
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
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsSapiTts {
    config: TtsConfig,
}

impl WindowsSapiTts {
    pub fn new(config: TtsConfig) -> Self {
        Self { config }
    }

    fn speak_blocking(config: TtsConfig, text: String) -> Result<(), TtsError> {
        if text.trim().is_empty() {
            return Err(TtsError::EmptyText);
        }

        let initialize = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if initialize.is_err() {
            return Err(TtsError::Backend(
                WindowsError::from_hresult(initialize).to_string(),
            ));
        }
        let _com = ComGuard;

        unsafe {
            let voice: ISpeechVoice = CoCreateInstance(&SpVoice, None, CLSCTX_ALL)
                .map_err(|error| TtsError::Backend(error.to_string()))?;
            voice
                .SetRate(config.rate.clamp(-10, 10))
                .map_err(|error| TtsError::Backend(error.to_string()))?;
            voice
                .SetVolume(config.volume.clamp(0, 100))
                .map_err(|error| TtsError::Backend(error.to_string()))?;

            let text = BSTR::from(text);
            voice
                .Speak(&text, SpeechVoiceSpeakFlags::default())
                .map_err(|error| TtsError::Backend(error.to_string()))?;
        }

        Ok(())
    }
}

#[async_trait]
impl TextToSpeech for WindowsSapiTts {
    async fn speak(&self, text: &str) -> Result<(), TtsError> {
        let config = self.config;
        let text = text.to_owned();
        tokio::task::spawn_blocking(move || Self::speak_blocking(config, text))
            .await
            .map_err(|error| TtsError::Worker(error.to_string()))?
    }
}

struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
