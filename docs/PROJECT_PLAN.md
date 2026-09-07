# Assisstant Desktop — Unified Project Plan

## 1. Product goal

Build a Windows-first AI system assistant with a lightweight Gemini-style desktop experience while moving primary speech recognition to an Android companion device.

The product architecture is now deliberately split:

- **Android Voice Satellite**: microphone + speech recognition + text-command transport;
- **Windows Desktop**: Assistant Core + context + Antigravity/Gemini reasoning + MCP + Windows tools + permission enforcement + spoken response.

The target experience is:

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
9. Local desktop STT must remain a fallback while the satellite migration stabilizes.
10. No paid speech API is required for the primary architecture.

## 3. Non-goals

The project will not:

- reverse-engineer Google credentials, Gemini consumer-app internals, or private APIs;
- expose unrestricted shell execution to the model;
- allow the Android device to bypass Assistant Core/MCP permissions;
- use `SpeechRecognizer` as a permanent always-listening loop;
- expose the current unencrypted LAN WebSocket directly to the public Internet;
- remove desktop-local voice fallback before the Android path has been locally validated;
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
- `vi-VN` and `en-US` initial recognition languages.

### Satellite desktop transport

- RFC 6455 WebSocket over the existing Tokio runtime;
- JSON application protocol, version 1;
- required pairing token;
- trusted LAN for the MVP;
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
                                  |
                                  v
+----------------------------------------------------------------+
| Windows Assisstant Desktop                                     |
|                                                                |
| Voice Satellite Adapter                                        |
|          |                                                     |
|          +----> Quick transcript UI                            |
|          |                                                     |
|          v                                                     |
|     Assistant Core                                             |
|        /      \                                                |
|       /        \                                               |
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

No Android module is permitted to execute Windows actions directly. All satellite requests must cross the same Assistant Core and permission boundaries as desktop text/voice requests.

## 6. Development history

Phases 0–24 established the current desktop foundation, including:

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

The local Zipformer path proved useful as an offline fallback but is not accurate enough to remain the preferred speech-input path for the desired user experience. The roadmap therefore changes at Phase 25 rather than continuing to optimize a small local recognizer indefinitely.

## 7. Current and next phases

### Phase 25 — Android Voice Satellite MVP — CURRENT

Deliverables:

- Android Kotlin/Compose app;
- microphone permission handling;
- push-to-talk;
- `vi-VN` / `en-US` recognition;
- prefer Android on-device recognition when available;
- system recognizer fallback;
- partial transcript shown only on Android;
- final transcript sent to desktop exactly once;
- authenticated WebSocket protocol v1;
- pairing token required before the desktop LAN listener starts;
- desktop busy rejection/single satellite turn;
- reuse existing Quick transcript event;
- feed text into existing Assistant Core/Antigravity/MCP path;
- desktop SAPI response;
- response language `VI`, `EN`, `Auto`;
- friendly/natural/concise response policy;
- documentation and local validation checklist;
- keep Zipformer desktop input as fallback.

Exit criteria:

- phone and PC on the same trusted LAN can pair;
- Vietnamese and English voice commands produce one final Assistant Core turn;
- Windows actions still pass through the normal MCP/permission boundary;
- desktop speaks the reply;
- invalid token and busy state fail cleanly;
- local fallback voice remains intact.

Source implementation is prepared in this phase. Target-device Windows/Android validation remains a local gate.

### Phase 26 — Pairing, settings, and device management

Replace environment-variable-first setup with productized management.

Deliverables:

- durable satellite settings;
- `assistant satellite status`;
- enable/disable LAN listener;
- generated high-entropy pairing token;
- QR pairing flow;
- trusted-device registry;
- revoke device/token;
- connection/last-seen diagnostics;
- private-network Windows Firewall guidance/automation where safe;
- bind-interface controls instead of broad defaults where practical.

Exit criteria:

A normal user can pair a phone without manually creating environment variables or copying a long endpoint configuration by hand.

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
- friendly concise response style remains model-side, not hard-coded canned phrases.

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
- better retry UX for `NO_MATCH`, network, recognizer-busy, and disconnected desktop;
- automatic fallback recommendation to desktop local STT when phone recognition is unavailable;
- connectivity health and latency metrics;
- duplicate-command protection with request IDs.

### Phase 30 — Secure remote voice satellite

LAN `ws://` is not the final remote transport.

Deliverables:

- WSS/certificate strategy or private overlay network such as Tailscale;
- no public raw port exposure;
- device-specific credentials;
- token rotation;
- replay/expiry protections as required by the final threat model;
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

- no token -> listener disabled;
- text-only command surface;
- bounded protocol/frame sizes;
- protocol-version validation;
- one satellite command at a time;
- satellite cannot answer Sensitive confirmation on behalf of the user in the MVP;
- Android never receives MCP credentials or permission-broker secrets;
- current `ws://` is trusted-LAN-only.

## 10. Cost and privacy policy

The architecture must not require a paid speech-recognition API.

Android speech recognition is provided by the device/platform:

- on-device recognition is used when available and selected;
- the normal system recognizer may, depending on the device/vendor, use an online service;
- no dedicated paid STT API key is required by Assisstant Desktop.

The desktop receives final text rather than phone microphone audio.

Antigravity remains the primary AI/reasoning backend under its existing authentication/quota model.

## 11. MVP acceptance scenarios

From the Android satellite, the user should be able to say natural variants of:

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

For each scenario:

1. Android recognizes speech;
2. only the final transcript is sent;
3. desktop shows/handles one request;
4. Antigravity/MCP or the appropriate safe path performs the operation;
5. permissions are not bypassed;
6. the reply language follows `VI`/`EN`/`Auto`;
7. Windows speaks the response.

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
