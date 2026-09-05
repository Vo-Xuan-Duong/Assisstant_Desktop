# Voice Runtime — Vietnamese STT

## Current architecture

The desktop assistant now uses a Vietnamese-specific sherpa-onnx Zipformer model as its primary local speech recognizer. Whisper remains only as an optional compatibility fallback.

```text
MicrophoneStream (CPAL)
      |
  AudioChunk
      |
UtteranceSegmenter
      |
  Utterance
      |
SpeechRecognizer
      |
Vietnamese Zipformer INT8       <-- primary
      |
Whisper local model             <-- fallback only
      |
 Transcript
      |
Assistant Core
```

The Assistant still keeps microphone audio local. Only the final transcript enters Assistant Core/Antigravity.

## Primary model

Pinned model family:

```text
sherpa-onnx-zipformer-vi-30M-int8-2026-02-09
```

Required runtime files:

```text
encoder.int8.onnx
decoder.onnx
joiner.int8.onnx
tokens.txt
```

Default model directory:

```text
%LOCALAPPDATA%/<Assisstant Desktop app-data>/models/stt/
  sherpa-onnx-zipformer-vi-30M-int8-2026-02-09/
```

The directory can be overridden with an absolute path:

```text
ASSISTANT_ZIPFORMER_MODEL_DIR
```

The official sherpa-onnx model documentation is:

```text
https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/zipformer-transducer-models.html
```

The upstream package contains the exact INT8 encoder/joiner, decoder and token vocabulary expected by the runtime.

## Runtime behavior

`WhisperRecognizer` is retained as the compatibility type name used by the current Tauri voice state, but its behavior changed:

1. resolve the Vietnamese Zipformer model directory;
2. when all four Zipformer files exist, create a sherpa-onnx `OfflineRecognizer`;
3. recognize the captured utterance with Zipformer using the microphone source sample rate;
4. if Zipformer fails for a turn and a legacy Whisper model exists, try Whisper once as fallback;
5. if Zipformer is not installed, an existing Whisper model can still be used during migration.

Successful primary transcripts report the engine as:

```text
sherpa-onnx/zipformer-vi-30m-int8
```

Fallback transcripts report:

```text
whisper.cpp/fallback
```

## Sample-rate handling

Sherpa-onnx accepts the source sample rate supplied by the microphone stream, so the primary recognizer does not perform the old forced 16 kHz whole-utterance resample.

The existing 16 kHz linear resampler is retained only for Whisper fallback compatibility.

## VAD

The current local utterance segmenter remains unchanged in this migration. It detects speech boundaries before ASR and therefore isolates the recognizer replacement from microphone capture and Assistant Core.

Default behavior remains:

- speech RMS threshold: `0.012`;
- speech start trigger: `120 ms`;
- pre-roll: `220 ms`;
- end-of-speech silence: `650 ms`;
- minimum utterance: `250 ms`;
- maximum utterance: `15 s`.

A later STT phase can replace this baseline with Silero VAD and partial/streaming transcript events without changing the assistant reasoning pipeline.

## Feature compatibility

The desktop Cargo feature remains named `voice-whisper` for compatibility with existing release scripts. Internally, `voice-runtime/whisper` now enables both:

- `sherpa-onnx` for the primary Vietnamese Zipformer recognizer;
- `whisper-rs` for optional fallback.

Renaming the public feature can be done later as a cleanup without coupling it to the functional STT migration.

## Resource installation

The previous verified single-file Whisper downloader must not write a Whisper binary into the new Zipformer encoder path. Therefore automatic installation for the STT manifest is disabled until the installer supports a verified multi-file transaction.

For this migration, install the official upstream Zipformer package manually into the model directory shown by the Resources panel. The runtime requires all four files listed above before it reports the primary STT resource as `Ready`.

The legacy fallback path remains:

```text
models/whisper/ggml-base.bin
```

and can still be overridden by:

```text
ASSISTANT_WHISPER_MODEL
```

Whisper is not required for the new primary STT resource to become `Ready`.

## Verification policy

No GitHub Action, build or native runtime test is manually dispatched as part of this repository change. Accuracy and microphone behavior should be validated locally on the target Windows machine with the Vietnamese Zipformer files installed.
