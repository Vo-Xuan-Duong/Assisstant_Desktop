# Assisstant Desktop

Windows-first desktop AI assistant powered by **Google Antigravity + Gemini + MCP + Rust/Tauri**.

Development is phase-based: finish one bounded subsystem, static-review it, update this README, then squash-merge to `main` before expanding the next subsystem.

> Current remote verification policy: **do not run GitHub Actions, tests, native runtime builds, or model downloads remotely**. Native Windows verification is performed later on the local machine.

## Current status

- **Latest completed phase on `main`: Phase 13B — Verified Resource Installer**
- **Latest completed main commit:** `f39648f8a803e1f4f5635bc356ff1c09a88c8d81`
- **Current branch:** `phase/13c-wake-keyword-preparation`
- **Current phase:** Phase 13C — Wake Keyword Preparation
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

Optional local resources are managed separately:

```text
RuntimePaths
    ↓
ResourceRegistry
    ↓
Backend Resource Catalog
   ↙                  ↘
Verified Download   Local Preparation
```

## Locked stack

- Rust — assistant core, bridge, MCP, Windows runtime, permission/runtime/resource services.
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
- SentencePiece — local GigaSpeech wake phrase tokenization when `wake-word` is enabled.
- reqwest — verified resource download transport.
- SHA-256 — resource integrity verification before install.

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
| 12C — Local Windows Verification Harness | ✅ | read-only prerequisite/runtime preflight |
| 13A — Runtime Resource Registry | ✅ | unified Whisper/wake paths/status + setup UI |
| 13B — Verified Resource Installer | ✅ | pinned manifest + verified Whisper install + progress UI |
| **13C — Wake Keyword Preparation** | **🚧** | local SentencePiece tokenization + validated `keywords.txt` generation |

## Recent merge points

```text
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
12C  46799a5...  local Windows verification harness
13A  5c9338d...  runtime resource registry + setup UI
13B  f39648f...  verified resource installer
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

Runtime data is not tied to repository root or process working directory.

```text
<app-local-data>/
├── context/
├── models/
│   ├── whisper/
│   └── wake/
├── permissions/
├── audit/
└── runtime/
    └── .agents/
        └── mcp_config.json
```

Antigravity uses `<app-local-data>/runtime` as its working directory. The desktop generates the MCP config with an absolute `assistant-mcp.exe` path. Tauri bundles `assistant-mcp` as an external sidecar, staged with the Rust target-triple suffix before dev/build.

## Runtime resources

One `ResourceRegistry` is reused by VoiceCapabilities, WakeService, Readiness and Resources UI.

Default layout:

```text
<app-local-data>/models/
├── whisper/
│   └── ggml-base.bin
└── wake/
    └── sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01/
        ├── encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx
        ├── decoder-epoch-12-avg-2-chunk-16-left-64.onnx
        ├── joiner-epoch-12-avg-2-chunk-16-left-64.int8.onnx
        ├── tokens.txt
        ├── keywords.txt
        └── bpe.model              # preparation-only
```

Resource overrides must be absolute:

```text
ASSISTANT_WHISPER_MODEL
ASSISTANT_WAKE_MODEL_DIR
ASSISTANT_WAKE_KEYWORDS
```

Open:

```text
Readiness → Resources
```

## Verified Whisper install

The backend manifest pins multilingual `ggml-base.bin`:

```text
expected bytes: 147951465
sha256: 60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe
license: MIT
```

Trust flow:

```text
resource_id
    ↓
trusted backend manifest
    ↓
HTTPS → unique .part
    ↓
byte count + streaming SHA-256
    ↓
flush / sync
    ↓
no-overwrite recheck
    ↓
atomic rename
```

The frontend/Gemini cannot supply arbitrary URLs, hashes or install destinations.

## Phase 13C — Wake Keyword Preparation

The wake model archive remains manual because model-specific redistribution terms and a pinned archive digest are not yet locked to the verified-download standard.

Phase 13C instead makes the application-specific keyword file locally:

```text
bpe.model + tokens.txt
        ↓
user phrase: HEY ASSISTANT
        ↓
SentencePiece
        ↓
validate each piece against tokens.txt
        ↓
reject <unk> / unsupported pieces
        ↓
`<pieces> @HEY_ASSISTANT`
        ↓
keywords.txt.part
        ↓
flush / sync / no-overwrite
        ↓
keywords.txt
```

`bpe.model` is preparation-only; it does not affect wake runtime readiness after a valid `keywords.txt` exists.

Current generator rules:

- phrase is normalized to uppercase;
- maximum 64 characters;
- English ASCII letters, spaces and apostrophes only for the current GigaSpeech model;
- no overwrite of existing `keywords.txt`;
- no network request;
- no MCP/Gemini/Antigravity call;
- generated output uses a canonical `@PHRASE_LABEL`.

Open:

```text
Readiness → Resources → Wake Word Resources
```

When `wake-word` is compiled and `bpe.model + tokens.txt` exist, the UI can generate the missing keyword file. The current WakeService is initialized at startup, so Phase 13C shows a restart notice after generation.

## Local Windows preflight

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-local.ps1
```

JSON output:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-local.ps1 -Json
```

The verifier is read-only and does not build, test, start the app, invoke Actions, download models or change runtime policy.

## Readiness model

The in-app Readiness panel checks Antigravity, Windows MCP, Permission Broker, Context Storage, TTS, Whisper and Wake Word. Whisper/wake checks reuse the ResourceRegistry paths used by the actual voice/wake runtimes.

## Context / privacy

Desktop context is collected on demand only. Screen and clipboard data are treated as untrusted context. Readiness/audit/verifier/resource output do not expose broker secrets, credentials, prompts, clipboard contents, screenshots, permission arguments or model contents. Wake phrase preparation stays local.

## Development rules

1. Develop one bounded phase at a time.
2. Keep native implementation separate from transport/UI where practical.
3. Use stable typed contracts.
4. Authorize before native mutation.
5. Prefer semantic Windows/UIA APIs over raw input.
6. Fail closed for unknown tools, stale targets, broker errors and malformed policy.
7. Static-review APIs before merge.
8. Do not run GitHub Actions/tests/native runtime builds/model downloads during remote development.
9. Squash-merge completed phases to `main`.
10. **Update README before every phase merge.**

## Next direction

After Phase 13C, the next bounded work is **wake phrase lifecycle/hot reload**: safely replacing an existing `keywords.txt`, restarting/reloading the background detector without restarting the whole desktop app, and persisting user-facing wake configuration. After that, remaining remote work should focus on settings persistence and release-readiness hardening rather than expanding tool surface.

Actual compiler/runtime failures from the Windows local verification harness still take precedence over adding new computer-use capabilities.

## Documentation

- [`docs/PROJECT_PLAN.md`](docs/PROJECT_PLAN.md)
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- [`docs/ANTIGRAVITY.md`](docs/ANTIGRAVITY.md)
- [`docs/MCP.md`](docs/MCP.md)
- [`docs/VOICE_DESKTOP.md`](docs/VOICE_DESKTOP.md)
- [`docs/WAKE_RUNTIME.md`](docs/WAKE_RUNTIME.md)
- [`docs/WAKE_KEYWORD_PREPARATION.md`](docs/WAKE_KEYWORD_PREPARATION.md)
- [`docs/UI_AUTOMATION_PATTERNS.md`](docs/UI_AUTOMATION_PATTERNS.md)
- [`docs/PERMISSION_GATEWAY.md`](docs/PERMISSION_GATEWAY.md)
- [`docs/RUNTIME_READINESS.md`](docs/RUNTIME_READINESS.md)
- [`docs/RUNTIME_PATHS_PACKAGING.md`](docs/RUNTIME_PATHS_PACKAGING.md)
- [`docs/LOCAL_WINDOWS_VERIFICATION.md`](docs/LOCAL_WINDOWS_VERIFICATION.md)
- [`docs/RUNTIME_RESOURCES.md`](docs/RUNTIME_RESOURCES.md)
