# Assisstant Desktop

Windows-first AI system assistant powered by **Google Antigravity CLI + Gemini + MCP + Rust/Tauri**, with an **Android Voice Satellite** as the preferred speech-input surface.

Assisstant Desktop is designed as a background system assistant rather than a conventional chatbot window. Windows owns reasoning, context, permissions, tools, UI, and spoken responses. Android owns microphone capture and speech recognition and sends only the final recognized text to the desktop.

## Architecture

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

- **Target:** Windows first with Android companion voice input.
- **Background host:** Tauri 2 + Rust.
- **Interaction UI:** React Quick Overlay + click-through edge surfaces.
- **Sensitive confirmation:** desktop-owned permission surface.
- **AI backend:** Google Antigravity CLI in headless `stream-json` mode.
- **Tool protocol:** MCP over stdio.
- **Windows integration:** Win32 / COM / UI Automation / CoreAudio.
- **Preferred voice input:** Android `SpeechRecognizer`.
- **Recognition languages:** `vi-VN` and `en-US`.
- **On-device speech:** preferred when Android reports support; otherwise system recognizer fallback.
- **Satellite transport:** authenticated RFC 6455 WebSocket on a trusted/private LAN.
- **Pairing:** persistent settings + local QR/deep-link flow + trusted devices + per-device revoke.
- **Android token storage:** Android Keystore AES-GCM protection at rest.
- **Desktop fallback STT:** sherpa-onnx Vietnamese Zipformer 30M INT8.
- **Desktop TTS:** Windows SAPI with locale-aware Vietnamese/English language selection.
- **Response language:** `VI`, `EN`, `Auto`.
- **Response style:** friendly, natural, concise.
- **Desktop wake:** sherpa-onnx wake runtime remains available for the fallback path.
- **Installer:** NSIS current-user package.
- **Safety:** unknown, blocked, stale, malformed, or unconfirmed Sensitive actions fail closed.

Source implementation is ahead of target-device validation. Windows/Android runtime validation is still required before public-release readiness is claimed.

## Why speech recognition moved to Android

The local Vietnamese Zipformer recognizer is small and efficient, but its recognition quality is not sufficient to remain the preferred input path for the desired assistant experience.

The project therefore treats Android as a **voice terminal**:

```text
Phone   = microphone + speech-to-text
Desktop = reasoning + tools + response + TTS
```

This avoids running a larger ASR model on the laptop and allows Android to use the speech-recognition stack already present on the phone. No dedicated paid STT API is required.

The system recognizer is vendor-dependent. On Google-enabled Android devices it may be backed by Google's speech-recognition service and may require Internet. When Android exposes an on-device recognizer, the companion app can prefer that path.

## Android Voice Satellite

Source:

```text
apps/android-satellite/
```

Current capabilities:

- Kotlin + Jetpack Compose;
- Android 8.0+;
- push-to-talk;
- `vi-VN` / `en-US` recognition;
- Android on-device recognizer when available;
- system recognizer fallback;
- partial transcript kept phone-side;
- one final text command submitted to Windows;
- local QR/deep-link pairing;
- stable installation `device_id`;
- trusted/revoked device management;
- pairing token encrypted at rest using Android Keystore + AES-GCM;
- desktop processing/speaking status;
- `VI`, `EN`, `Auto` response selector;
- no Android-side Windows tool execution;
- no Android-side TTS.

`SpeechRecognizer` is deliberately not used as an always-listening loop. Low-power wake/hardware activation is a later phase.

## Pair a phone

The desktop LAN listener is disabled until pairing is configured.

### Recommended: local QR

Run:

```powershell
assistant satellite pair --qr
```

If the PC has multiple network adapters or the detected address is not reachable from the phone:

```powershell
assistant satellite pair --qr --host 192.168.1.20
```

The QR is generated **locally inside the terminal**. The token is not uploaded to a QR website/service.

The QR contains a compact deep link:

```text
assd://p?h=<host>&p=<port>&t=<token>
```

Android validates the imported host, port, and token. Scanning the QR only imports pairing data; it **does not automatically connect**. The user must still tap **Kết nối**.

### Manual fallback

```powershell
assistant satellite pair
```

Then enter the printed token and:

```text
ws://<PC-LAN-IP>:8765
```

in the Android app.

Persistent desktop settings live under:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\settings\
```

The listener normally hot-reloads pairing/config changes in about one second.

## Satellite management

Use the canonical CLI:

```powershell
assistant satellite show
assistant satellite pair
assistant satellite pair --qr
assistant satellite pair --qr --host <PC-LAN-IP>
assistant satellite enable
assistant satellite disable
assistant satellite bind 0.0.0.0:8765
assistant satellite revoke
assistant satellite devices
assistant satellite revoke-device <device-id>
assistant satellite allow-device <device-id>
```

`assistant-satellite ...` remains as a compatibility helper, but new documentation and workflows should use `assistant satellite ...`.

Windows packaging stages:

```text
assistant.exe             canonical CLI router
assistant-core.exe        existing desktop management CLI
assistant-satellite.exe   satellite management implementation
assistant-mcp.exe         Windows MCP sidecar
```

## Trusted-device model

After the shared bootstrap token is verified, Windows records the Android installation identity in:

```text
satellite-devices.json
```

Per-device revocation uses independent marker files so a concurrent `last_seen` registry write cannot undo a revoke.

Examples:

```powershell
assistant satellite devices
assistant satellite revoke-device <device-id>
assistant satellite allow-device <device-id>
```

A connected revoked device is rechecked by the desktop and disconnected without rotating the shared token for other devices.

## Pairing-token storage on Android

New pairing tokens are not intentionally persisted as plaintext in the normal Android preferences file.

The app:

1. creates an AES key through `AndroidKeyStore`;
2. prefers AES-256 and falls back to AES-128 where required;
3. encrypts the token with `AES/GCM/NoPadding`;
4. stores only IV + ciphertext in private preferences;
5. migrates old plaintext tokens conservatively;
6. deletes legacy plaintext only after encrypted persistence succeeds.

If the Keystore secret becomes unreadable after restore/reset/invalidation, the app reports the problem and the phone can be paired again by QR.

## Spoken responses

The response is generated and spoken on Windows.

```text
VI    -> response text in Vietnamese
EN    -> response text in English
Auto  -> response text in the user's language
```

Examples:

```text
User: Mở Visual Studio Code
Assistant: Được, mình đã mở Visual Studio Code.

User: Open Visual Studio Code
Assistant: Sure, I’ve opened Visual Studio Code.
```

`voice-runtime` now has language-aware SAPI support:

```text
Vietnamese -> SAPI LANGID 42A (vi-VN)
English    -> SAPI LANGID 409 (en-US)
```

Response text is escaped before being placed into SAPI XML. If no installed SAPI voice matches the requested language, SAPI keeps the current/default voice instead of failing solely because that locale voice is missing.

The current default `speak()` path automatically detects Vietnamese from Vietnamese-specific characters/diacritics. An explicit `speak_with_language()` API is available; directly threading the satellite's `VI / EN / Auto` hint to that API is remaining Phase 27 work.

## Desktop fallback voice pipeline

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

The fallback model is sherpa-onnx Vietnamese Zipformer 30M INT8 and remains an `OfflineRecognizer`. Partial UI updates are simulated through bounded snapshot decodes. Only final text enters Assistant Core.

Resource id:

```text
stt_zipformer_vi
```

Install locally while the desktop runtime is running:

```powershell
assistant resources install stt_zipformer_vi
```

The fallback Vietnamese Zipformer model has separate upstream license terms (**CC-BY-NC-ND-4.0**) and should not be assumed suitable for commercial redistribution.

## User experience

Primary voice path:

```text
Phone -> push-to-talk -> Android STT -> final text
      -> Quick / Assistant Core -> Antigravity / MCP
      -> Windows action -> desktop spoken response
```

Desktop interaction remains available through:

```text
Alt + Space
text input
desktop microphone fallback
desktop wake path
```

Sensitive action:

```text
Assistant / MCP
      |
Sensitive tool
      |
Permission broker
      |
desktop confirmation
      |
Allow once / Deny
```

The phone cannot approve Sensitive actions or bypass the desktop permission boundary.

## Workspace

```text
apps/android-satellite       Android speech-input companion
apps/desktop                 React Quick/edge/permission surfaces
apps/desktop/src-tauri       Tauri/Rust background host + CLI adapters
crates/common                shared contracts
crates/assistant-core        state machine/request lifecycle
crates/antigravity-bridge    long-running Antigravity session
crates/context-engine        on-demand desktop context
crates/permission-broker     authenticated local confirmation broker
crates/permission-engine     risk/permission policy
crates/voice-runtime         desktop mic/VAD/STT/TTS/wake runtime
crates/windows-tools         native Windows operations
crates/windows-mcp           MCP server + permission gateway
```

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

Common commands:

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
assistant satellite help
```

## Development

### Windows prerequisites

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

in Android Studio, sync Gradle, and install on the target phone.

The source-first Android project does not commit the Gradle wrapper JAR/binary. Generate a wrapper locally if command-line Android builds are required.

See [`apps/android-satellite/README.md`](apps/android-satellite/README.md).

## Local validation gates

Do not treat source completion as target-device verification. Per project policy, remote GitHub Actions/build/test/model-download/installer execution is not required for these phases.

Validate locally on Windows + Android:

- `assistant satellite pair --qr` renders a scannable QR;
- Android imports but does not auto-connect;
- explicit Connect authenticates;
- `assistant satellite devices` shows the phone;
- per-device revoke disconnects/rejects that device;
- Android pairing survives restart through Keystore-backed storage;
- `vi-VN` and `en-US` recognition work on the target phone;
- partial speech remains phone-side;
- one final transcript creates one Assistant turn;
- MCP/permission rules remain authoritative;
- desktop speaks the response;
- `VI`, `EN`, `Auto` response behavior is correct;
- a suitable installed SAPI voice is used for Vietnamese/English when available;
- missing locale voice falls back safely;
- busy/disconnect/reconnect paths behave correctly;
- local Zipformer remains usable as fallback;
- NSIS packaging contains `assistant`, `assistant-core`, `assistant-satellite`, and `assistant-mcp` sidecars.

## Security notes

Current satellite rules include:

- no valid pairing config -> no listener;
- authentication required as first application message;
- protocol-version validation;
- bounded WebSocket frames;
- one active satellite command at a time;
- Assistant busy rejection;
- per-device revocation;
- Android Keystore protection for the phone-side token;
- no Android MCP/Win32/shell credentials;
- desktop Sensitive confirmation remains authoritative.

The current Windows bootstrap token is still persisted in the satellite settings layer and **Windows DPAPI hardening remains planned**.

The current transport is unencrypted `ws://` and is intended for a **trusted/private LAN only**. Do not expose port `8765` directly to the public Internet. Secure remote use belongs to the later WSS/private-overlay phase.

Legacy development environment overrides remain supported:

```text
ASSISTANT_VOICE_SATELLITE_TOKEN
ASSISTANT_VOICE_SATELLITE_BIND
```

## Roadmap

The authoritative roadmap is [`docs/PROJECT_PLAN.md`](docs/PROJECT_PLAN.md).

Near-term work:

1. complete Phase 26 hardening: Windows DPAPI, firewall/private-network UX, richer device diagnostics;
2. finish Phase 27: explicit `VI / EN / Auto` TTS hint wiring and optional installed-voice preferences;
3. Phase 28: phone wake/Quick Settings/hardware activation without continuous `SpeechRecognizer` looping;
4. Phase 29: recognition/reconnect/duplicate-command resilience;
5. Phase 30: secure remote satellite through WSS or a private overlay such as Tailscale;
6. Phase 31: conversational/full-duplex/barge-in UX.

## Key documentation

- [`docs/PROJECT_PLAN.md`](docs/PROJECT_PLAN.md)
- [`docs/VOICE_SATELLITE.md`](docs/VOICE_SATELLITE.md)
- [`docs/PHASE26_QR_PAIRING.md`](docs/PHASE26_QR_PAIRING.md)
- [`docs/PHASE26_TRUSTED_DEVICES.md`](docs/PHASE26_TRUSTED_DEVICES.md)
- [`docs/PHASE26_CANONICAL_CLI.md`](docs/PHASE26_CANONICAL_CLI.md)
- [`docs/PHASE27_LANGUAGE_AWARE_TTS.md`](docs/PHASE27_LANGUAGE_AWARE_TTS.md)
- [`apps/android-satellite/README.md`](apps/android-satellite/README.md)
- [`docs/CLI_MANAGEMENT.md`](docs/CLI_MANAGEMENT.md)
- [`docs/VOICE_STT.md`](docs/VOICE_STT.md)
- [`docs/VOICE_DESKTOP.md`](docs/VOICE_DESKTOP.md)
- [`docs/WAKE_RUNTIME.md`](docs/WAKE_RUNTIME.md)
- [`docs/PERMISSION_GATEWAY.md`](docs/PERMISSION_GATEWAY.md)
- [`docs/RELEASE_CHECKLIST.md`](docs/RELEASE_CHECKLIST.md)

## License

See the repository license for application source terms. Runtime/model resources may have separate upstream licenses. The current fallback Vietnamese Zipformer STT model is CC-BY-NC-ND-4.0.
