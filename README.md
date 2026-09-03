# Assisstant Desktop

Windows-first desktop AI assistant powered by **Google Antigravity + Gemini + MCP + Rust/Tauri**.

Development is phase-based: finish one bounded subsystem, static-review it, update this README, then squash-merge to `main` before expanding the next subsystem.

> Current remote verification policy: **do not run GitHub Actions or runtime tests**. Native Windows build/runtime verification is intentionally deferred to the local machine.

## Current status

- **Latest completed phase on `main`: Phase 12A — Runtime Readiness Diagnostics**
- **Latest completed main commit:** `24b39fb8bb34370f46e231b0b8b010e8d121b8c1`
- **Current branch:** `phase/12b-runtime-paths-packaging`
- **Current phase:** Phase 12B — Runtime Paths & Packaging Hardening
- **Desktop target:** Windows first
- **AI backend:** Antigravity CLI / Gemini
- **Tool protocol:** MCP over stdio
- **Safety default:** unknown/blocked actions fail closed

## Architecture

```text
Wake / Hotkey / Text
        ↓
Tauri Desktop + React + Edge Glow
        ↓
Assistant Core
   ↙          ↘
Context      Permission UI
   ↓             ↑
Antigravity Bridge ← Permission Broker
        ↓
Antigravity CLI / Gemini
        ↓
MCP Server
        ↓
Windows Tool Runtime
        ↓
Win32 / UIA / CoreAudio / GDI / Clipboard
        ↓
Windows
```

## Locked stack

- Rust — assistant core, bridge, MCP, Windows runtime, permission/runtime services.
- TypeScript + React — desktop UI.
- Tauri 2 — shell, tray, global shortcut, transparent edge UI, sidecar bundling.
- Tokio — async runtime.
- Antigravity CLI Headless — primary AI backend.
- MCP — agent-to-tool protocol.
- `windows-rs 0.62.2` — Win32/COM/UIA/CoreAudio.
- CPAL/WASAPI — microphone input.
- Whisper — optional local STT.
- Windows SAPI — local TTS.
- sherpa-onnx — optional wake word.

## Development progress

| Phase | Status | Capability |
|---|---|---|
| 0 — Foundation | ✅ | Workspace, common contracts, Assistant Core |
| 1 — Antigravity Runtime | ✅ | Long-running stream-json session |
| 2 — Windows MCP Foundation | ✅ | Native Windows tools + `assistant-mcp.exe` |
| 3 — Text Desktop MVP | ✅ | Tauri/React shell, tray, text chat |
| 4 — Context Engine | ✅ | Source window, clipboard, on-demand screenshot |
| 5A — Audio Runtime | ✅ | CPAL/WASAPI capture |
| 5B — VAD + STT | ✅ | VAD + optional Whisper |
| 5C — Desktop Voice Turn | ✅ | Listening → STT → Gemini → TTS |
| 6 — Gemini-like Edge UI | ✅ | Four click-through edge surfaces |
| 7A — Wake Detector | ✅ | sherpa wake abstraction |
| 7B — Background Wake Runtime | ✅ | Always-on wake worker |
| 7C — Wake-to-Conversation | ✅ | Wake automatically starts voice turn |
| 8A — UIA Foundation | ✅ | Structural UI Automation tree/actions |
| 8B — UIA MCP | ✅ | inspect/focus/invoke/value tools |
| 8C — Rich UIA Patterns | ✅ | toggle/select/expand/scroll |
| 9A — Permission Engine | ✅ | Allow / Ask / Deny fail-closed core |
| 9B — Permission Broker | ✅ | Authenticated loopback confirmation |
| 9C — Permission UX + Audit | ✅ | Confirming state + argument-free audit |
| 9D — Runtime Policy Overrides | ✅ | Live Moderate policy overrides |
| 10A — RangeValue | ✅ | Numeric UIA controls |
| 10B — UIA State Schema | ✅ | Semantic state enums |
| 10C — Grid + ScrollItem Native | ✅ | Grid metadata + ScrollIntoView |
| 10D — ScrollItem MCP | ✅ | `ui_scroll_into_view` |
| 10E — MCP Router Modularization | ✅ | Modular server routers |
| 11A — VirtualizedItem | ✅ | status + `Realize()` |
| 11B — Window Management | ✅ | minimize/maximize/restore/graceful close |
| 11C — Window Discovery & Activation | ✅ | bounded window list + activate |
| 11D — Monitor & Placement | ✅ | monitor geometry + move/resize |
| 12A — Runtime Readiness | ✅ | readiness panel across runtime dependencies |
| **12B — Runtime Paths & Packaging** | **🚧** | app-local-data runtime + bundled MCP sidecar |

## Recent merge points

```text
8A   b4293b5...  UI Automation foundation
8B   a73a85d...  UIA MCP tools
8C   fb7051d...  rich UIA patterns
9A   ae39077...  permission engine
9B   a4b763e...  permission broker
9C   ab8cf11...  permission UX + audit
9D   c4a15dc...  runtime policy overrides
10A  a412641...  RangeValue
10B  3581f12...  UIA state schema
10C  c60382b...  Grid + ScrollItem native
10D  0760aeb...  ScrollItem MCP
10E  9ad37fb...  MCP router modularization
11A  26c4d48...  VirtualizedItem
11B  7582688...  window management
11C  0f5aead...  window discovery + activation
11D  5b1409d...  monitor discovery + placement
12A  24b39fb...  runtime readiness diagnostics
```

## Current MCP capability surface

### System / desktop

```text
audio_get_volume
audio_set_volume
audio_set_mute
apps_open
apps_list
display_list
window_get_active
window_list
window_activate
window_set_bounds
window_set_state
window_close
system_get_info
media_play_pause
media_next
media_previous
clipboard_read_text
clipboard_write_text
```

### Semantic UI Automation

```text
ui_inspect
ui_focus
ui_invoke
ui_set_value
ui_set_range_value
ui_toggle
ui_select
ui_set_expanded
ui_scroll
ui_scroll_into_view
ui_virtualized_item_status
ui_realize
```

UI Automation uses **explicit HWND + semantic element path**, not pixel coordinates.

## Permission model

```text
Safe       → baseline Allow
Moderate   → Default / Allow / Ask / Deny runtime policy
Sensitive  → desktop confirmation / Allow once
Blocked    → Deny
Unknown    → Deny
```

Sensitive approval is one-shot. Broker timeout, malformed policy, missing UI response or broker failure never becomes implicit Allow.

## Phase 12B — Runtime Paths & Packaging Hardening

Phase 12B removes repository-root and current-working-directory assumptions.

Runtime layout:

```text
<app-local-data>/
├── context/
├── models/
├── permissions/
├── audit/
└── runtime/
    └── .agents/
        └── mcp_config.json
```

Desktop startup now:

```text
resolve RuntimePaths
      ↓
generate runtime/.agents/mcp_config.json
      ↓
resolve assistant-mcp.exe
      ↓
set Antigravity working_directory = runtime/
      ↓
create ContextEngine with app-local-data/context
```

MCP binary resolution order:

```text
ASSISTANT_MCP_BINARY
      ↓
Tauri bundled sidecar
      ↓
dev target/debug
      ↓
dev target/release
      ↓
expected bundled path for readiness diagnostics
```

Tauri config uses:

```text
bundle.externalBin = binaries/assistant-mcp
```

Before Tauri dev/build, `apps/desktop/scripts/stage-sidecar.mjs` builds `windows-mcp`, obtains the Rust host target triple, and stages:

```text
src-tauri/binaries/assistant-mcp-<target-triple>.exe
```

Generated executables remain ignored by Git. Runtime `.agents/mcp_config.json` is no longer tracked; `.agents/mcp_config.example.json` is documentation-only.

Context screenshots now live under `<app-local-data>/context` instead of a working-directory-relative `.assistant/context` folder.

Supported path overrides:

```text
ASSISTANT_RUNTIME_DIR
ASSISTANT_MCP_BINARY
ASSISTANT_WHISPER_MODEL
ASSISTANT_WAKE_MODEL_DIR
ASSISTANT_WAKE_KEYWORDS
```

## Readiness model

The desktop Readiness panel checks:

```text
Antigravity CLI
Windows MCP
Permission Broker
Context Storage
Windows TTS
Local Whisper STT
Wake Word
```

Levels are `ready`, `optional_missing`, or `blocking`. Phase 12B checks the generated app-local-data MCP config and resolved sidecar path rather than a repository-local config.

## Context / privacy

Desktop context is collected on demand only. Screen and clipboard data are treated as untrusted context. Readiness/audit do not expose broker secrets, credentials, prompts, clipboard contents, screenshots, permission arguments or audit payloads.

## Development rules

1. Develop one bounded phase at a time.
2. Keep native implementation separate from transport/UI where practical.
3. Use stable typed contracts.
4. Authorize before native mutation.
5. Prefer semantic Windows/UIA APIs over raw input.
6. Fail closed for unknown tools, stale targets, broker errors and malformed policy.
7. Static-review APIs before merge.
8. Do not run GitHub Actions/runtime tests during remote development.
9. Squash-merge completed phases to `main`.
10. **Update README before every phase merge.**

## Next direction

After Phase 12B, priority shifts to **local release verification and integration fixes** rather than rapidly expanding tool count:

- verify Windows build;
- verify Tauri sidecar bundle/install layout;
- verify generated runtime MCP config;
- verify packaged Antigravity session/auth behavior;
- verify Whisper/wake model installation paths;
- fix compile/runtime mismatches found locally before any raw mouse/keyboard automation.

## Documentation

- [`docs/PROJECT_PLAN.md`](docs/PROJECT_PLAN.md)
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- [`docs/ANTIGRAVITY.md`](docs/ANTIGRAVITY.md)
- [`docs/MCP.md`](docs/MCP.md)
- [`docs/VOICE_DESKTOP.md`](docs/VOICE_DESKTOP.md)
- [`docs/WAKE_RUNTIME.md`](docs/WAKE_RUNTIME.md)
- [`docs/UI_AUTOMATION_PATTERNS.md`](docs/UI_AUTOMATION_PATTERNS.md)
- [`docs/PERMISSION_GATEWAY.md`](docs/PERMISSION_GATEWAY.md)
- [`docs/RUNTIME_READINESS.md`](docs/RUNTIME_READINESS.md)
- [`docs/RUNTIME_PATHS_PACKAGING.md`](docs/RUNTIME_PATHS_PACKAGING.md)
