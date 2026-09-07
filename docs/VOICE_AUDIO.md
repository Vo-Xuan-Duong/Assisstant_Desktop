# Voice Runtime — Audio I/O

## Scope

The audio layer is the shared CPAL/WASAPI microphone abstraction used by both foreground voice turns and long-lived wake-word detection.

```text
Windows microphone
      |
   WASAPI / CPAL
      |
hardware PCM format
      |
normalize + downmix
      |
mono f32 AudioChunk
      |
      +--> voice-turn VAD / STT
      |
      +--> wake-word runtime
```

The two consumers intentionally have different cancellation semantics.

## Why CPAL

The desktop runtime needs one Rust abstraction over Windows audio input without introducing a Python process or separate audio service. On Windows, CPAL uses the native WASAPI backend.

The project pins CPAL `0.18.2`. Streams are started explicitly with `play()` after creation.

## Input configuration

`MicrophoneStream` uses the default Windows input device and its default supported stream configuration.

The runtime records:

- device description;
- sample rate;
- source channel count;
- source sample format.

It does not force 16 kHz at the hardware boundary. Recognition-specific resampling belongs to the STT layer.

## Capture modes

### Voice-turn capture

```rust
MicrophoneStream::open_default(config)
```

This is the normal command/voice-turn path. Opening the stream starts a fresh `voice-runtime` cancellation generation.

`next_chunk()` waits on both:

- the bounded CPAL audio queue;
- the voice cancellation watcher.

If Quick Stop cancels the active Listening generation, `next_chunk()` returns `None` immediately instead of waiting for another audio callback or the desktop's 25-second utterance timeout.

### Wake-word capture

```rust
MicrophoneStream::open_default_uncancellable(config)
```

Wake detection is long-lived infrastructure rather than one Assistant voice turn, so its microphone stream deliberately does **not** subscribe to the foreground voice-turn cancellation generation.

This prevents a Quick Stop from being interpreted by the wake runtime as an unexpected microphone end/error. Wake continues to be controlled by its own explicit `Suspend`, `Resume`, enable/disable, cooldown, reload, and shutdown commands.

The desktop still suspends wake while a foreground voice turn or TTS operation owns the relevant lifecycle. The uncancellable stream boundary is defense-in-depth: foreground cancellation and wake control remain separate mechanisms.

## Supported PCM formats

The default CPAL configuration may prefer formats other than `i16`, so the runtime handles:

- `f32`, `f64`;
- signed `i8`, `i16`, `i24`, `i32`, `i64`;
- unsigned `u8`, `u16`, `u24`, `u32`, `u64`.

DSD formats are rejected as unsupported for the speech pipeline.

## AudioChunk contract

Every chunk crossing from the realtime callback into the asynchronous runtime is:

```text
AudioChunk
├── samples: Vec<f32>    # mono, normalized approximately -1..=1
├── sample_rate: u32
└── level
    ├── rms
    └── peak
```

Multi-channel input is downmixed by averaging channels per frame.

`rms` and `peak` are part of the audio contract so Quick/edge visuals can react to microphone energy without coupling UI code directly to CPAL.

## Callback behavior

The CPAL callback runs on the audio backend's realtime/high-priority thread and must never wait for the desktop async runtime.

The implementation uses a bounded Tokio MPSC channel and `try_send`:

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

The callback still allocates a `Vec<f32>` per chunk. This remains a profiling/optimization point rather than a correctness issue.

## Runtime errors

Synchronous startup/control failures return `VoiceError`.

Backend errors reported after the stream starts are captured in `last_error()` so the voice/wake controller can surface device disconnection or permission/backend failures without crashing the application.

Voice-turn cancellation is not reported as a device error. A cancellable stream simply ends its asynchronous delivery for the cancelled generation.

## Lifecycle

`MicrophoneStream` supports:

- cancellable foreground open via `open_default()`;
- uncancellable infrastructure open via `open_default_uncancellable()`;
- `pause()`;
- `resume()`;
- async `next_chunk()`;
- drop-based stream shutdown.

Wake runtime uses only the uncancellable open path. Foreground desktop voice capture uses the cancellable open path.

## Verification policy

No GitHub Actions, builds, native microphone tests, or runtime tests are executed by the remote development process. Verify both capture modes on the target Windows machine, especially Stop-during-Listening and wake resume after a cancelled voice turn.
