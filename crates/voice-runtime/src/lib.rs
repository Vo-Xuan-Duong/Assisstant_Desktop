pub mod stt;
pub mod tts;
pub mod vad;
pub mod wake;
pub mod wake_runtime;
#[cfg(feature = "wake-sherpa")]
pub mod sherpa_wake;
#[cfg(feature = "whisper")]
pub mod whisper;

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, SampleFormat, SizedSample, Stream, StreamConfig, I24, U24,
};
use serde::Serialize;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::debug;

#[derive(Debug, Error)]
pub enum VoiceError {
    #[error("no default microphone/input device is available")]
    NoInputDevice,
    #[error("unsupported microphone sample format: {0}")]
    UnsupportedSampleFormat(String),
    #[error("audio backend error: {0}")]
    Audio(String),
}

impl From<cpal::Error> for VoiceError {
    fn from(error: cpal::Error) -> Self {
        Self::Audio(error.to_string())
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct AudioLevel {
    /// Root-mean-square signal level in the normalized 0..=1 sample domain.
    pub rms: f32,
    /// Maximum absolute sample value in this chunk.
    pub peak: f32,
}

impl AudioLevel {
    pub fn from_samples(samples: &[f32]) -> Self {
        if samples.is_empty() {
            return Self { rms: 0.0, peak: 0.0 };
        }

        let mut sum_squares = 0.0f64;
        let mut peak = 0.0f32;
        for sample in samples {
            let value = sample.clamp(-1.0, 1.0);
            peak = peak.max(value.abs());
            sum_squares += f64::from(value) * f64::from(value);
        }

        Self {
            rms: (sum_squares / samples.len() as f64).sqrt() as f32,
            peak,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioChunk {
    /// Interleaved hardware channels are downmixed to one normalized f32 channel.
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub level: AudioLevel,
}

impl AudioChunk {
    pub fn duration_seconds(&self) -> f32 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.samples.len() as f32 / self.sample_rate as f32
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MicrophoneInfo {
    pub device: String,
    pub sample_rate: u32,
    pub source_channels: u16,
    pub source_sample_format: String,
}

#[derive(Debug, Clone, Copy)]
pub struct MicrophoneConfig {
    /// Maximum number of audio callbacks waiting for the async consumer.
    /// The realtime callback never blocks; chunks are dropped when this queue is full.
    pub channel_capacity: usize,
}

impl Default for MicrophoneConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 16,
        }
    }
}

pub struct MicrophoneStream {
    stream: Stream,
    receiver: mpsc::Receiver<AudioChunk>,
    info: MicrophoneInfo,
    dropped_chunks: Arc<AtomicU64>,
    last_error: Arc<Mutex<Option<String>>>,
}

impl MicrophoneStream {
    pub fn open_default(config: MicrophoneConfig) -> Result<Self, VoiceError> {
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or(VoiceError::NoInputDevice)?;
        let supported = device.default_input_config()?;
        let sample_format = supported.sample_format();
        let stream_config: StreamConfig = supported.into();

        if stream_config.channels == 0 {
            return Err(VoiceError::Audio(
                "default microphone reported zero input channels".into(),
            ));
        }

        let channel_capacity = config.channel_capacity.max(1);
        let (sender, receiver) = mpsc::channel(channel_capacity);
        let dropped_chunks = Arc::new(AtomicU64::new(0));
        let last_error = Arc::new(Mutex::new(None));

        let stream = build_input_stream(
            &device,
            stream_config,
            sample_format,
            sender,
            Arc::clone(&dropped_chunks),
            Arc::clone(&last_error),
        )?;

        // CPAL 0.18 creates streams paused on all backends; start explicitly.
        stream.play()?;

        let info = MicrophoneInfo {
            device: device.to_string(),
            sample_rate: stream_config.sample_rate,
            source_channels: stream_config.channels,
            source_sample_format: sample_format.to_string(),
        };

        debug!(
            device = %info.device,
            sample_rate = info.sample_rate,
            channels = info.source_channels,
            sample_format = %info.source_sample_format,
            "default microphone stream opened"
        );

        Ok(Self {
            stream,
            receiver,
            info,
            dropped_chunks,
            last_error,
        })
    }

    pub fn info(&self) -> &MicrophoneInfo {
        &self.info
    }

    pub fn dropped_chunks(&self) -> u64 {
        self.dropped_chunks.load(Ordering::Relaxed)
    }

    pub fn last_error(&self) -> Option<String> {
        self.last_error
            .lock()
            .ok()
            .and_then(|error| error.clone())
    }

    pub async fn next_chunk(&mut self) -> Option<AudioChunk> {
        self.receiver.recv().await
    }

    pub fn pause(&self) -> Result<(), VoiceError> {
        self.stream.pause()?;
        Ok(())
    }

    pub fn resume(&self) -> Result<(), VoiceError> {
        self.stream.play()?;
        Ok(())
    }
}

fn build_input_stream(
    device: &cpal::Device,
    config: StreamConfig,
    sample_format: SampleFormat,
    sender: mpsc::Sender<AudioChunk>,
    dropped_chunks: Arc<AtomicU64>,
    last_error: Arc<Mutex<Option<String>>>,
) -> Result<Stream, VoiceError> {
    match sample_format {
        SampleFormat::F32 => build_typed_stream::<f32>(device, config, sender, dropped_chunks, last_error),
        SampleFormat::F64 => build_typed_stream::<f64>(device, config, sender, dropped_chunks, last_error),
        SampleFormat::I8 => build_typed_stream::<i8>(device, config, sender, dropped_chunks, last_error),
        SampleFormat::I16 => build_typed_stream::<i16>(device, config, sender, dropped_chunks, last_error),
        SampleFormat::I24 => build_typed_stream::<I24>(device, config, sender, dropped_chunks, last_error),
        SampleFormat::I32 => build_typed_stream::<i32>(device, config, sender, dropped_chunks, last_error),
        SampleFormat::I64 => build_typed_stream::<i64>(device, config, sender, dropped_chunks, last_error),
        SampleFormat::U8 => build_typed_stream::<u8>(device, config, sender, dropped_chunks, last_error),
        SampleFormat::U16 => build_typed_stream::<u16>(device, config, sender, dropped_chunks, last_error),
        SampleFormat::U24 => build_typed_stream::<U24>(device, config, sender, dropped_chunks, last_error),
        SampleFormat::U32 => build_typed_stream::<u32>(device, config, sender, dropped_chunks, last_error),
        SampleFormat::U64 => build_typed_stream::<u64>(device, config, sender, dropped_chunks, last_error),
        unsupported => Err(VoiceError::UnsupportedSampleFormat(unsupported.to_string())),
    }
}

fn build_typed_stream<T>(
    device: &cpal::Device,
    config: StreamConfig,
    sender: mpsc::Sender<AudioChunk>,
    dropped_chunks: Arc<AtomicU64>,
    last_error: Arc<Mutex<Option<String>>>,
) -> Result<Stream, VoiceError>
where
    T: SizedSample + Copy + Send + 'static,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels);
    let sample_rate = config.sample_rate;
    let callback_error = Arc::clone(&last_error);

    let stream = device.build_input_stream::<T, _, _>(
        config,
        move |data: &[T], _| {
            let samples = downmix_to_mono(data, channels);
            if samples.is_empty() {
                return;
            }

            let level = AudioLevel::from_samples(&samples);
            let chunk = AudioChunk {
                samples,
                sample_rate,
                level,
            };

            if sender.try_send(chunk).is_err() {
                dropped_chunks.fetch_add(1, Ordering::Relaxed);
            }
        },
        move |error| {
            if let Ok(mut slot) = callback_error.lock() {
                *slot = Some(error.to_string());
            }
        },
        None,
    )?;

    Ok(stream)
}

fn downmix_to_mono<T>(data: &[T], channels: usize) -> Vec<f32>
where
    T: Copy,
    f32: FromSample<T>,
{
    if channels == 0 {
        return Vec::new();
    }

    let frames = data.len() / channels;
    let mut mono = Vec::with_capacity(frames);
    for frame in data.chunks_exact(channels) {
        let sum = frame
            .iter()
            .copied()
            .map(<f32 as FromSample<T>>::from_sample_)
            .sum::<f32>();
        mono.push((sum / channels as f32).clamp(-1.0, 1.0));
    }
    mono
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_is_downmixed_to_mono() {
        let mono = downmix_to_mono(&[1.0f32, -1.0, 0.5, 0.5], 2);
        assert_eq!(mono, vec![0.0, 0.5]);
    }

    #[test]
    fn level_is_zero_for_silence() {
        let level = AudioLevel::from_samples(&[0.0, 0.0, 0.0]);
        assert_eq!(level.rms, 0.0);
        assert_eq!(level.peak, 0.0);
    }
}
