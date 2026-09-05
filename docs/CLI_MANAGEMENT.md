# CLI / TUI management migration

## Goal

Assisstant Desktop is moving from a conventional full desktop application toward a background assistant with two user-facing surfaces:

1. Gemini-style graphical interaction surface: perimeter glow, compact input/voice response overlay, and sensitive permission confirmation.
2. `assistant.exe`: terminal management surface for configuration, diagnostics, resources, and policy administration.

The existing full React management UI remains temporarily available while capabilities are migrated. It should not be treated as the long-term primary interface.

## Phase 1 implemented

A new workspace binary lives at:

```text
apps/cli
```

Build output:

```text
assistant.exe
```

Running it without a subcommand opens a read-only TUI dashboard:

```powershell
assistant
```

The TUI currently exposes four views:

- Dashboard
- Resources
- AI / Antigravity
- Permissions

Keyboard controls:

```text
1-4 / Left / Right    switch view
r                     refresh
q / Esc               quit
```

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

Once IPC exists, CLI/TUI edits can apply immediately without restarting the assistant.

## Phase 3: retire the full management UI

After the CLI/TUI has feature parity:

- remove normal navigation to the full React management surface;
- keep Tauri only as the background Windows shell / overlay host;
- retain graphical permission confirmation for Sensitive actions;
- retain the quick Gemini-style overlay for text/voice interaction;
- move resource installation, diagnostics, settings, permissions, and logs completely to CLI/TUI.

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
         CLI + Ratatui TUI
```

## Verification policy

This migration was prepared source-first. Do not interpret source presence as target-Windows runtime verification. Local validation should cover:

```powershell
cargo build -p assistant-cli
assistant status
assistant doctor
assistant
assistant ai show
assistant resources list
assistant permissions list
```

No workflow dispatch is required for normal local validation.
