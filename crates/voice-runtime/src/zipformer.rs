#[path = "stt_hotwords.rs"]
mod stt_hotwords;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig};
use tracing::{debug, warn};

use crate::{
    stt::{SpeechRecognizer, SttError, Transcript},
    vad::Utterance,
};
use stt_hotwords::{
    GENERATED_BPE_VOCAB_FILE, STT_HOTWORDS_FILE, PreparedHotwords,
    ensure_generated_bpe_vocab, load_prepared_hotwords,
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
    /// User-managed plain-text phrases. Missing/empty means contextual biasing is off.
    pub hotwords_path: PathBuf,
    /// Global token bonus used only by modified beam search.
    pub hotwords_score: f32,
    /// Beam width used only by the contextual recognizer.
    pub max_active_paths: i32,
}

impl ZipformerConfig {
    pub fn new(model_path_or_dir: impl Into<PathBuf>) -> Self {
        let path = model_path_or_dir.into();
        let model_dir = if path.file_name().and_then(|name| name.to_str())
            == Some(ZIPFORMER_ENCODER_FILE)
        {
            path.parent().map(Path::to_path_buf).unwrap_or(path)
        } else {
            path
        };
        let hotwords_path = model_dir.join(STT_HOTWORDS_FILE);
        Self {
            model_dir,
            language: Some("vi".into()),
            threads: 4,
            provider: "cpu".into(),
            hotwords_path,
            hotwords_score: 1.5,
            max_active_paths: 4,
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
    /// Generated locally from `bpe.model`; needed only for contextual BPE hotwords.
    pub bpe_vocab: PathBuf,
}

impl ZipformerModelPaths {
    pub fn from_dir(dir: &Path) -> Self {
        Self {
            encoder: dir.join(ZIPFORMER_ENCODER_FILE),
            decoder: dir.join(ZIPFORMER_DECODER_FILE),
            joiner: dir.join(ZIPFORMER_JOINER_FILE),
            tokens: dir.join(ZIPFORMER_TOKENS_FILE),
            bpe: dir.join(ZIPFORMER_BPE_FILE),
            bpe_vocab: dir.join(GENERATED_BPE_VOCAB_FILE),
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
    /// Baseline recognizer: preserves the existing low-overhead greedy path.
    recognizer: Arc<OfflineRecognizer>,
    /// Created only after at least one valid hotword is present.
    contextual: Arc<Mutex<Option<Arc<OfflineRecognizer>>>>,
    config: Arc<ZipformerConfig>,
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
        .filter_map(|(name, path)| {
            (!path.is_file()).then(|| format!("{name}: {}", path.display()))
        })
        .collect::<Vec<_>>();

        if !missing.is_empty() {
            return Err(SttError::Backend(format!(
                "Vietnamese Zipformer model is incomplete; missing {}",
                missing.join(", ")
            )));
        }

        let recognizer = create_native_recognizer(&config, false)?;
        let language = config.language.clone();
        Ok(Self {
            recognizer: Arc::new(recognizer),
            contextual: Arc::new(Mutex::new(None)),
            config: Arc::new(config),
            language,
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
        let paths = self.config.paths();

        let prepared = match load_prepared_hotwords(
            &self.config.hotwords_path,
            &paths.bpe,
            &paths.tokens,
        ) {
            Ok(value) => value,
            Err(error) => {
                warn!(
                    %error,
                    path = %self.config.hotwords_path.display(),
                    "STT contextual hotwords are unavailable; falling back to greedy decoding"
                );
                None
            }
        };

        let (text, engine) = match prepared {
            Some(hotwords) => {
                for rejected in &hotwords.rejected {
                    warn!(
                        phrase = %rejected.phrase,
                        reason = %rejected.reason,
                        "ignoring unsupported STT hotword"
                    );
                }

                if hotwords.is_empty() {
                    self.decode_greedy(sample_rate, &utterance.samples)?
                } else {
                    match self.decode_with_hotwords(sample_rate, &utterance.samples, hotwords) {
                        Ok(result) => result,
                        Err(error) => {
                            warn!(
                                %error,
                                "contextual STT setup failed; falling back to greedy decoding"
                            );
                            self.decode_greedy(sample_rate, &utterance.samples)?
                        }
                    }
                }
            }
            None => self.decode_greedy(sample_rate, &utterance.samples)?,
        };

        Ok(Transcript {
            text,
            language: self.language.clone().or_else(|| Some("vi".into())),
            engine: engine.into(),
            source_duration_seconds,
        })
    }

    fn decode_greedy(
        &self,
        sample_rate: i32,
        samples: &[f32],
    ) -> Result<(String, &'static str), SttError> {
        let stream = self.recognizer.create_stream();
        stream.accept_waveform(sample_rate, samples);
        self.recognizer.decode(&stream);
        let text = result_text(&stream)?;
        Ok((text, "sherpa-onnx/zipformer-vi-30m-int8"))
    }

    fn decode_with_hotwords(
        &self,
        sample_rate: i32,
        samples: &[f32],
        hotwords: PreparedHotwords,
    ) -> Result<(String, &'static str), SttError> {
        let recognizer = self.contextual_recognizer()?;
        debug!(
            phrases = hotwords.phrases.len(),
            score = self.config.hotwords_score,
            max_active_paths = self.config.max_active_paths,
            "decoding Vietnamese speech with contextual hotwords"
        );

        let stream = recognizer.create_stream_with_hotwords(&hotwords.inline);
        stream.accept_waveform(sample_rate, samples);
        recognizer.decode(&stream);
        let text = result_text(&stream)?;
        Ok((text, "sherpa-onnx/zipformer-vi-30m-int8+hotwords"))
    }

    fn contextual_recognizer(&self) -> Result<Arc<OfflineRecognizer>, SttError> {
        let mut slot = self
            .contextual
            .lock()
            .map_err(|_| SttError::Worker("contextual recognizer mutex is poisoned".into()))?;
        if let Some(recognizer) = slot.as_ref() {
            return Ok(Arc::clone(recognizer));
        }

        let paths = self.config.paths();
        ensure_generated_bpe_vocab(&paths.bpe, &paths.bpe_vocab).map_err(|error| {
            SttError::Backend(format!(
                "cannot prepare contextual Zipformer BPE vocabulary: {error}"
            ))
        })?;

        let recognizer = Arc::new(create_native_recognizer(&self.config, true)?);
        *slot = Some(Arc::clone(&recognizer));
        Ok(recognizer)
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

fn create_native_recognizer(
    config: &ZipformerConfig,
    contextual: bool,
) -> Result<OfflineRecognizer, SttError> {
    let paths = config.paths();
    let mut native = OfflineRecognizerConfig::default();
    native.model_config.transducer = OfflineTransducerModelConfig {
        encoder: Some(path_string(&paths.encoder)),
        decoder: Some(path_string(&paths.decoder)),
        joiner: Some(path_string(&paths.joiner)),
    };
    native.model_config.tokens = Some(path_string(&paths.tokens));
    native.model_config.provider = Some(config.provider.clone());
    native.model_config.num_threads = config.threads.max(1).min(i32::MAX as usize) as i32;
    native.model_config.debug = false;

    if contextual {
        native.decoding_method = Some("modified_beam_search".into());
        native.model_config.modeling_unit = Some("bpe".into());
        native.model_config.bpe_vocab = Some(path_string(&paths.bpe_vocab));
        native.max_active_paths = config.max_active_paths.max(1);
        native.hotwords_score = if config.hotwords_score.is_finite() {
            config.hotwords_score.max(0.0)
        } else {
            1.5
        };
    } else {
        native.decoding_method = Some("greedy_search".into());
    }

    let method = native.decoding_method.as_deref().unwrap_or("default");
    OfflineRecognizer::create(&native).ok_or_else(|| {
        SttError::Backend(format!(
            "sherpa-onnx could not create the Vietnamese Zipformer recognizer ({method})"
        ))
    })
}

fn result_text(stream: &sherpa_onnx::OfflineStream) -> Result<String, SttError> {
    Ok(stream
        .get_result()
        .ok_or_else(|| SttError::Backend("sherpa-onnx returned no recognition result".into()))?
        .text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" "))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
