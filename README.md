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

### Phase 0 — Foundation ✅

Implemented:

- Rust workspace;
- shared assistant state/session/event contracts;
- assistant core state machine;
- Antigravity streaming protocol model;
- project/architecture documentation.

### Phase 1 — Antigravity Integration ✅

Implemented:

- long-running `stream-json` Antigravity session;
- local CLI availability probe;
- broadcast delivery of Antigravity stream events;
- bounded stderr diagnostics;
- auth/quota/model/permission/process/transport error classification;
- explicit start/reset/restart lifecycle;
- safe session invalidation without automatically replaying side-effecting turns.

Next: **Phase 2 — Windows MCP Tools**.

The first Windows tool set will focus on deterministic operations such as volume, mute, application launch/active application, media controls, system info, and clipboard access.

## Documentation

- [`docs/PROJECT_PLAN.md`](docs/PROJECT_PLAN.md) — complete development plan and phase acceptance criteria.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — dependency/process/security boundaries.
- [`docs/ANTIGRAVITY.md`](docs/ANTIGRAVITY.md) — Antigravity runtime/auth/protocol/recovery design.

## Development policy

The project does not reverse-engineer Google credentials and does not expose unrestricted shell execution to the AI. Google authentication remains owned by Antigravity CLI; Windows actions are exposed as explicit MCP tools with assistant-owned permission controls.
