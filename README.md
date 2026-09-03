# Assisstant Desktop

Windows-first desktop AI assistant powered by **Google Antigravity + Gemini + MCP + Rust/Tauri**.

Development is phase-based: finish one bounded subsystem, static-review it, update this README, then squash-merge to `main` before expanding the next subsystem.

> Current remote verification policy: **do not run GitHub Actions, tests, or native runtime builds remotely**. Native Windows verification is performed later on the local machine.

## Current status

- **Latest completed phase on `main`: Phase 12B — Runtime Paths & Packaging Hardening**
- **Latest completed main commit:** `af9d712f545ea19ef5c2f7e9be5a1017a4555695`
- **Current branch:** `phase/12c-local-verification-harness`
- **Current phase:** Phase 12C — Local Windows Verification Harness
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
| 12B — Runtime Paths & Packaging | ✅ | app-local-data runtime + bundled MCP sidecar |
| **12C — Local Windows Verification Harness** | **🚧** | read-only prerequisite/runtime preflight for local verification |

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
12B  af9d712...  runtime paths + MCP sidecar packaging
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

## Runtime / packaging contract

Runtime data is no longer tied to repository root or process working directory.

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

Antigravity uses `<app-local-data>/runtime` as its working directory. The desktop generates the MCP config with an absolute `assistant-mcp.exe` path. Tauri bundles `assistant-mcp` as an external sidecar, staged with the Rust target-triple suffix before dev/build.

## Phase 12C — Local Windows Verification Harness

Current branch adds a read-only PowerShell preflight script:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-local.ps1
```

JSON output:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-local.ps1 -Json
```

The verifier checks:

```text
Windows host
repo structure
rustc / cargo / Rust windows-msvc target
pnpm
Antigravity CLI (`agy`)
MSVC/WebView2 hints
MCP debug/release binaries
Tauri target-triple staged sidecar
app-local-data generated MCP config
context directory
permission policy JSON
Whisper model (optional)
wake model resources (optional)
```

Result levels:

```text
ready
optional
blocking
info
```

Exit code is `1` only when at least one blocking prerequisite is found. The script **does not** build, run tests, start the app, invoke GitHub Actions, download models, or modify runtime policy.

Suggested manual sequence after preflight:

```powershell
pnpm install
pnpm --dir apps/desktop sidecar:stage:dev
pnpm --dir apps/desktop tauri dev
```

These commands are printed only as guidance; the verifier does not execute them.

## Readiness model

The in-app Readiness panel checks Antigravity, Windows MCP, Permission Broker, Context Storage, TTS, Whisper and Wake Word. Levels are `ready`, `optional_missing`, or `blocking`. The PowerShell harness is a separate **pre-start prerequisite check**, while the Readiness panel reports **live desktop runtime state**.

## Context / privacy

Desktop context is collected on demand only. Screen and clipboard data are treated as untrusted context. Readiness/audit/verifier output do not expose broker secrets, credentials, prompts, clipboard contents, screenshots, permission arguments or audit payloads.

## Development rules

1. Develop one bounded phase at a time.
2. Keep native implementation separate from transport/UI where practical.
3. Use stable typed contracts.
4. Authorize before native mutation.
5. Prefer semantic Windows/UIA APIs over raw input.
6. Fail closed for unknown tools, stale targets, broker errors and malformed policy.
7. Static-review APIs before merge.
8. Do not run GitHub Actions/tests/native runtime builds during remote development.
9. Squash-merge completed phases to `main`.
10. **Update README before every phase merge.**

## Next local milestone

After Phase 12C merges, the project should be run on the target Windows machine. Use the verifier first, then start Tauri locally. Actual compiler/runtime failures should drive the next integration-fix phase before adding more computer-use capabilities.

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
- [`docs/LOCAL_WINDOWS_VERIFICATION.md`](docs/LOCAL_WINDOWS_VERIFICATION.md)
