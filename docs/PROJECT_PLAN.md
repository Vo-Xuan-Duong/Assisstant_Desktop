# Assisstant Desktop — Unified Project Plan

## 1. Product goal

Build a Windows-first AI system assistant with a lightweight Gemini-style desktop experience while moving primary speech recognition to an Android companion device.

The product architecture is deliberately split:

- **Android Voice Satellite**: microphone + speech recognition + text-command transport;
- **Windows Desktop**: Assistant Core + context + Antigravity/Gemini reasoning + MCP + Windows tools + permission enforcement + spoken response.

Target experience:

```text
User speaks to phone
      |
Android SpeechRecognizer
      |
final text only
      |
authenticated WebSocket
      v
Windows Assistant Core
      |
Antigravity + MCP
      |
Windows action
      |
friendly VI / EN / Auto response
      |
Windows TTS
```

The user must be able to speak Vietnamese or English without requiring a paid speech API. Desktop-local Zipformer remains available as a fallback input path rather than the primary recognizer.

## 2. Product rules

1. The phone is an input satellite, not the AI brain.
2. The phone never receives direct MCP/Win32/shell access.
3. Only final recognized text may create an Assistant Core turn.
4. Partial speech hypotheses are UI-only.
5. Desktop permissions remain authoritative for Sensitive actions.
6. Desktop owns the assistant response and TTS.
7. Simple commands should receive short, friendly confirmations.
8. Response language must support `VI`, `EN`, and `Auto`.
9. Local desktop STT remains a fallback while the satellite migration stabilizes.
10. No paid speech API is required for the primary architecture.

## 3. Non-goals

The project will not:

- reverse-engineer Google credentials, Gemini consumer-app internals, or private APIs;
- expose unrestricted shell execution to the model;
- allow Android to bypass Assistant Core/MCP permissions;
- use `SpeechRecognizer` as a permanent always-listening loop;
- expose the current unencrypted LAN WebSocket directly to the public Internet;
- remove desktop-local voice fallback before Android voice is validated locally;
- require Python services in the production runtime.

## 4. Locked technology stack

### Windows desktop

- Rust 2024;
- Tauri 2;
- React + TypeScript for Quick/edge/permission surfaces;
- Tokio async runtime;
- Google Antigravity CLI in headless `stream-json` mode;
- MCP over stdio;
- `rmcp` for the Windows MCP server;
- `windows-rs` / Win32 / COM / UI Automation / CoreAudio;
- Windows SAPI for desktop TTS;
- CPAL/WASAPI + sherpa-onnx Zipformer for fallback desktop STT;
- sherpa-onnx wake runtime for the existing desktop wake path.

### Android Voice Satellite

- Kotlin;
- Jetpack Compose;
- Android `SpeechRecognizer`;
- `createOnDeviceSpeechRecognizer()` when Android reports on-device recognition support;
- system `SpeechRecognizer` fallback;
- OkHttp WebSocket client;
- `vi-VN` and `en-US` initial recognition languages;
- stable random per-installation `device_id` stored in private app preferences.

### Satellite desktop transport

- RFC 6455 WebSocket over the existing Tokio runtime;
- JSON application protocol v1;
- shared pairing token as bootstrap credential;
- trusted-device registry after token verification;
- race-resistant per-device revocation markers;
- trusted/private LAN for the MVP;
- one active phone connection during the current pairing phase;
- one active satellite command at a time.

## 5. Architecture

```text
                    +---------------------------+
                    | Android Voice Satellite   |
                    |                           |
                    | Mic -> SpeechRecognizer   |
                    | partial -> phone UI       |
                    | final -> text command     |
                    +-------------+-------------+
                                  |
                         authenticated WebSocket
                         token + device identity
                                  |
                                  v
+----------------------------------------------------------------+
| Windows Assisstant Desktop                                     |
|                                                                |
| Pairing + trusted-device gate                                  |
|          |                                                     |
| Voice Satellite Adapter                                        |
|          |                                                     |
|          +----> Quick transcript UI                            |
|          |                                                     |
|          v                                                     |
|     Assistant Core                                             |
|        /      \                                                |
| Context       Antigravity Bridge                               |
|                  |                                             |
|             Antigravity CLI                                    |
|                  |                                             |
|                 MCP                                            |
|                  |                                             |
|        Permission Gateway                                      |
|                  |                                             |
|            Windows Tools                                       |
|                  |                                             |
|             Win32 / UIA                                        |
|                                                                |
| Assistant response -> VI / EN / Auto -> Windows SAPI -> speaker|
+----------------------------------------------------------------+

Fallback input:

Windows Mic -> CPAL/WASAPI -> VAD -> Zipformer -> Assistant Core
```

No Android module may execute Windows actions directly. All satellite requests cross the same Assistant Core and permission boundaries as desktop text/voice requests.

## 6. Development history

Phases 0–24 established the desktop foundation, including:

- Rust workspace and shared contracts;
- Antigravity long-running bridge;
- Windows MCP tools;
- permission classes and Sensitive confirmation broker;
- Tauri Quick/edge/permission UI;
- context engine;
- terminal-first `assistant.exe` management;
- Windows SAPI TTS;
- desktop CPAL/VAD/Zipformer STT;
- wake word runtime;
- voice cancellation/barge-in primitives;
- stale STT cancellation;
- wake-capture isolation;
- RMS VAD hysteresis.

Phase 25 changed the preferred voice architecture from local desktop STT to Android Voice Satellite while keeping Zipformer as fallback. It was squash-merged to `main` through PR #76.

Phase 26 pairing bootstrap introduced persistent pairing/hot reload and was squash-merged through PR #77.

## 7. Current and next phases

### Phase 25 — Android Voice Satellite MVP — COMPLETED IN SOURCE

Delivered:

- Android Kotlin/Compose app;
- microphone permission handling;
- push-to-talk;
- `vi-VN` / `en-US` recognition;
- Android on-device recognizer preference;
- system recognizer fallback;
- partial transcript shown only on Android;
- final transcript sent to desktop exactly once;
- authenticated WebSocket transport;
- Assistant Core/Antigravity/MCP routing;
- desktop SAPI response;
- `VI` / `EN` / `Auto` response text;
- friendly/natural/concise response policy;
- desktop Zipformer retained as fallback.

Target-device Windows/Android validation remains a local release gate.

### Phase 26 — Pairing, settings, and device management — CURRENT

Goal: remove environment-variable-first setup and provide manageable, revocable phone trust.

#### Implemented

Pairing/listener bootstrap:

- durable `%LOCALAPPDATA%\com.voduong.assisstantdesktop\settings\satellite.json`;
- generated 64-character shared pairing token;
- `assistant-satellite` helper;
- masked token status;
- pair/token rotation;
- enable/disable listener;
- bind-address control;
- shared-token revoke;
- roughly one-second live configuration reload;
- active session dropped when listener/token config changes;
- legacy environment override compatibility.

Trusted-device layer:

- stable random Android installation `device_id`;
- device identity included in the WebSocket `hello` message;
- bounded/sanitized device id and name handling;
- `%LOCALAPPDATA%\com.voduong.assisstantdesktop\settings\satellite-devices.json` registry;
- first-seen and last-seen timestamps;
- automatic registration only after shared-token verification;
- `assistant-satellite devices`;
- `assistant-satellite revoke-device <device-id>`;
- `assistant-satellite allow-device <device-id>`;
- active-session device-trust polling and disconnect after revoke;
- dedicated revocation marker files so a concurrent last-seen write cannot accidentally undo a revoke.

Current helper surface:

```text
assistant-satellite show
assistant-satellite pair
assistant-satellite pair --bind 0.0.0.0:8765
assistant-satellite enable
assistant-satellite disable
assistant-satellite bind <host:port>
assistant-satellite revoke
assistant-satellite devices
assistant-satellite revoke-device <device-id>
assistant-satellite allow-device <device-id>
```

#### Remaining Phase 26 work

Priority order:

1. **Canonical management surface**
   - fold pairing/device commands under `assistant satellite ...`;
   - package/install the management path consistently with desktop releases.
2. **QR pairing**
   - generate a short-lived pairing payload on Windows;
   - include endpoint + bootstrap credential without manual typing;
   - Android scanner/import flow;
   - bind the accepted pairing to the installation `device_id`.
3. **Connection diagnostics/UI**
   - currently connected device;
   - last seen in user-friendly local time;
   - pairing/listener health in normal status/readiness surfaces.
4. **Network safety**
   - private-network Windows Firewall guidance or safe automation;
   - narrower/default bind-interface UX where practical.
5. **Credential hardening**
   - Windows DPAPI for sensitive persisted material;
   - Android Keystore-backed credential storage;
   - move from one shared bootstrap token toward device-specific credentials.

Phase 26 exit criteria:

A normal user can pair a phone by QR, inspect trusted devices, revoke one phone independently, and reconnect without manually configuring environment variables or typing long credentials/endpoints.

### Phase 27 — Language-aware desktop TTS

Current `VI`/`EN`/`Auto` controls generated response text. This phase improves actual voice selection.

Deliverables:

- enumerate installed Windows SAPI voices;
- detect voice locale metadata;
- preferred Vietnamese voice setting;
- preferred English voice setting;
- automatic locale-aware voice selection for `Auto`;
- rate/volume settings retained;
- graceful fallback to default SAPI voice;
- friendly concise response style remains model-side.

Exit criteria:

Vietnamese text is normally spoken by an installed Vietnamese-capable voice and English by an English-capable voice when available.

### Phase 28 — Fast activation and phone-side wake

Do not continuously loop Android `SpeechRecognizer`.

Candidates:

- local wake-word engine;
- notification/quick-settings tile;
- hardware/volume-button trigger;
- headset/Bluetooth action;
- Android assistant role if appropriate and maintainable.

Deliverables:

- low-power activation path;
- start SpeechRecognizer only after activation;
- connection recovery before command submission;
- cancellation command from phone to desktop Processing/Speaking;
- phone can interrupt desktop TTS safely.

### Phase 29 — Voice quality and resilience

Deliverables:

- confidence/alternative-result handling where Android exposes useful data;
- command vocabulary/context hints where platform support permits;
- app/tool-name normalization;
- retry UX for `NO_MATCH`, network, recognizer-busy, and disconnected desktop;
- fallback recommendation to desktop local STT when phone recognition is unavailable;
- connectivity health and latency metrics;
- duplicate-command protection with request IDs.

### Phase 30 — Secure remote voice satellite

LAN `ws://` is not the final remote transport.

Deliverables:

- WSS/certificate strategy or private overlay network such as Tailscale;
- no public raw port exposure;
- device-specific credentials;
- token rotation;
- replay/expiry protections;
- remote-device visibility/revocation.

### Phase 31 — Continuous/full-duplex assistant UX

Only after speech input, pairing, TTS locale handling, and cancellation are stable:

- conversational follow-ups;
- automatic barge-in from the satellite;
- interruption propagation to Antigravity/SAPI;
- optional AEC strategy if desktop microphone remains active during desktop speech;
- richer phone state UI without moving reasoning off the desktop.

## 8. Response behavior

Voice-originated satellite turns use these semantics:

### Vietnamese

```text
"Mở Visual Studio Code"
-> "Được, mình đã mở Visual Studio Code."
```

### English

```text
"Open Visual Studio Code"
-> "Sure, I’ve opened Visual Studio Code."
```

### Auto

Respond in the same language the user used.

Rules:

- simple successful command -> one short confirmation sentence;
- failure -> concise reason plus useful next action when appropriate;
- knowledge question -> longer response allowed;
- multi-step task -> concise state/result summaries;
- avoid verbose canned narration before every action.

## 9. Security policy

Windows tools remain categorized as:

- `SAFE` — harmless/read-only inspection;
- `MODERATE` — reversible user-facing actions;
- `SENSITIVE` — file/system/process mutations requiring explicit confirmation;
- `BLOCKED` — not exposed to the model.

Satellite-specific rules:

- no active shared pairing token -> listener disabled;
- text-only command surface;
- bounded protocol/frame sizes;
- protocol-version validation;
- valid shared token is required before device registration;
- stable installation identity is required after pairing;
- per-device revocation survives concurrent diagnostics updates via dedicated markers;
- revoked active device is disconnected on the next trust poll;
- one active phone connection in the current phase;
- one satellite command at a time;
- satellite cannot answer Sensitive confirmation on behalf of the user;
- Android never receives MCP/permission-broker/management secrets;
- current `ws://` is trusted-LAN-only.

## 10. Cost and privacy policy

The architecture must not require a paid speech-recognition API.

Android speech recognition is provided by the device/platform:

- on-device recognition is used when available and selected;
- the normal system recognizer may use an online vendor service;
- on Google-enabled Android devices the system recognizer may use Google's speech service;
- no dedicated paid STT API key is required by Assisstant Desktop.

The desktop receives final text rather than phone microphone audio.

Antigravity remains the primary AI/reasoning backend under its existing authentication/quota model.

## 11. Acceptance scenarios

Voice scenarios:

- `Mở Chrome`;
- `Mở Visual Studio Code`;
- `Âm lượng hiện tại bao nhiêu?`;
- `Đặt âm lượng 30%`;
- `Tắt tiếng`;
- `Bật lại tiếng`;
- `Pause nhạc`;
- `Chuyển bài`;
- `Ứng dụng nào đang active?`;
- `Máy đang dùng bao nhiêu RAM?`;
- `Open Visual Studio Code`;
- `What is the current volume?`.

For each voice scenario:

1. Android recognizes speech;
2. only final text is submitted;
3. one desktop request is created;
4. Antigravity/MCP or safe local path performs the operation;
5. permissions are not bypassed;
6. response language follows `VI`/`EN`/`Auto`;
7. Windows speaks the response.

Current Phase 26 pairing/device scenarios:

1. `assistant-satellite pair` creates a fresh token without manual environment setup;
2. running desktop applies listener changes without restart;
3. first valid Android connection creates one trusted device entry;
4. reconnect updates `last_seen_unix`;
5. `revoke-device` disconnects that active phone and prevents reconnect even with the correct shared token;
6. `allow-device` permits it again;
7. shared `revoke` disconnects the active phone and invalidates the bootstrap token;
8. rotating token does not silently clear explicit per-device revocation;
9. QR/canonical CLI still must be added before Phase 26 is called complete.

## 12. Definition of completion

The project is feature-complete when:

- Windows background runtime starts reliably with sign-in;
- Android is the preferred voice-input satellite;
- pairing is secure and user-manageable;
- Vietnamese and English voice commands are reliable on supported phones;
- desktop-local voice remains a usable fallback;
- Assistant Core/Antigravity/MCP can safely control Windows;
- Sensitive actions remain confirmation-gated;
- desktop responses are natural, friendly, concise, and language-aware;
- Windows TTS selects appropriate installed voices when possible;
- Quick/edge UI communicates assistant state clearly;
- cancellation and reconnect paths are stable;
- the system remains usable across phone/network/Antigravity/tool failures.

## 13. Validation policy

Repository changes are prepared source-first. GitHub Actions, remote builds, remote tests, native microphone runs, installers, and model downloads are not required during these implementation phases.

Windows and Android runtime validation is performed locally by the user on the target hardware before release readiness is declared.
