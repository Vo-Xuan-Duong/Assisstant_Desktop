# Voice Runtime — Phase 5B VAD + STT

## Scope

Phase 5B turns the normalized microphone chunks from Phase 5A into complete speech utterances and defines a replaceable speech-recognition interface.

```text
MicrophoneStream
      |
  AudioChunk
      |
UtteranceSegmenter
      |
  Utterance
      |
SpeechRecognizer
      |
WhisperRecognizer   (optional feature)
      |
 Transcript
```

The desktop UI is intentionally not connected to this pipeline until Phase 5C, when TTS and the full voice-turn lifecycle are available.

## Baseline VAD

`UtteranceSegmenter` is local and has no model dependency. It uses chunk RMS levels to detect speech boundaries.

Default behavior:

- speech RMS threshold: `0.012`;
- speech start trigger: `120 ms`;
- pre-roll: `220 ms`;
- end-of-speech silence: `650 ms`;
- minimum utterance: `250 ms`;
- maximum utterance: `15 s`.

Pre-roll avoids clipping the beginning of a phrase while the start threshold is being confirmed.

The VAD is intentionally a replaceable baseline. Later versions can use a neural VAD without changing the `Utterance` or STT interfaces.

## SpeechRecognizer contract

Recognition engines implement:

```text
SpeechRecognizer::transcribe(Utterance) -> Transcript
```

`Transcript` includes:

- text;
- language when known/configured;
- engine name;
- original utterance duration.

Recognition is asynchronous at the product boundary so CPU-heavy engines can run outside Tauri/Tokio's normal async task execution.

## Resampling

Whisper requires mono `f32` PCM at 16 kHz. Microphone hardware commonly runs at 44.1 or 48 kHz.

Phase 5B includes a replaceable whole-utterance linear resampler:

```text
hardware mono f32
44.1/48 kHz
     |
resample_mono
     |
16 kHz mono f32
```

It runs after VAD, never inside the realtime audio callback.

The linear resampler is an integration baseline, not the final quality ceiling. A future band-limited resampler can replace it behind the same function boundary if local speech-recognition benchmarks justify the additional dependency/CPU cost.

## Whisper feature

Whisper support is **disabled by default**.

```toml
voice-runtime = { ..., features = ["whisper"] }
```

Only enabling this feature pulls `whisper-rs` / whisper.cpp native compilation into the build.

This separation is deliberate because the Whisper native build requires additional C/C++ tooling and should not make every ordinary Tauri build heavier.

## WhisperRecognizer

`WhisperRecognizer`:

- loads a local ggml/gguf-compatible model path accepted by whisper.cpp;
- defaults to Vietnamese (`vi`), but language can be changed or set to auto detection;
- defaults to CPU execution;
- runs recognition in `tokio::task::spawn_blocking`;
- disables model stdout progress/realtime printing;
- creates a fresh Whisper state for each complete utterance;
- uses greedy decoding as the low-latency baseline.

Whisper model installation/download is not handled by this phase. Model lifecycle belongs to a later application-data/model-management step.

## Why not call Antigravity for STT

Speech-to-text is deliberately local:

- no additional API/quota consumption;
- microphone audio is not sent to Antigravity merely for transcription;
- latency is independent of network round trips;
- the Antigravity quota is reserved for reasoning/tool orchestration.

Only the resulting text enters Assistant Core.

## Build and verification policy

Default builds do not compile Whisper.

No GitHub Actions or runtime tests are executed during repository development. The feature-gated native build and model accuracy are to be verified locally on the Windows development machine when the Whisper feature is enabled.
