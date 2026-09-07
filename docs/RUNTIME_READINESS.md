# Runtime Readiness Diagnostics

Runtime readiness aggregates the health of the AI backend, MCP/tool path, permission broker, context storage, voice resources, and wake runtime.

Checks are classified as:

```text
ready
optional_missing
blocking
```

The report is collected on demand; it is not continuously polled.

## Current checks

### 1. Antigravity CLI

Uses the existing `AntigravityClient::health()` / `CliHealth` implementation.

Antigravity is launched with its working directory set to the generated runtime directory under Tauri app-local-data. This makes MCP discovery independent of the process working directory used to start the desktop application.

### 2. Windows MCP

At desktop startup `RuntimePaths::prepare()` creates:

```text
<app-local-data>/runtime/.agents/mcp_config.json
```

The generated config contains one `assistant-windows` server and an absolute command path to the resolved `assistant-mcp.exe`.

MCP binary resolution order:

```text
ASSISTANT_MCP_BINARY
        ↓
Tauri bundled sidecar in resource directory
        ↓
dev target/debug fallback
        ↓
dev target/release fallback
        ↓
expected bundled path (reported missing)
```

Readiness verifies the generated config exists/parses and the resolved sidecar file exists.

### 3. Permission Broker

Uses the existing desktop permission service. Diagnostics expose only:

- broker bound status;
- policy path;
- audit path;
- policy-load error;
- pending confirmation count.

The broker address and random secret remain private.

### 4. Context Storage

Context artifacts use:

```text
<app-local-data>/context
```

Readiness verifies the directory is writable with a PID-specific `create_new` probe file that is removed immediately.

### 5. Windows TTS

The compiled Windows SAPI backend is reported as ready. Device/runtime behavior remains part of local Windows verification.

### 6. Vietnamese Zipformer STT

The primary local recognizer is sherpa-onnx Vietnamese Zipformer 30M INT8.

Default model directory:

```text
<app-local-data>/models/stt/sherpa-onnx-zipformer-vi-30M-int8-2026-02-09
```

Required runtime files:

```text
encoder.int8.onnx
decoder.onnx
joiner.int8.onnx
tokens.txt
```

Classification:

- voice STT feature not compiled → `optional_missing`;
- no runtime model files → `optional_missing`;
- only part of the bundle exists → `optional_missing` with an incomplete-bundle detail;
- all four runtime files exist → `ready`.

`bpe.model` is preparation/context data and is not required by the current offline recognition path.

The model directory can be overridden with an absolute path:

```text
ASSISTANT_ZIPFORMER_MODEL_DIR
```

The legacy desktop feature name `voice-whisper` is retained only as a compatibility alias; it enables Zipformer and does not make Whisper the primary recognizer.

### 7. Wake Word

Wake readiness reuses `WakeService::status()`.

- feature absent → Optional;
- resources unavailable → Optional;
- resources available but disabled by user → Ready;
- resources available and worker active → Ready.

## Overall readiness

```text
if any check == blocking
    overall = blocking
else if any check == optional_missing
    overall = optional_missing
else
    overall = ready
```

## Runtime path overrides

Relevant path overrides:

```text
ASSISTANT_RUNTIME_DIR
ASSISTANT_MCP_BINARY
ASSISTANT_ZIPFORMER_MODEL_DIR
ASSISTANT_WAKE_MODEL_DIR
ASSISTANT_WAKE_KEYWORDS
```

`ASSISTANT_RUNTIME_DIR` changes only the generated Antigravity/MCP runtime directory. `ASSISTANT_MCP_BINARY` and model overrides are intended for diagnostics/development and must resolve independently of the process working directory.

## Privacy

Readiness does not read or display:

- prompts or chat history;
- clipboard content;
- screenshots;
- permission arguments;
- audit entry contents;
- broker secret/address;
- Antigravity credentials.

Only status metadata and diagnostic paths are shown.

## Local verification checklist

1. Run without `agy` and confirm Antigravity = Blocking.
2. Install/login Antigravity and confirm it becomes Ready.
3. Run Tauri dev and confirm the sidecar staging command creates the target-triple binary under `src-tauri/binaries/`.
4. Confirm startup creates `<app-local-data>/runtime/.agents/mcp_config.json`.
5. Confirm the generated MCP command is absolute and points to the bundled/dev-resolved `assistant-mcp.exe`.
6. Start the app from a different working directory and confirm MCP/context paths remain unchanged.
7. Confirm Context Storage points to `<app-local-data>/context`.
8. Build without voice STT and confirm Zipformer STT = Optional.
9. Build with voice STT but without the model bundle and confirm the expected app-local-data STT directory is shown.
10. Place only part of the Zipformer bundle in the model directory and confirm readiness reports it as incomplete/optional.
11. Install `stt_zipformer_vi` and confirm all four runtime files resolve and readiness becomes Ready.
12. Set `ASSISTANT_ZIPFORMER_MODEL_DIR` to an absolute test directory and confirm runtime and `verify-local.ps1` report the same location.
13. Corrupt the runtime Moderate policy file and confirm Permission Broker = Blocking.
14. Confirm readiness never displays broker secret, prompt/clipboard/screenshot contents, permission arguments or credentials.

No GitHub Actions, builds, runtime tests, model downloads, or native Windows verification are executed during the remote development phase.
