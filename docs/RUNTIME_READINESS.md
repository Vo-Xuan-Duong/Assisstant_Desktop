# Runtime Readiness Diagnostics

Phase 12A shifts development from adding capabilities to **integration hardening**.

The desktop now exposes one aggregate readiness report that answers a practical local-run question:

> Which parts of the assistant are ready, optional-but-missing, or blocking the full system?

## UI

The Runtime card contains a **Readiness** button.

Opening it runs the checks on demand and displays:

```text
Ready
Optional
Blocking
```

The panel is not polled continuously and does not add startup work.

## Readiness levels

### `ready`

The subsystem/resource required by that check is currently available.

### `optional_missing`

The core text assistant can still operate, but an optional capability is unavailable.

Current optional checks:

- local Whisper STT;
- wake-word runtime/resources.

### `blocking`

The full intended assistant runtime cannot use a required integration safely/reliably.

Current examples:

- Antigravity CLI missing/unhealthy;
- Windows MCP config missing/malformed;
- configured `assistant-mcp.exe` missing;
- Permission policy malformed;
- context artifact directory not writable.

## Checks

### 1. Antigravity CLI

Uses the existing `AntigravityClient::health()` / `CliHealth` logic.

No second CLI-discovery implementation is introduced.

### 2. Windows MCP

Config path resolution:

```text
ASSISTANT_MCP_CONFIG
        |
        +-- set --> that path
        |
        +-- unset --> .agents/mcp_config.json
```

The repository now contains an active non-secret `.agents/mcp_config.json`:

```json
{
  "mcpServers": {
    "assistant-windows": {
      "command": "target\\release\\assistant-mcp.exe",
      "cwd": ".",
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

Readiness verifies:

1. config file can be read;
2. JSON is valid;
3. `assistant-windows` exists;
4. configured executable exists.

Because the current config points to:

```text
target\release\assistant-mcp.exe
```

local full computer-use readiness requires building the release MCP binary.

This is a **diagnostic check only**. Phase 12A does not run builds/tests/Actions automatically.

## 3. Permission Broker

Uses the existing desktop service created during Tauri startup.

A service status exposes only:

- broker bound = yes/no;
- policy file path;
- audit file path;
- policy-load error;
- count of pending confirmation requests.

The loopback address and random session secret are deliberately **not** exposed in the readiness report.

A malformed runtime policy is Blocking because Moderate tools must remain fail-closed rather than silently ignoring a broken policy.

## 4. Context Storage

The check uses `ContextEngine::artifact_dir()` and verifies the directory can be created/written.

The write probe:

- uses a PID-specific filename;
- uses `create_new`;
- never overwrites an existing file;
- is removed immediately after the probe.

Current Context Engine default remains:

```text
.assistant/context
```

which is relative to the process working directory.

The report explicitly surfaces that path. Migrating context artifacts to Tauri app-local-data is intentionally deferred to a dedicated path/packaging hardening phase rather than silently changing persistence semantics inside diagnostics.

## 5. Windows TTS

Phase 12A reports the compiled Windows SAPI backend as ready.

Actual audio-device/runtime behavior is still part of local Windows verification.

## 6. Local Whisper STT

When `voice-whisper` is not compiled:

```text
optional_missing
```

When compiled but the model path is missing:

```text
optional_missing
```

When compiled and the model file exists:

```text
ready
```

The model path is included in the report when known.

## 7. Wake Word

Wake readiness reuses `WakeService::status()`.

- feature absent → Optional
- model/resources unavailable → Optional
- resources available but wake disabled by user → Ready
- resources available and worker active → Ready

Being intentionally disabled is not treated as a missing dependency.

## Overall readiness

```text
if any check == Blocking
    overall = Blocking
else if any check == OptionalMissing
    overall = OptionalMissing
else
    overall = Ready
```

This means `OptionalMissing` represents a usable core assistant with optional local voice/wake pieces absent.

## Privacy

Readiness does **not** read or display:

- prompts;
- chat history;
- clipboard content;
- screenshots;
- permission arguments;
- audit entry content;
- broker secret;
- Antigravity credentials.

Only status metadata and diagnostic paths are shown.

## Local verification checklist

When the project is tested on Windows locally:

1. Run with `agy` absent and confirm Antigravity = Blocking.
2. Install/login Antigravity and confirm it becomes Ready.
3. Rename/remove `.agents/mcp_config.json` and confirm Windows MCP = Blocking.
4. Restore config but leave `assistant-mcp.exe` unbuilt and confirm the binary path is reported.
5. Build the MCP release executable manually and confirm Windows MCP becomes Ready.
6. Build without `voice-whisper` and confirm Whisper = Optional.
7. Build with Whisper but without a model and confirm model path is reported.
8. Add the model and confirm Whisper = Ready.
9. Build without wake support and confirm Wake = Optional.
10. Make the context directory unwritable and confirm Context Storage = Blocking.
11. Corrupt the runtime Moderate policy file and confirm Permission Broker = Blocking.
12. Confirm the readiness panel never displays broker secret, clipboard data, prompts, screenshots, or permission arguments.

No GitHub Actions or runtime tests are executed during the remote development phase.
