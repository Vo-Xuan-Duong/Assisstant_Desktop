# Assisstant Desktop

Windows-first desktop AI assistant powered by **Google Antigravity + Gemini reasoning + MCP + Rust/Tauri**.

Development is phase-based: each subsystem is completed behind a stable interface before the next subsystem is integrated.

## Architecture

```text
Desktop UI / Voice
        |
        v
  Assistant Core
        |
        v
Antigravity Bridge
        |
        v
Antigravity CLI / Gemini
        |
        v
       MCP
        |
        v
Windows Tool Runtime
        |
        v
      Windows
```

## Locked stack

- Rust for assistant/runtime/system code.
- TypeScript + React for the Tauri UI.
- Tauri 2 for the Windows desktop shell.
- Tokio for asynchronous Rust work.
- Antigravity CLI Headless for the primary AI backend.
- MCP for AI-to-tool integration.
- `windows-rs` for Windows APIs.
- CPAL/WASAPI + whisper.cpp for the later local voice pipeline.
- SQLite for local persistent state.

## Project status

### Phase 0 — Foundation ✅

- Rust workspace and common domain contracts.
- Assistant Core state machine.
- Antigravity streaming protocol model.
- Architecture and project plan.

### Phase 1 — Antigravity Integration ✅

- Long-running `stream-json` process/session.
- CLI health probing.
- Stream event broadcasting.
- Diagnostics and typed failure classification.
- Explicit start/reset/restart lifecycle.

### Phase 2 — Windows MCP Tools ✅

Native Windows implementation and a separate `assistant-mcp.exe` stdio server now expose:

- `audio_get_volume`
- `audio_set_volume`
- `audio_set_mute`
- `apps_open`
- `apps_list`
- `window_get_active`
- `system_get_info`
- `media_play_pause`
- `media_next`
- `media_previous`
- `clipboard_read_text`
- `clipboard_write_text`

Public MCP names are also the keys used by the native risk catalogue, which prepares the next desktop permission layer.

## Next phase

**Phase 3 — Text Desktop MVP**

Tauri 2 + React + TypeScript will provide the first usable desktop shell: tray, assistant overlay, text conversation, runtime health, settings, and permission surfaces. Voice remains deferred until the text/system integration is stable.

## Documentation

- [`docs/PROJECT_PLAN.md`](docs/PROJECT_PLAN.md)
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- [`docs/ANTIGRAVITY.md`](docs/ANTIGRAVITY.md)
- [`docs/MCP.md`](docs/MCP.md)
