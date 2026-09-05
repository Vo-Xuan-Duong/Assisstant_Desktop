# CLI / terminal management migration

## Goal

Assisstant Desktop is moving from a conventional full desktop application toward a background assistant with two user-facing surfaces:

1. Gemini-style graphical interaction surface: perimeter glow, compact input/voice response overlay, and sensitive permission confirmation.
2. `assistant.exe`: terminal management surface for configuration, diagnostics, resources, and policy administration.

The existing full React management UI remains temporarily available while capabilities are migrated. It should not be treated as the long-term primary interface.

## Phase 1 implemented

The terminal manager is a second binary target of the existing `assisstant-desktop` Rust package:

```text
apps/desktop/src-tauri/src/bin/assistant.rs
```

Build output:

```text
assistant.exe
```

Keeping it in the existing package means the first CLI migration adds no new crates and does not require changing the locked dependency graph.

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

This first terminal surface deliberately uses the standard console instead of introducing a TUI framework dependency. Once local build/lock validation is part of the CLI migration, the same management model can be rendered with Ratatui without changing the command or IPC contracts.

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

The CLI writes the same `settings/wake.json` contract used by the wake desktop service.

### Native resources

```powershell
assistant resources list
```

The first CLI phase inspects the same Vietnamese Zipformer and wake-word model layouts as the Tauri resource registry. Resource download/install remains in the desktop resource installer until its Tauri-specific progress emitter is separated from the installer core.

### Permission policy

```powershell
assistant permissions list
assistant permissions set apps_open ask
assistant permissions set clipboard_read_text deny
assistant permissions clear apps_open
```

The CLI uses the native `TOOL_CATALOG` and `permission-engine` types. It refuses to override Safe, Sensitive, or Blocked tools; only Moderate-tool overrides can be persisted, matching the existing security boundary.

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

## Current live-runtime boundary

Phase 1 intentionally does not claim that file edits are hot-reloaded into an already running Tauri process.

```text
assistant.exe
    |
    +-- reads shared state directly
    +-- writes durable settings/policy atomically
    |
    X  no live request/response IPC yet
```

After a mutating command, the CLI explicitly reports that the background runtime must currently be restarted before the change is guaranteed to be active.

This is temporary.

## Phase 2: local management IPC

The next migration step is a versioned local IPC contract between `assistant.exe` and the background Tauri runtime. Windows Named Pipes are the preferred transport.

Planned operations:

```text
runtime.status
overlay.show
overlay.hide
ai.get
ai.set
wake.get
wake.set
resources.list
resources.install
permissions.list
permissions.set
logs.tail
runtime.shutdown
runtime.restart
```

Once IPC exists, CLI edits can apply immediately without restarting the assistant.

## Phase 3: retire the full management UI

After the CLI/terminal surface has feature parity:

- remove normal navigation to the full React management surface;
- keep Tauri only as the background Windows shell / overlay host;
- retain graphical permission confirmation for Sensitive actions;
- retain the quick Gemini-style overlay for text/voice interaction;
- move resource installation, diagnostics, settings, permissions, and logs completely to terminal management.

Target architecture:

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
     +-- local management IPC
              |
              v
         assistant.exe
         CLI / terminal UI
```

## Local verification

This migration was prepared source-first. Do not interpret source presence as target-Windows runtime verification. Local validation should cover:

```powershell
cargo build -p assisstant-desktop --bin assistant
.\target\debug\assistant.exe status
.\target\debug\assistant.exe doctor
.\target\debug\assistant.exe
.\target\debug\assistant.exe ai show
.\target\debug\assistant.exe resources list
.\target\debug\assistant.exe permissions list
```

No workflow dispatch is required for normal local validation.
