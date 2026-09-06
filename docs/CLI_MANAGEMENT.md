# CLI / terminal management migration

## Goal

Assisstant Desktop is a background assistant with two user-facing management/interaction surfaces:

1. Gemini-style graphical interaction: perimeter glow, compact input/voice response overlay, and Sensitive permission confirmation.
2. `assistant.exe`: terminal management for configuration, diagnostics, resources, policy administration, live runtime control, and logs.

The former full React chat/settings application has been removed from the frontend source tree. The graphical runtime now contains only Quick, Edge, and the permission confirmation surface.

## Current architecture

```text
Windows startup
     |
     v
Assisstant Desktop background runtime
     |
     +-- Edge glow
     +-- Quick text/voice overlay
     +-- Sensitive confirmation surface
     +-- Antigravity / Gemini
     +-- MCP / Windows tools
     +-- Voice / wake runtime
     +-- bounded persistent runtime log
     |
     +-- authenticated loopback management protocol
              |
              v
         assistant.exe
         CLI / terminal UI
```

## Terminal manager

`assistant.exe` is a second binary target of the existing `assisstant-desktop` Rust package:

```text
apps/desktop/src-tauri/src/bin/assistant.rs
```

The Windows packaging pipeline stages and bundles it alongside the desktop runtime and `assistant-mcp.exe`.

Running it without a subcommand opens the current interactive terminal dashboard:

```powershell
assistant
```

Pages:

```text
Dashboard
Resources
AI / Antigravity
Permissions
```

Controls:

```text
1-4    switch page
r      refresh
q      quit
```

The renderer intentionally remains dependency-light until local Windows build validation is complete. A Ratatui renderer can replace it later without changing the command or IPC contracts.

## Shared data root

On Windows:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop
```

Development override:

```powershell
assistant --data-dir D:\assistant-data status
```

or:

```powershell
$env:ASSISTANT_APP_DATA="D:\assistant-data"
assistant status
```

Existing runtime path overrides are honored for STT, wake model, permission policy, Antigravity binary and runtime logs.

## Phase 1: durable CLI configuration

Implemented commands:

```powershell
assistant status
assistant status --json
assistant paths
assistant doctor

assistant ai show
assistant ai models
assistant ai set --model <model-id>
assistant ai set --effort <value>
assistant ai reset

assistant wake show
assistant wake enable
assistant wake disable

assistant resources list

assistant permissions list
assistant permissions set <tool> <allow|ask|deny>
assistant permissions clear <tool>
```

AI, wake and permission files use atomic temporary-file + backup + rename writes. Permission overrides remain limited to Moderate tools; Safe, Sensitive and Blocked baselines cannot be overridden by the terminal manager.

## Phase 2A: authenticated live management protocol

The background runtime owns:

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

Transport properties:

- raw JSON request/response over TCP;
- listener binds only to `127.0.0.1` on an ephemeral port;
- no HTTP endpoint and no LAN listener;
- every request requires the current random runtime secret;
- request size is bounded;
- short read/write timeouts protect normal management operations;
- management mutations are serialized;
- endpoint file is atomically replaced on startup and removed on shutdown only by its owning runtime secret.

The architecture intentionally follows the project's existing loopback + secret permission-broker pattern without adding a new dependency or changing `Cargo.lock`. A future Windows Named Pipe transport can preserve the same versioned command contract.

## Phase 2B: direct CLI runtime control

The terminal manager consumes the management protocol directly rather than relying only on settings-file polling.

### Runtime

```powershell
assistant runtime status
assistant runtime status --json
assistant runtime ping
assistant runtime restart
```

`runtime restart` restarts the Antigravity agent session, not the whole Windows application process.

### Overlay

```powershell
assistant overlay show
assistant overlay hide
```

This directly controls the compact Gemini-style quick surface and associated edge glow.

### AI configuration

When the background runtime is available:

```powershell
assistant ai show
assistant ai set --model <id>
assistant ai set --effort <value>
assistant ai reset
```

uses authenticated IPC. The runtime persists the same `settings/antigravity.json` and updates `AntigravityClient` immediately.

When the runtime is not running, mutating AI commands fall back to the durable settings file and are loaded on the next start.

### Wake enable/disable

```powershell
assistant wake show
assistant wake enable
assistant wake disable
```

uses the live `WakeService` when the background process is available. If it is not running, enable/disable falls back to the durable preference file.

### Wake phrase

```powershell
assistant wake phrase "HEY ASSISTANT"
```

is a validated runtime operation. It requires a running background runtime and reuses the existing backend resource path:

```text
phrase
  |
  v
SentencePiece bpe.model
  |
  v
validate every token against wake tokens.txt
  |
  v
write staged keywords.txt
  |
  v
load native Sherpa wake detector
  |
  +-- validation fails -> restore previous keywords
  |
  v
hot reload WakeService
  |
  v
persist validated phrase preference
```

The CLI deliberately fails rather than persisting a misleading phrase when tokenizer/model validation cannot be completed.

### Resource installation

```powershell
assistant resources list
assistant resources install stt_zipformer_vi
```

`resources install` requires the background runtime and calls the existing `ResourceInstaller`; there is no second CLI downloader.

The Vietnamese Zipformer path preserves:

- immutable model revision;
- pinned expected sizes;
- SHA-256 verification for model/LFS assets;
- bounded `tokens.txt` download and structural validation;
- staging-directory cleanup on failure;
- atomic directory promotion after verification.

The current wake model itself remains intentionally non-installable automatically until its archive SHA-256 and redistribution/license terms are resolved. CLI does not bypass that safety gate.

`wake_keywords` is generated through:

```powershell
assistant wake phrase <text>
```

so the phrase is explicit and validated.

## Phase 2C: persistent runtime logs

The desktop background process initializes a process-wide tracing sink before Tauri starts. Formatted runtime records are mirrored to stderr for development and to a bounded local file for normal background diagnostics.

Default Windows paths:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\logs\assistant.log
%LOCALAPPDATA%\com.voduong.assisstantdesktop\logs\assistant.log.1
```

Optional override:

```powershell
$env:ASSISTANT_LOG_DIR="D:\assistant-logs"
```

The override must be an absolute directory path so runtime behavior does not depend on the current working directory.

Rotation policy:

```text
active file      assistant.log    ~5 MiB maximum
one backup       assistant.log.1
```

When the active file reaches the bound, the previous active content is copied to `assistant.log.1` and the active file is truncated for continued logging. Only one backup is retained, keeping disk growth bounded without introducing a new logging dependency.

Terminal access:

```powershell
assistant logs
assistant logs --lines 300
assistant logs -n 300
assistant logs --follow
assistant logs -f
```

Behavior:

- default output shows the most recent 120 lines;
- `--lines` is bounded to 1..10000;
- `--follow` prints the current tail and then watches the active file every 500 ms;
- follower detects active-file truncation caused by rotation and resets its offset;
- `assistant status`, `assistant paths` and `assistant doctor` expose log readiness/path.

### Log privacy

Runtime logs stay in the current user's local application-data directory unless explicitly overridden. They should still be treated as sensitive diagnostics: depending on the operation and existing tracing callsites they may contain error text, local file paths, application/tool names, model/runtime state, or transcript-related operational details. Do not publish `assistant.log` without reviewing/redacting it first.

The log sink does not intentionally log the management IPC secret, permission-broker secret, tokens, or credentials. Existing code should continue avoiding secret values in tracing fields.

## Phase 2D: terminal-first graphical runtime

Completed:

1. Sensitive confirmation moved to `PermissionSurface` in the hidden `main` WebView.
2. Wake-triggered voice ownership moved to the lifetime-owned `QuickOverlay` WebView.
3. Quick no longer exposes expand-to-full-management.
4. `main.tsx` routes only `PermissionSurface`, `QuickOverlay`, and `EdgeOverlay`.
5. The retired `App.tsx`, `MainSurface.tsx`, graphical management panels, and their private CSS were deleted.
6. Frontend `api.ts`, `permissionApi.ts`, and `types.ts` were reduced to contracts used by mounted surfaces.
7. Tray **Mở Assistant** and tray left-click now surface Quick.
8. A normal second application launch is redirected by the single-instance callback to Quick.
9. Quick-show failure no longer falls back to the permission-only `main` window.
10. The obsolete `assistant_quick_expand` Tauri command was removed.

The graphical surface is intentionally limited to:

```text
Edge glow
Quick text/voice composer
Short assistant response
Voice visualization
Sensitive permission confirmation
```

`main` is not a general-purpose UI surface. `show_main_window()` remains only because the native permission broker uses it to focus the Sensitive confirmation host.

## Internal protocol v1

Currently implemented commands:

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
resources.install
```

Protocol additions are backward-compatible within v1. Existing authentication and permission boundaries remain unchanged.

The management endpoint does not expose Windows MCP tools directly. Tool execution still goes through Assistant Core / Antigravity / MCP and the existing permission gateway.

## Existing file watcher

The background runtime still watches:

```text
settings/antigravity.json
settings/wake.json
```

approximately every 500 ms. This remains useful for offline/durable edits and older CLI behavior, although the primary live CLI path is direct IPC.

Moderate permission overrides continue to be read by MCP during authorization, so policy changes apply to subsequent requests without a desktop restart.

## Local verification gates

The migration is source-complete but not target-Windows verified. Validate locally before treating it as release-ready:

```powershell
cargo build -p assisstant-desktop --bin assistant --locked
cargo build -p assisstant-desktop --features voice-stt,wake-word --locked

.\target\debug\assistant.exe status
.\target\debug\assistant.exe doctor
.\target\debug\assistant.exe runtime ping
.\target\debug\assistant.exe runtime status
.\target\debug\assistant.exe overlay show
.\target\debug\assistant.exe overlay hide
.\target\debug\assistant.exe ai show
.\target\debug\assistant.exe resources list
.\target\debug\assistant.exe logs
.\target\debug\assistant.exe logs --follow
```

With the background runtime running and required assets present:

```powershell
.\target\debug\assistant.exe resources install stt_zipformer_vi
.\target\debug\assistant.exe wake phrase "HEY ASSISTANT"
```

Also verify graphical lifecycle:

```text
Alt+Space -> Quick
Wake -> Quick -> exactly one voice turn
Tray show/click -> Quick
Second normal launch -> existing process -> Quick
Second --background launch -> no focus steal
Sensitive tool -> PermissionSurface
```

Do not interpret source presence as proof that target-Windows native DLL loading, model download, microphone behavior, wake hot reload, log rotation, Tauri packaging, or lifecycle behavior has been validated.
