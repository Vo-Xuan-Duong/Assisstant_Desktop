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
12. The desktop diagnostics WebView must never receive/decrypt the pairing token.
13. Automatic acoustic barge-in must not be claimed without a real echo-reference/AEC-capable architecture.
14. Source completion is not target-device verification; GitHub Actions may validate source/build contracts, while target hardware/device acceptance remains local.
15. Automated readiness status must not be presented as release certification.

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
- Android 8+ (`minSdk 26`);
- Android `SpeechRecognizer`;
- `createOnDeviceSpeechRecognizer()` where supported;
- system recognizer fallback;
- OkHttp WebSocket;
- Android Keystore AES-GCM;
- Quick Settings Tile;
- static launcher shortcut with a NoDisplay trampoline activity.

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
  push-to-talk
  Quick Settings tile
  launcher shortcut
  manual interrupt-to-talk
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

Quick Control UI
  |- Satellite -> readiness/device diagnostics
  |               + bounded listener enable/disable
  |               + bounded known-device revoke/allow
  |- TTS -> bounded installed SAPI voice selection for VI/EN
  |- System -> read-only existing runtime readiness checks
  `- pairing secret never enters the WebView
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

## 7. Phase 26 — Pairing, trust and Windows hardening — COMPLETE IN SOURCE

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
- first/last-seen registry;
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

See [`PHASE26_WINDOWS_HARDENING.md`](PHASE26_WINDOWS_HARDENING.md).

## 8. Phase 26B — Desktop Satellite diagnostics UI — COMPLETE IN SOURCE

Goal: show useful satellite state in product UI without making the WebView a management/security authority.

Delivered by extending the existing `assistant_readiness` payload with a non-secret satellite snapshot:

- listener enabled/disabled;
- paired state;
- bind address;
- credential storage class;
- environment override presence without its value;
- managed Tailscale state/port;
- trusted/revoked device counts;
- device id/name/first-seen/last-seen/revoked metadata;
- warnings for malformed files, legacy credential storage, environment overrides and remote-state drift.

The Quick surface originally exposed a compact read-only **Satellite** panel. Phase 32A folds it into **Voice**, Phase 32B adds bounded listener/device controls, and Phase 33A renames the combined product surface to **Control** when the System self-check tab is added.

Security boundary:

```text
settings / device registry / remote state
        |
        v
non-secret readiness snapshot
        |
        v
Quick Control -> Satellite tab

pairing token --------X------> WebView
```

A disabled/unpaired satellite is `optional_missing`, not a blocking failure for text Assistant/MCP operation.

See [`PHASE26B_SATELLITE_DIAGNOSTICS_UI.md`](PHASE26B_SATELLITE_DIAGNOSTICS_UI.md).

## 9. Phase 27 — Language-aware/selectable desktop TTS — COMPLETE IN SOURCE

Delivered:

- `TtsLanguage::{Vietnamese, English, Auto}`;
- explicit satellite response-language hint into TTS;
- safe SAPI locale fallback;
- enumerate installed SAPI voices;
- stable SAPI token-ID preferences;
- separate VI/EN selected voices;
- canonical `assistant tts ...` CLI;
- no restart required for preference changes;
- explicitly selected voice preserved instead of being overridden by automatic `<lang>` selection.

Commands:

```powershell
assistant tts voices
assistant tts show
assistant tts set vi <voice>
assistant tts set en <voice>
assistant tts clear <vi|en|all>
```

Phase 32A adds bounded product UI for these existing preferences; target-machine validation is still required.

## 10. Phase 28 — Fast Android activation — COMPLETE IN SOURCE

Delivered:

- Quick Settings **Assistant Voice** tile;
- Android 14+ `PendingIntent` tile launch compatibility;
- reconnect before listening;
- push-to-talk;
- manual interrupt-to-talk;
- microphone permission remains explicit;
- no continuous SpeechRecognizer loop.

See [`PHASE28_29_ANDROID_ACTIVATION_RESILIENCE.md`](PHASE28_29_ANDROID_ACTIVATION_RESILIENCE.md).

## 11. Phase 28B — Launcher voice shortcut — COMPLETE IN SOURCE

Delivered static shortcut:

```text
Long-press app icon -> Nói AI
```

Implementation:

```text
static shortcuts.xml
  -> VoiceShortcutActivity (NoDisplay, taskAffinity="")
  -> MainActivity + EXTRA_START_VOICE=true
  -> existing voice activation state machine
```

The trampoline contains no pairing/network/AI/tool logic. It reuses existing checks for:

- pairing;
- microphone permission;
- reconnect;
- Processing/Speaking cancellation acknowledgement;
- SpeechRecognizer lifecycle.

No endpoint/token is embedded in shortcut metadata and no notification/background-service permission is added.

See [`PHASE28B_ANDROID_LAUNCHER_SHORTCUT.md`](PHASE28B_ANDROID_LAUNCHER_SHORTCUT.md).

## 12. Phase 29 — Voice resilience — COMPLETE IN SOURCE FOR MVP

Delivered:

- WebSocket reconnect backoff `1/2/4/8/15s`;
- stale-connection generation protection;
- reconnect disabled on auth failure/device revoke;
- no automatic replay of sent commands;
- accepted command request-ID deduplication;
- bounded recognizer busy retry;
- on-device recognizer -> system recognizer fallback;
- recovery status separated from terminal recognition errors;
- phone Stop control;
- cancellation propagation to Antigravity/SAPI;
- Executing/Confirming remain intentionally non-cancellable.

Potential domain-specific command normalization remains deferred until real-device diagnostics demonstrate a concrete need.

## 13. Phase 29B — Android voice diagnostics — COMPLETE IN SOURCE; LOCAL DATA COLLECTION REQUIRED

Delivered read-only diagnostics:

- final recognizer engine (`on-device` or `system`);
- monotonic STT elapsed time;
- optional first confidence score when Android supplies a valid `0..1` value;
- up to two additional final recognition candidates for display only;
- completed desktop-turn elapsed time from successful command submission through AI/tool/TTS completion;
- cancellation/error paths clear in-flight turn timing;
- conversational auto-follow-ups avoid rewriting settings/Keystore on every recognition window.

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

## 14. Phase 30 — Secure remote satellite through Tailscale — COMPLETE IN SOURCE; LOCAL VALIDATION REQUIRED

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

See [`PHASE30_TAILSCALE_REMOTE.md`](PHASE30_TAILSCALE_REMOTE.md).

## 15. Phase 31A — Safe conversational turn-taking — COMPLETE IN SOURCE; LOCAL VALIDATION REQUIRED

Goal: repeated voice turns without tapping the microphone after every desktop response while avoiding microphone capture during desktop TTS.

Android has persisted **Hội thoại liên tục** mode:

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
- **Kết thúc hội thoại** cancels Android listening/pending follow-up without cancelling a Windows action simply because no further follow-up is wanted;
- explicit **Dừng Assistant** remains the control for safe desktop cancellation;
- disconnect, TTS failure, terminal recognizer failure, missing permission, or invalid next-turn state ends the conversational session;
- `NO_MATCH` / speech timeout ends the session instead of creating an infinite retry loop;
- bounded recognizer fallback/recovery messages are non-terminal status events.

See [`PHASE31_CONVERSATIONAL_VOICE.md`](PHASE31_CONVERSATIONAL_VOICE.md).

## 16. Phase 31B — Automatic acoustic barge-in / true full duplex — DEFERRED

Do **not** enable automatic acoustic barge-in with the existing RMS VAD or ordinary Android SpeechRecognizer alone.

Current physical audio topology:

```text
Windows speaker -> room air -> Android microphone
```

If Android listens while desktop TTS is speaking, it can transcribe the Assistant itself. The phone currently has no synchronized far-end playback reference from Windows, so a conventional reference-aware echo canceller cannot reliably remove desktop speech.

A future implementation needs an actual AEC-capable topology, for example:

- route capture and response playback through one endpoint that owns an echo reference;
- stream synchronized desktop TTS reference audio to the phone and process raw capture before recognition;
- use one WebRTC-like media session with AEC/NS/AGC before STT.

Until that architecture is implemented and locally validated:

```text
normal conversation -> wait until TTS completes -> auto follow-up
mid-response interruption -> explicit Nói ngắt Assistant / Quick Settings / launcher activation
```

is the supported behavior.

## 16A. Phase 32A — Desktop TTS settings UI — COMPLETE IN SOURCE; LOCAL VALIDATION REQUIRED

Goal: move the common VI/EN SAPI voice-selection workflow into the Quick product surface while reusing the runtime `tts.conf` and keeping the WebView bounded.

Delivered:

- the former Satellite trigger becomes a unified **Voice** panel;
- **Satellite** tab preserves the existing diagnostics;
- **TTS** tab enumerates installed Windows SAPI voices;
- separate Vietnamese/English selectors;
- **Tự động theo locale** clears an explicit preference;
- exact installed token IDs are validated before persistence;
- stale removed voice selections are surfaced to the user;
- desktop UI and `WindowsSapiTts` share `default_voice_preferences_path()`; the CLI's normal default resolver targets the same `%LOCALAPPDATA%` file while retaining its explicit `--data-dir` override;
- SAPI enumeration/update work runs on blocking worker threads so COM initialization does not inherit the Tauri UI apartment;
- changes apply on the next utterance because `WindowsSapiTts` reloads preferences per spoken response;
- Quick auto-dismiss remains held while the panel is open.

Security boundary:

```text
WebView
  -> vi | en
  -> null OR exact currently-installed SAPI token id
  -> bounded Tauri command
  -> existing tts.conf

arbitrary path / shell / registry command --------X
Satellite pairing token --------------------------X
```

Satellite mutations remain CLI-only in this phase.

See [`PHASE32A_TTS_SETTINGS_UI.md`](PHASE32A_TTS_SETTINGS_UI.md).

## 16B. Phase 32B — Bounded Satellite management UI — COMPLETE IN SOURCE; LOCAL VALIDATION REQUIRED

Goal: expose only reversible, narrow Satellite controls in the Quick product surface while keeping credential/network authority in the CLI.

Delivered:

- **Bật listener / Tắt listener** against the runtime's persisted Satellite settings;
- enable requires an existing valid persisted pairing token;
- enabling a legacy plaintext pairing token migrates it to current-user DPAPI;
- graphical listener mutation is rejected when `ASSISTANT_VOICE_SATELLITE_TOKEN` is active;
- graphical listener mutation is rejected while Tailscale managed state owns listener/restore lifecycle;
- per-known-device **Thu hồi / Cho phép lại** controls;
- device ID is bounded and validated again in Rust;
- unknown device IDs are rejected;
- revoke creates the authoritative marker without rewriting a registry that an active connection may be updating;
- allow clears a legacy registry `revoked=true` flag before removing the authoritative marker, so failure remains fail-closed;
- successful UI mutations refresh the readiness snapshot;
- pairing token, bind, firewall and Tailscale command authority remain outside the WebView.

Security boundary:

```text
WebView
  -> enabled: bool
  OR
  -> existing device_id + revoked: bool
        |
        v
bounded Tauri commands
        |
        +-- satellite.json enabled flag
        `-- satellite-revoked marker / legacy registry flag

pair/create/rotate token ----------------X
arbitrary bind host:port ----------------X
netsh/firewall --------------------------X
Tailscale lifecycle ---------------------X
arbitrary path/shell args ---------------X
```

See [`PHASE32B_SATELLITE_MANAGEMENT_UI.md`](PHASE32B_SATELLITE_MANAGEMENT_UI.md).

## 16C. Phase 33A — Local Validation Dashboard — COMPLETE IN SOURCE; LOCAL VALIDATION REQUIRED

Goal: make the existing automated runtime readiness state directly usable during local acceptance without creating a second health subsystem or pretending automated checks certify the release.

Delivered:

- the combined Quick **Voice** surface becomes **Control**;
- adds a third **System** tab next to Satellite and TTS;
- reuses `assistant_readiness` and `RuntimeReadinessReport` without adding native authority;
- displays aggregate runtime state and Ready/Optional/Blocking counts;
- displays every current readiness check;
- sorts Blocking first, then Optional, then Ready;
- exposes each check detail and only the local path already present in the readiness DTO;
- explicitly states that Android, microphone, QR, Tailscale/mobile-data and installer acceptance remain real-device work;
- no test runner, shell command, process launch, network probe, microphone execution or system mutation is added.

Current automated readiness sources include Antigravity, MCP, Permission Broker, context storage, TTS, Satellite, local STT resource and wake runtime/resource.

Security boundary:

```text
native readiness collectors
        |
        v
assistant_readiness
        |
        v
bounded DTO
        |
        v
Quick Control -> System

pairing token ----------------X
arbitrary file read ----------X
shell/process execution ------X
network/firewall/Tailscale ---X
microphone execution ---------X
```

See [`PHASE33A_LOCAL_VALIDATION_DASHBOARD.md`](PHASE33A_LOCAL_VALIDATION_DASHBOARD.md).

## 16D. Phase 33B — Persisted local release acceptance gate — COMPLETE IN SOURCE; LOCAL VALIDATION REQUIRED

Goal: make the fixed 58-item target-device acceptance matrix directly trackable in the product without granting the WebView new native authority.

Delivered:

- compact **Release** panel backed by frontend-local versioned persistence;
- fixed catalog of all 58 required acceptance checks;
- per-check `pending | passed | failed | blocked` status;
- malformed persisted schema/status/timestamp data fails visibly instead of being silently rewritten;
- `pending` removes the stored item and reset removes the versioned checklist key;
- combined release gate incorporates the existing runtime readiness report;
- runtime Blocking, storage/readiness errors, or manual Failed/Blocked results produce a blocked gate;
- remaining Pending checks keep the gate pending;
- only 58/58 Passed with runtime readiness not blocking produces Ready;
- no new Tauri command, shell/process execution, generic file read, credential access, firewall/Tailscale authority, or Sensitive-action approval path.

The checklist is evidence tracking, not automated proof that a physical test occurred.

See [`PHASE33B_RELEASE_ACCEPTANCE.md`](PHASE33B_RELEASE_ACCEPTANCE.md) and [`RELEASE_READINESS.md`](RELEASE_READINESS.md).

## 16E. Repository CI and release hardening — COMPLETE IN SOURCE; TARGET RELEASE VALIDATION STILL REQUIRED

Repository automation now provides:

- Windows CI for deterministic assets/native preparation, MCP/helper sidecar staging, Rust formatting/tests, frontend build, full desktop feature compilation, and release-contract verification;
- Android CI pinned to JDK 17 / Gradle 9.6.0 / Android 17 API 37 packages, running lint, pairing-payload JVM tests, debug APK assembly, and artifact upload;
- a `workflow_dispatch`-only Windows release workflow with unsigned-candidate and signed-public modes;
- SHA-256 installer manifests;
- ephemeral CurrentUser PFX import/cleanup for signed-public mode;
- draft GitHub Release creation only for a signed-public `main` build, with write permission isolated to the draft-release job.

CI validates source/build contracts. It does not replace the 58 physical acceptance checks or installed NSIS validation.

See [`CI_RELEASE_AUTOMATION.md`](CI_RELEASE_AUTOMATION.md).

## 17. Response behavior

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

## 18. Security invariants

Windows tools:

- `SAFE` — harmless/read-only;
- `MODERATE` — reversible user-facing action;
- `SENSITIVE` — explicit confirmation;
- `BLOCKED` — not exposed.

Satellite/Control UI invariants:

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
- desktop diagnostics never expose the token;
- launcher shortcut cannot bypass pairing/microphone/cancel checks;
- conversational mode does not lower desktop permission boundaries;
- TTS UI can persist only `vi`/`en` plus an exact installed SAPI token ID or automatic fallback;
- TTS UI cannot choose an arbitrary settings path or execute shell/registry commands;
- Satellite UI can toggle only the persisted listener state and known-device revoke state;
- Satellite UI cannot pair/rotate/revoke the shared credential, edit bind, run firewall commands, or manage Tailscale lifecycle;
- listener UI refuses to compete with environment-token override or Tailscale managed state;
- System self-check is read-only and receives only the existing bounded readiness DTO;
- runtime self-check must never be presented as release certification.

## 19. Cost/privacy model

Speech recognition does not require a dedicated paid API key.

- on-device Android recognition when available;
- otherwise system/vendor recognizer;
- Google-enabled phones may use Google Speech Services and may need Internet;
- desktop receives recognized text rather than a continuous Android microphone stream.

Antigravity/Gemini reasoning remains under its existing authentication/quota model.

## 20. Local acceptance matrix

Before release readiness, validate on target Windows + Android hardware.

### Core/Android

1. QR pairing scans correctly.
2. Android only connects after explicit user action.
3. `vi-VN` and `en-US` recognition are acceptable.
4. only final text creates one turn.
5. permissions cannot be bypassed.
6. per-device revoke disconnects/rejects the phone.
7. Android Keystore survives restart and migration.
8. reconnect does not duplicate Windows actions.
9. Quick Settings activation and interrupt-to-talk work.
10. launcher **Nói AI** appears on the target launcher and forwards into the existing MainActivity/task correctly.
11. launcher activation cannot bypass pairing or microphone permission.
12. STT engine label matches the recognizer actually used after fallback.
13. confidence is optional and absent values are handled safely.
14. alternatives remain UI-only and never create extra commands.
15. STT/desktop-turn timings behave sensibly across success/cancel/error.
16. conversation mode does not listen during desktop TTS.
17. a follow-up recognition window opens after TTS completion.
18. follow-up turns retain expected Assistant conversation context.
19. silence/`NO_MATCH` stops conversation mode rather than looping.
20. **Kết thúc hội thoại** prevents a pending microphone follow-up.

### Windows security/TTS/UI

21. DPAPI token persists/reloads across desktop restart for the same Windows user.
22. legacy plaintext migrates as designed.
23. firewall helper creates only the expected Private+LocalSubnet rule.
24. VI/EN/Auto produce correct response language.
25. selected Vietnamese/English SAPI voices are used where installed.
26. missing/removed preferred voice falls back safely.
27. SAPI cancellation works.
28. Quick Satellite diagnostics accurately reflects listener/bind/credential class/device state.
29. no pairing token appears in the frontend/devtools payload.
30. malformed satellite state surfaces a warning instead of crashing the Quick UI.
31. opening Control holds Quick auto-dismiss while interacting with the panel.

### Remote

32. Tailscale Serve maps the tailnet port to `127.0.0.1`.
33. Android works over mobile data while both devices remain in the permitted tailnet.
34. revoke still blocks a remote phone.
35. remote disable restores previous local state when safe.
36. unrelated Tailscale Serve configuration remains untouched.
37. no Funnel/public endpoint is created.

### Fallback/release

38. Zipformer fallback and desktop wake still work.
39. staged/installed canonical/helper executables resolve correctly.
40. NSIS package/startup flow works on the target Windows installation.

### Phase 32A TTS UI

41. Quick **Control -> TTS** lists the same installed voices as `assistant tts voices`.
42. selecting VI/EN in Quick is reflected by `assistant tts show` and the next matching spoken response when both use the normal/default application-data path.
43. **Tự động theo locale** clears the explicit language preference and restores fallback behavior.
44. malformed/stale TTS settings surface an error/warning without crashing Quick or silently rewriting invalid configuration.

### Phase 32B Satellite UI

45. Quick can disable a normally paired local listener while preserving pairing.
46. Quick can re-enable that listener without re-pairing.
47. enable is rejected when no valid persisted pairing exists.
48. environment-token override locks/rejects listener mutation.
49. Tailscale managed state locks/rejects listener mutation.
50. revoking one connected device closes/rejects that device while another trusted device remains usable.
51. allowing the device restores access without rotating the shared token.
52. pairing token, bind, firewall and Tailscale lifecycle remain unavailable to the WebView.

### Phase 33A System self-check

53. Quick **Control -> System** renders all current runtime readiness checks without crashing when optional components are missing.
54. Blocking checks sort before Optional and Ready checks.
55. summary Ready/Optional/Blocking counts match the visible rows.
56. **Làm mới** reflects a real local configuration change in the existing readiness report.
57. paths remain compact/truncated safely and no new generic filesystem read capability exists.
58. no pairing credential or other secret is introduced into the System payload/UI, and the UI does not label the automated self-check as release certification.

## 21. Definition of feature-complete MVP

Core feature-complete source now includes:

- Android preferred voice input;
- safe Windows Assistant Core/Antigravity/MCP control;
- pairing/trust/credential protection;
- non-secret product diagnostics;
- VI/EN responses and selectable TTS;
- bounded Quick UI for VI/EN installed SAPI voice preferences;
- bounded Quick UI for Satellite listener enable/disable and known-device revoke/allow;
- read-only Quick System self-check backed by the existing readiness engine;
- persisted Quick Release acceptance tracking for the fixed 58-item matrix and combined runtime/manual gate;
- Windows/Android repository CI plus bounded manual Windows release-candidate automation;
- Stop/reconnect/deduplication;
- read-only recognition/turn diagnostics;
- Quick Settings + launcher shortcut activation;
- trusted-LAN and tailnet-only remote modes;
- safe conversational follow-up turn-taking;
- desktop fallback voice;
- Sensitive confirmation gates.

The project is **feature-complete for the current MVP in source**, but **not release-ready until local acceptance passes**.

Automatic acoustic full duplex remains outside the current release-readiness claim until a real AEC/reference-audio design exists.

## 22. Remaining roadmap

### Required before release

- run the local acceptance matrix on target Windows and Android hardware;
- use **Quick -> Control -> System** as the automated runtime starting point, not as a substitute for real-device checks;
- fix any device-specific integration issues found by that validation;
- validate Windows packaging/NSIS/startup behavior;
- validate Tailscale remote mode over mobile data;
- validate installed VI/EN SAPI voices, Phase 32A TTS UI, Phase 32B Satellite controls, Phase 33A System self-check, and Android recognizer behavior.

### Optional product improvements

- `main` branch protection with Windows and Android CI as required checks after the hardening workflows are merged;
- notification activation if a persistent notification is actually desired;
- headset/hardware-button activation where Android/device policy permits it;
- domain/app-name normalization only after collected recognition diagnostics show repeatable errors.

### Deferred research

- Phase 31B reference-aware AEC/full-duplex media architecture.

## 23. Validation policy

Repository development remains source-first, but GitHub Actions is an allowed source/build gate.

Windows CI and Android CI may run on pull requests and `main` to validate formatting, unit tests, compilation, packaging contracts, and debug artifacts. The dispatch-only Windows release workflow may build candidates when explicitly invoked. Physical Android/Windows acceptance, installer execution on the target machine, microphone/wake behavior, Tailscale mutations/mobile-data checks, and model downloads remain local/manual validation work. CI success must never be presented as equivalent to 58/58 target-device acceptance.
