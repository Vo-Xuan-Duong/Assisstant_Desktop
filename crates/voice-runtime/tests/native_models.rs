#![cfg(all(feature = "zipformer", feature = "wake-sherpa"))]

use std::path::PathBuf;

use voice_runtime::{
    AudioChunk, AudioLevel,
    sherpa_wake::SherpaWakeWordDetector,
    stt::SpeechRecognizer,
    vad::Utterance,
    wake::{SherpaWakeConfig, WakeWordDetector},
    wake_keywords::prepare_gigaspeech_keyword,
    zipformer::{ZipformerConfig, ZipformerModelPaths, ZipformerRecognizer},
};

const ZIPFORMER_DIR: &str = "stt/sherpa-onnx-zipformer-vi-30M-int8-2026-02-09";

/// Exercises real native libraries and models without accessing a microphone.
/// Run explicitly with ASSISTANT_TEST_MODELS_DIR pointing to the models directory.
#[tokio::test]
#[ignore = "requires locally installed Vietnamese Zipformer and GigaSpeech model files"]
async fn native_models_load_and_process_audio() {
    let Some(models_os) = std::env::var_os("ASSISTANT_TEST_MODELS_DIR") else {
        eprintln!("Skipping native_models test: ASSISTANT_TEST_MODELS_DIR is not set");
        return;
    };
    let models = PathBuf::from(models_os);
    let wake_dir = models.join("wake/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01");
    let bpe = wake_dir.join("bpe.model");
    let tokens = wake_dir.join("tokens.txt");
    let stt_dir = models.join(ZIPFORMER_DIR);
    let stt_paths = ZipformerModelPaths::from_dir(&stt_dir);

    if !bpe.is_file() || !tokens.is_file() || !stt_paths.is_complete() {
        eprintln!(
            "Skipping native_models test: model resources not found under {}",
            models.display()
        );
        return;
    }
    let prepared = prepare_gigaspeech_keyword(
        &wake_dir.join("bpe.model"),
        &wake_dir.join("tokens.txt"),
        "HELLO WORLD",
    )
    .expect("SentencePiece must load and produce supported wake tokens");
    assert_eq!(prepared.canonical_label, "HELLO_WORLD");

    let mut detector = SherpaWakeWordDetector::load(SherpaWakeConfig::gigaspeech_int8(
        &wake_dir,
        wake_dir.join("keywords.txt"),
    ))
    .expect("Sherpa must load the installed wake model");
    let silence = vec![0.0; 32_000];
    assert!(
        detector
            .process(&AudioChunk {
                level: AudioLevel::from_samples(&silence),
                samples: silence.clone(),
                sample_rate: 16_000,
            })
            .expect("Sherpa wake inference must succeed")
            .is_none()
    );
    detector.reset().unwrap();

    let recognizer = ZipformerRecognizer::load(ZipformerConfig::new(stt_dir))
        .expect("Vietnamese Zipformer must load the installed model bundle");
    let transcript = recognizer
        .transcribe(Utterance {
            samples: silence,
            sample_rate: 16_000,
        })
        .await
        .expect("Zipformer inference must succeed");
    assert_eq!(transcript.source_duration_seconds, 2.0);
}
