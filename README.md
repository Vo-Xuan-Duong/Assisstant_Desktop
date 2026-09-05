# Assisstant Desktop

Windows-first desktop AI assistant powered by **Google Antigravity CLI + Gemini + MCP + Rust/Tauri**.

The project is no longer a small prototype. The current codebase contains the complete text/voice/wake desktop pipeline, a permission-gated Windows MCP runtime, semantic UI Automation, resource management, Windows lifecycle integration, release packaging, and deterministic local read-only commands.

## Current status

- **Target:** Windows first.
- **Runtime:** Tauri 2 + React frontend with a Rust backend.
- **AI backend:** Google Antigravity CLI in headless `stream-json` mode.
- **Tool protocol:** MCP over stdio.
- **Windows integration:** Win32 / COM / UI Automation / CoreAudio / CPAL.
- **Voice:** local Whisper STT + Windows SAPI TTS.
- **Wake word:** sherpa-onnx.
- **Installer:** NSIS current-user package.
- **Safety default:** unknown, blocked, stale, malformed, or unconfirmed sensitive actions fail closed.
- **Feature status:** Phases 0–17 are integrated; current work is runtime hardening and local release validation.

The previous README stopped at Phase 14/15. `main` has since added the local Safe intent fast-path, expanded Windows system control, Antigravity settings/model management, native build preparation, and Windows CI coverage.

## Architecture

```text
Wake / Alt+Space / Mic / Text
             |
             v
      Tauri Desktop + React
             |
             v
        Assistant Core
        /           \
       /             \
Local Safe Path    Context Engine
       |             |
       |             v
       |       Antigravity Bridge
       |             |
       |             v
       |       Antigravity CLI
       |             |
       |             v
       |            MCP
       |             |
       |      Permission Gateway
       |             |
       +------> Windows Tools
                     |
                     v
          Win32 / UIA / CoreAudio
```

No frontend component calls Win32 directly. Model-facing actions go through explicit tool contracts and the permission layer.

## Workspace

```text
apps/desktop/src-tauri    Tauri desktop backend
apps/desktop              React/TypeScript UI
crates/common             Shared contracts
crates/assistant-core     State machine and request lifecycle
crates/antigravity-bridge Long-running Antigravity session
crates/context-engine     On-demand desktop context
crates/permission-broker  Authenticated local confirmation broker
crates/permission-engine  Risk and permission policy
crates/voice-runtime      Microphone, VAD, Whisper, TTS, wake runtime
crates/windows-tools      Native Windows operations
crates/windows-mcp        MCP server and permission gateway
```

## Locked stack

- Rust `1.98.1` / edition 2024.
- TypeScript + React.
- Tauri 2.
- Tokio.
- `rmcp` for the MCP server.
- `windows-rs 0.62.2`.
- CPAL/WASAPI microphone capture.
- whisper.cpp local STT.
- Windows SAPI local TTS.
- sherpa-onnx + SentencePiece wake-word stack.
- NSIS Windows installer.
- pnpm.

## Integrated development phases

| Phase | Status | Capability |
|---|---|---|
| 0 | Complete | Workspace, contracts, Assistant Core |
| 1 | Complete | Long-running Antigravity `stream-json` runtime |
| 2 | Complete | Windows MCP foundation |
| 3 | Complete | Tauri/React text desktop MVP |
| 4 | Complete | Active-window / clipboard / screen context |
| 5A–5C | Complete | Microphone, VAD, Whisper, voice turn, TTS |
| 6 | Complete | Gemini-like edge overlay UI |
| 7A–7C | Complete | Wake detector, background wake, wake-to-voice |
| 8A–8C | Complete | Semantic Windows UI Automation |
| 9A–9D | Complete | Permission policy, broker, confirmation UX, audit, overrides |
| 10A–10E | Complete | Range/Grid/ScrollItem/UIA schema/router hardening |
| 11A–11D | Complete | Virtualized UIA, window discovery/control, monitor placement |
| 12A–12C | Complete | Readiness, runtime paths, MCP packaging, local verifier |
| 13A–13D | Complete | Resource registry/install, wake keyword preparation/hot reload |
| 14A–14B | Complete | Windows lifecycle and release readiness |
| 15 | Complete | Deterministic release preparation/signing contract |
| 16A | Complete | Deterministic local Safe intent fast-path |
| 17 | Complete | Expanded Windows system control |
| Runtime hardening | In progress | Timeouts, context privacy, settings correctness, local E2E validation |

## Assistant lifecycle

Assistant Core owns the main state machine:

```text
Idle
Listening
Processing
Executing
Confirming
Speaking
Error
```

The core uses a single-flight request gate. Permission confirmation is a real lifecycle state rather than an unrelated modal state.

## Deterministic local Safe fast-path

A deliberately small set of read-only requests bypasses Antigravity and executes locally:

```text
audio_get_volume
apps_list
window_get_active
system_get_info
```

Examples:

```text
Âm lượng hiện tại bao nhiêu?
Ứng dụng nào đang chạy?
Cửa sổ active hiện tại là gì?
Máy đang dùng bao nhiêu RAM?
```

Before local execution the desktop re-validates the exact tool in `windows_tools::TOOL_CATALOG` and refuses it unless the tool is still classified `Safe`.

Mutating or ambiguous requests continue through Antigravity + MCP + permission handling.

## MCP capability surface

### Audio / applications / system

```text
audio_get_volume
audio_set_volume
audio_set_mute
apps_open
apps_list
system_get_info
media_play_pause
media_next
media_previous
clipboard_read_text
clipboard_write_text
```

### Display / windows

```text
display_list
display_turn_off
window_get_active
window_list
window_activate
window_set_bounds
window_set_state
window_close
```

### Power / process / input

```text
system_lock
system_logoff
system_shutdown
system_restart
process_terminate
input_send_hotkey
input_type_text
```

### Filesystem

```text
file_info
file_list
file_create_directory
file_copy
file_move
file_delete
```

Filesystem mutation is intentionally bounded: absolute paths are required, overwrite is disabled, recursive deletion is not exposed, filesystem roots are rejected, and symlink/junction mutation is refused.

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

UI Automation uses explicit HWND + semantic element paths rather than unrestricted pixel-coordinate automation.

## Permission model

```text
Safe       -> product baseline Allow
Moderate   -> Default / Allow / Ask / Deny runtime policy
Sensitive  -> explicit desktop confirmation / Allow once
Blocked    -> Deny
Unknown    -> Deny
```

Runtime overrides are enforced as a **Moderate-only invariant**. Safe, Sensitive, and Blocked decisions cannot be downgraded through the generic permission-policy override path.

Sensitive confirmations use an authenticated local broker:

```text
127.0.0.1 ephemeral port
+ RAM-only random session secret
+ request UUID
+ exact tool/risk/arguments
+ bounded timeout
+ AllowOnce / Deny
```

Missing broker state, malformed payloads, timeouts, stale responses, and UI failures deny execution.

Permission audit records intentionally exclude prompts, clipboard contents, screenshots, broker secrets, and credentials.

## Antigravity runtime

The desktop maintains a long-running Antigravity CLI session using:

```text
--input-format stream-json
--output-format stream-json
```

### Turn timeout

An Antigravity turn is bounded so a live CLI process that stops producing a final result cannot hold Assistant Core in `Processing` indefinitely.

Default:

```text
180 seconds
```

Optional override:

```powershell
$env:ASSISTANT_ANTIGRAVITY_TURN_TIMEOUT_SECONDS="240"
```

Accepted values are clamped to 15–1800 seconds. A timeout invalidates the session; shutdown has a short grace period and then terminates a process that does not exit.

### CLI and authentication status

The settings UI distinguishes **CLI availability** from verified account authentication.

`agy --help` only proves that the binary is runnable. It does not prove that a cached Google account session is valid. Authentication is ultimately verified when a real Antigravity agent session starts. If the CLI has no cached credentials, the normal Antigravity authentication flow is used.

The **Đăng nhập / Đổi tài khoản** action opens the normal interactive Antigravity CLI rather than extracting or managing Google credentials inside this application.

### Model selection

Available model choices come from the installed CLI:

```powershell
agy models
```

The application no longer fabricates a hard-coded fallback model catalogue when discovery fails. `Default` and `Custom model` remain available, and an invalid custom model is expected to fail loudly in Antigravity headless mode.

Persistent desktop settings are stored under app-local-data:

```text
settings/antigravity.json
```

Changing model/effort resets the active Antigravity process so the next turn starts with the new configuration.

## Desktop context and privacy

Context is collected only when the user request indicates it is needed.

Possible context sources:

- source/active-window metadata;
- clipboard text;
- active-window screenshot.

Desktop-derived text is formatted as explicitly untrusted escaped fields. Clipboard text is quoted and bounded to 16,000 characters before being included in an agent prompt. The prompt explicitly tells the model not to follow instructions embedded in desktop field values.

Screenshots use a single transient path:

```text
<app-local-data>/context/active-window.png
```

The artifact is removed when its request snapshot is dropped. Temporary PNG files are also cleaned on failed writes/renames.

This reduces persistence of sensitive screen content while keeping the local file available for the duration of the active agent turn.

## Voice pipeline

```text
Microphone
   |
   v
CPAL / WASAPI
   |
   v
VAD
   |
   v
Whisper multilingual base
   |
   v
complete_prompt()
   |
   +--> local Safe path
   |
   +--> Antigravity + MCP
   |
   v
Windows SAPI TTS
```

Whisper is configured for Vietnamese in the desktop voice turn.

The microphone callback never blocks waiting for the async consumer; bounded queues drop excess chunks instead of blocking the realtime audio callback.

## Wake word

Wake uses sherpa-onnx with local model resources and a generated `keywords.txt`.

Wake phrase lifecycle:

```text
phrase
  |
SentencePiece + token validation
  |
new keywords.txt.part
  |
old keywords.txt -> backup
  |
new file -> final
  |
load detector
  |
success -> persist phrase
failure -> rollback
```

Manual Mic and wake activation use the same backend voice-turn path.

## Runtime resources

Default app-local-data layout:

```text
<app-local-data>/
├── context/
├── models/
│   ├── whisper/
│   │   └── ggml-base.bin
│   └── wake/
├── settings/
│   ├── wake.json
│   └── antigravity.json
├── permissions/
│   └── policy.json
├── audit/
│   └── permissions.jsonl
└── runtime/
    └── .agents/
        └── mcp_config.json
```

Whisper automatic installation is backend-manifest-driven with a pinned source revision, byte size, and SHA-256.

Wake-model automatic download remains disabled until the archive checksum and redistribution terms are explicitly pinned. Wake keywords are generated locally from the installed model tokenizer/resources.

Resource path overrides must be absolute:

```text
ASSISTANT_WHISPER_MODEL
ASSISTANT_WAKE_MODEL_DIR
ASSISTANT_WAKE_KEYWORDS
ASSISTANT_RUNTIME_DIR
ASSISTANT_MCP_BINARY
```

## Windows lifecycle

- Single-instance plugin.
- Normal second launch focuses the existing process.
- `Alt + Space` global activation shortcut.
- Tray show/hide/quit controls.
- Opt-in **Khởi động cùng Windows**.
- Autostart uses `--background` and remains hidden in the tray.
- Wake/background services remain alive while the main window is hidden.

## Runtime readiness

The desktop exposes diagnostics for:

- Antigravity CLI;
- generated MCP configuration;
- bundled/dev MCP sidecar;
- permission broker and policy file;
- writable context storage;
- Windows TTS;
- Whisper resource state;
- wake resource/runtime state.

Optional voice/wake resources do not block text assistant operation.

## Development

Prerequisites on Windows:

- Node.js `^20.19.0 || >=22.12.0`;
- pnpm 10;
- Rust `1.98.1` MSVC toolchain;
- Visual Studio C++ Build Tools / Windows SDK;
- CMake;
- Antigravity CLI for AI turns.

Install dependencies:

```powershell
pnpm install --frozen-lockfile
```

Prepare native dependencies and assets:

```powershell
pnpm desktop:native:prepare
pnpm desktop:assets:prepare
```

Read-only local prerequisite check:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-local.ps1
```

Start desktop development:

```powershell
pnpm desktop:dev
```

Repository scripts also provide:

```powershell
pnpm check
pnpm test
pnpm test:models
```

`test:models` requires the relevant local model resources. It exercises native model integration with synthetic audio rather than opening the microphone.

## CI

The repository contains a Windows CI workflow for pull requests and pushes to `main`. It installs frozen pnpm dependencies, prepares native dependencies/assets, stages the MCP sidecar, checks Rust formatting, runs Rust tests, builds the frontend, checks the full desktop feature set, and verifies the release contract.

Do not manually dispatch remote workflows merely to probe the application runtime. Native microphone, wake, installer, account, and real desktop behavior still require local Windows validation.

## Release build

Windows combines:

```text
apps/desktop/src-tauri/tauri.conf.json
+
apps/desktop/src-tauri/tauri.windows.conf.json
```

The Windows overlay enables:

```text
voice-whisper
wake-word
NSIS current-user installer
icon.ico
native runtime DLL resources
```

Prepare deterministic release inputs:

```powershell
pnpm desktop:release:prepare
```

Verify an unsigned local release candidate:

```powershell
pnpm desktop:release:verify
```

Build:

```powershell
pnpm desktop:release:build
```

Public signing uses:

```text
apps/desktop/src-tauri/tauri.windows.signed.conf.json
apps/desktop/src-tauri/scripts/sign-windows.ps1
```

Required local variables:

```powershell
$env:ASSISTANT_WINDOWS_CERT_SHA1="<certificate SHA-1 thumbprint>"
$env:ASSISTANT_WINDOWS_TIMESTAMP_URL="<RFC3161 timestamp URL>"
```

Public gate/build:

```powershell
pnpm desktop:release:verify:public
pnpm desktop:release:build:public
```

Never commit PFX files, private keys, passwords, Google credentials, signing tokens, API keys, or cloud secrets.

## What is still required before a public release

The repository is feature-complete enough for local release-candidate validation, but a public release should wait until all of the following have been exercised on the target Windows machine:

1. fresh install of the NSIS package;
2. first launch and runtime readiness;
3. Antigravity login and a real multi-turn session;
4. local Safe command execution without AI;
5. MCP tool execution and permission confirmation;
6. microphone -> VAD -> Whisper -> Antigravity -> TTS;
7. wake detection -> voice turn;
8. tray, hide/show, `Alt + Space`, single-instance and autostart;
9. resource installation/keyword preparation;
10. shutdown/restart/process/file/input safety confirmations;
11. signed installer verification before public distribution.

There is currently no requirement for a Gemini API key in the primary architecture; the application uses the user's normal Antigravity CLI authentication/session.

## Development rules

1. Keep native implementation separate from transport/UI where practical.
2. Prefer explicit typed contracts.
3. Authorize before native mutation.
4. Prefer semantic Windows/UIA APIs over unrestricted raw shell automation.
5. Fail closed for unknown tools, stale targets, broker failures, malformed policy, and missing confirmation.
6. Keep screen/clipboard context on-demand and treat it as untrusted input.
7. Do not add arbitrary shell/PowerShell execution to the model-facing tool surface.
8. Keep dependency/model/release inputs deterministic and verifiable.
9. Update this README when capability or release status changes.
10. Validate native Windows behavior locally before calling a phase production-ready.

## Documentation

Detailed design notes remain under `docs/`, including:

- `docs/PROJECT_PLAN.md`
- `docs/ARCHITECTURE.md`
- `docs/ANTIGRAVITY.md`
- `docs/MCP.md`
- `docs/PERMISSION_GATEWAY.md`
- `docs/RUNTIME_PERMISSION_POLICY.md`
- `docs/LOCAL_INTENT_FAST_PATH.md`
- `docs/WINDOWS_SYSTEM_CONTROL.md`
- `docs/UI_AUTOMATION.md`
- `docs/VOICE_DESKTOP.md`
- `docs/WAKE_RUNTIME.md`
- `docs/WAKE_KEYWORD_PREPARATION.md`
- `docs/WAKE_HOT_RELOAD.md`
- `docs/RUNTIME_READINESS.md`
- `docs/RUNTIME_PATHS_PACKAGING.md`
- `docs/RUNTIME_RESOURCES.md`
- `docs/WINDOWS_LIFECYCLE.md`
- `docs/LOCAL_WINDOWS_VERIFICATION.md`
- `docs/RELEASE_CHECKLIST.md`

## License

MIT.
