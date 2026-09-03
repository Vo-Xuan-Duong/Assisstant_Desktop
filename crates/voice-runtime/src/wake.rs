use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use crate::AudioChunk;

#[derive(Debug, Error)]
pub enum WakeError {
    #[error("wake-word model/resource is missing: {0}")]
    MissingResource(String),
    #[error("wake-word configuration is invalid: {0}")]
    InvalidConfig(String),
    #[error("wake-word backend failed: {0}")]
    Backend(String),
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WakeDetection {
    /// Canonical keyword label returned by the detector, e.g. `HEY_ASSISTANT`.
    pub keyword: String,
    /// Detector-provided start timestamp when available.
    pub start_time_seconds: f32,
}

/// A streaming wake-word detector. Implementations consume audio chunks outside
/// the realtime CPAL callback and retain their own decoding state between calls.
pub trait WakeWordDetector: Send {
    fn process(&mut self, chunk: &AudioChunk) -> Result<Option<WakeDetection>, WakeError>;

    /// Reset decoder state after an activation or when microphone routing changes.
    fn reset(&mut self) -> Result<(), WakeError>;
}

#[derive(Debug, Clone)]
pub struct SherpaWakeConfig {
    pub encoder: PathBuf,
    pub decoder: PathBuf,
    pub joiner: PathBuf,
    pub tokens: PathBuf,
    pub keywords: PathBuf,
    pub num_threads: i32,
    pub max_active_paths: i32,
    pub keywords_score: f32,
    pub keywords_threshold: f32,
}

impl SherpaWakeConfig {
    /// Conventional layout for the English GigaSpeech 3.3M int8 KWS model.
    /// `keywords.txt` is intentionally supplied by the application because its
    /// contents must be generated with the model's BPE tokenizer.
    pub fn gigaspeech_int8(model_dir: impl AsRef<Path>, keywords: impl Into<PathBuf>) -> Self {
        let model_dir = model_dir.as_ref();
        Self {
            encoder: model_dir.join("encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx"),
            decoder: model_dir.join("decoder-epoch-12-avg-2-chunk-16-left-64.onnx"),
            joiner: model_dir.join("joiner-epoch-12-avg-2-chunk-16-left-64.int8.onnx"),
            tokens: model_dir.join("tokens.txt"),
            keywords: keywords.into(),
            num_threads: 1,
            max_active_paths: 4,
            keywords_score: 1.0,
            keywords_threshold: 0.25,
        }
    }

    pub fn validate(&self) -> Result<(), WakeError> {
        for (name, path) in [
            ("encoder", &self.encoder),
            ("decoder", &self.decoder),
            ("joiner", &self.joiner),
            ("tokens", &self.tokens),
            ("keywords", &self.keywords),
        ] {
            if !path.is_file() {
                return Err(WakeError::MissingResource(format!(
                    "{name}: {}",
                    path.display()
                )));
            }
        }

        if self.num_threads <= 0 {
            return Err(WakeError::InvalidConfig(
                "num_threads must be greater than zero".into(),
            ));
        }
        if self.max_active_paths <= 0 {
            return Err(WakeError::InvalidConfig(
                "max_active_paths must be greater than zero".into(),
            ));
        }
        if !self.keywords_score.is_finite() || self.keywords_score <= 0.0 {
            return Err(WakeError::InvalidConfig(
                "keywords_score must be a positive finite value".into(),
            ));
        }
        if !self.keywords_threshold.is_finite()
            || !(0.0..=1.0).contains(&self.keywords_threshold)
        {
            return Err(WakeError::InvalidConfig(
                "keywords_threshold must be a finite value from 0 to 1".into(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_gigaspeech_config_is_cpu_small_model_oriented() {
        let config = SherpaWakeConfig::gigaspeech_int8("models/kws", "keywords.txt");
        assert_eq!(config.num_threads, 1);
        assert_eq!(config.max_active_paths, 4);
        assert_eq!(config.keywords_threshold, 0.25);
        assert!(config.encoder.to_string_lossy().ends_with("int8.onnx"));
    }
}
