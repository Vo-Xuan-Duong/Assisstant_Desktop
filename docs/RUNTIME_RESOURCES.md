# Runtime Resources

Phase 13A introduced one source of truth for optional local AI resources. Phase 13B adds a verified installer for pinned resources. Phase 13C adds local wake keyword preparation without relaxing the wake archive trust boundary.

## Architecture

```text
RuntimePaths
    ↓
ResourceRegistry
    ├── Whisper path/status
    └── Wake runtime + preparation paths/status
           ↓
    ┌──────┼────────────────────┐
    ↓      ↓                    ↓
Voice   WakeService      Readiness / Setup UI
                              ↓
                     Resource action catalog
                        ↙             ↘
               Verified download   Local generation
```

The frontend never supplies an arbitrary download URL, hash or destination. It requests a backend-defined resource/action ID.

## Resource states

```text
ready
missing
incomplete
not_compiled
```

- `ready` — the build feature is enabled and all runtime-required files exist.
- `missing` — the feature is enabled but the required runtime resource is absent.
- `incomplete` — a multi-file resource contains only some runtime-required files.
- `not_compiled` — the current build does not enable that optional feature. Paths remain visible so resources may be prepared before rebuilding.

Preparation-only files do not change runtime readiness.

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
            ├── keywords.txt
            └── bpe.model              # preparation-only
```

Runtime wake readiness requires encoder/decoder/joiner/tokens/keywords. `bpe.model` is needed only to create or later replace a keyword file.

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

returns runtime resource state, exact paths and preparation-file state. It does not read model contents or send model data to Gemini.

## Backend resource catalog

```text
assistant_resource_catalog
```

returns backend-owned manifests/actions containing:

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

The install/action command is still backend-ID based:

```text
assistant_resource_install(resource_id, phrase?)
```

`phrase` is consumed only by the local `wake_keywords` action. There is no argument for a custom URL, checksum, destination or expected size.

## Whisper verified install

Phase 13B enables automatic install only for the pinned multilingual Whisper base model.

```text
resource id: whisper
package: single_file
file: ggml-base.bin
expected bytes: 147951465
sha256: 60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe
license: MIT
```

Install flow:

```text
resource_id
   ↓
trusted backend manifest
   ↓
HTTPS request
   ↓
unique .part file
   ↓
stream bytes + SHA-256
   ↓
byte-count + SHA-256 verification
   ↓
flush + sync_all
   ↓
no-overwrite recheck
   ↓
atomic rename
```

Safety behavior:

- existing destination files are never overwritten;
- one install per resource ID may run at a time;
- partial files use random UUID names;
- oversized/short downloads fail;
- SHA-256 mismatch fails;
- failed downloads/finalization remove `.part` where possible;
- verified installation occurs only after final rename succeeds.

## Wake model archive remains manual

The wake archive manifest remains:

```text
installable = false
```

Automatic archive download/extraction stays disabled because the model-specific redistribution/license and pinned archive digest are not yet held to the same verified standard as Whisper.

A future archive installer must not infer permission to redistribute a model merely from the sherpa-onnx code license.

## Phase 13C — local wake keyword preparation

Phase 13C adds the virtual resource action:

```text
wake_keywords
```

It performs no network request.

Required preparation inputs:

```text
bpe.model
tokens.txt
```

Generation flow:

```text
user phrase
   ↓
normalize / validate
   ↓
SentencePiece using local bpe.model
   ↓
reject <unk>
   ↓
validate every piece against local tokens.txt
   ↓
construct `<pieces> @CANONICAL_LABEL`
   ↓
unique keywords.txt.part
   ↓
write + flush + sync_all
   ↓
no-overwrite recheck
   ↓
rename to keywords.txt
```

Current phrase constraints are intentionally aligned with the English GigaSpeech wake model: ASCII letters, spaces and apostrophes, at most 64 characters.

`bpe.model` is shown in the Resource UI as a preparation file. Its absence does not make an already configured wake runtime unavailable if `keywords.txt` already exists.

Phase 13C refuses to overwrite an existing `keywords.txt`. Replacing a configured phrase and hot-reloading WakeService are deferred to the next bounded phase.

## Progress event

Verified network installs emit:

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

The local `wake_keywords` action returns synchronously through the same resource action result and then refreshes Registry/Readiness.

## First-run UI

Open:

```text
Readiness → Resources
```

For Whisper it displays the pinned source/version/license/size and verified-install progress.

For Wake Word it displays:

- runtime-required files;
- preparation-only `bpe.model`;
- archive policy (`Manual install required`);
- a local wake phrase field;
- `Tạo keywords.txt` when the tokenizer resources are available and the destination does not already exist.

After keyword creation the UI shows that an application restart is currently required for WakeService to initialize with the new file.

## Privacy and security boundary

The resource subsystem does not expose prompts, credentials, broker secrets, screenshots, clipboard contents or permission arguments.

Wake phrase preparation is local-only and is not an MCP/Gemini/Antigravity tool. The phrase is tokenized on the device against local model files.

## Local verification checklist

Registry:

1. Open Readiness → Resources with no models installed.
2. Confirm Whisper/wake paths use app-local-data.
3. Confirm partial wake runtime files produce `incomplete` when compiled.
4. Confirm `bpe.model` alone does not change runtime readiness.
5. Launch from another working directory and confirm paths remain stable.

Verified Whisper installer:

1. Ensure `ggml-base.bin` is absent.
2. Start `Tải và xác minh` and observe download/verification/install stages.
3. Confirm final path is correct and no `.part` remains.
4. Confirm Registry/Readiness refresh to Ready when `voice-whisper` is compiled.
5. Confirm overwrite is refused when the final file already exists.

Wake keyword preparation:

1. Install/copy the wake model manually, including `tokens.txt` and `bpe.model`, but omit `keywords.txt`.
2. Enter `HEY ASSISTANT` in Resources and create the keyword file.
3. Confirm the generated line contains BPE pieces followed by `@HEY_ASSISTANT`.
4. Confirm unsupported characters or unknown tokenizer pieces are rejected.
5. Confirm a second create attempt refuses to overwrite the file.
6. Restart the desktop and verify WakeService recognizes the generated resource.

Remote development still does not execute native builds, tests, runtime downloads or GitHub Actions.
