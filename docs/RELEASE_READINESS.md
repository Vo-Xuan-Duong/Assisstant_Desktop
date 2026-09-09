# Release Readiness

This document defines the final boundary between **source-complete MVP** and **release-ready build**.

## Source-complete scope

The current source stack includes:

- Windows Rust/Tauri Assistant host;
- Antigravity/Gemini reasoning bridge;
- MCP Windows tools and permission gateway;
- Android Voice Satellite with `vi-VN` / `en-US` SpeechRecognizer;
- authenticated pairing and trusted-device registry;
- Android Keystore and Windows DPAPI credential protection;
- command deduplication, reconnect and safe cancellation;
- Quick Settings and launcher `Nói AI` activation;
- recognition confidence/alternatives/timing diagnostics;
- Tailscale Serve tailnet-only remote mode;
- safe conversational follow-up after desktop TTS completion;
- Windows SAPI VI/EN voice preferences;
- Quick Control Satellite/TTS/System surfaces;
- bounded Satellite listener/device controls;
- automated runtime readiness self-check;
- Phase 33B persistent local acceptance tracking and combined release gate.

## Explicitly outside the current release claim

Phase 31B automatic acoustic full duplex remains deferred.

The existing topology has desktop speaker output entering the Android microphone through room acoustics without a synchronized far-end reference. Enabling continuous recognition during desktop TTS would therefore risk self-transcription. A future implementation requires a real reference-aware AEC/media architecture.

## What still requires the target machine

Repository source work cannot establish these facts without the user's actual Windows/Android environment:

- Windows native build with the intended MSVC/SDK toolchain;
- actual Android recognizer behavior for `vi-VN` / `en-US`;
- QR/deep-link behavior on the target phone;
- Android Keystore persistence/migration;
- Windows DPAPI persistence across restart;
- installed SAPI voice behavior;
- microphone, wake and SAPI cancellation behavior;
- real Firewall rule result;
- Tailscale Serve behavior over mobile data;
- NSIS packaging/install/startup behavior;
- real launcher/Quick Settings lifecycle;
- conversation follow-up timing in actual room acoustics.

These are not missing source features. They are release acceptance evidence.

## Recommended final workflow

```text
1. Merge the source stack in order
   #96 -> #97 -> #98 -> Phase 33B PR

2. Build/install locally

3. Open Quick -> Control -> System
   fix any runtime Blocking item

4. Open Quick -> Release
   execute each real-device acceptance check
   mark Passed / Failed / Blocked

5. Fix any Failed/Blocked behavior

6. Repeat until
   runtime blockers = 0
   manual required passed = 58/58
   Release gate = Ready

7. Validate the final NSIS artifact/startup flow

8. Tag/publish the release only after those local checks pass
```

## Repository validation policy

Implementation remains source-first. GitHub Actions, native builds/tests, installer execution, microphone execution, Android device execution, Firewall mutation, Tailscale mutation and model downloads are not run as part of repository editing.
