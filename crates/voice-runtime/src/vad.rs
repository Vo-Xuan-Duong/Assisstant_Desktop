use std::collections::VecDeque;

use serde::Serialize;

use crate::AudioChunk;

#[derive(Debug, Clone, Copy)]
pub struct VadConfig {
    /// RMS threshold in normalized f32 units used by the baseline local VAD.
    pub speech_rms_threshold: f32,
    /// Consecutive speech required before an utterance starts.
    pub start_trigger_ms: u32,
    /// Silence duration that closes an active utterance.
    pub end_silence_ms: u32,
    /// Audio retained before the speech trigger so initial consonants are not clipped.
    pub pre_roll_ms: u32,
    /// Utterances shorter than this are discarded as likely noise/clicks.
    pub min_utterance_ms: u32,
    /// Safety bound preventing an accidental endless recording.
    pub max_utterance_ms: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            speech_rms_threshold: 0.012,
            start_trigger_ms: 120,
            end_silence_ms: 650,
            pre_roll_ms: 220,
            min_utterance_ms: 250,
            max_utterance_ms: 15_000,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Utterance {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl Utterance {
    pub fn duration_seconds(&self) -> f32 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.samples.len() as f32 / self.sample_rate as f32
        }
    }
}

#[derive(Debug, Clone)]
pub enum VadEvent {
    Idle,
    SpeechStarted,
    SpeechContinues,
    UtteranceReady(Utterance),
    DiscardedShortUtterance,
}

pub struct UtteranceSegmenter {
    config: VadConfig,
    sample_rate: Option<u32>,
    pre_roll: VecDeque<f32>,
    active: Vec<f32>,
    active_started: bool,
    speech_run_samples: usize,
    silence_run_samples: usize,
}

impl UtteranceSegmenter {
    pub fn new(config: VadConfig) -> Self {
        Self {
            config,
            sample_rate: None,
            pre_roll: VecDeque::new(),
            active: Vec::new(),
            active_started: false,
            speech_run_samples: 0,
            silence_run_samples: 0,
        }
    }

    pub fn push(&mut self, chunk: AudioChunk) -> VadEvent {
        if chunk.sample_rate == 0 || chunk.samples.is_empty() {
            return VadEvent::Idle;
        }

        if self.sample_rate != Some(chunk.sample_rate) {
            self.reset_for_rate(chunk.sample_rate);
        }

        let sample_rate = chunk.sample_rate;
        let speech = chunk.level.rms >= self.config.speech_rms_threshold;

        if !self.active_started {
            self.push_pre_roll(&chunk.samples, sample_rate);
            if speech {
                self.speech_run_samples = self
                    .speech_run_samples
                    .saturating_add(chunk.samples.len());
            } else {
                self.speech_run_samples = 0;
            }

            if self.speech_run_samples >= samples_for_ms(sample_rate, self.config.start_trigger_ms) {
                self.active.extend(self.pre_roll.drain(..));
                self.active_started = true;
                self.speech_run_samples = 0;
                self.silence_run_samples = 0;
                return VadEvent::SpeechStarted;
            }

            return VadEvent::Idle;
        }

        self.active.extend_from_slice(&chunk.samples);
        if speech {
            self.silence_run_samples = 0;
        } else {
            self.silence_run_samples = self
                .silence_run_samples
                .saturating_add(chunk.samples.len());
        }

        let reached_silence = self.silence_run_samples
            >= samples_for_ms(sample_rate, self.config.end_silence_ms);
        let reached_max = self.active.len()
            >= samples_for_ms(sample_rate, self.config.max_utterance_ms);

        if reached_silence || reached_max {
            return self.finish();
        }

        VadEvent::SpeechContinues
    }

    pub fn flush(&mut self) -> VadEvent {
        if self.active_started {
            self.finish()
        } else {
            VadEvent::Idle
        }
    }

    pub fn reset(&mut self) {
        self.sample_rate = None;
        self.pre_roll.clear();
        self.active.clear();
        self.active_started = false;
        self.speech_run_samples = 0;
        self.silence_run_samples = 0;
    }

    fn reset_for_rate(&mut self, sample_rate: u32) {
        self.reset();
        self.sample_rate = Some(sample_rate);
    }

    fn push_pre_roll(&mut self, samples: &[f32], sample_rate: u32) {
        self.pre_roll.extend(samples.iter().copied());
        let maximum = samples_for_ms(sample_rate, self.config.pre_roll_ms);
        while self.pre_roll.len() > maximum {
            self.pre_roll.pop_front();
        }
    }

    fn finish(&mut self) -> VadEvent {
        let sample_rate = self.sample_rate.unwrap_or(0);
        let samples = std::mem::take(&mut self.active);
        self.active_started = false;
        self.speech_run_samples = 0;
        self.silence_run_samples = 0;
        self.pre_roll.clear();

        if samples.len() < samples_for_ms(sample_rate, self.config.min_utterance_ms) {
            return VadEvent::DiscardedShortUtterance;
        }

        VadEvent::UtteranceReady(Utterance {
            samples,
            sample_rate,
        })
    }
}

impl Default for UtteranceSegmenter {
    fn default() -> Self {
        Self::new(VadConfig::default())
    }
}

fn samples_for_ms(sample_rate: u32, milliseconds: u32) -> usize {
    ((u64::from(sample_rate) * u64::from(milliseconds)) / 1000)
        .min(usize::MAX as u64) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AudioLevel;

    fn chunk(level: f32, milliseconds: usize) -> AudioChunk {
        let sample_rate = 16_000;
        let length = sample_rate as usize * milliseconds / 1000;
        AudioChunk {
            samples: vec![level; length],
            sample_rate,
            level: AudioLevel {
                rms: level.abs(),
                peak: level.abs(),
            },
        }
    }

    #[test]
    fn silence_does_not_start_utterance() {
        let mut vad = UtteranceSegmenter::default();
        assert!(matches!(vad.push(chunk(0.0, 200)), VadEvent::Idle));
    }

    #[test]
    fn speech_then_silence_produces_utterance() {
        let mut vad = UtteranceSegmenter::default();
        assert!(matches!(vad.push(chunk(0.1, 150)), VadEvent::SpeechStarted));
        let event = vad.push(chunk(0.0, 700));
        assert!(matches!(event, VadEvent::UtteranceReady(_)));
    }
}
