# Assisstant Desktop

Windows-first AI system assistant powered by **Google Antigravity CLI + Gemini + MCP + Rust/Tauri**, with an **Android Voice Satellite** as the preferred speech-input surface.

The phone is deliberately low-authority: it captures speech and sends final recognized text. Windows owns reasoning, context, permissions, Windows tools, response generation, and spoken output.

## Architecture

```text
Android Voice Satellite
  Microphone
    |
  Android SpeechRecognizer
  |- partial transcript -> phone UI only
  `- final text
    |
  authenticated WebSocket
    |  LAN, or encrypted Tailscale tailnet
    v
+---------------------------------------------------------------+
| Assisstant Desktop                                            |
|                                                               |
| Voice Satellite Adapter -> Quick transcript                   |
|           |                                                   |
|     Assistant Core                                            |
|       /       \                                               |
|   Context     Antigravity / Gemini                            |
|                   |                                           |
|                  MCP                                          |
|                   |                                           |
|          Permission Gateway                                   |
|                   |                                           |
|             Windows Tools                                     |
|                                                               |
| response -> VI / EN / Auto -> Windows SAPI -> desktop speaker |
+---------------------------------------------------------------+

Fallback input:
Windows microphone -> CPAL/WASAPI -> VAD -> Zipformer -> Assistant Core
```

## Current source status

Implemented in source:

- Rust/Tauri Windows background host;
- React Quick/edge/permission UI;
- Antigravity `stream-json` bridge;
- MCP Windows tools and fail-closed permission model;
- Android 8+ Kotlin/Compose Voice Satellite;
- Android `SpeechRecognizer` for `vi-VN` / `en-US`;
- on-device recognizer preference with system recognizer fallback;
- final-text-only command submission;
- local QR pairing;
- trusted Android device registry and per-device revoke;
- Android Keystore AES-GCM protection for the pairing token;
- Windows current-user DPAPI protection for the persisted pairing token;
- command request IDs and duplicate-execution protection;
- Android Stop / manual interrupt-to-talk;
- Quick Settings `Assistant Voice` tile;
- bounded WebSocket reconnect backoff;
- bounded SpeechRecognizer fallback/retry;
- `VI`, `EN`, `Auto` response-language propagation;
- selectable installed Windows SAPI voices for Vietnamese and English;
- locale/default SAPI fallback;
- Windows Firewall diagnostics/Private+LocalSubnet rule helper;
- Tailscale Serve based tailnet-only remote satellite transport;
- safe conversational follow-up mode that only reopens the microphone after desktop TTS finishes;
- sherpa-onnx Vietnamese Zipformer retained as desktop fallback STT;
- existing desktop wake-word runtime retained for the fallback path.

Source implementation is intentionally ahead of target-device verification. Windows/Android/Tailscale runtime validation is performed locally before release readiness is declared.

## Why speech recognition moved to Android

The small local Vietnamese Zipformer is useful as a fallback but does not provide the desired primary recognition quality on the target laptop.

The preferred model is:

```text
Phone   = microphone + speech-to-text
Desktop = reasoning + tools + response + TTS
```

No dedicated paid speech-recognition API is required. Android can use an on-device recognizer where available; otherwise it uses the system recognition service. On Google-enabled phones that system service may be backed by Google Speech Services and may require Internet.

## Android Voice Satellite

Source:

```text
apps/android-satellite/
```

Current behavior:

- push-to-talk;
- `vi-VN` / `en-US`;
- partial transcript remains on the phone;
- only final text creates an Assistant turn;
- stable per-installation `device_id`;
- QR/deep-link pairing;
- Android Keystore credential persistence;
- `VI` / `EN` / `Auto` desktop response selector;
- desktop state/response display;
- Stop and manual interrupt-to-talk;
- Quick Settings activation;
- reconnect backoff `1s -> 2s -> 4s -> 8s -> 15s max`;
- no automatic replay of a command after transport failure;
- bounded on-device -> system recognizer fallback;
- optional **Hội thoại liên tục** turn-taking mode.

`SpeechRecognizer` is **not** kept running continuously as an always-listening loop.

## Pair a phone on the local LAN

Recommended:

```powershell
assistant satellite pair --qr
```

If the PC has multiple adapters:

```powershell
assistant satellite pair --qr --host 192.168.1.20
```

The QR is generated locally and contains:

```text
assd://p?h=<host>&p=<port>&t=<token>
```

Scanning imports configuration only; Android still requires an explicit **Kết nối** action.

Manual fallback:

```powershell
assistant satellite pair
```

then enter the printed token and `ws://<PC-LAN-IP>:8765` in the Android app.

## Satellite management

Canonical commands:

```powershell
assistant satellite show
assistant satellite doctor
assistant satellite pair
assistant satellite pair --qr
assistant satellite enable
assistant satellite disable
assistant satellite bind 0.0.0.0:8765
assistant satellite revoke
assistant satellite devices
assistant satellite revoke-device <device-id>
assistant satellite allow-device <device-id>
```

`assistant-satellite ...` remains a compatibility helper; new workflows should use `assistant satellite ...`.

Persistent data is stored under:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\settings\
```

The desktop normally hot-reloads satellite settings in about one second.

## Pairing and device trust

Authentication is layered:

```text
bootstrap token
      |
stable Android device_id
      |
trusted-device registry
      |
per-device revoke markers
      |
desktop permission model
```

Known devices are recorded in `satellite-devices.json`. Per-device revocation is authoritative even if a phone still has the current bootstrap token.

```powershell
assistant satellite devices
assistant satellite revoke-device <device-id>
assistant satellite allow-device <device-id>
```

## Credential protection

### Android

Pairing tokens are encrypted at rest using an Android Keystore managed AES-GCM key. AES-256 is preferred with AES-128 fallback. Legacy plaintext is deleted only after encrypted persistence succeeds.

### Windows

New/rewritten `satellite.json` files store the token as a current-user Windows DPAPI envelope:

```json
"token": "dpapi:<hex-ciphertext>"
```

The runtime decrypts it before WebSocket authentication. Legacy plaintext files remain readable and migrate to DPAPI on the next mutating satellite command.

Check with:

```powershell
assistant satellite doctor
```

DPAPI/Keystore protect credentials **at rest**; neither claims to protect a credential already present in a compromised running process.

## Windows Firewall

For ordinary trusted-LAN mode:

```powershell
assistant satellite firewall show
assistant satellite firewall install
assistant satellite firewall remove
```

The managed rule is bounded to:

```text
Inbound TCP
configured satellite port
Private profile
LocalSubnet only
```

The CLI never self-elevates. Run install/remove in an Administrator terminal if Windows requires it.

## Secure remote access with Tailscale

Phase 30 uses **Tailscale Serve**, not Funnel.

```text
Android + Tailscale
      |
 encrypted tailnet
      |
Tailscale Serve TCP on Windows
      |
127.0.0.1:<satellite-port>
      |
Assisstant Desktop
```

Commands:

```powershell
assistant satellite remote tailscale show
assistant satellite remote tailscale enable
assistant satellite remote tailscale pair --qr
assistant satellite remote tailscale disable
```

`enable` moves the desktop backend to loopback and configures persistent raw TCP Tailscale Serve. `pair --qr` uses the PC's Tailscale IPv4. `disable` removes only the Assistant-managed Serve port and restores previous local bind/enabled state when it is still safe to do so.

The integration never runs `tailscale funnel` and never uses `tailscale serve reset`, so it neither creates public exposure nor clears unrelated Serve configuration.

The Android app needs no Tailscale SDK: install the normal Tailscale Android client, join the intended tailnet, then use the remote pairing QR.

See [`docs/PHASE30_TAILSCALE_REMOTE.md`](docs/PHASE30_TAILSCALE_REMOTE.md).

## Spoken responses

Android sends the desired response mode:

```text
VI    -> Vietnamese response + Vietnamese TTS hint
EN    -> English response + English TTS hint
Auto  -> match the user's language
```

Examples:

```text
User: Mở Visual Studio Code
Assistant: Được, mình đã mở Visual Studio Code.

User: Open Visual Studio Code
Assistant: Sure, I’ve opened Visual Studio Code.
```

List/select installed SAPI voices:

```powershell
assistant tts voices
assistant tts voices --json
assistant tts show
assistant tts set vi <index-or-voice-id>
assistant tts set en <index-or-voice-id>
assistant tts clear vi
assistant tts clear en
assistant tts clear all
```

Preferences use stable SAPI token IDs and apply on the next utterance. Explicitly selected voices are preserved; automatic mode uses locale-aware SAPI fallback.

## Stop and interrupt-to-talk

Android can cancel only phases that are safe to interrupt:

```text
Listening   -> cancellable
Processing  -> cancellable
Speaking    -> cancellable
Executing   -> intentionally non-cancellable
Confirming  -> intentionally non-cancellable
```

Manual interrupt-to-talk is implemented. The phone sends a separate authenticated control request and starts a fresh recognition turn only after the desktop accepts cancellation.

## Conversational follow-up mode

Enable **Hội thoại liên tục** on Android to continue speaking without manually tapping after every desktop response.

```text
Android listens
  -> final transcript
  -> desktop Assistant turn
  -> desktop TTS completes
  -> 450 ms guard delay
  -> one new Android recognition window
```

The switch alone does not activate the microphone; a session starts from a user-initiated voice turn. Follow-up turns reuse the same desktop Assistant session/context.

Conversation mode stops instead of looping indefinitely when there is silence/`NO_MATCH`, a terminal recognizer error, TTS failure, permission loss, desktop disconnect, or an invalid next-turn state.

Android exposes **Kết thúc hội thoại** to cancel Android listening/pending follow-up. This does not cancel a Windows action just because the user does not want another turn; **Dừng Assistant** remains the explicit desktop cancellation control.

See [`docs/PHASE31_CONVERSATIONAL_VOICE.md`](docs/PHASE31_CONVERSATIONAL_VOICE.md).

## Why automatic acoustic full duplex is still not claimed

The current preferred audio topology is:

```text
Windows speaker -> room air -> Android microphone
```

If Android listens during desktop TTS, it can hear and transcribe the Assistant itself. The phone currently has no synchronized far-end reference audio from the Windows speaker/TTS path, so there is no reliable reference-aware echo cancellation path.

Therefore the project does **not** keep SpeechRecognizer active while desktop TTS is speaking and does not treat RMS/VAD thresholds as a substitute for AEC.

A future full-duplex implementation would need a real reference-aware media topology, such as moving capture/playback into one AEC-capable endpoint or a WebRTC-like audio path carrying an echo reference.

Until then:

- normal conversation -> wait for TTS completion, then auto-listen;
- mid-response interruption -> explicit **Nói ngắt Assistant** / Quick Settings cancellation.

## Desktop fallback voice

```text
Windows microphone
  -> CPAL/WASAPI
  -> RMS VAD + hysteresis
  -> bounded partial Zipformer previews (UI only)
  -> final Zipformer decode
  -> Assistant Core
```

Resource id:

```text
stt_zipformer_vi
```

Install locally:

```powershell
assistant resources install stt_zipformer_vi
```

The fallback model has separate upstream license terms and should not be assumed suitable for commercial redistribution without reviewing those terms.

## Permission model

```text
Safe       -> baseline Allow
Moderate   -> Allow / Ask / Deny runtime override
Sensitive  -> explicit desktop Allow once confirmation
Blocked    -> Deny
Unknown    -> Deny
```

The phone cannot approve Sensitive actions or bypass MCP/permission controls.

## Workspace

```text
apps/android-satellite       Android speech-input companion
apps/desktop                 React Quick/edge/permission surfaces
apps/desktop/src-tauri       Tauri host + canonical/management helpers
crates/common                shared contracts
crates/assistant-core        state machine/request lifecycle
crates/antigravity-bridge    Antigravity session bridge
crates/context-engine        on-demand desktop context
crates/permission-broker     local confirmation broker
crates/permission-engine     risk policy
crates/voice-runtime         desktop mic/VAD/STT/TTS/wake
crates/windows-tools         native Windows operations + DPAPI helper
crates/windows-mcp           MCP server + permission gateway
```

Windows packaging stages canonical and helper executables including:

```text
assistant.exe
assistant-core.exe
assistant-satellite.exe
assistant-satellite-remote.exe
assistant-tts.exe
assistant-mcp.exe
```

## Development

Windows prerequisites:

- Node.js `^20.19.0 || >=22.12.0`;
- pnpm 10;
- Rust `1.98.1` MSVC toolchain;
- Visual Studio C++ Build Tools / Windows SDK;
- CMake;
- Antigravity CLI.

```powershell
pnpm install --frozen-lockfile
pnpm desktop:native:prepare
pnpm desktop:assets:prepare
pnpm desktop:dev
```

Android: open `apps/android-satellite/` in Android Studio, sync Gradle, and install on the target phone.

## Local validation gates

Do not equate source completion with device verification. Validate locally:

- Windows desktop builds with `--locked`;
- staged/installed helper resolution works;
- QR scans on the target phone;
- Android Keystore migration/persistence works;
- Windows DPAPI round-trip and restart work;
- trusted-device revoke works while connected;
- firewall rule is exactly Private + LocalSubnet;
- `vi-VN` / `en-US` recognition works on the target phone;
- selected Vietnamese/English SAPI voices work on the target Windows installation;
- Stop/interrupt-to-talk works across Processing/Speaking;
- reconnect does not replay an already-submitted Windows action;
- Tailscale remote works over mobile data with LAN unavailable;
- Tailscale disable restores previous local state and leaves unrelated Serve configuration intact;
- conversation mode waits until desktop TTS has finished before reopening the microphone;
- conversation follow-up retains expected Assistant context;
- silence/`NO_MATCH` ends conversation mode rather than retrying forever;
- **Kết thúc hội thoại** prevents pending auto-listen;
- fallback Zipformer/wake still work.

Per project policy, repository implementation does not run remote GitHub Actions, native builds/tests, installers, microphones, Tailscale mutations, or model downloads.

## Remaining roadmap

The core functional MVP is implemented in source through **Phase 31A**. Remaining work is primarily validation, product polish, and optional higher-complexity capabilities:

- local Windows/Android/Tailscale acceptance testing;
- richer device/latency/recognizer diagnostics in product UI;
- optional notification/headset/hardware activation surfaces;
- optional confidence/alternative-result and app-name normalization UX;
- **Phase 31B** automatic acoustic full duplex only after a real AEC/reference-audio architecture exists.

Automatic acoustic barge-in will not be enabled by pretending RMS VAD can distinguish user speech from the desktop speaker.

Authoritative roadmap: [`docs/PROJECT_PLAN.md`](docs/PROJECT_PLAN.md).

## Key documentation

- [`docs/PROJECT_PLAN.md`](docs/PROJECT_PLAN.md)
- [`docs/VOICE_SATELLITE.md`](docs/VOICE_SATELLITE.md)
- [`docs/PHASE26_WINDOWS_HARDENING.md`](docs/PHASE26_WINDOWS_HARDENING.md)
- [`docs/PHASE27C_SAPI_VOICE_PREFERENCES.md`](docs/PHASE27C_SAPI_VOICE_PREFERENCES.md)
- [`docs/PHASE28_29_ANDROID_ACTIVATION_RESILIENCE.md`](docs/PHASE28_29_ANDROID_ACTIVATION_RESILIENCE.md)
- [`docs/PHASE30_TAILSCALE_REMOTE.md`](docs/PHASE30_TAILSCALE_REMOTE.md)
- [`docs/PHASE31_CONVERSATIONAL_VOICE.md`](docs/PHASE31_CONVERSATIONAL_VOICE.md)
