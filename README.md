# Assisstant Desktop

Windows-first AI assistant powered by **Google Antigravity CLI + Gemini + MCP + Rust/Tauri**.

The product is designed as a background system assistant rather than a conventional chatbot window. Daily interaction uses a compact Gemini-style overlay and perimeter glow; the existing full desktop surface is retained for management and sensitive confirmations while the project moves toward a CLI/TUI management experience.

## Current status

- **Target:** Windows first.
- **Runtime:** Tauri 2 + Rust backend + React overlay/management frontend.
- **AI backend:** Google Antigravity CLI in headless `stream-json` mode.
- **Tool protocol:** MCP over stdio.
- **Windows integration:** Win32 / COM / UI Automation / CoreAudio / CPAL.
- **Primary STT:** sherpa-onnx Vietnamese Zipformer 30M INT8.
- **TTS:** Windows SAPI.
- **Wake word:** sherpa-onnx.
- **Installer:** NSIS current-user package.
- **Safety default:** unknown, blocked, stale, malformed, or unconfirmed sensitive actions fail closed.

The repository is at a late beta / technical release-candidate stage. Text, tool, voice, wake, permission, overlay, Windows lifecycle, and release packaging are integrated; target-Windows runtime validation is still required before a public release.

## User interaction

Normal assistant invocation does not open the full desktop UI.

```text
Current application
      |
Alt + Space / Wake
      |
      +--> perimeter edge glow
      |
      +--> compact bottom overlay
              |
              +-- text input
              +-- microphone
              +-- short response
              +-- expand when needed
```

The full application remains available for advanced settings, resource management, diagnostics, and sensitive permission confirmation.

## Architecture

```text
Wake / Alt+Space / Mic / Text
             |
             v
      Quick Tauri Overlay
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

No React component calls Win32 directly. Model-facing mutations go through explicit MCP tool contracts and the permission layer.

## Workspace

```text
apps/desktop/src-tauri    Tauri desktop/backend shell
apps/desktop              React quick overlay + management UI
crates/common             Shared contracts
crates/assistant-core     State machine and request lifecycle
crates/antigravity-bridge Long-running Antigravity session
crates/context-engine     On-demand desktop context
crates/permission-broker  Authenticated local confirmation broker
crates/permission-engine  Risk and permission policy
crates/voice-runtime      Microphone, VAD, STT, TTS, wake runtime
crates/windows-tools      Native Windows operations
crates/windows-mcp        MCP server and permission gateway
```

## Locked stack

- Rust `1.98.1` / edition 2024.
- TypeScript + React.
- Tauri 2.
- Tokio.
- `rmcp` for MCP.
- `windows-rs 0.62.2`.
- CPAL/WASAPI microphone capture.
- sherpa-onnx `1.13.7` for Vietnamese STT and wake-word native inference.
- Windows SAPI local TTS.
- SentencePiece for wake keyword preparation.
- NSIS Windows installer.
- pnpm.

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

## Vietnamese voice pipeline

The normal desktop build no longer uses Whisper as its primary recognizer.

```text
Microphone
   |
   v
CPAL / WASAPI
   |
   v
UtteranceSegmenter / VAD
   |
   v
Vietnamese Zipformer 30M INT8
sherpa-onnx OfflineRecognizer
   |
   v
final transcript
   |
   v
complete_prompt()
   |
   +--> deterministic local Safe path
   |
   +--> Antigravity + MCP
   |
   v
Windows SAPI TTS
```

The current migration replaces the recognizer while preserving the existing utterance boundary: recognition starts after VAD has completed one utterance. Partial/streaming transcript events are not implemented yet.

Successful primary transcripts report:

```text
sherpa-onnx/zipformer-vi-30m-int8
```

The microphone callback never blocks waiting for the async consumer; bounded queues drop excess chunks instead of blocking the realtime audio callback.

### Model

Runtime resource id:

```text
stt_zipformer_vi
```

Model family:

```text
sherpa-onnx-zipformer-vi-30M-int8-2026-02-09
```

Required runtime files:

```text
encoder.int8.onnx
decoder.onnx
joiner.int8.onnx
tokens.txt
```

The installer also stores `bpe.model` for future contextual-biasing work.

Default location:

```text
<app-local-data>/models/stt/
  sherpa-onnx-zipformer-vi-30M-int8-2026-02-09/
```

Absolute override:

```text
ASSISTANT_ZIPFORMER_MODEL_DIR
```

### Verified multi-file installation

Resource Setup installs the STT model as a transaction:

1. create an adjacent staging directory;
2. download from an immutable model revision;
3. verify exact size and SHA-256 for encoder, decoder, joiner, and `bpe.model`;
4. download `tokens.txt` under a strict size bound;
5. validate its 2000 sequential token ids and expected special tokens;
6. atomically promote the complete staging directory;
7. delete staging data if any step fails.

The installer refuses to overwrite a non-empty model directory.

### Model license

The selected Vietnamese model is licensed **CC-BY-NC-ND-4.0**. It is downloaded at runtime and is not bundled into the application installer. Runtime download does not remove the upstream non-commercial/no-derivatives restrictions. A commercial distribution must select a model with suitable commercial terms.

### Feature compatibility

The preferred desktop feature name for new commands is:

```text
voice-stt
```

The historical feature name remains valid:

```text
voice-whisper
```

Both routes compile the same Zipformer desktop STT path. The historical name is retained only because the current Tauri lifecycle and release configuration still use it. Normal desktop builds do **not** enable `whisper-rs`.

`voice-runtime/whisper` remains an explicit legacy backend for migration/testing only.

See [`docs/VOICE_STT.md`](docs/VOICE_STT.md) for the detailed STT contract.

## Wake word

Wake uses sherpa-onnx with local model resources and a generated `keywords.txt`.

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

Manual Mic and wake activation reuse the same backend voice-turn path.

Wake-model automatic download remains disabled until archive checksum and redistribution terms are explicitly pinned.

## Deterministic local Safe fast-path

A deliberately small set of read-only requests bypasses Antigravity and executes locally:

```text
audio_get_volume
apps_list
window_get_active
system_get_info
```

Before execution the desktop re-validates the exact tool in `windows_tools::TOOL_CATALOG` and refuses it unless the tool is still classified `Safe`.

Mutating or ambiguous requests continue through Antigravity + MCP + permission handling.

## MCP capability surface

Main tool families include:

```text
audio / media
applications
system information and power
clipboard
display and windows
process
keyboard/text input
filesystem
semantic Windows UI Automation
```

Filesystem mutations are intentionally bounded: absolute paths are required, overwrite is disabled, recursive deletion is not exposed, filesystem roots are rejected, and symlink/junction mutation is refused.

UI Automation uses explicit HWND + semantic element paths rather than unrestricted pixel-coordinate automation.

## Permission model

```text
Safe       -> baseline Allow
Moderate   -> Default / Allow / Ask / Deny runtime policy
Sensitive  -> explicit confirmation / Allow once
Blocked    -> Deny
Unknown    -> Deny
```

Runtime overrides are enforced as a **Moderate-only invariant**. Safe, Sensitive, and Blocked decisions cannot be downgraded through the generic override path.

Sensitive confirmations use an authenticated local broker with an ephemeral loopback port, RAM-only secret, request UUID, exact tool/risk/arguments, bounded timeout, and fail-closed behavior.

Permission audit records intentionally exclude prompts, clipboard contents, screenshots, broker secrets, and credentials.

## Antigravity runtime

The desktop maintains a long-running Antigravity CLI session using:

```text
--input-format stream-json
--output-format stream-json
```

Turns are bounded by `ASSISTANT_ANTIGRAVITY_TURN_TIMEOUT_SECONDS` (default 180 seconds, clamped to 15–1800). A timeout invalidates the session so a stalled CLI process cannot leave Assistant Core permanently busy.

Model discovery comes from:

```powershell
agy models
```

The application does not fabricate a hard-coded model catalogue when discovery fails.

`agy --help` is treated only as CLI availability, not proof that a Google account session is valid. Account authentication is verified when a real Antigravity session starts.

## Desktop context and privacy

Context is collected only when the request indicates it is needed. Possible sources are source-window metadata, clipboard text, and an active-window screenshot.

Desktop-derived text is escaped and explicitly marked as untrusted. Clipboard context is bounded. Screenshots use a transient app-local-data artifact that is removed when the request snapshot is dropped.

The external foreground HWND is captured before the quick overlay takes focus, so active-window/context requests still refer to the application the user was using when the assistant was invoked.

## Runtime resources

Default layout:

```text
<app-local-data>/
├── context/
├── models/
│   ├── stt/
│   │   └── sherpa-onnx-zipformer-vi-30M-int8-2026-02-09/
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

Relevant absolute-path overrides:

```text
ASSISTANT_ZIPFORMER_MODEL_DIR
ASSISTANT_WAKE_MODEL_DIR
ASSISTANT_WAKE_KEYWORDS
ASSISTANT_RUNTIME_DIR
ASSISTANT_MCP_BINARY
```

## Windows lifecycle

- single instance;
- `Alt + Space` quick assistant activation;
- compact always-on-top quick overlay;
- click-through perimeter edge glow;
- tray show/hide/quit controls;
- opt-in Windows autostart;
- autostart background mode;
- wake/background services remain alive while the management window is hidden.

## Development

Prerequisites on Windows:

- Node.js `^20.19.0 || >=22.12.0`;
- pnpm 10;
- Rust `1.98.1` MSVC toolchain;
- Visual Studio C++ Build Tools / Windows SDK;
- CMake;
- Antigravity CLI.

Install dependencies:

```powershell
pnpm install --frozen-lockfile
```

Prepare native dependencies and assets:

```powershell
pnpm desktop:native:prepare
pnpm desktop:assets:prepare
```

Start desktop development:

```powershell
pnpm desktop:dev
```

Available validation scripts include:

```powershell
pnpm check
pnpm test
pnpm test:models
```

`test:models` uses installed local Zipformer + wake resources and synthetic audio; it does not open the microphone.

## CI and local verification

The repository contains Windows CI for pull requests and pushes to `main`. Do not manually dispatch remote workflows merely to probe native runtime behavior.

Native microphone quality, STT accuracy/latency, wake-word behavior, installer download, native DLL loading, Antigravity account state, and real desktop automation must be validated on the target Windows machine.

No manual test, build, model download, installer run, or workflow dispatch is implied by a source-only merge.

## Release

Windows release configuration currently retains the historical `voice-whisper` feature name for compatibility; that feature now resolves to Zipformer STT rather than Whisper.

Prepare release inputs:

```powershell
pnpm desktop:release:prepare
```

Verify release contract:

```powershell
pnpm desktop:release:verify
```

Build:

```powershell
pnpm desktop:release:build
```

A public release should wait until the current target-Windows STT/wake/overlay/permission/install path has been exercised locally.

## Key documentation

- [`docs/VOICE_STT.md`](docs/VOICE_STT.md)
- [`docs/VOICE_DESKTOP.md`](docs/VOICE_DESKTOP.md)
- [`docs/WAKE_RUNTIME.md`](docs/WAKE_RUNTIME.md)
- [`docs/RUNTIME_RESOURCES.md`](docs/RUNTIME_RESOURCES.md)
- [`docs/RUNTIME_READINESS.md`](docs/RUNTIME_READINESS.md)
- [`docs/PERMISSION_GATEWAY.md`](docs/PERMISSION_GATEWAY.md)
- [`docs/EDGE_UI.md`](docs/EDGE_UI.md)
- [`docs/RELEASE_CHECKLIST.md`](docs/RELEASE_CHECKLIST.md)

## License

See the repository license for application source terms. Runtime/model resources may have separate upstream licenses; in particular, the current Vietnamese Zipformer STT model is CC-BY-NC-ND-4.0.
