# Runtime Resources

Phase 13A introduced one source of truth for optional local AI resources. Phase 13B adds a verified installer on top of that registry without weakening the existing path/status contract.

## Architecture

```text
RuntimePaths
    ↓
ResourceRegistry
    ├── Whisper path/status
    └── Wake model/keywords paths/status
           ↓
    ┌──────┼───────────────┐
    ↓      ↓               ↓
Voice   WakeService     Readiness / Setup UI
                           ↓
                  Resource install catalog
                           ↓
                  Verified installer
```

The frontend never supplies an arbitrary download URL. It requests installation by trusted resource ID; backend code resolves that ID through a compiled manifest.

## Resource states

```text
ready
missing
incomplete
not_compiled
```

- `ready` — the build feature is enabled and all required files exist.
- `missing` — the feature is enabled but the required resource is absent.
- `incomplete` — a multi-file resource contains only some required files.
- `not_compiled` — the current build does not enable that optional feature. Paths remain visible so resources may be prepared before rebuilding.

Readiness maps optional missing/not-compiled resources to `optional_missing`; text assistant operation remains available.

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

The wake layout comes from the same `SherpaWakeConfig::gigaspeech_int8(...)` contract used by the detector. `keywords.txt` must be generated against the selected tokenizer/model; it is not merely a plain wake phrase.

## Environment overrides

```text
ASSISTANT_WHISPER_MODEL
ASSISTANT_WAKE_MODEL_DIR
ASSISTANT_WAKE_KEYWORDS
```

Overrides must be absolute paths. Relative values are rejected during desktop setup so resource resolution cannot change with process working directory.

## Registry command

```text
assistant_resources
```

returns resource state and exact local paths only. It does not read model contents or send model data to Gemini.

## Verified installer catalog

```text
assistant_resource_catalog
```

returns backend-owned manifests containing:

```text
id
version
package_kind
installable
source_url
source_page
license
expected_bytes
sha256
note
```

The catalog is informational on the frontend. The install command accepts only a `resource_id`:

```text
assistant_resource_install(resource_id)
```

There is no command argument for a custom URL, hash, destination or expected size.

## Whisper automatic install

Phase 13B enables automatic install only for the pinned multilingual Whisper base model used by the application.

Pinned manifest:

```text
resource id: whisper
package: single_file
file: ggml-base.bin
expected bytes: 147951465
sha256: 60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe
license: MIT
```

The source revision and URL are compiled into `resource_manifest.rs`.

Install flow:

```text
resource_id
   ↓
trusted backend manifest
   ↓
HTTPS request
   ↓
unique .part file in destination directory
   ↓
stream bytes + SHA-256
   ↓
expected byte-count check
   ↓
SHA-256 check
   ↓
flush + sync_all
   ↓
re-check destination does not already exist
   ↓
atomic rename
   ↓
registry refresh
```

Important behavior:

- existing destination files are never overwritten;
- one install per resource ID may run at a time;
- partial files use a random UUID name;
- downloads exceeding the pinned size fail immediately;
- final byte size must equal the manifest size;
- SHA-256 must exactly match the manifest;
- failed downloads/finalization remove the `.part` file where possible;
- destination creation races fail closed rather than overwrite a file;
- verified installation occurs only after final rename succeeds.

## Progress event

The backend emits:

```text
resource:install_progress
```

with stages:

```text
starting
downloading
verified
installed
failed
```

The Resources panel displays progress and refreshes both Resource Registry and Readiness after a successful installation.

## Wake-word auto-install remains disabled

The wake manifest is intentionally present but has:

```text
installable = false
```

Reasons:

1. the existing GigaSpeech archive checksum/redistribution contract has not yet been pinned to the same standard as Whisper;
2. wake installation is archive-based rather than single-file;
3. `keywords.txt` is application-specific and must be generated using the model tokenizer;
4. changing to a newer wake model should be an explicit runtime migration, not an implicit download substitution.

Until those items are resolved, the Resources panel shows the expected wake files and reports `Manual install required`.

## First-run UI

Open:

```text
Readiness → Resources
```

For each resource it displays:

- registry state;
- local root and file paths;
- manifest version;
- expected size;
- license;
- upstream source page;
- install policy;
- live verified-download progress when supported.

A resource whose file already exists is never offered for automatic overwrite, even if the current build feature is disabled.

## Privacy and security boundary

The resource subsystem does not expose prompts, credentials, broker secrets, screenshots, clipboard contents or permission arguments.

Automatic download trust is anchored in backend code, not model output or frontend input. The model/Gemini cannot choose a resource URL or checksum through these commands.

## Local verification checklist

Registry:

1. Open Readiness → Resources with no models installed.
2. Confirm Whisper and wake paths use app-local-data.
3. Add only one wake file and confirm wake becomes `incomplete` when the feature is compiled.
4. Add all required wake files and confirm it becomes `ready`.
5. Launch from a different working directory and confirm paths remain stable.
6. Set a relative resource override and confirm setup rejects it.

Verified Whisper installer:

1. Ensure `ggml-base.bin` is absent.
2. Open Resources and confirm Whisper shows `Tải và xác minh`.
3. Start installation and confirm progress moves through download/verification/install stages.
4. Confirm the final path is the registry Whisper path and no `.part` file remains.
5. Confirm Registry and Readiness refresh to Ready when `voice-whisper` is compiled.
6. Repeat install with the final file present and confirm overwrite is refused/disabled.
7. Interrupt network access during download and confirm no final model file is created.
8. Confirm wake word still has no automatic install action.

Remote development still does not execute native builds, tests, runtime downloads or GitHub Actions.
