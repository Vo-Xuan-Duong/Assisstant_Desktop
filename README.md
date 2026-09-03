# Assisstant Desktop

Windows-first desktop AI assistant powered by **Google Antigravity + Gemini reasoning + MCP + Rust/Tauri**.

The project is developed phase-by-phase: each subsystem is completed behind a stable interface before it is integrated into the full assistant.

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
- Tokio for asynchronous runtime work.
- Antigravity CLI Headless for the primary AI backend.
- MCP for AI-to-tool integration.
- `windows-rs` for Windows APIs.
- CPAL/WASAPI + whisper.cpp for the later local voice pipeline.
- SQLite for local persistent state.

## Current status

**Phase 0 — Foundation**

Implemented:

- Rust workspace;
- shared assistant state/session/event contracts;
- assistant core state machine;
- Antigravity continuous `stream-json` protocol model;
- long-running Antigravity process/session bridge;
- unified project plan and architecture boundaries.

Next: **Phase 1 — harden Antigravity integration** (CLI discovery, health/auth/quota classification, restart/recovery, streamed event delivery).

## Documentation

- [`docs/PROJECT_PLAN.md`](docs/PROJECT_PLAN.md) — complete development plan and phase acceptance criteria.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — dependency/process/security boundaries.

## Development policy

The project does not reverse-engineer Google credentials and does not expose unrestricted shell execution to the AI. Google authentication remains owned by Antigravity CLI; Windows actions are exposed later as explicit MCP tools with assistant-owned permission controls.
