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
      +------------------------------+
      |                              |
active snapshot                complete Utterance
      |                              |
      v                              v
throttled offline decode       final offline decode
      |                              |
voice:transcript               final Transcript
(partial, UI-only)                   |
      |                              v
      +-------> Quick UI       Assistant Core
                                  |
                               Antigravity
```

Microphone audio remains local. Only the **final** transcript enters Assistant Core/Antigravity.

## Partial transcript behavior

The current Vietnamese model bundle is an **offline** Zipformer recognizer. It is not an `OnlineRecognizer` model, so the desktop does not pretend that the recognizer itself is natively streaming.

Instead, the voice runtime provides a bounded simulated-streaming preview:

- VAD keeps the authoritative active utterance buffer;
- `UtteranceSegmenter::active_snapshot()` copies the current active audio without consuming or modifying the final buffer;
- once at least `650 ms` of active audio is available, the desktop may submit a snapshot for partial decoding;
- snapshot submissions are throttled to at most once every `850 ms`;
- a Tokio `watch` channel keeps the newest pending snapshot instead of building an unbounded decode queue;
- repeated/empty partial text is suppressed;
- partial results are emitted only as `voice:transcript` UI events with `is_final=false`;
- the full VAD utterance is decoded again at end-of-speech and emitted with `is_final=true`;
- only that final transcript is passed to `complete_prompt()`.

Quick therefore shows live text such as:

```text
Đang nhận dạng: mở visual studio code
```

and converts it to:

```text
Bạn nói: mở Visual Studio Code
```

when final recognition completes.

Partial decode failure is non-fatal. The normal final recognition path remains authoritative.

### Native decode serialization

Partial snapshots and the final utterance reuse the same `OfflineRecognizer`. `ZipformerRecognizer` therefore owns a shared decode gate so native sherpa-onnx decode calls are serialized.

This avoids concurrent calls into the same offline recognizer while still keeping microphone capture asynchronous. If a partial worker is cancelled when VAD closes the utterance, any already-running blocking decode may finish internally, but it cannot publish a stale partial event after the worker is aborted. The final decode waits for the native decode gate when necessary.

Voice-turn cancellation adds a second stale-result boundary. A cancelled Zipformer decode may finish inside native `OfflineRecognizer::decode`, but its result is discarded before it can become a final transcript or Assistant prompt.

### What this is not

This is **not true online ASR**. True token/frame streaming requires a sherpa-onnx `OnlineRecognizer` compatible model bundle and corresponding resource-manifest/installer changes.

A later phase can migrate to an online Vietnamese model if accuracy, latency, licensing, and distribution constraints are acceptable. The `voice:transcript` UI contract can remain unchanged.

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

The local utterance segmenter remains lightweight and model-free, but now uses **RMS hysteresis** rather than one threshold for both speech entry and continuation.

Default thresholds/timing:

- speech **start** RMS threshold: `0.012`;
- speech **continue** RMS threshold: `0.0075`;
- speech start trigger: `120 ms`;
- pre-roll: `220 ms`;
- end-of-speech silence: `650 ms`;
- minimum utterance: `250 ms`;
- maximum utterance: `15 s`.

Behavior:

```text
Idle
  |
RMS >= 0.012 for >= 120 ms
  |
SpeechStarted
  |
  +-- RMS >= 0.0075 ------> keep speech active / reset silence run
  |
  +-- RMS < 0.0075 ------> accumulate silence
                                |
                          >= 650 ms
                                |
                         UtteranceReady
```

The lower continuation threshold is intentionally only used **after** speech has started. Audio at e.g. RMS `0.009` cannot open a new utterance, but it can keep an existing utterance alive. This reduces premature end-of-speech decisions around quiet Vietnamese trailing syllables without making idle start detection more sensitive to background noise.

If a caller supplies a continuation threshold higher than the start threshold, the segmenter clamps the effective continuation threshold down to the start threshold so configuration cannot accidentally create reverse hysteresis.

The active-snapshot API still does not consume or mutate the authoritative final utterance buffer. Voice cancellation also remains orthogonal to segmentation: Stop ends the foreground microphone generation, while the wake microphone uses its own uncancellable capture path and Wake Suspend/Resume lifecycle.

A later measured voice-quality phase can add adaptive noise-floor logic or replace the RMS segmenter with Silero/neural VAD without changing Assistant Core or the `voice:transcript` frontend contract.

## Contextual biasing status

The selected model is a transducer and sherpa-onnx supports per-stream hotwords with `modified_beam_search`, but this branch currently uses `greedy_search` and does **not** enable contextual hotwords yet. `bpe.model` is installed now so application/project-name biasing can be added as a separate measured change.

## Local verification

After pulling this phase on the target Windows machine, verify manually:

1. start a microphone voice turn and speak for at least 2–3 seconds;
2. confirm Quick shows one or more `Đang nhận dạng:` updates before end-of-speech;
3. stop speaking and confirm the label becomes `Bạn nói:` with the final transcript;
4. confirm only one Assistant request is produced for the voice turn;
5. speak a phrase that ends quietly and confirm the final syllable is less likely to be clipped;
6. expose the microphone to low steady room noise below the start threshold and confirm it does not open a voice turn;
7. confirm true silence still closes active speech after roughly `650 ms`;
8. speak a long sentence and confirm partial updates remain bounded rather than arriving for every microphone chunk;
9. cancel Listening/final STT and confirm no cancelled transcript becomes an Assistant request;
10. repeat wake → voice turns and confirm no stale partial text leaks into a later invocation.

No GitHub Action, native build, microphone test or model download is manually dispatched as part of this remote repository change. Validate accuracy, latency and native DLL loading locally on the target Windows machine after merging.
