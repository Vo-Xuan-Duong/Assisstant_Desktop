# Voice Runtime — Vietnamese STT

## Current architecture

The desktop assistant uses a Vietnamese-specific sherpa-onnx Zipformer model as the primary local speech recognizer.

```text
MicrophoneStream (CPAL)
      |
  AudioChunk
      |
UtteranceSegmenter / VAD
      |
  complete Utterance
      |
SpeechRecognizer
      |
Vietnamese Zipformer 30M INT8
      |
 Transcript
      |
Assistant Core
```

Microphone audio remains local. Only the final transcript enters Assistant Core/Antigravity.

This migration replaces the recognizer, not the capture boundary: the current implementation still waits for the existing VAD to finish one utterance before running offline recognition. Partial/streaming transcript events are not implemented yet.

## Primary model

Model family:

```text
sherpa-onnx-zipformer-vi-30M-int8-2026-02-09
```

Sherpa export revision pinned by the resource installer:

```text
83e140db6d23fbb8480fd5fb868f74ab80e7092c
```

Required runtime files:

```text
encoder.int8.onnx
decoder.onnx
joiner.int8.onnx
tokens.txt
```

Preparation file installed with the bundle for future contextual-biasing work:

```text
bpe.model
```

Default model directory:

```text
%LOCALAPPDATA%/<Assisstant Desktop app-data>/models/stt/
  sherpa-onnx-zipformer-vi-30M-int8-2026-02-09/
```

Override with an absolute path:

```text
ASSISTANT_ZIPFORMER_MODEL_DIR
```

Successful transcripts report:

```text
sherpa-onnx/zipformer-vi-30m-int8
```

## Resource installation

The STT resource id is:

```text
stt_zipformer_vi
```

Resource Setup can install the model directly. Installation is transactional:

1. create a staging directory beside the final model directory;
2. download the immutable model revision;
3. verify exact byte size and SHA-256 for `encoder.int8.onnx`, `decoder.onnx`, `joiner.int8.onnx` and `bpe.model`;
4. download `tokens.txt` with a strict size bound;
5. validate the token file as UTF-8, exactly 2000 sequential token ids, and the expected special tokens;
6. atomically rename the complete staging directory into the runtime model path;
7. remove the staging directory if any step fails.

The installer refuses to overwrite a non-empty existing model directory.

## Model license

The upstream Vietnamese model is licensed:

```text
CC-BY-NC-ND-4.0
```

That license is non-commercial and no-derivatives. The application therefore downloads the model at runtime and does not bundle the model files into the installer. Runtime download does not remove the upstream license restrictions; a future commercial distribution must select a model with suitable commercial terms.

## Sample-rate handling

Sherpa-onnx `OfflineStream::accept_waveform` accepts the source sample rate supplied by CPAL. The Zipformer path therefore does not use the old Whisper-specific forced 16 kHz whole-utterance resample.

The generic resampler remains in `stt.rs` solely for the optional legacy Whisper backend.

## Feature compatibility

The canonical voice runtime feature is:

```text
voice-stt
```

The current Tauri Windows build configuration still enables the historical feature name:

```text
voice-whisper
```

That name is now a compatibility alias for `voice-stt`. It enables `voice-runtime/zipformer` only and does not pull `whisper-rs` into the normal desktop build.

Inside `voice-runtime`, the historical `WhisperConfig` / `WhisperRecognizer` symbols are temporarily re-exported to `ZipformerConfig` / `ZipformerRecognizer` whenever the Zipformer feature is active. This keeps the existing Tauri lifecycle stable while the engine is replaced. The actual legacy Whisper implementation remains available only when `voice-runtime/whisper` is explicitly enabled without Zipformer.

## Current VAD

The existing local utterance segmenter is unchanged:

- speech RMS threshold: `0.012`;
- speech start trigger: `120 ms`;
- pre-roll: `220 ms`;
- end-of-speech silence: `650 ms`;
- minimum utterance: `250 ms`;
- maximum utterance: `15 s`.

A later STT phase can replace this baseline with Silero VAD and add streaming/partial transcript events without changing Assistant Core.

## Contextual biasing status

The selected model is a transducer and sherpa-onnx supports per-stream hotwords with `modified_beam_search`, but this branch currently uses `greedy_search` and does **not** enable contextual hotwords yet. `bpe.model` is installed now so application/project-name biasing can be added as a separate measured change.

## Verification policy

No GitHub Action, native build, microphone test or model download is manually dispatched as part of this repository change. Validate accuracy, latency and native DLL loading locally on the target Windows machine after merging.
