# Assisstant Desktop

Windows-first desktop AI assistant powered by **Google Antigravity + Gemini reasoning + MCP + Rust/Tauri**.

The project is developed phase-by-phase: each subsystem is given a stable contract, reviewed, then squash-merged into `main` before the next subsystem is expanded.

> Verification policy: development in this repository currently uses **static API/code review only**. GitHub Actions and runtime tests are intentionally not run during these phases. Native Windows behavior is verified later on a local Windows machine.

## Current status

- **Latest completed phase on `main`: Phase 11D — Monitor Discovery & Window Placement**
- **Current development branch:** `phase/12a-runtime-readiness`
- **Current phase:** Phase 12A — Runtime Readiness Diagnostics
- **Latest completed `main` commit:** `5b1409db94291610e35d36e923b55cc5a4086c34`
- **Desktop target:** Windows first
- **AI backend:** Antigravity CLI / Gemini
- **Tool protocol:** MCP over stdio
- **Default safety posture:** fail closed for unknown/blocked actions
- **Current integration priority:** make local/runtime readiness explicit before packaging

## Architecture

```text
Wake word / Hotkey / Text
          |
          v
     Tauri Desktop
   React + Edge Glow
          |
          v
     Assistant Core
          |
          +--------------------+
          |                    |
          v                    v
   Context Engine        Permission UI
          |                    ^
          v                    |
 Antigravity Bridge      Permission Broker
          |                    ^
          v                    |
 Antigravity CLI / Gemini      |
          |                    |
          v                    |
        MCP Server ------------+
          |
          v
   Windows Tool Runtime
          |
     +----+------------------------------+
     |        |         |       |        |
   Win32    UIA      CoreAudio  GDI   Clipboard
     |
     v
   Windows
```

## Locked stack

- **Rust** — assistant core, runtime, native Windows integration, MCP server.
- **TypeScript + React** — desktop UI.
- **Tauri 2** — Windows desktop shell, tray, global shortcut and transparent edge windows.
- **Tokio** — async runtime and blocking/native task isolation.
- **Antigravity CLI Headless** — primary AI backend using the user's Antigravity/Gemini access.
- **MCP** — AI-to-tool integration.
- **windows-rs 0.62.2** — Win32, COM, UI Automation, Core Audio and related APIs.
- **CPAL/WASAPI** — local microphone capture.
- **Whisper** — optional local STT, feature-gated.
- **Windows SAPI** — local TTS.
- **sherpa-onnx** — optional local wake-word engine, feature-gated.

## Development progress

| Phase | Status | Main capability |
|---|---|---|
| 0 — Foundation | ✅ | Workspace, common contracts, Assistant Core state machine |
| 1 — Antigravity Runtime | ✅ | Long-running `stream-json` CLI session, health/error lifecycle |
| 2 — Windows MCP Foundation | ✅ | Native Windows tools + `assistant-mcp.exe` |
| 3 — Text Desktop MVP | ✅ | Tauri/React shell, tray, hotkey, text conversation |
| 4 — Context Engine | ✅ | Source window, clipboard and on-demand screenshot context |
| 5A — Audio Runtime | ✅ | CPAL/WASAPI microphone capture, normalized mono audio |
| 5B — VAD + STT | ✅ | Local endpointing + optional Whisper recognizer |
| 5C — Desktop Voice Turn | ✅ | Listening → STT → Gemini → TTS lifecycle |
| 6 — Gemini-like Edge UI | ✅ | Four click-through edge surfaces, state/RMS animations |
| 7A — Wake Detector | ✅ | Feature-gated sherpa keyword spotting contract |
| 7B — Background Wake Runtime | ✅ | Always-on wake worker with microphone ownership barriers |
| 7C — Wake-to-Conversation | ✅ | Wake detection automatically starts the existing voice turn |
| 8A — UI Automation Foundation | ✅ | Structural UIA tree + path-based semantic actions |
| 8B — UIA MCP Tools | ✅ | `ui_inspect`, focus, invoke and value actions through MCP |
| 8C — Rich UIA Patterns | ✅ | Toggle, selection, expand/collapse and scroll patterns |
| 9A — Permission Engine | ✅ | Fail-closed `Allow / Ask / Deny` policy core |
| 9B — Permission Broker | ✅ | Authenticated loopback one-shot confirmation broker |
| 9C — Permission UX + Audit | ✅ | Core `Confirming` state + argument-free audit records |
| 9D — Runtime Policy Overrides | ✅ | Live Moderate-tool `Default / Allow / Ask / Deny` overrides |
| 10A — RangeValue | ✅ | Semantic bounded numeric UI controls |
| 10B — UIA State Schema | ✅ | Stable semantic toggle/expand state enums |
| 10C — Grid + ScrollItem Native | ✅ | Grid/GridItem metadata + native `ScrollIntoView` |
| 10D — ScrollItem MCP | ✅ | `ui_scroll_into_view` through permission gateway |
| 10E — MCP Router Modularization | ✅ | System/UI routers split without public contract changes |
| 11A — VirtualizedItem | ✅ | Status + semantic `Realize()` with mandatory reinspection |
| 11B — Window Management | ✅ | Minimize/maximize/restore + graceful close with stale-HWND guard |
| 11C — Window Discovery & Activation | ✅ | Bounded top-level window discovery + explicit foreground activation |
| 11D — Monitor Discovery & Window Placement | ✅ | Monitor/work-area geometry + explicit move/resize without raw input |
| **12A — Runtime Readiness Diagnostics** | **🚧** | Actionable readiness report across AI/MCP/security/context/voice/wake |

## Recent merge points

```text
Phase 5C   4f27508...  Desktop TTS + voice integration
Phase 6    c6952cf...  Gemini-like edge UI
Phase 7A   4fada99...  Wake detector abstraction
Phase 7B   80f53d4...  Background wake runtime
Phase 7C   658ca14...  Wake-to-conversation
Phase 8A   b4293b5...  Native UI Automation foundation
Phase 8B   a73a85d...  UI Automation MCP exposure
Phase 8C   fb7051d...  Rich UIA patterns
Phase 9A   ae39077...  Fail-closed permission engine
Phase 9B   a4b763e...  Desktop permission broker
Phase 9C   ab8cf11...  Confirming state + audit
Phase 9D   c4a15dc...  Runtime Moderate overrides
Phase 10A  a412641...  RangeValue support
Phase 10B  3581f12...  UIA state schema normalization
Phase 10C  c60382b...  Grid/GridItem + ScrollItem native
Phase 10D  0760aeb...  ScrollItem MCP
Phase 10E  9ad37fb...  MCP router modularization
Phase 11A  26c4d48...  VirtualizedItem support
Phase 11B  7582688...  Explicit window management
Phase 11C  0f5aead...  Window discovery + activation
Phase 11D  5b1409d...  Monitor discovery + window placement
```

## Current Windows/MCP capability surface

### System and desktop

- `audio_get_volume`
- `audio_set_volume`
- `audio_set_mute`
- `apps_open`
- `apps_list`
- `display_list`
- `window_get_active`
- `window_list`
- `window_activate`
- `window_set_bounds`
- `window_set_state`
- `window_close`
- `system_get_info`
- `media_play_pause`
- `media_next`
- `media_previous`
- `clipboard_read_text`
- `clipboard_write_text`

### Semantic UI Automation

- `ui_inspect`
- `ui_focus`
- `ui_invoke`
- `ui_set_value`
- `ui_set_range_value`
- `ui_toggle`
- `ui_select`
- `ui_set_expanded`
- `ui_scroll`
- `ui_scroll_into_view`
- `ui_virtualized_item_status`
- `ui_realize`

The UI Automation layer uses **explicit HWND + semantic element path** instead of pixel coordinates. Mutation tools resolve the element again immediately before execution and fail when the path is stale.

## Window control contract

Window actions use an explicit identity pair:

```text
window_handle
+
expected_process_id
```

The process ID is checked again immediately before mutation. This prevents a recycled HWND from silently retargeting a different process.

Current risk model:

```text
display_list       → Safe
window_list        → Safe
window_activate    → Moderate
window_set_bounds  → Moderate
window_set_state   → Moderate
window_close       → Sensitive
```

`window_close` posts `WM_CLOSE`; it does **not** terminate the process and preserves normal unsaved-changes/application shutdown behavior.

## Phase 12A — Runtime Readiness Diagnostics

The current branch adds one aggregate diagnostic command and a dedicated desktop panel.

Readiness checks:

```text
Antigravity CLI
Windows MCP
Permission Broker
Context Storage
Windows TTS
Local Whisper STT
Wake Word
```

Each check is classified as:

```text
ready
optional_missing
blocking
```

Overall readiness is derived conservatively:

```text
any blocking
    → blocking

no blocking + optional component missing
    → optional_missing

all required/selected components ready
    → ready
```

`Windows MCP` checks the active MCP configuration and the configured `assistant-mcp.exe` path. The repository now includes `.agents/mcp_config.json` for the local/dev layout; `ASSISTANT_MCP_CONFIG` can override the config location.

Missing Whisper or wake resources are optional and do not block text assistant operation. Missing Antigravity, malformed permission policy, unavailable broker, unwritable context storage or a missing MCP executable are surfaced as blocking for the capability that requires them.

The readiness report intentionally does **not** expose broker secrets, credentials, prompt/chat text, clipboard contents, screenshots, permission arguments or audit contents.

## Permission model

```text
Tool Call
   |
   v
Risk Catalogue
   |
   +-- Safe ------> Allow by baseline policy
   |
   +-- Moderate --> Default / Allow / Ask / Deny runtime policy
   |
   +-- Sensitive -> Desktop confirmation / Allow once
   |
   +-- Blocked ---> Deny
   |
   +-- Unknown ---> Deny
```

Sensitive approvals are one-shot. Broker timeout, authentication failure, missing UI response or malformed runtime policy never becomes an implicit Allow.

Permission audit records intentionally exclude prompt text, clipboard content, field values and tool arguments.

## Voice flow

```text
Wake microphone
      |
      v
sherpa keyword detector
      |
      v
release wake microphone
      |
      v
Whisper voice turn
      |
      v
Antigravity / Gemini
      |
      v
MCP / Windows
      |
      v
Windows SAPI TTS
      |
      v
resume wake runtime
```

The wake microphone is released before the full voice turn opens the microphone, preventing two CPAL/WASAPI streams from competing for the device.

## Context and privacy model

Desktop context is **on demand**, not continuously uploaded.

For computer-use requests the Context Engine can provide the source application that was active before the Assistant took focus:

```text
active_window_handle
active_window_title
active_process_id
active_executable
```

Screenshots are captured only when the user request needs screen context. Local desktop context is marked as **untrusted context** before being inserted into the AI request.

Phase 12A currently reports that context artifacts still use a working-directory-relative path. Moving that storage to app-local-data is intentionally deferred to Phase 12B so packaging/path migration remains a bounded change.

## Development rules

1. Develop one bounded phase at a time.
2. Keep native implementation separate from MCP transport where practical.
3. Use stable typed contracts between modules.
4. Run permission authorization **before** native mutations.
5. Prefer semantic Windows/UI Automation APIs over raw keyboard/mouse or pixel clicking.
6. Fail closed for unknown tools, stale targets, broker errors and malformed policy.
7. Static-review API signatures before merge.
8. Do not run GitHub Actions/runtime tests during the current remote development workflow.
9. Squash-merge completed phases into `main`.
10. **Update this README before every phase merge so it remains the project progress source of truth.**

## Next direction

After Phase 12A, development moves to **Phase 12B — Runtime Paths & Packaging Hardening**:

- move context artifacts from working-directory-relative storage to app-local-data;
- remove assumptions that MCP resources live under repo-local `target/release` in packaged builds;
- define explicit dev vs packaged runtime resource resolution;
- keep environment overrides for diagnostics/recovery;
- update readiness to reflect the new packaged runtime contract.

Raw mouse/keyboard automation remains deferred until semantic APIs are exhausted and a separate safety design is in place.

## Documentation

Key design documents live under [`docs/`](docs/), including:

- [`docs/PROJECT_PLAN.md`](docs/PROJECT_PLAN.md)
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- [`docs/ANTIGRAVITY.md`](docs/ANTIGRAVITY.md)
- [`docs/MCP.md`](docs/MCP.md)
- [`docs/VOICE_DESKTOP.md`](docs/VOICE_DESKTOP.md)
- [`docs/WAKE_RUNTIME.md`](docs/WAKE_RUNTIME.md)
- [`docs/UI_AUTOMATION_PATTERNS.md`](docs/UI_AUTOMATION_PATTERNS.md)
- [`docs/PERMISSION_GATEWAY.md`](docs/PERMISSION_GATEWAY.md)
- [`docs/MCP_ROUTER_MODULARIZATION.md`](docs/MCP_ROUTER_MODULARIZATION.md)
- [`docs/UI_AUTOMATION_VIRTUALIZED_ITEM.md`](docs/UI_AUTOMATION_VIRTUALIZED_ITEM.md)
- [`docs/WINDOW_MANAGEMENT.md`](docs/WINDOW_MANAGEMENT.md)
- [`docs/WINDOW_DISCOVERY_ACTIVATION.md`](docs/WINDOW_DISCOVERY_ACTIVATION.md)
- [`docs/MONITOR_WINDOW_PLACEMENT.md`](docs/MONITOR_WINDOW_PLACEMENT.md)
- [`docs/RUNTIME_READINESS.md`](docs/RUNTIME_READINESS.md)
