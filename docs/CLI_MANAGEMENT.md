# CLI / terminal management migration

## Goal

Assisstant Desktop is moving from a conventional full desktop application toward a background assistant with two user-facing surfaces:

1. Gemini-style graphical interaction surface: perimeter glow, compact input/voice response overlay, and sensitive permission confirmation.
2. `assistant.exe`: terminal management surface for configuration, diagnostics, resources, and policy administration.

The existing full React management UI remains temporarily available while capabilities are migrated. It should not be treated as the long-term primary interface.

## Phase 1: terminal manager

The terminal manager is a second binary target of the existing `assisstant-desktop` Rust package:

```text
apps/desktop/src-tauri/src/bin/assistant.rs
```

Build output:

```text
assistant.exe
```

Keeping it in the existing package adds no new crate and preserves the locked dependency graph.

Running it without a subcommand opens an interactive terminal dashboard:

```powershell
assistant
```

The dashboard exposes four pages:

- Dashboard
- Resources
- AI / Antigravity
- Permissions

Controls:

```text
1-4    switch page
r      refresh
q      quit
```

The current terminal surface deliberately uses the standard console rather than adding a TUI dependency before local `--locked` build validation. The command and runtime contracts are independent from the renderer, so Ratatui can replace this renderer later without changing the management API.

## Command mode

### Status

```powershell
assistant status
assistant status --json
assistant doctor
assistant paths
```

### Antigravity / Gemini

```powershell
assistant ai show
assistant ai models
assistant ai set --model <model-id>
assistant ai set --effort <value>
assistant ai set --model <model-id> --effort <value>
assistant ai reset
```

The CLI writes the same `settings/antigravity.json` contract consumed by the desktop runtime.

### Wake preferences

```powershell
assistant wake show
assistant wake enable
assistant wake disable
assistant wake phrase "hey assistant"
```

`enable` and `disable` write the same `settings/wake.json` contract used by the wake service. `wake phrase` currently persists the phrase preference only; changing the actual detector phrase still requires regeneration/validation of `keywords.txt` and remains part of the resource migration.

### Native resources

```powershell
assistant resources list
```

The CLI inspects the same Vietnamese Zipformer and wake-word model layouts as the Tauri resource registry. Resource download/install remains in the desktop resource installer until the installer core is separated from its Tauri progress emitter.

### Permission policy

```powershell
assistant permissions list
assistant permissions set apps_open ask
assistant permissions set clipboard_read_text deny
assistant permissions clear apps_open
```

The CLI uses the native `TOOL_CATALOG` and `permission-engine` types. It refuses to override Safe, Sensitive, or Blocked tools; only Moderate-tool overrides can be persisted. The MCP permission gateway already reads the policy file during authorization, so Moderate override changes affect subsequent tool requests without restarting the desktop process.

## Shared data directory

On Windows the CLI resolves the same Tauri application-local data root:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop
```

The root can be overridden for development with an absolute path:

```powershell
assistant --data-dir D:\assistant-data status
```

or:

```powershell
$env:ASSISTANT_APP_DATA="D:\assistant-data"
assistant status
```

Existing runtime overrides are honored for:

```text
ASSISTANT_ZIPFORMER_MODEL_DIR
ASSISTANT_WAKE_MODEL_DIR
ASSISTANT_PERMISSION_POLICY_PATH
ASSISTANT_ANTIGRAVITY_BIN
```

## Atomic settings writes

CLI configuration writes use a temporary file + backup + rename sequence. This mirrors the desktop stores and avoids replacing a valid settings file with a partially written JSON document.

## Phase 2A: live background management

The background host now creates an authenticated local management endpoint at:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\runtime\management.json
```

The endpoint file contains:

```text
protocol version
127.0.0.1 port
per-runtime 256-bit secret
process id
```

The transport is raw JSON over a TCP listener bound only to `127.0.0.1`. It is not an HTTP service and is not reachable through a LAN interface. Every request must include the current per-runtime secret. Requests are bounded to 64 KiB, have short read/write timeouts, and are handled serially to avoid concurrent management mutations.

This follows the project's existing loopback + secret security pattern used by the permission broker while avoiding a new dependency or `Cargo.lock` change. A Windows Named Pipe can still replace the transport later without changing the versioned command contract.

### Internal protocol v1

Implemented runtime commands:

```text
runtime.ping
runtime.status
runtime.restart_agent
overlay.show
overlay.hide
ai.get
ai.set
wake.get
wake.set_enabled
resources.list
```

The endpoint is primarily a runtime contract in Phase 2A. The current CLI parser will surface the remaining direct IPC commands in the next CLI pass.

## CLI hot reload

The background host also watches the two durable CLI-managed setting files every 500 ms:

```text
settings/antigravity.json
settings/wake.json
```

Therefore these existing CLI commands now have a live path when the new background runtime is running:

```powershell
assistant ai set ...
assistant ai reset
assistant wake enable
assistant wake disable
```

Behavior:

```text
assistant.exe
    |
    +-- atomic settings write
             |
             v
background settings watcher
             |
             +-- AntigravityClient.update_model_config(...)
             |
             +-- WakeService.set_enabled(...)
```

If the background runtime is not running, the same files remain durable and are loaded on the next application start.

`wake phrase` is intentionally excluded from this promise because changing the detector phrase requires generating a valid SentencePiece keyword sequence, validating the native detector, and atomically replacing `keywords.txt`.

## Runtime lifecycle

The management endpoint is owned by the compact overlay host lifecycle. On shutdown its task is aborted and `management.json` is removed only if it still contains the secret owned by that runtime instance. A stale previous endpoint is atomically replaced at startup.

## Remaining CLI migration

The next work should move the remaining management-only capabilities away from React:

```text
assistant overlay show|hide
assistant runtime status|restart
assistant resources install <id>
assistant wake phrase <text>     # with keyword generation + native validation
assistant logs [--follow]
```

After these have terminal parity, the normal full React management surface can be retired.

## Final target architecture

```text
Windows startup
     |
     v
Assisstant Desktop background runtime
     |
     +-- Edge glow
     +-- Quick text/voice overlay
     +-- Sensitive confirmation surface
     |
     +-- authenticated local management contract
              |
              v
         assistant.exe
         CLI / terminal UI
```

The full graphical window should eventually no longer be part of ordinary use.

## Local verification

This migration was prepared source-first. Do not interpret source presence as target-Windows runtime verification. Local validation should cover:

```powershell
cargo build -p assisstant-desktop --bin assistant --locked
cargo build -p assisstant-desktop --features voice-stt,wake-word --locked

.\target\debug\assistant.exe status
.\target\debug\assistant.exe doctor
.\target\debug\assistant.exe
.\target\debug\assistant.exe ai show
.\target\debug\assistant.exe resources list
.\target\debug\assistant.exe permissions list
```

For live reload, start the desktop runtime, change model/effort and wake enabled state through `assistant.exe`, and confirm the runtime changes without restarting the process.

No workflow dispatch is required for normal local validation.
