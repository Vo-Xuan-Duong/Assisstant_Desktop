# Assisstant Desktop

Windows-first desktop AI assistant powered by **Google Antigravity + Gemini + MCP + Rust/Tauri**.

Development is phase-based: finish one bounded subsystem, static-review it, update this README, then squash-merge to `main` before expanding the next subsystem.

> Remote policy: **do not run GitHub Actions, tests, native runtime builds, installers, signing operations, or model downloads remotely**. Native Windows verification happens on the local machine.

## Current status

- **Latest completed phase on `main`: Phase 14B — Release Readiness**
- **Latest completed phase commit:** `718f290a66336111486ff22baf121c4fccdc3e63`
- **Current branch:** `phase/15-release-preparation`
- **Current phase:** Phase 15 — Deterministic Release Preparation & Signing
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

Local model/resources stay outside the installed executable and are resolved through app-local-data:

```text
RuntimePaths
    ↓
ResourceRegistry
   ↙         ↘
Whisper     Wake
               ↓
        phrase preparation
               ↓
          hot reload
```

## Locked stack

- Rust — core, Antigravity bridge, MCP, permissions, Windows runtime and voice/resource services.
- Rust `1.85.0` — pinned release baseline.
- TypeScript + React — desktop UI.
- Tauri 2 — Windows shell, tray, global shortcut, edge overlay and bundling.
- Tokio — async runtime.
- Antigravity CLI Headless — primary AI backend.
- MCP over stdio — model/tool protocol.
- `windows-rs 0.62.2` — Win32/COM/UIA/CoreAudio.
- CPAL/WASAPI — microphone capture.
- Whisper — local STT.
- Windows SAPI — local TTS.
- sherpa-onnx + SentencePiece — wake-word runtime and local phrase preparation.
- NSIS — current-user Windows installer.

## Development progress

| Phase | Status | Capability |
|---|---|---|
| 0 — Foundation | ✅ | Workspace, contracts, Assistant Core |
| 1 — Antigravity Runtime | ✅ | Long-running stream-json session |
| 2 — Windows MCP Foundation | ✅ | Native Windows tools + `assistant-mcp.exe` |
| 3 — Text Desktop MVP | ✅ | Tauri/React shell, tray, text chat |
| 4 — Context Engine | ✅ | source window, clipboard, on-demand screenshot |
| 5A — Audio Runtime | ✅ | CPAL/WASAPI capture |
| 5B — VAD + STT | ✅ | VAD + Whisper |
| 5C — Desktop Voice Turn | ✅ | listening → STT → Gemini → TTS |
| 6 — Gemini-like Edge UI | ✅ | four click-through edge surfaces |
| 7A — Wake Detector | ✅ | sherpa wake abstraction |
| 7B — Background Wake Runtime | ✅ | always-on wake worker |
| 7C — Wake-to-Conversation | ✅ | wake automatically starts voice turn |
| 8A–8C — UI Automation | ✅ | inspect/focus/invoke/value/toggle/select/expand/scroll |
| 9A–9D — Permissions | ✅ | fail-closed policy, authenticated broker, UX/audit, runtime overrides |
| 10A–10E — Rich UIA | ✅ | range/grid/scroll-item/state schema/router modularization |
| 11A–11D — Windows Control | ✅ | virtualization/window discovery/state/monitor placement |
| 12A — Runtime Readiness | ✅ | readiness diagnostics across dependencies |
| 12B — Runtime Paths & Packaging | ✅ | app-local-data runtime + bundled MCP sidecar |
| 12C — Local Windows Verifier | ✅ | read-only prerequisite/runtime preflight |
| 13A — Runtime Resource Registry | ✅ | unified Whisper/wake paths and setup UI |
| 13B — Verified Resource Installer | ✅ | pinned manifest + SHA-256 install flow |
| 13C — Wake Keyword Preparation | ✅ | SentencePiece + validated `keywords.txt` generation |
| 13D — Wake Lifecycle & Hot Reload | ✅ | transactional replacement + persisted wake settings |
| 14A — Windows Lifecycle | ✅ | single instance + opt-in autostart + hidden background launch |
| 14B — Release Readiness | ✅ | full-feature NSIS contract + fail-closed release verifier |
| **15 — Release Preparation & Signing** | **🚧** | reproducible icon + lockfile preparation + real local signing gate |

## MCP capability surface

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

UI Automation uses **explicit HWND + semantic element paths**, not pixel coordinates.

## Permission model

```text
Safe       → baseline Allow
Moderate   → Default / Allow / Ask / Deny runtime policy
Sensitive  → desktop confirmation / Allow once
Blocked    → Deny
Unknown    → Deny
```

Sensitive approval is one-shot. Timeout, malformed policy, broker failure or missing UI response never becomes implicit Allow.

## Runtime / packaging contract

```text
<app-local-data>/
├── context/
├── models/
│   ├── whisper/
│   └── wake/
├── settings/
│   └── wake.json
├── permissions/
├── audit/
└── runtime/
    └── .agents/
        └── mcp_config.json
```

Antigravity uses `<app-local-data>/runtime` as its working directory. The desktop generates MCP configuration with an absolute path to the installed `assistant-mcp.exe` sidecar.

## Release build contract

Windows automatically merges:

```text
apps/desktop/src-tauri/tauri.conf.json
        +
apps/desktop/src-tauri/tauri.windows.conf.json
```

The Windows overlay guarantees:

```text
voice-whisper + wake-word
NSIS currentUser
icons/icon.ico
```

Public builds add one more Tauri config overlay:

```text
tauri.windows.signed.conf.json
```

which only adds the reviewed `bundle.windows.signCommand`; normal features/installer/icon settings continue to come from the Windows config.

## Reproducible release assets

Tracked icon source:

```text
apps/desktop/src-tauri/icons/app-icon.svg
apps/desktop/src-tauri/icons/icon.ico.b64
```

The binary ICO is generated locally and ignored by Git:

```text
apps/desktop/src-tauri/icons/icon.ico
```

`prepare-release.ps1` decodes the tracked payload and verifies SHA-256 before the file is accepted by the build.

Materialize assets only:

```powershell
pnpm desktop:assets:prepare
```

## Dependency lockfiles

The two dependency lockfiles must come from the real package resolvers; they are never fabricated remotely:

```text
Cargo.lock
pnpm-lock.yaml
```

Generate them locally together with release assets:

```powershell
pnpm desktop:release:prepare
```

The command runs:

```text
cargo generate-lockfile
pnpm install --lockfile-only --ignore-scripts
```

Review and commit both lockfiles before packaging.

## Release verification

Read-only local gate:

```powershell
pnpm desktop:release:verify
```

It checks, among other things:

- Windows/MSVC release environment;
- Rust toolchain pin;
- both dependency lockfiles;
- icon source/payload/materialized SHA-256;
- bundle identity and version alignment;
- full voice/wake feature selection;
- NSIS/current-user policy;
- MCP sidecar contract;
- release scripts;
- clean Git worktree.

It does not build, sign, install, download models or invoke Actions.

## Local unsigned release candidate

After committing both lockfiles:

```powershell
pnpm install --frozen-lockfile
pnpm desktop:release:build
```

The command materializes release assets, verifies readiness, stages the release MCP sidecar through the existing Tauri build hook, builds the frontend and produces the NSIS installer.

## Public signed release

The signing implementation is committed, but signing identity is not:

```text
apps/desktop/src-tauri/tauri.windows.signed.conf.json
apps/desktop/src-tauri/scripts/sign-windows.ps1
```

Set local environment variables:

```powershell
$env:ASSISTANT_WINDOWS_CERT_SHA1 = "<certificate SHA-1 thumbprint>"
$env:ASSISTANT_WINDOWS_TIMESTAMP_URL = "<certificate-provider RFC3161 timestamp URL>"
```

The certificate must be installed with its private key under:

```text
Cert:\CurrentUser\My
```

Public verification:

```powershell
pnpm desktop:release:verify:public
```

The stricter gate validates the signed overlay, thumbprint shape, timestamp URL, actual certificate/private-key availability, certificate expiry and Windows `signtool.exe` availability.

Public build:

```powershell
pnpm desktop:release:build:public
```

Tauri passes each signing target through `sign-windows.ps1`; the script uses SHA-256 signing + RFC3161 timestamping and then runs `signtool verify`.

Never commit PFX files, private keys, passwords, client secrets, signing tokens or cloud credentials.

## Windows lifecycle

- single-instance plugin is registered before other lifecycle plugins;
- normal second launch focuses the running instance;
- tray **Khởi động cùng Windows** controls native autostart;
- autostart uses fixed `--background`;
- background startup keeps the runtime/tray/wake service alive without opening the main window;
- explicit launch, tray click, `Alt + Space` or wake detection can surface the existing process.

## Local Windows preflight

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-local.ps1
```

JSON output:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-local.ps1 -Json
```

This verifier is read-only and does not build/test/start the application.

## Runtime resources

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
        └── bpe.model
```

Resource overrides must be absolute:

```text
ASSISTANT_WHISPER_MODEL
ASSISTANT_WAKE_MODEL_DIR
ASSISTANT_WAKE_KEYWORDS
```

Whisper verified install uses a backend-owned URL/size/SHA-256 manifest; frontend/model requests cannot inject arbitrary resource URLs or install destinations.

## Wake phrase lifecycle

```text
phrase
  ↓
SentencePiece + token validation
  ↓
new keywords.txt.part
  ↓
old keywords.txt → backup
  ↓
new keywords.txt → final
  ↓
load replacement detector
  ↓
success? no → rollback
  ↓ yes
persist phrase + remove backup
```

Wake preferences live in `<app-local-data>/settings/wake.json`.

## Updater policy

Automatic updater artifacts remain disabled until all of the following are defined together:

- authenticated update endpoint;
- updater signing-key lifecycle;
- release publication workflow;
- rollback/recovery policy;
- installer/update test matrix.

Manual signed releases remain the initial public distribution model.

## Context / privacy

Desktop context is collected only on demand. Screen/clipboard data are treated as untrusted context. Readiness/audit/verifier/resource output do not expose broker secrets, credentials, prompts, clipboard contents, screenshots, permission arguments or model contents.

## Development rules

1. Develop one bounded phase at a time.
2. Keep native implementation separate from transport/UI where practical.
3. Use stable typed contracts.
4. Authorize before native mutation.
5. Prefer semantic Windows/UIA APIs over raw input.
6. Fail closed for unknown tools, stale targets, broker errors and malformed policy.
7. Static-review APIs before merge.
8. Do not run GitHub Actions/tests/native builds/installers/signing/model downloads during remote development.
9. Squash-merge completed phases to `main`.
10. Update README before every phase merge.

## Next direction

After Phase 15, repository-side release preparation is complete. Remaining work is inherently local/machine-dependent:

1. resolve and review `Cargo.lock`;
2. resolve and review `pnpm-lock.yaml`;
3. compile, install and exercise the application on Windows;
4. fix any compiler/runtime/install findings;
5. provide a real trusted Windows signing identity before public distribution.

No further computer-use capability should be added until those local verification findings are resolved.

## Documentation

- [`docs/PROJECT_PLAN.md`](docs/PROJECT_PLAN.md)
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- [`docs/ANTIGRAVITY.md`](docs/ANTIGRAVITY.md)
- [`docs/MCP.md`](docs/MCP.md)
- [`docs/VOICE_DESKTOP.md`](docs/VOICE_DESKTOP.md)
- [`docs/WAKE_RUNTIME.md`](docs/WAKE_RUNTIME.md)
- [`docs/WAKE_KEYWORD_PREPARATION.md`](docs/WAKE_KEYWORD_PREPARATION.md)
- [`docs/WAKE_HOT_RELOAD.md`](docs/WAKE_HOT_RELOAD.md)
- [`docs/WINDOWS_LIFECYCLE.md`](docs/WINDOWS_LIFECYCLE.md)
- [`docs/RELEASE_CHECKLIST.md`](docs/RELEASE_CHECKLIST.md)
- [`docs/PERMISSION_GATEWAY.md`](docs/PERMISSION_GATEWAY.md)
- [`docs/RUNTIME_READINESS.md`](docs/RUNTIME_READINESS.md)
- [`docs/RUNTIME_PATHS_PACKAGING.md`](docs/RUNTIME_PATHS_PACKAGING.md)
- [`docs/LOCAL_WINDOWS_VERIFICATION.md`](docs/LOCAL_WINDOWS_VERIFICATION.md)
- [`docs/RUNTIME_RESOURCES.md`](docs/RUNTIME_RESOURCES.md)
