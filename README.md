# Assisstant Desktop

Windows-first AI assistant powered by **Google Antigravity CLI + Gemini + MCP + Rust/Tauri**.

Assisstant Desktop is designed as a background system assistant, not a conventional chatbot window. Normal graphical interaction is intentionally limited to a Gemini-style quick overlay, perimeter glow, and Sensitive permission confirmation. Configuration, diagnostics, resources, permissions, runtime control, and logs are managed through `assistant.exe`.

## Current status

- **Target:** Windows first.
- **Background host:** Tauri 2 + Rust.
- **Interaction UI:** React QuickOverlay + four click-through edge surfaces.
- **Sensitive confirmation UI:** dedicated hidden-by-default permission surface.
- **Management:** `assistant.exe` CLI / terminal dashboard.
- **AI backend:** Google Antigravity CLI in headless `stream-json` mode.
- **Tool protocol:** MCP over stdio.
- **Windows integration:** Win32 / COM / UI Automation / CoreAudio / CPAL.
- **Primary STT:** sherpa-onnx Vietnamese Zipformer 30M INT8.
- **TTS:** Windows SAPI.
- **Wake word:** sherpa-onnx.
- **Installer:** NSIS current-user package.
- **Safety default:** unknown, blocked, stale, malformed, or unconfirmed Sensitive actions fail closed.

The project is at a late beta / technical release-candidate stage. The major runtime, overlay, CLI-management, voice, wake, MCP, permission, and packaging paths are implemented in source. Target-Windows compile/runtime validation is still required before a public release.

## User experience

Normal use:

```text
Current Windows application
          |
   Alt + Space / Wake
          |
          +------> Edge Glow
          |
          +------> Quick Assistant
                       |
                       +-- text input
                       +-- microphone
                       +-- short response
```

Sensitive tool request:

```text
Assistant / MCP
      |
Sensitive tool
      |
      v
Permission broker
      |
      v
compact confirmation window
      |
Allow once / Deny
```

Management:

```powershell
assistant
assistant status
assistant doctor
assistant logs --follow
```

The old full React chat/settings application is no longer mounted and its retired management source has been removed. The remaining frontend source is intentionally limited to QuickOverlay, EdgeOverlay, PermissionSurface, and their shared contracts/styles.

## Architecture

```text
Wake / Alt+Space / Mic / Text
             |
             v
        Quick Overlay
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
       |       Antigravity CLI
       |             |
       |            MCP
       |             |
       |      Permission Gateway
       |             |
       +------> Windows Tools
                     |
                     v
          Win32 / UIA / CoreAudio
```

Management is a separate surface over the same background runtime:

```text
assistant.exe
     |
     +-- durable settings / policy
     |
     +-- authenticated management protocol
              |
              v
       background Tauri runtime
```

The management endpoint is raw JSON over an ephemeral `127.0.0.1` port with a per-runtime 256-bit secret. It is not an HTTP/LAN service and does not expose Windows MCP tools directly.

## Workspace

```text
apps/desktop/src-tauri    background Tauri/Rust host + assistant.exe
apps/desktop              Quick, edge, permission React surfaces
crates/common             shared contracts
crates/assistant-core     state machine and request lifecycle
crates/antigravity-bridge long-running Antigravity session
crates/context-engine     on-demand desktop context
crates/permission-broker  authenticated local confirmation broker
crates/permission-engine  risk and permission policy
crates/voice-runtime      microphone, VAD, STT, TTS, wake runtime
crates/windows-tools      native Windows operations
crates/windows-mcp        MCP server and permission gateway
```

## Assistant lifecycle

Assistant Core owns:

```text
Idle
Listening
Processing
Executing
Confirming
Speaking
Error
```

The core uses a single-flight request gate. Permission confirmation is a real lifecycle state.

## Vietnamese voice pipeline

The normal desktop build no longer uses Whisper as its primary recognizer.

```text
Microphone
   |
CPAL / WASAPI
   |
UtteranceSegmenter / VAD
   |
   +---- active snapshots ----> throttled Zipformer decode
   |                               |
   |                               +--> voice:transcript (UI only)
   |
   +---- complete utterance ---> final Zipformer decode
                                   |
                              final transcript
                                   |
                            complete_prompt()
                                   |
                   +---------------+---------------+
                   |                               |
        deterministic local Safe path        Antigravity + MCP
                                                   |
                                           Windows SAPI TTS
```

The bundled Vietnamese model is an offline recognizer. During Listening, the desktop now performs bounded simulated-streaming preview decodes from VAD snapshots so Quick can show `Đang nhận dạng: ...`. These partial results never enter Assistant Core. When VAD closes the utterance, the full audio is decoded again, Quick receives `Bạn nói: ...`, and only that final transcript is submitted to the assistant.

Partial snapshot submission begins after roughly 650 ms of active audio and is throttled to roughly one request every 850 ms. Native offline decode calls are serialized so partial and final recognition do not race the same sherpa-onnx recognizer. True online ASR remains a future model/runtime migration rather than being emulated at the Assistant Core boundary.

STT resource id:

```text
stt_zipformer_vi
```

Required runtime files:

```text
encoder.int8.onnx
decoder.onnx
joiner.int8.onnx
tokens.txt
```

The installer also retains `bpe.model` for model preparation/context work.

Install from terminal while the background runtime is running:

```powershell
assistant resources install stt_zipformer_vi
```

The command reuses the same verified `ResourceInstaller` as the native runtime; there is no second CLI downloader. The transaction uses an immutable model revision, pinned file sizes/SHA-256 where applicable, bounded `tokens.txt` validation, staging cleanup, and atomic promotion.

The selected Vietnamese model is **CC-BY-NC-ND-4.0**. A commercial distribution must use a model with suitable terms.

## Wake word

Wake uses sherpa-onnx. QuickOverlay is created for the lifetime of the background application even while hidden, so it owns wake-triggered voice turns.

```text
wake detected
   |
show QuickOverlay
   |
wait 180 ms
   |
assistant_voice_turn
   |
Zipformer STT -> Assistant Core -> TTS
```

Change the active phrase through the terminal:

```powershell
assistant wake phrase "HEY ASSISTANT"
```

The command performs SentencePiece encoding, validates tokens, stages `keywords.txt`, validates the native Sherpa detector, rolls back on failure, and only persists the phrase after successful hot reload.

Wake-model automatic download remains disabled until archive checksum and redistribution/license terms are explicitly pinned.

## Deterministic local Safe fast-path

A small read-only set bypasses Antigravity:

```text
audio_get_volume
apps_list
window_get_active
system_get_info
```

The desktop re-validates every local mapping against `windows_tools::TOOL_CATALOG` and refuses execution unless the tool remains `Safe`.

Mutating or ambiguous requests continue through Antigravity + MCP + permission handling.

## Permission model

```text
Safe       -> baseline Allow
Moderate   -> Allow / Ask / Deny runtime override
Sensitive  -> explicit Allow once confirmation
Blocked    -> Deny
Unknown    -> Deny
```

Runtime overrides are a Moderate-only invariant. Safe, Sensitive, and Blocked baselines cannot be weakened through the generic override path.

Sensitive confirmations use an authenticated local broker with an ephemeral loopback endpoint, RAM-only secret, request UUID, exact tool/risk/arguments, bounded timeout, and fail-closed behavior.

The normal `main` Tauri window is now a hidden permission-only host. It no longer mounts the full management application.

## CLI management

Running without a subcommand opens the current terminal dashboard:

```powershell
assistant
```

Important commands:

```powershell
assistant status [--json]
assistant doctor
assistant paths

assistant runtime status [--json]
assistant runtime ping
assistant runtime restart

assistant conversation reset

assistant startup show
assistant startup enable
assistant startup disable

assistant overlay show
assistant overlay hide

assistant ai show
assistant ai models
assistant ai login
assistant ai set --model <id>
assistant ai set --effort <value>
assistant ai reset

assistant wake show
assistant wake enable
assistant wake disable
assistant wake phrase <text>

assistant resources list
assistant resources install stt_zipformer_vi

assistant permissions list
assistant permissions set <tool> <allow|ask|deny>
assistant permissions clear <tool>

assistant logs
assistant logs --lines 300
assistant logs --follow
```

AI/wake configuration uses live authenticated management IPC when the background process is running and durable settings fallback where safe when it is offline. Antigravity login launch, conversation reset, and Windows autostart mutations are live-runtime operations and intentionally require the background process.

`assistant startup ...` is the canonical terminal-first autostart management path. The existing tray autostart checkbox remains temporarily for compatibility; its checkmark is initialized when the desktop process starts and does not yet live-refresh after an autostart change made through the CLI. This legacy duplicate control should be removed after local Windows verification.

## Persistent logs

Default Windows files:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\logs\assistant.log
%LOCALAPPDATA%\com.voduong.assisstantdesktop\logs\assistant.log.1
```

The active log is bounded to roughly 5 MiB and one backup is retained. `assistant logs --follow` tracks the active file across normal truncation/rotation.

Logs are local diagnostics but may contain operational errors, local paths, application/tool names, or transcript-related runtime details. Review/redact them before sharing.

## Desktop context and privacy

Context is request-driven. Possible sources are source-window metadata, clipboard text, and an active-window screenshot.

Desktop-derived text is escaped and marked untrusted. Clipboard context is bounded. Screenshot artifacts are transient and removed when their request snapshot is dropped.

The external foreground HWND is captured before Quick takes focus, so active-window/context requests still refer to the application the user was using when the Assistant was invoked.

## Runtime data layout

```text
<app-local-data>/
├── context/
├── models/
│   ├── stt/
│   └── wake/
├── settings/
│   ├── wake.json
│   └── antigravity.json
├── permissions/
│   └── policy.json
├── audit/
│   └── permissions.jsonl
├── logs/
│   ├── assistant.log
│   └── assistant.log.1
└── runtime/
    ├── management.json
    └── .agents/
        └── mcp_config.json
```

## Windows lifecycle

- single instance;
- `Alt + Space` toggles Quick Assistant;
- wake shows Quick and starts one voice turn;
- compact always-on-top quick overlay;
- click-through perimeter edge glow;
- permission-only main window hidden by default;
- tray show/hide/background services;
- Windows autostart managed through the authenticated terminal management path;
- authenticated local management channel;
- persistent bounded runtime log.

## Development

Windows prerequisites:

- Node.js `^20.19.0 || >=22.12.0`;
- pnpm 10;
- Rust `1.98.1` MSVC toolchain;
- Visual Studio C++ Build Tools / Windows SDK;
- CMake;
- Antigravity CLI.

Install:

```powershell
pnpm install --frozen-lockfile
```

Prepare native dependencies/assets:

```powershell
pnpm desktop:native:prepare
pnpm desktop:assets:prepare
```

Start development:

```powershell
pnpm desktop:dev
```

Validation scripts already present include:

```powershell
pnpm check
pnpm test
pnpm test:models
```

`test:models` uses installed local native models/synthetic audio and does not open the microphone.

## Local validation gates

Recent CLI, logging, Zipformer, wake, Quick voice, and UI-retirement changes were prepared source-first. Before calling the current code release-ready, verify on the target Windows machine:

```powershell
cargo build -p assisstant-desktop --bin assistant --locked
cargo build -p assisstant-desktop --features voice-stt,wake-word --locked

assistant status
assistant doctor
assistant runtime ping
assistant runtime status
assistant conversation reset
assistant startup show
assistant startup enable
assistant startup disable
assistant overlay show
assistant overlay hide
assistant ai login
assistant logs --follow
```

Also exercise:

- `Alt + Space` on the source monitor;
- manual Mic turn;
- partial `Đang nhận dạng:` updates during a 2–3 second utterance;
- final `Bạn nói:` transcript before the Assistant response completes;
- exactly one Assistant request per voice turn despite partial transcript events;
- wake -> exactly one voice turn;
- Vietnamese STT quality/latency;
- Sensitive Deny / Allow Once / timeout / queued requests;
- permission surface auto-hide and edge cleanup;
- STT model install from CLI;
- wake phrase validation/hot reload;
- Antigravity login/account/session reset behavior;
- autostart enable/disable and background startup after sign-in;
- real Windows automation tools;
- NSIS packaging and installed `assistant.exe`.

No manual build/test/model download/installer/workflow run is implied by the source merges performed during this migration.

## Release

Windows release config still retains the historical `voice-whisper` feature name as a compatibility alias; it resolves to Zipformer STT rather than whisper-rs.

```powershell
pnpm desktop:release:prepare
pnpm desktop:release:verify
pnpm desktop:release:build
```

A public release should wait until the current target-Windows STT/wake/quick/permission/CLI/install/logging path has been exercised locally.

## Key documentation

- [`docs/CLI_MANAGEMENT.md`](docs/CLI_MANAGEMENT.md)
- [`docs/EDGE_UI.md`](docs/EDGE_UI.md)
- [`docs/VOICE_STT.md`](docs/VOICE_STT.md)
- [`docs/VOICE_DESKTOP.md`](docs/VOICE_DESKTOP.md)
- [`docs/WAKE_RUNTIME.md`](docs/WAKE_RUNTIME.md)
- [`docs/RUNTIME_RESOURCES.md`](docs/RUNTIME_RESOURCES.md)
- [`docs/RUNTIME_READINESS.md`](docs/RUNTIME_READINESS.md)
- [`docs/PERMISSION_GATEWAY.md`](docs/PERMISSION_GATEWAY.md)
- [`docs/RELEASE_CHECKLIST.md`](docs/RELEASE_CHECKLIST.md)

## License

See the repository license for application source terms. Runtime/model resources may have separate upstream licenses; the current Vietnamese Zipformer STT model is CC-BY-NC-ND-4.0.