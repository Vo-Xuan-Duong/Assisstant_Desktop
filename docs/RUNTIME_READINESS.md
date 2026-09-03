# Runtime Readiness Diagnostics

Phase 12A introduced an aggregate readiness report. Phase 12B keeps the same UI contract but moves path resolution to a packaging-safe runtime layout.

The Runtime card contains a **Readiness** button. Opening it runs checks on demand and classifies each subsystem as:

```text
ready
optional_missing
blocking
```

The panel is not polled continuously.

## Current checks

### 1. Antigravity CLI

Uses the existing `AntigravityClient::health()` / `CliHealth` implementation.

Antigravity is launched with its working directory set to the generated runtime directory under Tauri app-local-data. This makes MCP discovery independent of the process working directory used to start the desktop application.

### 2. Windows MCP

Phase 12B no longer relies on a tracked `.agents/mcp_config.json` in the repository.

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

Context artifacts now use:

```text
<app-local-data>/context
```

instead of a working-directory-relative `.assistant/context` path.

Readiness verifies the directory is writable with a PID-specific `create_new` probe file that is removed immediately.

### 5. Windows TTS

The compiled Windows SAPI backend is reported as ready. Device/runtime behavior remains part of local Windows verification.

### 6. Local Whisper STT

- feature absent → `optional_missing`
- feature present but model missing → `optional_missing`
- feature + model available → `ready`

The default model location is also under app-local-data unless `ASSISTANT_WHISPER_MODEL` overrides it.

### 7. Wake Word

Wake readiness reuses `WakeService::status()`.

- feature absent → Optional
- resources unavailable → Optional
- resources available but disabled by user → Ready
- resources available and worker active → Ready

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

Phase 12B supports:

```text
ASSISTANT_RUNTIME_DIR
ASSISTANT_MCP_BINARY
ASSISTANT_WHISPER_MODEL
ASSISTANT_WAKE_MODEL_DIR
ASSISTANT_WAKE_KEYWORDS
```

`ASSISTANT_RUNTIME_DIR` changes the generated Antigravity/MCP runtime directory. `ASSISTANT_MCP_BINARY` is intended for local diagnostics/development and overrides sidecar discovery.

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

When testing on Windows locally:

1. Run without `agy` and confirm Antigravity = Blocking.
2. Install/login Antigravity and confirm it becomes Ready.
3. Run Tauri dev and confirm the sidecar staging command creates the target-triple binary under `src-tauri/binaries/`.
4. Confirm startup creates `<app-local-data>/runtime/.agents/mcp_config.json`.
5. Confirm the generated MCP command is absolute and points to the bundled/dev-resolved `assistant-mcp.exe`.
6. Start the app from a different working directory and confirm MCP/context paths remain unchanged.
7. Confirm Context Storage points to `<app-local-data>/context`.
8. Build without `voice-whisper` and confirm Whisper = Optional.
9. Build with Whisper but without a model and confirm the expected app-local-data model path is shown.
10. Corrupt the runtime Moderate policy file and confirm Permission Broker = Blocking.
11. Confirm readiness never displays broker secret, prompt/clipboard/screenshot contents, permission arguments or credentials.

No GitHub Actions or runtime tests are executed during the remote development phase.
