use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use whisper_rs::{
    install_logging_hooks, FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
};

use crate::{
    stt::{prepare_for_whisper, SpeechRecognizer, SttError, Transcript},
    vad::Utterance,
};

#[derive(Debug, Clone)]
pub struct WhisperConfig {
    pub model_path: PathBuf,
    /// ISO-style Whisper language code such as `vi` or `en`. None enables auto detection.
    pub language: Option<String>,
    pub threads: usize,
    /// CPU is the project default. GPU backends still require matching whisper-rs Cargo features.
    pub use_gpu: bool,
}

impl WhisperConfig {
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
            language: Some("vi".into()),
            threads: 4,
            use_gpu: false,
        }
    }
}

#[derive(Clone)]
pub struct WhisperRecognizer {
    context: Arc<WhisperContext>,
    config: WhisperConfig,
}

impl WhisperRecognizer {
    pub fn load(config: WhisperConfig) -> Result<Self, SttError> {
        if !config.model_path.is_file() {
            return Err(SttError::Backend(format!(
                "Whisper model does not exist: {}",
                config.model_path.display()
            )));
        }

        install_logging_hooks();

        let mut context_params = WhisperContextParameters::default();
        context_params.use_gpu = config.use_gpu;
        let model_path = config.model_path.to_string_lossy();
        let context = WhisperContext::new_with_params(&model_path, context_params)
            .map_err(|error| SttError::Backend(error.to_string()))?;

        Ok(Self {
            context: Arc::new(context),
            config,
        })
    }

    fn transcribe_blocking(&self, utterance: Utterance) -> Result<Transcript, SttError> {
        let source_duration_seconds = utterance.duration_seconds();
        let audio = prepare_for_whisper(&utterance)?;
        let mut state = self
            .context
            .create_state()
            .map_err(|error| SttError::Backend(error.to_string()))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(self.config.threads.max(1).min(i32::MAX as usize) as i32);
        params.set_translate(false);
        params.set_no_context(true);
        params.set_no_timestamps(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_language(self.config.language.as_deref());

        state
            .full(params, &audio)
            .map_err(|error| SttError::Backend(error.to_string()))?;

        let text = state
            .as_iter()
            .map(|segment| segment.to_string())
            .collect::<String>()
            .trim()
            .to_owned();

        Ok(Transcript {
            text,
            language: self.config.language.clone(),
            engine: "whisper.cpp".into(),
            source_duration_seconds,
        })
    }
}

#[async_trait]
impl SpeechRecognizer for WhisperRecognizer {
    async fn transcribe(&self, utterance: Utterance) -> Result<Transcript, SttError> {
        let recognizer = self.clone();
        tokio::task::spawn_blocking(move || recognizer.transcribe_blocking(utterance))
            .await
            .map_err(|error| SttError::Worker(error.to_string()))?
    }
}
