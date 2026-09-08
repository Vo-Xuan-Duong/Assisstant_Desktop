# Assisstant Desktop — Unified Project Plan

## 1. Product goal

Build a Windows-first AI system assistant with Android as the preferred voice-input satellite.

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

The phone is not another AI brain and never receives direct MCP, Win32, shell, or desktop permission authority.

## 2. Locked product rules

1. Partial Android speech hypotheses remain phone-side UI only.
2. Only final speech recognition may create an Assistant turn.
3. Sensitive Windows actions remain desktop-confirmation gated.
4. Desktop owns reasoning, tool execution, response generation, and spoken output.
5. Response language supports `VI`, `EN`, and `Auto`.
6. Simple successful commands should receive one short natural confirmation.
7. Android/system recognition is the preferred STT path; local Vietnamese Zipformer is fallback.
8. No dedicated paid STT API is required by the primary architecture.
9. Remote voice must not expose a raw public WebSocket port.
10. `SpeechRecognizer` must not become an uncontrolled 24/7 recognition loop.
11. Recognition confidence/alternatives are diagnostics and must not silently change a Windows command.
12. Automatic acoustic barge-in must not be claimed without a real echo-reference/AEC-capable architecture.
13. Source completion is not target-device verification; native validation is performed locally by the user.

## 3. Locked technology stack

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
  optional conversational turn-taking
  read-only STT diagnostics
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

The local 30M Vietnamese Zipformer remains fallback because it did not meet the desired primary recognition quality.

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
- roughly one-second hot reload;
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
- runtime decrypt before authentication.

### Diagnostics/network

```powershell
assistant satellite doctor
assistant satellite firewall show
assistant satellite firewall install
assistant satellite firewall remove
```

The managed LAN firewall rule is bounded to inbound TCP, configured port, Private profile, and LocalSubnet.

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
- explicit selected voice preserved instead of being overridden by automatic `<lang>` selection.

Commands:

```powershell
assistant tts voices
assistant tts show
assistant tts set vi <voice>
assistant tts set en <voice>
assistant tts clear <vi|en|all>
```

Remaining here is local voice validation/product settings polish, not missing core source capability.

## 9. Phase 28 — Fast Android activation — COMPLETE IN SOURCE

Delivered:

- Quick Settings `Assistant Voice` tile;
- Android 14+ `PendingIntent` launch compatibility;
- reconnect before listening;
- push-to-talk;
- manual interrupt-to-talk;
- microphone permission remains explicit;
- no continuous SpeechRecognizer loop.

Optional later convenience surfaces: notification action, headset/hardware trigger, dedicated local wake word.

## 10. Phase 29 — Voice resilience — COMPLETE IN SOURCE FOR MVP

Delivered:

- WebSocket reconnect backoff `1/2/4/8/15s`;
- stale-connection generation protection;
- reconnect disabled on auth failure/device revoke;
- no automatic replay of sent commands;
- accepted-command request-ID deduplication;
- bounded recognizer busy retry;
- on-device recognizer -> system recognizer fallback;
- recovery status separated from terminal recognition errors;
- phone Stop control;
- cancellation propagation to Antigravity/SAPI;
- Executing/Confirming remain intentionally non-cancellable.

Potential domain-specific command normalization remains deferred until real-device diagnostics demonstrate a concrete need.

## 11. Phase 29B — Android voice diagnostics — COMPLETE IN SOURCE; LOCAL DATA COLLECTION REQUIRED

Delivered read-only diagnostics:

- final recognizer engine label (`on-device` or `system`);
- elapsed STT time measured with monotonic `SystemClock`;
- optional first confidence score when the recognizer supplies a valid `0..1` value;
- up to two additional final recognition candidates for display only;
- completed desktop-turn elapsed time from successful command submission through AI/tool/TTS response completion;
- cancellation/error paths clear in-flight turn timing;
- conversational auto-follow-ups no longer rewrite settings/Keystore on every recognition window.

Safety rule:

```text
first RESULTS_RECOGNITION candidate
        |
        +--> exact command submitted to desktop

confidence / alternatives
        |
        `--> Android diagnostics UI only
```

No confidence threshold changes, rejects, approves, or rewrites a Windows action. Missing confidence is valid.

See [`PHASE29B_VOICE_DIAGNOSTICS.md`](PHASE29B_VOICE_DIAGNOSTICS.md).

## 12. Phase 30 — Secure remote satellite through Tailscale — COMPLETE IN SOURCE; LOCAL VALIDATION REQUIRED

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
- existing pairing/trust/permission layers remain active;
- non-secret restore state tracks previous bind/enabled state;
- enable rolls back local settings when Serve/state setup fails;
- managed-state drift fails closed;
- disable removes only the managed `--tcp=<port>` Serve endpoint;
- no `tailscale serve reset`;
- previous local state restored only when current state still matches the managed endpoint;
- QR uses `tailscale ip -4`;
- Android uses the normal Tailscale client; no Tailscale SDK dependency.

This phase carries raw `ws://` packets inside Tailscale's encrypted tailnet. Independent WSS/TLS termination is not claimed.

## 13. Phase 31A — Safe conversational turn-taking — COMPLETE IN SOURCE; LOCAL VALIDATION REQUIRED

Goal: repeated voice turns without tapping the microphone after every desktop response, while avoiding microphone capture during desktop TTS.

Android has a persisted **Hội thoại liên tục** option.

```text
Android listens
  -> final transcript
  -> same desktop Assistant session
  -> desktop processes/responds
  -> desktop SAPI TTS completes
  -> response callback reaches Android
  -> 450 ms guard delay
  -> one fresh SpeechRecognizer follow-up window
```

Properties:

- enabling the switch alone does not turn on the microphone;
- the session starts only from user voice activation;
- desktop session/context is reused naturally across follow-up turns;
- **Kết thúc hội thoại** cancels Android listening/pending follow-up without cancelling a Windows action simply because no more follow-up is wanted;
- explicit **Dừng Assistant** remains the control for safe desktop cancellation;
- disconnect, TTS failure, terminal recognizer failure, missing permission, or invalid next-turn state ends the conversational session;
- `NO_MATCH` / speech timeout ends the session instead of creating an infinite retry loop;
- bounded recognizer fallback/recovery messages are non-terminal status events.

See [`PHASE31_CONVERSATIONAL_VOICE.md`](PHASE31_CONVERSATIONAL_VOICE.md).

## 14. Phase 31B — Automatic acoustic barge-in / true full duplex — DEFERRED UNTIL A REAL AEC ARCHITECTURE EXISTS

Do **not** enable automatic acoustic barge-in with the existing RMS VAD or ordinary Android SpeechRecognizer alone.

Current physical audio topology:

```text
Windows speaker -> room air -> Android microphone
```

If Android listens while desktop TTS is speaking, it can transcribe the Assistant itself. The phone currently has no synchronized far-end playback reference from Windows, so a conventional reference-aware echo canceller cannot reliably remove desktop speech.

A future implementation needs an actual AEC-capable topology, for example:

- route both capture and response playback through one endpoint that owns an echo reference;
- stream synchronized desktop TTS reference audio to the phone and process raw capture before recognition;
- move full voice I/O to a single WebRTC-like media session with AEC/NS/AGC before STT.

Until that architecture is implemented and locally validated:

```text
normal conversation -> wait until TTS completes -> auto follow-up
mid-response interruption -> explicit Nói ngắt Assistant / Quick Settings cancel
```

is the supported behavior.

## 15. Response behavior

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

## 16. Security invariants

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
- no Assistant-controlled Funnel/public port exposure;
- diagnostics never alter command/permission semantics;
- conversational mode does not lower desktop permission boundaries.

## 17. Cost/privacy model

Speech recognition does not require a dedicated paid API key.

- on-device Android recognition when available;
- otherwise system/vendor recognizer;
- Google-enabled phones may use Google Speech Services and may need Internet;
- desktop receives recognized text rather than a continuous Android microphone stream.

Antigravity/Gemini reasoning remains under its existing authentication/quota model.

## 18. Local acceptance matrix

Before release readiness, validate on target Windows + Android hardware.

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
10. STT engine label matches the recognizer actually used after fallback.
11. confidence is optional and absent values are handled safely.
12. alternatives remain UI-only and never create extra commands.
13. STT/desktop-turn timings behave sensibly across success/cancel/error.
14. conversation mode does not listen during desktop TTS.
15. a follow-up recognition window opens after TTS completion.
16. follow-up turns retain expected Assistant conversation context.
17. silence/`NO_MATCH` stops conversation mode rather than looping.
18. **Kết thúc hội thoại** prevents a pending microphone follow-up.

### Windows security/TTS

19. DPAPI token persists/reloads across desktop restart for the same Windows user.
20. legacy plaintext migrates as designed.
21. firewall helper creates only the expected Private+LocalSubnet rule.
22. VI/EN/Auto produce correct response language.
23. selected Vietnamese/English SAPI voices are used where installed.
24. missing/removed preferred voice falls back safely.
25. SAPI cancellation works.

### Remote

26. Tailscale Serve maps the tailnet port to `127.0.0.1`.
27. Android works over mobile data while both devices remain in the permitted tailnet.
28. revoke still blocks a remote phone.
29. remote disable restores previous local state when safe.
30. unrelated Tailscale Serve configuration remains untouched.
31. no Funnel/public endpoint is created.

### Fallback/release

32. Zipformer fallback and desktop wake still work.
33. staged/installed canonical/helper executables resolve correctly.
34. NSIS package/startup flow works on the target Windows installation.

## 19. Definition of feature-complete MVP

Core feature-complete source now includes:

- Android preferred voice input;
- safe Windows Assistant Core/Antigravity/MCP control;
- pairing/trust/credential protection;
- VI/EN responses and selectable TTS;
- Stop/reconnect/deduplication;
- read-only recognition/turn diagnostics;
- trusted-LAN and tailnet-only remote modes;
- safe conversational follow-up turn-taking;
- desktop fallback voice;
- Sensitive confirmation gates.

The project is **not release-ready until local acceptance passes**.

Automatic acoustic full duplex remains outside the current release-readiness claim until a real AEC/reference-audio design exists.

## 20. Validation policy

Repository development remains source-first.

Do not run/require remote GitHub Actions, native builds/tests, installer runs, microphone execution, Tailscale mutations, or model downloads during these implementation phases. The user validates them locally on target hardware.
