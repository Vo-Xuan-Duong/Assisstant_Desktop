# Assisstant Desktop — Unified Project Plan

## 1. Product goal

Build a Windows-first AI system assistant with a lightweight Gemini-style desktop experience and an Android phone as the preferred speech-input satellite.

The product is intentionally split into two authority levels:

- **Android Voice Satellite** — microphone, Android speech recognition, pairing, and final-text transport;
- **Windows Desktop** — Assistant Core, context, Antigravity/Gemini reasoning, MCP, permissions, Windows tools, and spoken response.

Target experience:

```text
User speaks to Android phone
        |
Android SpeechRecognizer
        |
partial text -> phone UI only
final text
        |
authenticated WebSocket
        v
Windows Assistant Core
        |
Antigravity + MCP
        |
permission-gated Windows action
        |
friendly VI / EN / Auto response
        |
locale-aware Windows SAPI TTS
```

The primary speech path must not require a paid STT API. Desktop-local Zipformer remains an offline fallback rather than the preferred recognizer.

## 2. Product rules

1. The phone is an input satellite, not the AI brain.
2. The phone never receives direct MCP, Win32, shell, or permission-broker authority.
3. Partial speech hypotheses are UI-only.
4. Only final recognized text may create an Assistant Core turn.
5. Desktop permissions remain authoritative for Sensitive actions.
6. Desktop owns reasoning, tool execution, response generation, and TTS.
7. Voice responses should be friendly, natural, and concise.
8. Response language supports `VI`, `EN`, and `Auto`.
9. Desktop-local STT remains available as fallback while the satellite path matures.
10. No paid speech-recognition API is required by the primary architecture.
11. Current `ws://` transport is trusted-LAN-only and must not be exposed directly to the public Internet.
12. Repository work is source-first; Windows/Android build, test, installer, microphone, and model validation is performed locally by the user.

## 3. Locked technology stack

### Windows desktop

- Rust 2024;
- Tauri 2;
- React + TypeScript;
- Tokio;
- Antigravity CLI using headless `stream-json` mode;
- MCP over stdio;
- `rmcp` Windows MCP server;
- `windows-rs` / Win32 / COM / UI Automation / CoreAudio;
- Windows SAPI TTS;
- CPAL/WASAPI + sherpa-onnx Vietnamese Zipformer as fallback desktop STT;
- sherpa-onnx wake runtime for the existing desktop wake path.

### Android Voice Satellite

- Kotlin;
- Jetpack Compose;
- Android `SpeechRecognizer`;
- `createOnDeviceSpeechRecognizer()` when supported;
- system `SpeechRecognizer` fallback;
- `vi-VN` and `en-US` recognition;
- OkHttp WebSocket client;
- Android Keystore for pairing-token protection at rest.

### Satellite transport

- RFC 6455 WebSocket over the existing Tokio runtime;
- JSON application protocol v1;
- shared high-entropy bootstrap token;
- stable Android installation `device_id`;
- trusted-device registry and per-device revocation;
- trusted/private LAN for the current transport.

## 4. Current architecture

```text
+-------------------------------+
| Android Voice Satellite       |
|                               |
| Mic                           |
|  -> SpeechRecognizer          |
|  -> partial transcript UI     |
|  -> final transcript only     |
|                               |
| pairing token -> Keystore     |
+---------------+---------------+
                |
         authenticated WS
                |
                v
+---------------------------------------------------------------+
| Assisstant Desktop                                            |
|                                                               |
| Voice Satellite Adapter                                       |
|      -> Quick transcript                                      |
|      -> Assistant Core                                        |
|             |                                                 |
|             +-> Context                                       |
|             +-> Antigravity                                   |
|                    |                                          |
|                   MCP                                         |
|                    |                                          |
|             Permission Gateway                                |
|                    |                                          |
|               Windows Tools                                   |
|                                                               |
| response -> VI / EN / Auto -> Windows SAPI -> desktop speaker |
+---------------------------------------------------------------+

Fallback input:
Windows Mic -> CPAL/WASAPI -> VAD -> Zipformer -> Assistant Core
```

## 5. Completed foundation — Phases 0–24

The desktop foundation already includes:

- Rust workspace/shared contracts;
- long-running Antigravity bridge;
- MCP Windows tools;
- permission classes and Sensitive confirmation flow;
- Tauri Quick/edge/permission UI;
- context engine;
- terminal management surface;
- Windows SAPI TTS;
- CPAL/VAD/Zipformer local STT;
- wake-word runtime;
- explicit voice cancellation/barge-in primitives;
- stale STT cancellation;
- foreground/wake microphone isolation;
- RMS VAD hysteresis.

The local 30M Vietnamese Zipformer remains useful for fallback but did not meet the desired primary voice-recognition quality, which drove the Android Voice Satellite redesign.

## 6. Phase 25 — Android Voice Satellite MVP — COMPLETED IN SOURCE

Delivered:

- Kotlin/Compose Android app;
- push-to-talk;
- microphone permission handling;
- `vi-VN` / `en-US` recognition;
- preference for Android on-device recognition when supported;
- system recognizer fallback;
- partial transcript displayed only on Android;
- final text submitted once to Windows;
- authenticated WebSocket protocol;
- desktop busy rejection/single satellite turn;
- Quick transcript reuse;
- Assistant Core/Antigravity/MCP routing;
- desktop response/TTS;
- `VI`, `EN`, `Auto` response policy;
- friendly, natural, concise model-response policy;
- desktop Zipformer retained as fallback.

## 7. Phase 26 — Pairing, trust, and device management — FUNCTIONALLY COMPLETE; HARDENING REMAINS

### Delivered

#### Persistent listener configuration

- `%LOCALAPPDATA%\com.voduong.assisstantdesktop\settings\satellite.json`;
- generated 64-character pairing secret;
- enable/disable/rebind/revoke;
- roughly one-second hot reload;
- config/token rotation drops the active satellite session;
- legacy `ASSISTANT_VOICE_SATELLITE_TOKEN` / `ASSISTANT_VOICE_SATELLITE_BIND` compatibility.

#### Trusted devices

- stable random Android installation `device_id`;
- device name in protocol `hello`;
- trusted registry in `satellite-devices.json`;
- first/last-seen timestamps;
- per-device revoke/allow;
- authoritative independent revocation markers to avoid last-seen write races;
- active revoked session disconnected on the runtime trust poll.

#### QR pairing

Canonical command:

```powershell
assistant satellite pair --qr
```

Optional explicit LAN address:

```powershell
assistant satellite pair --qr --host 192.168.1.20
```

Properties:

- QR generated locally in the terminal;
- no external QR web service;
- compact `assd://p?...` deep link;
- Android validates endpoint/port/token before import;
- scan imports pairing data but does not automatically connect;
- explicit **Kết nối** action remains required.

#### Canonical CLI

Primary user-facing surface:

```powershell
assistant satellite show
assistant satellite pair --qr
assistant satellite devices
assistant satellite revoke-device <device-id>
assistant satellite allow-device <device-id>
assistant satellite enable
assistant satellite disable
assistant satellite revoke
```

Windows packaging now stages:

```text
assistant.exe             canonical router
assistant-core.exe        existing management implementation
assistant-satellite.exe   satellite management implementation
assistant-mcp.exe         Windows MCP sidecar
```

`assistant-satellite ...` remains a compatibility surface.

#### Android secret-at-rest hardening

- new pairing tokens are encrypted using an Android Keystore AES-GCM key;
- private preferences contain IV/ciphertext rather than intentional plaintext token storage;
- AES-256 is preferred with AES-128 fallback;
- legacy plaintext token migration deletes plaintext only after encrypted persistence succeeds;
- unreadable/incomplete Keystore state is surfaced instead of silently treated as an unpaired device.

### Remaining Phase 26 hardening

- protect the Windows-side bootstrap token with DPAPI instead of plaintext JSON storage;
- improve connected-device diagnostics in the desktop UI;
- provide Windows Firewall/private-network guidance or carefully bounded automation;
- narrow bind-interface UX where practical;
- eventually replace the shared bootstrap token with stronger device-specific credentials.

The current `ws://` listener remains trusted-LAN-only.

## 8. Phase 27 — Language-aware desktop TTS — CURRENT

### Delivered in the TTS engine

`voice-runtime` now supports:

```text
TtsLanguage::Vietnamese
TtsLanguage::English
TtsLanguage::Auto
```

Windows SAPI speech uses SAPI XML language selection:

```text
vi-VN -> LANGID 42A
en-US -> LANGID 409
```

Additional behavior:

- model text is XML-escaped before being placed in SAPI markup;
- `WindowsSapiTts::speak()` auto-detects Vietnamese text by Vietnamese-specific characters/diacritics;
- other VI/EN text defaults to en-US;
- `TextToSpeech::speak_with_language()` is available for explicit language hints;
- the existing dedicated COM worker, cancellation, purge, Skip fallback, rate, and volume remain intact;
- if no SAPI voice matches the requested language, SAPI keeps the current voice instead of failing solely for that reason.

### Remaining Phase 27 work

- thread the satellite's explicit `VI / EN / Auto` selection directly into `speak_with_language()`;
- optionally enumerate installed SAPI voices for diagnostics/settings;
- allow preferred Vietnamese and English voice selection when multiple voices are installed;
- expose voice/rate/volume preferences in product settings;
- validate actual Vietnamese and English voices locally on the target Windows machine.

## 9. Phase 28 — Fast phone activation

Do not use Android `SpeechRecognizer` as an always-listening loop.

Candidate activation surfaces:

- local wake-word engine;
- Quick Settings tile;
- notification action;
- hardware/volume-button trigger;
- headset/Bluetooth action;
- Android assistant role where maintainable.

Deliverables:

- low-power activation path;
- recognition starts only after activation;
- connection recovery before command submission;
- phone-side cancel command for desktop Processing/Speaking;
- safe interruption of desktop TTS.

## 10. Phase 29 — Voice quality and resilience

Deliverables:

- useful recognition confidence/alternative handling where Android exposes it;
- app/tool-name normalization;
- better handling for `NO_MATCH`, network errors, recognizer busy, and disconnected desktop;
- duplicate-command protection using request IDs;
- connectivity/latency diagnostics;
- graceful guidance to desktop fallback STT when phone recognition is unavailable.

## 11. Phase 30 — Secure remote satellite

The current raw LAN WebSocket is not a remote-Internet design.

Deliverables:

- WSS strategy or private overlay network such as Tailscale;
- no public raw port exposure;
- device-specific credentials;
- credential rotation/expiry/replay controls as required by the final threat model;
- remote device visibility/revocation.

## 12. Phase 31 — Conversational/full-duplex UX

After recognition, trust, TTS locale handling, and cancellation are stable:

- conversational follow-ups;
- satellite-originated barge-in;
- cancellation propagation to Antigravity/SAPI;
- optional AEC if desktop microphone remains active while desktop speakers are talking;
- richer phone state UI without moving reasoning authority off Windows.

## 13. Voice response behavior

### Vietnamese

```text
User: Mở Visual Studio Code
Assistant: Được, mình đã mở Visual Studio Code.
```

### English

```text
User: Open Visual Studio Code
Assistant: Sure, I’ve opened Visual Studio Code.
```

### Auto

Respond in the same language used by the user.

Response rules:

- simple successful command -> one short natural confirmation;
- failure -> concise reason and useful next action when appropriate;
- knowledge question -> longer answer allowed;
- multi-step task -> concise progress/result summaries;
- avoid repetitive canned narration.

## 14. Security policy

Windows tools remain categorized as:

- `SAFE` — harmless/read-only inspection;
- `MODERATE` — reversible user-facing actions;
- `SENSITIVE` — consequential mutations requiring confirmation;
- `BLOCKED` — not exposed to the model.

Satellite-specific security:

- no valid pairing config -> no listener;
- first application message must authenticate;
- protocol version checked;
- bounded WebSocket frame size;
- one active satellite command at a time;
- desktop busy requests are rejected;
- phone cannot approve Sensitive confirmation on behalf of the user;
- per-device revoke remains authoritative even when the phone retains the shared token;
- Android does not receive MCP credentials or permission-broker secrets;
- QR and pairing tokens are credentials and must not be published;
- `ws://` remains private-LAN-only until Phase 30.

## 15. Cost/privacy policy

Speech recognition must not require a paid API.

Android recognition behavior:

- use on-device recognition when available and selected;
- otherwise use the device's system recognizer;
- on Google-enabled phones that service may be backed by Google's recognition service and may require Internet;
- Assisstant Desktop does not require a dedicated paid STT key for this path.

The desktop receives recognized text, not a continuous microphone audio stream from the phone.

Antigravity remains the AI/reasoning backend under its existing authentication/quota model.

## 16. Local acceptance scenarios

After locally building/installing the current Windows and Android source, verify:

1. `assistant satellite pair --qr` produces a scannable QR.
2. Android imports pairing but does not connect automatically.
3. Explicit **Kết nối** authenticates successfully.
4. `assistant satellite devices` shows the phone.
5. `revoke-device` disconnects that phone and blocks reconnect.
6. Android pairing token survives restart through Keystore-backed persistence.
7. `vi-VN` recognizes commands such as `Mở Visual Studio Code`.
8. `en-US` recognizes `Open Visual Studio Code`.
9. partial transcript remains phone-only.
10. one final transcript creates one Assistant Core turn.
11. Windows permissions are not bypassed.
12. `VI`, `EN`, and `Auto` produce the intended response language.
13. Windows SAPI speaks Vietnamese/English with a matching installed voice when available.
14. absence of a Vietnamese voice still yields a safe default-voice fallback.
15. Stop/cancellation still works while SAPI is speaking.
16. desktop Zipformer remains usable as fallback.

## 17. Definition of completion

The product reaches feature-complete status when:

- Windows runtime starts reliably with sign-in;
- Android is the preferred voice-input surface;
- pairing/trust is secure and user-manageable;
- Vietnamese/English voice commands are reliable on supported phones;
- desktop-local voice remains a usable fallback;
- Assistant Core/Antigravity/MCP safely controls Windows;
- Sensitive actions remain confirmation-gated;
- desktop responses are concise, friendly, and language-aware;
- Windows TTS chooses appropriate voices when possible;
- cancellation/reconnect paths are stable;
- remote usage, if enabled, does not expose raw public WebSocket ports;
- the system remains usable across phone/network/AI/tool failures.

## 18. Validation policy

Repository changes are source-first.

Do not treat source completion as runtime verification. GitHub Actions, remote builds, remote tests, native microphone execution, Android device runs, installers, and model downloads are not required during these implementation phases.

Windows and Android runtime validation is performed locally by the user on target hardware before release readiness is declared.
