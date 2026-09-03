# Voice Runtime — Phase 5A Audio I/O

## Scope

Phase 5A establishes the local microphone runtime only. It deliberately does **not** add speech-to-text, VAD, TTS, wake-word detection, or continuous assistant behavior yet.

```text
Windows microphone
      |
   WASAPI
      |
    CPAL
      |
hardware PCM format
      |
normalize/downmix
      |
mono f32 AudioChunk
      |
future VAD / STT
```

## Why CPAL

The desktop runtime needs one small Rust abstraction over Windows audio input without introducing Python or a separate service. On Windows, CPAL uses the native WASAPI backend.

The project pins CPAL `0.18.2`. CPAL 0.18 creates streams paused, so the runtime explicitly calls `play()` after stream creation.

## Input configuration

`MicrophoneStream::open_default` uses the default Windows input device and its default supported stream configuration.

The runtime records:

- device description;
- sample rate;
- source channel count;
- source sample format.

It does not force 16 kHz at the hardware boundary. Resampling belongs to the STT stage because the best target rate depends on the selected recognition engine.

## Supported PCM formats

The default CPAL configuration may now prefer formats other than `i16`, so the runtime handles:

- `f32`, `f64`;
- signed `i8`, `i16`, `i24`, `i32`, `i64`;
- unsigned `u8`, `u16`, `u24`, `u32`, `u64`.

DSD formats are rejected as unsupported for the speech pipeline.

## AudioChunk contract

Every chunk crossing from the audio callback into the asynchronous runtime is:

```text
AudioChunk
├── samples: Vec<f32>    # mono, normalized approximately -1..=1
├── sample_rate: u32
└── level
    ├── rms
    └── peak
```

Multi-channel hardware input is downmixed by averaging channels per frame.

`rms` and `peak` are intentionally part of the audio contract because the later Gemini-like border UI can react to microphone energy without coupling UI code to CPAL.

## Callback behavior

The CPAL callback runs on the audio backend's realtime/high-priority thread. It must not wait for the desktop async runtime.

The implementation therefore uses a bounded Tokio MPSC channel and `try_send`:

```text
CPAL callback
    |
normalize chunk
    |
try_send
  /     \
success  queue full
  |         |
consumer   drop chunk
            |
       increment counter
```

`MicrophoneStream::dropped_chunks()` exposes the total number of chunks dropped due to backpressure.

The first implementation still allocates a `Vec<f32>` per callback. That is acceptable for the Phase 5A integration baseline but remains a future optimization point if profiling shows callback pressure.

## Runtime errors

Synchronous startup/control failures return `VoiceError`.

Backend errors reported after the stream starts are captured in `last_error()` so the UI/voice controller can surface device disconnection or permission/backend failures without crashing the application.

## Lifecycle

`MicrophoneStream` supports:

- open + immediate start;
- `pause()`;
- `resume()`;
- async `next_chunk()`;
- drop-based stream shutdown.

The desktop application is **not connected to this stream in Phase 5A**. Phase 5B adds VAD/STT first, then Phase 5C connects the complete voice turn lifecycle to Tauri.

## Build policy

Whisper is intentionally not a default dependency of this phase. Native Whisper bindings bring a C/C++/CMake/Clang toolchain into the build, so they will be feature-gated in Phase 5B rather than making every Tauri build compile the recognition backend.

## Verification policy

No GitHub Actions or runtime tests are executed as part of repository development. The microphone path is statically reviewed against CPAL 0.18.2 APIs and is intended to be verified locally on the Windows development machine.
