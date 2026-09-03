use async_trait::async_trait;
use serde::Serialize;
use thiserror::Error;

use crate::vad::Utterance;

pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

#[derive(Debug, Error)]
pub enum SttError {
    #[error("invalid speech audio: {0}")]
    InvalidAudio(String),
    #[error("speech recognition backend failed: {0}")]
    Backend(String),
    #[error("speech recognition worker failed: {0}")]
    Worker(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct Transcript {
    pub text: String,
    pub language: Option<String>,
    pub engine: String,
    pub source_duration_seconds: f32,
}

impl Transcript {
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

#[async_trait]
pub trait SpeechRecognizer: Send + Sync {
    async fn transcribe(&self, utterance: Utterance) -> Result<Transcript, SttError>;
}

/// Convert a complete mono utterance to a fixed sample rate.
///
/// This baseline resampler uses linear interpolation. It lives outside the
/// realtime microphone callback and is intentionally replaceable by a higher
/// quality band-limited implementation without changing the recognizer trait.
pub fn resample_mono(
    samples: &[f32],
    source_rate: u32,
    target_rate: u32,
) -> Result<Vec<f32>, SttError> {
    if source_rate == 0 || target_rate == 0 {
        return Err(SttError::InvalidAudio(
            "sample rate must be greater than zero".into(),
        ));
    }
    if samples.is_empty() {
        return Err(SttError::InvalidAudio("utterance is empty".into()));
    }
    if source_rate == target_rate {
        return Ok(samples.to_vec());
    }

    let output_len = ((samples.len() as u128 * u128::from(target_rate))
        .div_ceil(u128::from(source_rate)))
        .min(usize::MAX as u128) as usize;
    let mut output = Vec::with_capacity(output_len);
    let ratio = source_rate as f64 / target_rate as f64;

    for output_index in 0..output_len {
        let source_position = output_index as f64 * ratio;
        let left_index = source_position.floor() as usize;
        if left_index >= samples.len() - 1 {
            output.push(*samples.last().unwrap_or(&0.0));
            continue;
        }

        let right_index = left_index + 1;
        let fraction = (source_position - left_index as f64) as f32;
        let left = samples[left_index];
        let right = samples[right_index];
        output.push((left + (right - left) * fraction).clamp(-1.0, 1.0));
    }

    Ok(output)
}

pub fn prepare_for_whisper(utterance: &Utterance) -> Result<Vec<f32>, SttError> {
    resample_mono(
        &utterance.samples,
        utterance.sample_rate,
        WHISPER_SAMPLE_RATE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_rate_preserves_samples() {
        let samples = vec![0.0, 0.5, -0.5];
        assert_eq!(resample_mono(&samples, 16_000, 16_000).unwrap(), samples);
    }

    #[test]
    fn resampling_changes_length_by_rate_ratio() {
        let samples = vec![0.0f32; 48_000];
        let output = resample_mono(&samples, 48_000, 16_000).unwrap();
        assert_eq!(output.len(), 16_000);
    }
}
