# Wake Word Runtime

Phase 7A establishes the wake-word engine boundary. It does **not** start a permanent background microphone yet; background lifecycle integration is a separate phase.

## Engine

The selected first implementation is `sherpa-onnx` keyword spotting through its native Rust API.

The dependency is optional:

```toml
voice-runtime = { ..., features = ["wake-sherpa"] }
```

Default desktop/text builds therefore do not compile or link sherpa-onnx.

The selected baseline model family for an English wake phrase is:

```text
sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01
```

The int8 encoder/joiner variants are preferred for the always-on CPU path.

## Runtime boundary

```text
CPAL / WASAPI
      |
      | AudioChunk (mono f32)
      v
WakeWordDetector
      |
      +-- SherpaWakeWordDetector (optional feature)
      |
      v
WakeDetection
```

The detector is **not** invoked inside CPAL's realtime callback. CPAL already places audio chunks on a bounded async channel; a later background task consumes those chunks and calls the detector.

## Contract

```rust
pub trait WakeWordDetector: Send {
    fn process(&mut self, chunk: &AudioChunk)
        -> Result<Option<WakeDetection>, WakeError>;

    fn reset(&mut self) -> Result<(), WakeError>;
}
```

This keeps future wake engines replaceable without changing microphone or desktop activation code.

## Sherpa resources

`SherpaWakeConfig::gigaspeech_int8(...)` expects this model layout:

```text
model/
├── encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx
├── decoder-epoch-12-avg-2-chunk-16-left-64.onnx
├── joiner-epoch-12-avg-2-chunk-16-left-64.int8.onnx
├── tokens.txt
├── bpe.model
└── keywords.txt
```

`bpe.model` is needed to generate custom keyword tokens; the runtime itself reads `keywords.txt`.

## Do not hand-write BPE tokens

Keyword files are model-specific. For an English phrase such as:

```text
HEY ASSISTANT
```

generate the runtime keyword file with the model tokenizer instead of guessing tokens:

```powershell
sherpa-onnx-cli text2token `
  --tokens .\model\tokens.txt `
  --tokens-type bpe `
  --bpe-model .\model\bpe.model `
  .\keywords_raw.txt `
  .\keywords.txt
```

Where `keywords_raw.txt` can contain:

```text
HEY ASSISTANT
```

The generated `keywords.txt` is then supplied to `SherpaWakeConfig`.

This is intentionally an installation/model-preparation step. Automatic model acquisition is not part of Phase 7A.

## Default detector tuning

The current baseline follows the small CPU-oriented settings:

```text
provider            cpu
num_threads         1
max_active_paths    4
keywords_score      1.0
keywords_threshold  0.25
```

These are configuration values, not permanent product constants. False-positive/false-negative tuning must be done on the user's microphone/environment during local testing.

## Internal sample-rate handling

`AudioChunk` retains the actual microphone sample rate. The sherpa online stream accepts that source sample rate and performs internal resampling when the feature extractor expects another rate, so Phase 7A does not add a second wake-word resampler.

## Detection lifecycle

For each audio chunk:

```text
accept_waveform(sample_rate, samples)
        |
        v
while spotter.is_ready(stream)
        |
        v
decode(stream)
        |
        v
get_result(stream)
        |
    keyword empty? ----- yes ---> continue
        |
        no
        v
WakeDetection
        |
        v
spotter.reset(stream)
```

Resetting immediately after a detected keyword is required before continuing to decode new audio on the same stream.

## Build behavior

The sherpa Rust crate uses prebuilt native libraries for supported platforms. Because this is a heavier native dependency than the base audio runtime, it remains behind `wake-sherpa`.

Later the desktop app will expose its own feature such as:

```text
wake-word -> voice-runtime/wake-sherpa
```

so production builds can explicitly choose whether always-on keyword spotting is included.

## Phase 7B

The next phase will add:

1. a background `WakeRuntime` task;
2. microphone ownership/lifecycle management;
3. enable/disable status;
4. wake detection event publication;
5. desktop activation without keyboard interaction;
6. cooldown/debounce after a successful wake;
7. pause wake listening while the Assistant is recording a full voice turn or speaking.

That separation prevents Phase 7A's inference engine from owning application lifecycle policy.

## Local verification for 7A

When compiling with `wake-sherpa` locally:

1. prepare the GigaSpeech KWS model files;
2. generate `keywords.txt` using the matching `bpe.model`;
3. construct `SherpaWakeConfig` with those paths;
4. feed recorded/streaming `AudioChunk` values into `SherpaWakeWordDetector`;
5. confirm the configured phrase returns `WakeDetection`;
6. confirm ordinary speech does not trigger under the selected threshold;
7. tune score/threshold before enabling background mode.

No GitHub Action is required for this verification.
