# Assisstant Desktop — Unified Project Plan

## 1. Product goal

Build a Windows-first AI system assistant with Android as the preferred voice-input satellite.

Authority split:

```text
Android
  microphone + SpeechRecognizer + activation + transport
        |
        | final recognized text only
        v
Windows
  Assistant Core + context + Antigravity/Gemini + MCP
  + permission gateway + Windows tools + response + TTS
```

The phone is never another AI brain and never receives direct MCP/Win32/permission authority.

## 2. Locked product rules

1. Partial Android speech hypotheses remain phone-side UI only.
2. Only final speech recognition may create an Assistant turn.
3. Sensitive Windows actions remain desktop-confirmation gated.
4. Desktop owns all tool execution and spoken responses.
5. Response language is `VI`, `EN`, or `Auto`.
6. Simple successful commands should receive one short natural confirmation.
7. Android/system recognition is the preferred STT path; the local Vietnamese Zipformer is fallback.
8. No dedicated paid STT API is required by the primary architecture.
9. Remote voice must not expose a raw public WebSocket port.
10. Source completion is not target-device verification; native validation is performed locally by the user.

## 3. Current technology stack

### Windows

- Rust 2024;
- Tauri 2 + React/TypeScript;
- Tokio;
- Antigravity CLI `stream-json`;
- MCP over stdio;
- Win32 / COM / UI Automation / CoreAudio through `windows-rs`;
- Windows SAPI TTS;
- Windows DPAPI current-user credential protection;
- CPAL/WASAPI + sherpa-onnx fallback STT/wake.

### Android

- Kotlin + Jetpack Compose;
- Android 8+;
- Android `SpeechRecognizer`;
- `createOnDeviceSpeechRecognizer()` when supported;
- system recognizer fallback;
- OkHttp WebSocket;
- Android Keystore AES-GCM;
- Quick Settings Tile.

### Transport/security

- RFC 6455 WebSocket + JSON protocol v1;
- bootstrap token;
- stable Android installation `device_id`;
- trusted-device registry;
- per-device revoke;
- command request-ID replay protection;
- trusted-LAN mode;
- Tailscale Serve tailnet-only remote mode.

## 4. Current architecture

```text
Android Voice Satellite
  SpeechRecognizer
  Quick Settings / push-to-talk / interrupt-to-talk
  Keystore-protected credential
           |
     authenticated WebSocket
           |
    LAN or Tailscale tailnet
           |
           v
Assisstant Desktop
  Voice Satellite Adapter
       |
  Assistant Core
   /        \
Context    Antigravity/Gemini
               |
              MCP
               |
       Permission Gateway
               |
          Windows Tools
               |
       VI / EN / Auto
               |
         Windows SAPI
```

Fallback:

```text
Windows Mic -> CPAL/WASAPI -> VAD -> Zipformer -> Assistant Core
```

## 5. Phases 0–24 — Desktop foundation — COMPLETE IN SOURCE

Delivered:

- Rust workspace/contracts;
- Antigravity bridge;
- MCP Windows tools;
- permission/risk system;
- Tauri Quick/edge/permission UI;
- context engine;
- management CLI;
- SAPI TTS;
- desktop microphone/VAD/Zipformer;
- wake runtime;
- cancellation/barge-in primitives;
- stale STT cancellation;
- wake/foreground microphone isolation;
- VAD hysteresis.

The local 30M Vietnamese Zipformer remains a fallback because it did not meet the desired primary recognition quality.

## 6. Phase 25 — Android Voice Satellite MVP — COMPLETE IN SOURCE

Delivered:

- Kotlin/Compose app;
- push-to-talk;
- `vi-VN` / `en-US` recognition;
- on-device preference + system recognizer fallback;
- partial UI-only transcript;
- one final text submission;
- authenticated desktop WebSocket receiver;
- Assistant Core/Antigravity/MCP routing;
- desktop SAPI response;
- `VI`, `EN`, `Auto` response policy;
- desktop fallback STT retained.

## 7. Phase 26 — Pairing, trust, Windows hardening — COMPLETE IN SOURCE

Delivered:

### Pairing/config

- persistent `settings/satellite.json`;
- high-entropy bootstrap token;
- hot reload;
- enable/disable/rebind/revoke;
- canonical `assistant satellite ...` CLI;
- compatibility helper `assistant-satellite`;
- local terminal QR + `assd://p` deep link.

### Device trust

- stable Android installation `device_id`;
- first/last seen registry;
- per-device revoke/allow;
- independent revocation marker files;
- active revoked session disconnect.

### Credential protection

Android:

- Keystore AES-GCM;
- AES-256 preference / AES-128 fallback;
- conservative plaintext migration.

Windows:

- current-user DPAPI `CryptProtectData` / `CryptUnprotectData`;
- persisted `dpapi:<hex>` envelope;
- legacy plaintext read/migration;
- runtime decrypt before authentication;
- no extra crypto/encoding dependency.

### Diagnostics/network

```powershell
assistant satellite doctor
assistant satellite firewall show
assistant satellite firewall install
assistant satellite firewall remove
```

The managed LAN firewall rule is bounded to inbound TCP, the configured port, Private profile, and LocalSubnet.

## 8. Phase 27 — Language-aware/selectable desktop TTS — COMPLETE IN SOURCE

Delivered:

- `TtsLanguage::{Vietnamese, English, Auto}`;
- explicit satellite response-language hint into TTS;
- safe SAPI XML/locale fallback;
- enumerate installed SAPI voices;
- stable SAPI token-ID preferences;
- separate VI/EN selected voices;
- canonical `assistant tts ...` CLI;
- no restart required for preference changes;
- explicit voice token preserved instead of being overridden by `<lang>` auto-selection.

Commands:

```powershell
assistant tts voices
assistant tts show
assistant tts set vi <voice>
assistant tts set en <voice>
assistant tts clear <vi|en|all>
```

Remaining here is target-machine voice validation/product UI polish, not missing core source capability.

## 9. Phase 28 — Fast Android activation — COMPLETE IN SOURCE

Delivered:

- Quick Settings `Assistant Voice` tile;
- Android 14+ `PendingIntent` launch compatibility;
- reconnect before listening;
- push-to-talk;
- manual interrupt-to-talk;
- microphone permission remains explicit;
- no continuous SpeechRecognizer loop.

Possible later convenience surfaces (not blockers): notification action, headset/hardware trigger, dedicated local wake word.

## 10. Phase 29 — Voice resilience — COMPLETE IN SOURCE FOR MVP

Delivered:

- WebSocket reconnect backoff `1/2/4/8/15s`;
- stale-connection generation protection;
- reconnect disabled on auth failure/device revoke;
- no automatic replay of sent commands;
- accepted-command request-ID deduplication;
- bounded recognizer busy retry;
- on-device recognizer -> system recognizer fallback;
- phone Stop control;
- cancellation propagation to Antigravity/SAPI;
- Executing/Confirming remain intentionally non-cancellable.

Possible quality follow-ups:

- confidence/alternative-result UX where useful;
- domain/app-name normalization;
- richer latency/recognizer diagnostics.

## 11. Phase 30 — Secure remote satellite through Tailscale — COMPLETE IN SOURCE; LOCAL VALIDATION REQUIRED

Goal: remote Android control without raw public port exposure.

Architecture:

```text
Android + Tailscale
       |
 encrypted tailnet
       |
Tailscale Serve raw TCP
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

Properties:

- uses **Tailscale Serve**, never Funnel;
- backend moves to loopback before remote sharing;
- existing Assistant pairing/trust/permission layers remain active;
- remote state tracks the previous bind;
- enable rolls back the bind if Serve setup fails;
- state-drift fails closed;
- disable removes only the managed `--tcp=<port>` Serve endpoint;
- no `tailscale serve reset`;
- previous bind restored only when current state still matches the managed loopback bind;
- QR uses `tailscale ip -4`;
- Android requires only the normal Tailscale client, no Tailscale SDK dependency.

This phase carries raw `ws://` packets inside Tailscale's encrypted tailnet. Independent WSS/TLS termination is not claimed.

## 12. Phase 31 — Conversational/full-duplex UX — NEXT

Already available foundations:

- Android Stop;
- manual interrupt-to-talk;
- Processing cancellation;
- SAPI cancellation;
- request IDs;
- reconnect;
- status events.

Remaining work must be separated into two concepts:

### 31A — Conversational UX

- keep/reuse short-lived conversation context across Android voice turns;
- improve follow-up status/state presentation;
- make repeated voice turns feel continuous without giving the phone reasoning authority;
- clarify when a new utterance starts a fresh task versus follows the current conversation.

### 31B — Automatic acoustic barge-in / full duplex

Do **not** enable automatic acoustic barge-in with the existing RMS VAD alone.

If a microphone is kept active while desktop speakers play, a production implementation requires an AEC-capable audio path (plus appropriate noise suppression/endpointing) so speaker output is not misclassified as user speech.

Until that path is designed and locally validated, the safe product behavior remains:

```text
Quick Settings / Nói ngắt Assistant
        -> explicit cancel
        -> start SpeechRecognizer
```

Because Android is the preferred microphone and Windows is the speaker, a later phone-side barge-in design may be preferable to keeping the desktop microphone live.

## 13. Response behavior

Vietnamese:

```text
User: Mở Visual Studio Code
Assistant: Được, mình đã mở Visual Studio Code.
```

English:

```text
User: Open Visual Studio Code
Assistant: Sure, I’ve opened Visual Studio Code.
```

Rules:

- simple success -> one short confirmation;
- failure -> concise reason + useful recovery when appropriate;
- knowledge question -> fuller answer allowed;
- long task -> concise progress/result summaries;
- never claim an action succeeded unless the tool result did.

## 14. Security invariants

Windows tools:

- `SAFE` — harmless/read-only;
- `MODERATE` — reversible user-facing action;
- `SENSITIVE` — explicit confirmation;
- `BLOCKED` — not exposed.

Satellite invariants:

- no valid pairing -> no usable listener;
- first application message authenticates;
- protocol version checked;
- bounded WebSocket frames;
- one satellite command admitted at a time;
- accepted command IDs are deduplicated;
- desktop busy requests are rejected;
- device revoke remains authoritative;
- phone cannot approve Sensitive actions;
- Android receives no MCP/Win32 secrets;
- Android token at rest -> Keystore;
- Windows token at rest -> DPAPI;
- LAN rule -> Private + LocalSubnet;
- remote mode -> Tailscale Serve/tailnet only;
- no Assistant-controlled Funnel/public port exposure.

## 15. Cost/privacy model

Speech recognition does not require a dedicated paid API key.

- on-device Android recognition when available;
- otherwise system/vendor recognizer;
- Google-enabled phones may use Google Speech Services and may need Internet;
- desktop receives recognized text rather than a continuous Android microphone stream.

Antigravity/Gemini reasoning remains under its existing authentication/quota model.

## 16. Local acceptance matrix

Before release readiness, validate on target Windows + Android hardware:

### Core/Android

1. QR pairing scans correctly.
2. Android only connects after explicit action.
3. `vi-VN` and `en-US` recognition are acceptable.
4. only final text creates one turn.
5. permissions cannot be bypassed.
6. per-device revoke disconnects/rejects the phone.
7. Android Keystore survives restart and migration.
8. reconnect does not duplicate Windows actions.
9. Quick Settings activation and interrupt-to-talk work.

### Windows security/TTS

10. DPAPI token persists/reloads across desktop restart for the same Windows user.
11. legacy plaintext migrates as designed.
12. firewall helper creates only the expected Private+LocalSubnet rule.
13. VI/EN/Auto produce correct response language.
14. selected Vietnamese/English SAPI voices are used where installed.
15. missing/removed preferred voice falls back safely.
16. SAPI cancellation works.

### Remote

17. Tailscale Serve maps the tailnet port to `127.0.0.1`.
18. Android works over mobile data while both devices remain in the permitted tailnet.
19. revoke still blocks a remote phone.
20. `remote tailscale disable` restores the previous bind.
21. unrelated Tailscale Serve configuration remains untouched.
22. no Funnel/public endpoint is created.

### Fallback/release

23. Zipformer fallback and desktop wake still work.
24. staged/installed canonical/helper executables resolve correctly.
25. NSIS package/startup flow works on the target Windows installation.

## 17. Definition of feature-complete MVP

The MVP is feature-complete in source when:

- Android is the preferred voice-input surface;
- Assistant Core/Antigravity/MCP safely controls Windows;
- pairing/trust/credential storage is secure enough for the defined threat model;
- VI/EN responses and selectable TTS work;
- Stop/reconnect/deduplication work;
- trusted-LAN and tailnet-only remote modes exist;
- desktop fallback voice remains available;
- Sensitive actions remain confirmation-gated.

With Phase 30 implemented, those core source capabilities exist. The project is **not release-ready until local acceptance passes**.

Phase 31 is an experience enhancement beyond the functional MVP, especially automatic acoustic full duplex.

## 18. Validation policy

Repository development remains source-first.

Do not run/require remote GitHub Actions, native builds/tests, installer runs, microphone execution, Tailscale mutations, or model downloads during these implementation phases. The user validates them locally on target hardware.
