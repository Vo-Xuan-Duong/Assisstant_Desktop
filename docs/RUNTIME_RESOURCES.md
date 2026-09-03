# Runtime Resource Registry

Phase 13A introduces one desktop-side source of truth for optional local AI resources.

The registry does **not** download models. It resolves expected paths, validates the local file layout, exposes status to Tauri/React, and is reused by voice, wake-word and Readiness.

## Why this exists

Before Phase 13A, Whisper, WakeService and Readiness each had some path/status logic of their own. That made it possible for different UI surfaces to disagree about whether a resource was installed.

The new flow is:

```text
RuntimePaths
    ↓
ResourceRegistry
    ├── Whisper path/status
    └── Wake model/keywords paths/status
           ↓
    ┌──────┼───────────┐
    ↓      ↓           ↓
Voice   WakeService  Readiness
                    + Setup UI
```

Status is recomputed from the filesystem each time `assistant_resources` or Readiness is requested. Copying files into the expected location can therefore be detected by pressing **Kiểm tra lại** without changing application configuration.

## Resource states

```text
ready
missing
incomplete
not_compiled
```

- `ready` — the build feature is enabled and all required files exist.
- `missing` — the feature is enabled but no usable resource is installed.
- `incomplete` — a multi-file resource has only some required files.
- `not_compiled` — the current build does not enable that optional feature. Expected paths are still shown so resources can be prepared before rebuilding.

Readiness maps `missing`, `incomplete` and `not_compiled` to `optional_missing`; text assistant operation remains available.

## Default local-data layout

```text
<app-local-data>/
└── models/
    ├── whisper/
    │   └── ggml-base.bin
    └── wake/
        └── sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01/
            ├── encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx
            ├── decoder-epoch-12-avg-2-chunk-16-left-64.onnx
            ├── joiner-epoch-12-avg-2-chunk-16-left-64.int8.onnx
            ├── tokens.txt
            └── keywords.txt
```

The wake layout is derived from the same `SherpaWakeConfig::gigaspeech_int8(...)` contract used by the actual detector.

`keywords.txt` must be generated against the selected model/tokenizer. It is not just a plain wake phrase string.

## Environment overrides

```text
ASSISTANT_WHISPER_MODEL
ASSISTANT_WAKE_MODEL_DIR
ASSISTANT_WAKE_KEYWORDS
```

Phase 13A requires override values to be **absolute paths**. Relative overrides are rejected during desktop setup so resource resolution never changes because the app was launched from a different working directory.

## Desktop command

```text
assistant_resources
```

returns:

```text
resources[]
  id
  label
  state
  compiled
  root_path
  detail
  files[]
    name
    path
    exists
```

No model contents are read or sent to Gemini by this command. It checks only local paths and file existence.

## First-run UI

Open:

```text
Readiness → Resources
```

The panel shows resource state, expected root path, every required file, present/missing status and build-feature status.

Phase 13A deliberately has no **Download** button.

## Why automatic download is deferred

Before downloading a model automatically, the project must lock:

1. canonical source URL;
2. immutable model/version identifier;
3. expected SHA-256 checksum;
4. license/redistribution requirements;
5. archive extraction layout;
6. partial-download/retry behavior;
7. disk-space requirements;
8. update policy.

Those concerns belong to Phase 13B rather than being hidden inside the registry.

## Phase 13B integration point

A future installer can target a resource by registry ID:

```text
whisper
wake_word
```

and install to the exact paths already displayed by Phase 13A. After installation, the existing registry refresh path can report the resource as Ready without changing the voice/wake APIs.

## Privacy

The registry exposes only paths, feature flags, file-exists booleans and descriptive status. It does not read model bytes, prompts, clipboard data, screenshots, credentials, broker secrets or permission arguments.

## Local verification checklist

1. Open Readiness → Resources with no models installed.
2. Confirm Whisper and wake resources show the expected app-local-data paths.
3. Add only one wake model file and confirm Wake becomes `incomplete`.
4. Add all required wake files and confirm resource status becomes `ready` when the wake feature is compiled.
5. Place `ggml-base.bin` at the Whisper path and confirm Whisper becomes `ready` when the feature is compiled.
6. Verify VoiceCapabilities and Readiness agree with the resource panel.
7. Launch the app from a different working directory and confirm paths do not change.
8. Set a relative resource override and confirm setup rejects it.

Remote development still does not execute builds, runtime tests or GitHub Actions.
