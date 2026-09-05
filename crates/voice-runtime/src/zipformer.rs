use std::{path::{Path, PathBuf}, sync::Arc};

use async_trait::async_trait;
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig};

use crate::{
    stt::{SpeechRecognizer, SttError, Transcript},
    vad::Utterance,
};

pub const ZIPFORMER_ENCODER_FILE: &str = "encoder.int8.onnx";
pub const ZIPFORMER_DECODER_FILE: &str = "decoder.onnx";
pub const ZIPFORMER_JOINER_FILE: &str = "joiner.int8.onnx";
pub const ZIPFORMER_TOKENS_FILE: &str = "tokens.txt";
pub const ZIPFORMER_BPE_FILE: &str = "bpe.model";

#[derive(Debug, Clone)]
pub struct ZipformerConfig {
    pub model_dir: PathBuf,
    /// Kept so the existing desktop voice state can continue assigning `vi`
    /// during the migration away from the historical Whisper symbol names.
    pub language: Option<String>,
    pub threads: usize,
    pub provider: String,
}

impl ZipformerConfig {
    pub fn new(model_path_or_dir: impl Into<PathBuf>) -> Self {
        let path = model_path_or_dir.into();
        let model_dir = if path.file_name().and_then(|name| name.to_str()) == Some(ZIPFORMER_ENCODER_FILE) {
            path.parent().map(Path::to_path_buf).unwrap_or(path)
        } else {
            path
        };
        Self {
            model_dir,
            language: Some("vi".into()),
            threads: 4,
            provider: "cpu".into(),
        }
    }

    pub fn paths(&self) -> ZipformerModelPaths {
        ZipformerModelPaths::from_dir(&self.model_dir)
    }
}

#[derive(Debug, Clone)]
pub struct ZipformerModelPaths {
    pub encoder: PathBuf,
    pub decoder: PathBuf,
    pub joiner: PathBuf,
    pub tokens: PathBuf,
    pub bpe: PathBuf,
}

impl ZipformerModelPaths {
    pub fn from_dir(dir: &Path) -> Self {
        Self {
            encoder: dir.join(ZIPFORMER_ENCODER_FILE),
            decoder: dir.join(ZIPFORMER_DECODER_FILE),
            joiner: dir.join(ZIPFORMER_JOINER_FILE),
            tokens: dir.join(ZIPFORMER_TOKENS_FILE),
            bpe: dir.join(ZIPFORMER_BPE_FILE),
        }
    }

    pub fn required_files(&self) -> [&Path; 4] {
        [
            self.encoder.as_path(),
            self.decoder.as_path(),
            self.joiner.as_path(),
            self.tokens.as_path(),
        ]
    }

    pub fn is_complete(&self) -> bool {
        self.required_files().into_iter().all(Path::is_file)
    }
}

#[derive(Clone)]
pub struct ZipformerRecognizer {
    recognizer: Arc<OfflineRecognizer>,
    language: Option<String>,
}

impl ZipformerRecognizer {
    pub fn load(config: ZipformerConfig) -> Result<Self, SttError> {
        let paths = config.paths();
        let missing = [
            ("encoder", paths.encoder.as_path()),
            ("decoder", paths.decoder.as_path()),
            ("joiner", paths.joiner.as_path()),
            ("tokens", paths.tokens.as_path()),
        ]
        .into_iter()
        .filter_map(|(name, path)| (!path.is_file()).then(|| format!("{name}: {}", path.display())))
        .collect::<Vec<_>>();

        if !missing.is_empty() {
            return Err(SttError::Backend(format!(
                "Vietnamese Zipformer model is incomplete; missing {}",
                missing.join(", ")
            )));
        }

        let mut native = OfflineRecognizerConfig::default();
        native.model_config.transducer = OfflineTransducerModelConfig {
            encoder: Some(path_string(&paths.encoder)),
            decoder: Some(path_string(&paths.decoder)),
            joiner: Some(path_string(&paths.joiner)),
        };
        native.model_config.tokens = Some(path_string(&paths.tokens));
        native.model_config.provider = Some(config.provider);
        native.model_config.num_threads = config.threads.max(1).min(i32::MAX as usize) as i32;
        native.model_config.debug = false;
        native.decoding_method = Some("greedy_search".into());

        let recognizer = OfflineRecognizer::create(&native).ok_or_else(|| {
            SttError::Backend("sherpa-onnx could not create the Vietnamese Zipformer recognizer".into())
        })?;

        Ok(Self {
            recognizer: Arc::new(recognizer),
            language: config.language,
        })
    }

    fn transcribe_blocking(&self, utterance: Utterance) -> Result<Transcript, SttError> {
        if utterance.samples.is_empty() || utterance.sample_rate == 0 {
            return Err(SttError::InvalidAudio(
                "utterance is empty or has an invalid sample rate".into(),
            ));
        }

        let sample_rate = i32::try_from(utterance.sample_rate).map_err(|_| {
            SttError::InvalidAudio("sample rate cannot be represented by sherpa-onnx".into())
        })?;
        let source_duration_seconds = utterance.duration_seconds();
        let stream = self.recognizer.create_stream();
        stream.accept_waveform(sample_rate, &utterance.samples);
        self.recognizer.decode(&stream);

        let text = stream
            .get_result()
            .ok_or_else(|| SttError::Backend("sherpa-onnx returned no recognition result".into()))?
            .text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        Ok(Transcript {
            text,
            language: self.language.clone().or_else(|| Some("vi".into())),
            engine: "sherpa-onnx/zipformer-vi-30m-int8".into(),
            source_duration_seconds,
        })
    }
}

#[async_trait]
impl SpeechRecognizer for ZipformerRecognizer {
    async fn transcribe(&self, utterance: Utterance) -> Result<Transcript, SttError> {
        let recognizer = self.clone();
        tokio::task::spawn_blocking(move || recognizer.transcribe_blocking(utterance))
            .await
            .map_err(|error| SttError::Worker(error.to_string()))?
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
