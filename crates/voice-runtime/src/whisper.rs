use std::{env, path::{Path, PathBuf}, sync::Arc};

use async_trait::async_trait;
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig};
use tracing::warn;
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, install_logging_hooks,
};

use crate::{
    stt::{SpeechRecognizer, SttError, Transcript, prepare_for_whisper},
    vad::Utterance,
};

const ZIPFORMER_ENCODER: &str = "encoder.int8.onnx";
const ZIPFORMER_DECODER: &str = "decoder.onnx";
const ZIPFORMER_JOINER: &str = "joiner.int8.onnx";
const ZIPFORMER_TOKENS: &str = "tokens.txt";
const ZIPFORMER_DEFAULT_DIR: &str = "sherpa-onnx-zipformer-vi-30M-int8-2026-02-09";

#[derive(Debug, Clone)]
pub struct WhisperConfig {
    /// Compatibility anchor used by the desktop voice state. New builds pass
    /// the Zipformer encoder path; older builds may still pass a Whisper model.
    pub model_path: PathBuf,
    /// ISO-style Whisper language code used only by the fallback recognizer.
    pub language: Option<String>,
    pub threads: usize,
    pub use_gpu: bool,
    pub zipformer_model_dir: Option<PathBuf>,
    pub fallback_whisper_model_path: Option<PathBuf>,
}

impl WhisperConfig {
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        let model_path = model_path.into();
        let zipformer_model_dir = resolve_zipformer_dir(&model_path);
        let fallback_whisper_model_path = resolve_whisper_fallback(&model_path);
        Self {
            model_path,
            language: Some("vi".into()),
            threads: 4,
            use_gpu: false,
            zipformer_model_dir,
            fallback_whisper_model_path,
        }
    }
}

#[derive(Debug, Clone)]
struct ZipformerPaths {
    encoder: PathBuf,
    decoder: PathBuf,
    joiner: PathBuf,
    tokens: PathBuf,
}

impl ZipformerPaths {
    fn from_dir(dir: &Path) -> Self {
        Self {
            encoder: dir.join(ZIPFORMER_ENCODER),
            decoder: dir.join(ZIPFORMER_DECODER),
            joiner: dir.join(ZIPFORMER_JOINER),
            tokens: dir.join(ZIPFORMER_TOKENS),
        }
    }

    fn is_complete(&self) -> bool {
        self.encoder.is_file()
            && self.decoder.is_file()
            && self.joiner.is_file()
            && self.tokens.is_file()
    }
}

#[derive(Clone)]
pub struct WhisperRecognizer {
    zipformer: Option<Arc<OfflineRecognizer>>,
    fallback_context: Option<Arc<WhisperContext>>,
    config: WhisperConfig,
}

impl WhisperRecognizer {
    pub fn load(config: WhisperConfig) -> Result<Self, SttError> {
        let zipformer = config
            .zipformer_model_dir
            .as_deref()
            .map(ZipformerPaths::from_dir)
            .filter(ZipformerPaths::is_complete)
            .map(|paths| create_zipformer(&paths, config.threads))
            .transpose()?;

        // Avoid loading Whisper into memory when Zipformer is ready. If a
        // Zipformer turn fails, the fallback model is loaded only for that
        // recovery path.
        let fallback_context = if zipformer.is_none() {
            load_whisper_fallback(&config)?
        } else {
            None
        };

        if zipformer.is_none() && fallback_context.is_none() {
            let primary = config
                .zipformer_model_dir
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<unresolved>".into());
            return Err(SttError::Backend(format!(
                "Vietnamese Zipformer model is incomplete at {primary}, and no Whisper fallback is available"
            )));
        }

        Ok(Self {
            zipformer,
            fallback_context,
            config,
        })
    }

    fn transcribe_blocking(&self, utterance: Utterance) -> Result<Transcript, SttError> {
        if let Some(recognizer) = &self.zipformer {
            match transcribe_zipformer(recognizer, &utterance) {
                Ok(transcript) if !transcript.is_empty() => return Ok(transcript),
                Ok(_) => warn!("Vietnamese Zipformer returned an empty transcript; trying fallback"),
                Err(error) => warn!(%error, "Vietnamese Zipformer failed; trying Whisper fallback"),
            }
        }

        if let Some(context) = &self.fallback_context {
            return transcribe_whisper(context, &self.config, &utterance);
        }

        if let Some(context) = load_whisper_fallback(&self.config)? {
            return transcribe_whisper(&context, &self.config, &utterance);
        }

        Err(SttError::Backend(
            "Vietnamese Zipformer failed and Whisper fallback is unavailable".into(),
        ))
    }
}

fn create_zipformer(paths: &ZipformerPaths, threads: usize) -> Result<Arc<OfflineRecognizer>, SttError> {
    let mut native = OfflineRecognizerConfig::default();
    native.model_config.transducer = OfflineTransducerModelConfig {
        encoder: Some(path_string(&paths.encoder)),
        decoder: Some(path_string(&paths.decoder)),
        joiner: Some(path_string(&paths.joiner)),
    };
    native.model_config.tokens = Some(path_string(&paths.tokens));
    native.model_config.provider = Some("cpu".into());
    native.model_config.num_threads = threads.max(1).min(i32::MAX as usize) as i32;
    native.model_config.debug = false;
    native.decoding_method = Some("greedy_search".into());

    OfflineRecognizer::create(&native)
        .map(Arc::new)
        .ok_or_else(|| SttError::Backend("sherpa-onnx could not create Vietnamese Zipformer recognizer".into()))
}

fn transcribe_zipformer(
    recognizer: &OfflineRecognizer,
    utterance: &Utterance,
) -> Result<Transcript, SttError> {
    if utterance.samples.is_empty() || utterance.sample_rate == 0 {
        return Err(SttError::InvalidAudio("utterance is empty or has an invalid sample rate".into()));
    }
    let sample_rate = i32::try_from(utterance.sample_rate)
        .map_err(|_| SttError::InvalidAudio("sample rate cannot be represented by sherpa-onnx".into()))?;

    let stream = recognizer.create_stream();
    // sherpa-onnx accepts the microphone source rate and performs feature-side
    // resampling when required, so we do not resample a second time here.
    stream.accept_waveform(sample_rate, &utterance.samples);
    recognizer.decode(&stream);
    let text = stream
        .get_result()
        .ok_or_else(|| SttError::Backend("sherpa-onnx returned no recognition result".into()))?
        .text
        .trim()
        .to_owned();

    Ok(Transcript {
        text,
        language: Some("vi".into()),
        engine: "sherpa-onnx/zipformer-vi-30m-int8".into(),
        source_duration_seconds: utterance.duration_seconds(),
    })
}

fn load_whisper_fallback(config: &WhisperConfig) -> Result<Option<Arc<WhisperContext>>, SttError> {
    let Some(path) = config.fallback_whisper_model_path.as_deref() else {
        return Ok(None);
    };
    if !path.is_file() {
        return Ok(None);
    }

    install_logging_hooks();
    let mut context_params = WhisperContextParameters::default();
    context_params.use_gpu = config.use_gpu;
    WhisperContext::new_with_params(path, context_params)
        .map(Arc::new)
        .map(Some)
        .map_err(|error| SttError::Backend(error.to_string()))
}

fn transcribe_whisper(
    context: &WhisperContext,
    config: &WhisperConfig,
    utterance: &Utterance,
) -> Result<Transcript, SttError> {
    let source_duration_seconds = utterance.duration_seconds();
    let audio = prepare_for_whisper(utterance)?;
    let mut state = context
        .create_state()
        .map_err(|error| SttError::Backend(error.to_string()))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(config.threads.max(1).min(i32::MAX as usize) as i32);
    params.set_translate(false);
    params.set_no_context(true);
    params.set_no_timestamps(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_language(config.language.as_deref());

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
        language: config.language.clone(),
        engine: "whisper.cpp/fallback".into(),
        source_duration_seconds,
    })
}

fn resolve_zipformer_dir(anchor: &Path) -> Option<PathBuf> {
    if let Some(value) = env::var_os("ASSISTANT_ZIPFORMER_MODEL_DIR") {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return Some(path);
        }
    }

    if anchor.file_name().and_then(|name| name.to_str()) == Some(ZIPFORMER_ENCODER) {
        return anchor.parent().map(Path::to_path_buf);
    }

    let models_root = anchor.parent()?.parent()?;
    Some(models_root.join("stt").join(ZIPFORMER_DEFAULT_DIR))
}

fn resolve_whisper_fallback(anchor: &Path) -> Option<PathBuf> {
    if let Some(value) = env::var_os("ASSISTANT_WHISPER_MODEL") {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return Some(path);
        }
    }

    if anchor.file_name().and_then(|name| name.to_str()) != Some(ZIPFORMER_ENCODER) {
        return Some(anchor.to_path_buf());
    }

    let models_root = anchor.parent()?.parent()?.parent()?;
    Some(models_root.join("whisper").join("ggml-base.bin"))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
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
