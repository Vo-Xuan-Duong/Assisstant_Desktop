# Text Desktop MVP

## Scope

Phase 3 adds the first user-facing Windows shell around the already-completed Assistant Core, Antigravity bridge, and Windows MCP layer.

It intentionally does **not** add microphone input, STT, TTS, wake word, screen capture, or the final edge-glow assistant UI yet.

## Runtime architecture

```text
React / TypeScript UI
        |
        | Tauri invoke + events
        v
Tauri Rust desktop process
        |
        +--> Assistant Core
        |       |
        |       v
        |   Antigravity Bridge
        |       |
        |       v
        |      agy
        |
        `--> runtime health / tray / shortcut
```

Antigravity remains responsible for loading the Windows MCP server from its MCP configuration.

## User-facing functionality

- text prompt and response conversation;
- Antigravity CLI availability/health display;
- assistant state events (`idle`, `processing`, `error`, etc.);
- restart/reset controls;
- system tray with show/hide/quit;
- left-click tray restore;
- `Alt + Space` global activation shortcut;
- closing the main window hides it instead of terminating the tray assistant.

## Frontend

The frontend is a pnpm workspace package at `apps/desktop` using:

- React;
- TypeScript;
- Vite;
- `@tauri-apps/api`.

The MVP UI is deliberately neutral. The Gemini-like screen-edge effect is a later isolated UI module so visual animation cannot destabilize the core desktop/agent integration.

## Backend

The Tauri backend is a Rust workspace member at `apps/desktop/src-tauri`.

It owns:

- one `AntigravityClient`;
- one `AssistantCore`;
- one assistant session id;
- a Tauri event sink that forwards core state changes to the React frontend.

The desktop process does not store Google credentials. Antigravity authentication continues to be owned by the installed `agy` CLI.

## Commands

The frontend can invoke:

- `assistant_health`
- `assistant_submit`
- `assistant_restart`
- `assistant_reset`

These commands are the desktop boundary. The React code does not spawn processes or call Windows APIs directly.

## Local development

From the repository root:

```text
pnpm install
pnpm desktop:dev
```

Or from `apps/desktop`:

```text
pnpm install
pnpm tauri dev
```

Before testing MCP tool calls, build/configure `assistant-mcp.exe` as documented in `docs/MCP.md` and ensure Antigravity CLI is signed in normally.

## Verification policy

No GitHub Action is introduced for this phase. Windows/Tauri/Antigravity integration is verified locally because it depends on WebView2, the installed Antigravity CLI, an authenticated Google session, and native Windows APIs.
