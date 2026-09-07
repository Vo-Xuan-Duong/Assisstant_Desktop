# Assisstant Desktop

Windows-first AI system assistant powered by **Google Antigravity CLI + Gemini + MCP + Rust/Tauri**, with an **Android Voice Satellite** as the preferred speech-input surface.

Assisstant Desktop is designed as a background system assistant rather than a conventional chatbot window. The Windows app owns reasoning, context, permissions, tools, UI, and spoken responses. The Android companion app owns microphone capture and speech recognition, then sends only the final recognized text to the desktop.

## Current architecture

```text
                    Android Voice Satellite
                    -----------------------
                    Microphone
                       |
                    SpeechRecognizer
                    |- partial -> phone UI only
                    `- final text
                       |
                 authenticated WebSocket
                       |
                       v
+------------------------------------------------------------+
| Assisstant Desktop                                         |
|                                                            |
| Voice Satellite Adapter                                    |
|       |                                                    |
|       +----> Quick transcript UI                           |
|       |                                                    |
|       v                                                    |
| Assistant Core                                             |
|    /        \                                              |
| Context     Antigravity Bridge                             |
|                |                                           |
|          Antigravity CLI / Gemini                          |
|                |                                           |
|               MCP                                          |
|                |                                           |
|        Permission Gateway                                  |
|                |                                           |
|          Windows Tools                                     |
|                |                                           |
|       Win32 / UIA / CoreAudio                              |
|                                                            |
| Response -> VI / EN / Auto -> Windows SAPI -> speakers     |
+------------------------------------------------------------+

Fallback voice input:

Windows microphone -> CPAL/WASAPI -> VAD -> Zipformer -> Assistant Core
```

## Current status

- **Target:** Windows first, with Android companion input.
- **Background host:** Tauri 2 + Rust.
- **Interaction UI:** React QuickOverlay + four click-through edge surfaces.
- **Sensitive confirmation UI:** dedicated hidden-by-default permission surface.
- **Management:** `assistant.exe` CLI / terminal dashboard plus `assistant-satellite` pairing helper.
- **AI backend:** Google Antigravity CLI in headless `stream-json` mode.
- **Tool protocol:** MCP over stdio.
- **Windows integration:** Win32 / COM / UI Automation / CoreAudio.
- **Preferred voice input:** Android Voice Satellite using Android `SpeechRecognizer`.
- **Android recognition:** `vi-VN` / `en-US`, on-device when available, system recognizer fallback.
- **Satellite transport:** authenticated RFC 6455 WebSocket over a trusted LAN.
- **Satellite pairing:** durable local settings, generated 64-character token, enable/disable/bind/revoke, live reload.
- **Desktop fallback STT:** sherpa-onnx Vietnamese Zipformer 30M INT8.
- **Desktop TTS:** Windows SAPI.
- **Response language:** Vietnamese, English, or Auto.
- **Response style:** friendly, natural, concise; simple commands normally receive one short confirmation.
- **Desktop wake word:** sherpa-onnx remains available for the local fallback path.
- **Installer:** NSIS current-user package.
- **Safety default:** unknown, blocked, stale, malformed, or unconfirmed Sensitive actions fail closed.

The desktop runtime, Antigravity/MCP path, Quick UI, permission path, local voice fallback, Android Voice Satellite, and the first persistent pairing layer are implemented source-first. Target Windows/Android runtime validation is still required before a public release.

## Why voice recognition moved to Android

The existing local Zipformer recognizer is small and efficient, but its Vietnamese recognition quality is not sufficient to remain the preferred voice-input path for the desired assistant experience.

Instead of continuing to spend desktop CPU on a small local ASR model, the project now treats the phone as a **voice terminal**:

```text
Phone: microphone + speech-to-text
Desktop: reasoning + tools + response + TTS
```

This keeps the desktop architecture intact while allowing Android to use the speech-recognition stack already available on the device. No dedicated paid STT API is required by the project.

The Android system recognizer is device/vendor-dependent and may use an online recognition service. On Google-enabled Android devices the normal system recognizer can be backed by Google's speech-recognition service. When Android reports an on-device recognizer, the companion app can prefer that path.

## Android Voice Satellite

Source:

```text
apps/android-satellite/
```

The current MVP provides:

- Kotlin + Jetpack Compose UI;
- push-to-talk;
- microphone permission handling;
- Vietnamese `vi-VN` recognition;
- English `en-US` recognition;
- Android on-device recognizer when supported;
- fallback to the normal system recognizer;
- partial transcript shown on the phone only;
- exactly one final text command sent to the desktop;
- pairing token authentication;
- desktop processing/speaking status;
- response language selector: `VI`, `EN`, `Auto`;
- desktop response text display;
- no phone-side tool execution or TTS.

`SpeechRecognizer` is intentionally not kept running continuously. Always-on phone wake/hardware activation is a later phase.

### Desktop satellite setup

The LAN receiver is **disabled by default**. Create a pairing from the desktop:

```powershell
assistant-satellite pair
```

This creates and persists:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\settings\satellite.json
```

with a generated 64-character pairing token. The full token is printed when pairing is created; `assistant-satellite show` displays only a masked version.

Useful commands:

```powershell
assistant-satellite show
assistant-satellite pair
assistant-satellite pair --bind 0.0.0.0:8765
assistant-satellite enable
assistant-satellite disable
assistant-satellite bind 0.0.0.0:8765
assistant-satellite revoke
```

A running desktop watches the pairing file and normally applies changes within about one second. Token rotation/revoke also drops the active phone connection so an old authenticated session cannot continue.

Configure the Android app with:

```text
ws://<PC-LAN-IP>:8765
```

and the token printed by `assistant-satellite pair`.

Legacy development environment overrides remain supported:

```text
ASSISTANT_VOICE_SATELLITE_TOKEN
ASSISTANT_VOICE_SATELLITE_BIND
```

When the legacy token environment variable is present it takes precedence over the persistent pairing file for that desktop process.

The current transport is unencrypted `ws://`, so it is for a **trusted/private LAN only**. Do not expose port `8765` directly to the public Internet. QR pairing, per-device trust, stronger secret storage, and WSS/private-overlay networking remain later work.

See [`docs/VOICE_SATELLITE.md`](docs/VOICE_SATELLITE.md) for protocol and security details.

## Spoken responses

The response is generated and spoken on the Windows desktop.

Modes:

```text
VI    -> always reply in Vietnamese
EN    -> always reply in English
Auto  -> reply in the language used by the user
```

Voice-originated prompts also include a concise response policy so ordinary commands sound natural rather than like verbose CLI output.

Examples:

```text
User: "Mở Visual Studio Code"
Assistant: "Được, mình đã mở Visual Studio Code."

User: "Open Visual Studio Code"
Assistant: "Sure, I’ve opened Visual Studio Code."
```

The current implementation still uses the configured/default Windows SAPI voice instance. Automatic selection of installed Vietnamese vs English SAPI voices is planned in the next TTS phase; `VI`/`EN`/`Auto` currently controls generated response text first.

## Desktop fallback voice pipeline

Desktop-local STT is retained for offline/fallback use:

```text
Microphone
   |
CPAL / WASAPI
   |
RMS VAD + hysteresis
   |
active snapshots -> bounded partial Zipformer previews -> Quick UI only
   |
complete utterance -> final Zipformer decode
   |
Assistant Core
   |
Antigravity / MCP
   |
Windows SAPI TTS
```

The model is sherpa-onnx Vietnamese Zipformer 30M INT8 and remains an `OfflineRecognizer`; its partial UI updates are simulated by bounded snapshot decodes. Only final text enters Assistant Core.

STT resource id:

```text
stt_zipformer_vi
```

Install while the desktop runtime is running:

```powershell
assistant resources install stt_zipformer_vi
```

The current Vietnamese Zipformer model has separate upstream license terms (**CC-BY-NC-ND-4.0**) and should not be assumed suitable for commercial redistribution.

## User experience

Primary voice path:

```text
Phone
  |
Push-to-talk
  |
Android speech recognition
  |
final text
  |
Desktop Quick Overlay
  |
Assistant Core / Antigravity / MCP
  |
Windows action
  |
Desktop spoken response
```

Desktop interaction remains available through:

```text
Alt + Space
Text input
Desktop microphone fallback
Desktop wake path
```

Sensitive request:

```text
Assistant / MCP
      |
Sensitive tool
      |
Permission broker
      |
compact desktop confirmation
      |
Allow once / Deny
```

The Android satellite cannot approve Sensitive actions or bypass the desktop permission boundary.

## Workspace

```text
apps/android-satellite       Android speech-input companion
apps/desktop/src-tauri       background Tauri/Rust host + assistant.exe + assistant-satellite
apps/desktop                 Quick, edge, permission React surfaces
crates/common                shared contracts
crates/assistant-core        state machine and request lifecycle
crates/antigravity-bridge    long-running Antigravity session
crates/context-engine        on-demand desktop context
crates/permission-broker     authenticated local confirmation broker
crates/permission-engine     risk and permission policy
crates/voice-runtime         desktop mic/VAD/STT/TTS/wake fallback runtime
crates/windows-tools         native Windows operations
crates/windows-mcp           MCP server and permission gateway
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

The core uses a single-flight request gate. Permission confirmation is a real lifecycle state. Satellite commands also use a dedicated single-command gate. The pairing bootstrap currently admits one active satellite phone connection at a time so revocation and token rotation have a simple, deterministic session boundary.

## Satellite security boundary

The phone is deliberately low-authority.

It may submit only bounded text commands. It receives response/state messages, but it does not receive:

- MCP credentials;
- direct Win32 access;
- raw shell access;
- permission-broker secrets;
- Antigravity credentials;
- desktop management IPC secrets.

Current receiver rules include:

- no active pairing token -> no listener;
- generated high-entropy pairing token;
- persistent enable/disable/bind/revoke settings;
- hot reload of persistent pairing settings;
- active-session drop on token rotation/revoke;
- first-message authentication timeout;
- protocol version validation;
- masked client WebSocket frames;
- bounded WebSocket payload size;
- one active satellite phone connection in the current pairing phase;
- one satellite command at a time;
- busy rejection during another Assistant turn;
- existing Sensitive confirmation remains authoritative.

The current token is a local credential stored in application data and Android app preferences. DPAPI/Android Keystore-backed storage and per-device credentials are planned hardening work.

## Deterministic local Safe fast-path

The desktop text/local voice path includes a small read-only deterministic set:

```text
audio_get_volume
apps_list
window_get_active
system_get_info
```

Mutating or ambiguous requests continue through Antigravity + MCP + permission handling.

The satellite adapter currently sends satellite commands through the normal Assistant Core/Antigravity path so the friendly `VI`/`EN`/`Auto` response policy is consistently applied. Fast-path unification for satellite turns can follow once response synthesis is separated cleanly from tool routing.

## Permission model

```text
Safe       -> baseline Allow
Moderate   -> Allow / Ask / Deny runtime override
Sensitive  -> explicit Allow once confirmation
Blocked    -> Deny
Unknown    -> Deny
```

Runtime overrides cannot weaken Safe/Sensitive/Blocked invariants. Sensitive confirmations remain desktop-owned and fail closed.

## CLI management

Running without a subcommand opens the terminal dashboard:

```powershell
assistant
```

Useful commands include:

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
assistant logs --follow
```

Satellite pairing currently has a dedicated helper:

```powershell
assistant-satellite show
assistant-satellite pair
assistant-satellite enable
assistant-satellite disable
assistant-satellite bind 0.0.0.0:8765
assistant-satellite revoke
```

Folding these commands under `assistant satellite ...`, adding QR pairing, and adding trusted-device/last-seen diagnostics remain Phase 26 follow-up work.

## Development

### Windows

Prerequisites:

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

Prepare native dependencies/assets when required:

```powershell
pnpm desktop:native:prepare
pnpm desktop:assets:prepare
```

Start desktop development:

```powershell
pnpm desktop:dev
```

### Android

Open:

```text
apps/android-satellite/
```

in Android Studio, sync Gradle, and install the app on the target phone.

The current source-first Android phase does not include the Gradle wrapper JAR/binary. Generate a local wrapper if command-line Android builds are needed.

See [`apps/android-satellite/README.md`](apps/android-satellite/README.md).

## Local validation gates

No remote GitHub Actions/build/test/model-download/installer run is implied by this migration. Validate on the target Windows PC and Android phone.

Satellite checks:

- phone and PC are on the same trusted LAN;
- `assistant-satellite pair` creates a token/settings file;
- running desktop hot-reloads the new pairing without restart;
- Android pairs with the correct token;
- invalid token is rejected;
- microphone permission flow works;
- `vi-VN` recognizes a command and sends exactly one final transcript;
- `en-US` recognizes an English command;
- partial recognition stays phone-side;
- Quick shows the final satellite transcript;
- normal MCP/permission path executes the Windows action;
- desktop speaks the response;
- `VI`, `EN`, and `Auto` response modes behave as expected;
- busy state is rejected cleanly;
- disconnect/reconnect works;
- `assistant-satellite disable` stops the listener through hot reload;
- `assistant-satellite revoke` disconnects an active phone and rejects the old token;
- re-pairing with a fresh token works without desktop restart;
- Windows Firewall is limited to the intended private/trusted profile;
- local Zipformer microphone path remains usable as fallback.

Existing desktop checks should still cover hotkey, Quick/edge UI, permission Allow/Deny/timeout, Antigravity login/session reset, startup behavior, Windows tools, logs, and NSIS packaging.

## Roadmap

The current roadmap is maintained in [`docs/PROJECT_PLAN.md`](docs/PROJECT_PLAN.md).

Phase 26 pairing work currently has durable settings, token generation, enable/disable/bind/revoke, and hot reload. Remaining pairing work is QR setup, integration into the main `assistant` CLI, trusted-device/last-seen diagnostics, firewall UX, and stronger secret-at-rest handling.

Following phases are:

1. locale-aware SAPI voice selection for Vietnamese/English;
2. phone wake/quick/hardware activation without continuous SpeechRecognizer looping;
3. voice quality/retry/duplicate-command resilience;
4. secure remote satellite transport (WSS/private overlay such as Tailscale);
5. later full-duplex/barge-in conversational UX.

## Key documentation

- [`docs/PROJECT_PLAN.md`](docs/PROJECT_PLAN.md)
- [`docs/VOICE_SATELLITE.md`](docs/VOICE_SATELLITE.md)
- [`apps/android-satellite/README.md`](apps/android-satellite/README.md)
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

See the repository license for application source terms. Runtime/model resources may have separate upstream licenses. The current fallback Vietnamese Zipformer STT model is CC-BY-NC-ND-4.0.
