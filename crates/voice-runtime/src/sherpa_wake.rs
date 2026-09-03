use sherpa_onnx::{KeywordSpotter, KeywordSpotterConfig, OnlineStream};

use crate::{
    wake::{SherpaWakeConfig, WakeDetection, WakeError, WakeWordDetector},
    AudioChunk,
};

/// Streaming keyword detector backed by sherpa-onnx.
///
/// This object is intended to live on a dedicated wake-word task. It never runs
/// inside CPAL's realtime audio callback.
pub struct SherpaWakeWordDetector {
    spotter: KeywordSpotter,
    stream: OnlineStream,
}

impl SherpaWakeWordDetector {
    pub fn load(config: SherpaWakeConfig) -> Result<Self, WakeError> {
        config.validate()?;

        let mut native = KeywordSpotterConfig::default();
        native.model_config.transducer.encoder = Some(path_string(&config.encoder));
        native.model_config.transducer.decoder = Some(path_string(&config.decoder));
        native.model_config.transducer.joiner = Some(path_string(&config.joiner));
        native.model_config.tokens = Some(path_string(&config.tokens));
        native.model_config.provider = Some("cpu".into());
        native.model_config.num_threads = config.num_threads;
        native.model_config.debug = false;
        native.max_active_paths = config.max_active_paths;
        native.keywords_score = config.keywords_score;
        native.keywords_threshold = config.keywords_threshold;
        native.keywords_file = Some(path_string(&config.keywords));

        let spotter = KeywordSpotter::create(&native).ok_or_else(|| {
            WakeError::Backend("sherpa-onnx could not create KeywordSpotter".into())
        })?;
        let stream = spotter.create_stream();

        Ok(Self { spotter, stream })
    }
}

impl WakeWordDetector for SherpaWakeWordDetector {
    fn process(&mut self, chunk: &AudioChunk) -> Result<Option<WakeDetection>, WakeError> {
        if chunk.samples.is_empty() {
            return Ok(None);
        }

        let sample_rate = i32::try_from(chunk.sample_rate).map_err(|_| {
            WakeError::InvalidConfig(format!(
                "microphone sample rate {} cannot be represented by sherpa-onnx",
                chunk.sample_rate
            ))
        })?;

        // sherpa-onnx's OnlineStream accepts the source sample rate and performs
        // internal resampling when it differs from the feature extractor rate.
        self.stream.accept_waveform(sample_rate, &chunk.samples);

        while self.spotter.is_ready(&self.stream) {
            self.spotter.decode(&self.stream);
            if let Some(result) = self.spotter.get_result(&self.stream) {
                if !result.keyword.is_empty() {
                    let detection = WakeDetection {
                        keyword: result.keyword,
                        start_time_seconds: result.start_time,
                    };
                    // sherpa requires a reset immediately after a keyword fires
                    // before more audio is decoded on the same stream.
                    self.spotter.reset(&self.stream);
                    return Ok(Some(detection));
                }
            }
        }

        Ok(None)
    }

    fn reset(&mut self) -> Result<(), WakeError> {
        self.spotter.reset(&self.stream);
        Ok(())
    }
}

fn path_string(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}
