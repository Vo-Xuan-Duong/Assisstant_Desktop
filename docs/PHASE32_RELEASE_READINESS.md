# Phase 32 — Local Release Readiness Runner

## Goal

The source MVP is feature-complete, but release readiness depends on the actual Windows installation, Android phone, speech services, SAPI voices and Tailscale environment.

Phase 32 adds a **read-only** local acceptance runner. It does not replace the existing build/package verification scripts and it does not claim hardware validation automatically.

## Run

After the desktop runtime is installed/staged and running:

```powershell
pnpm desktop:release:acceptance
```

or directly:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release-readiness.ps1
```

If `assistant.exe` is not on `PATH`:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release-readiness.ps1 \
  -AssistantPath "C:\path\to\assistant.exe"
```

Machine-readable output:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release-readiness.ps1 -Json
```

## Automated checks

The runner invokes only read-only canonical CLI commands:

```text
assistant version
assistant status --json
assistant runtime ping
assistant startup show
assistant permissions list
assistant satellite doctor
assistant satellite devices
assistant tts voices --json
assistant tts show --json
```

These are required automated checks for the current MVP acceptance run.

The following network-mode inspections are advisory because a target release can be validated in LAN mode, Tailscale mode, or both:

```text
assistant satellite firewall show
assistant satellite remote tailscale show
```

A required command returning non-zero causes the runner to exit `1`. Advisory failures are printed but do not fail the automated gate.

## What the runner never does

It does **not**:

- create/rotate a pairing token;
- pair, revoke or allow a device;
- add/remove Windows Firewall rules;
- enable/disable Tailscale Serve;
- use Tailscale Funnel;
- start microphone capture or SpeechRecognizer;
- speak audio;
- install STT/wake models;
- run Cargo/Gradle/pnpm test suites;
- build or install NSIS packages;
- trigger GitHub Actions.

This keeps the acceptance command safe to run repeatedly while inspecting an installed system.

## Why it does not call `assistant doctor`

The older core doctor predates Android-primary voice and treats the local Zipformer STT bundle as a blocker. Zipformer is now a fallback resource, so its absence must not automatically invalidate the Android-primary MVP.

The Phase 32 runner therefore composes the canonical read-only commands that reflect the current product architecture instead of inheriting that outdated blocking assumption.

A later cleanup can align the core doctor semantics with the runtime `assistant_readiness` model, where fallback STT/wake resources are optional unless explicitly required by the user's configuration.

## Manual target-hardware gates

The runner always prints the manual checks that still require real hardware/user observation:

1. Android app build/install;
2. QR pairing and trusted-device registration;
3. in-app, Quick Settings and **Nói AI** launcher activation;
4. `vi-VN` / `en-US` recognition quality and recognizer fallback;
5. desktop Vietnamese/English SAPI voice output;
6. Stop / interrupt-to-talk behavior;
7. conversational follow-up only after TTS completion;
8. no token exposure in the desktop Satellite diagnostics WebView payload;
9. Tailscale remote operation over mobile data with LAN unavailable;
10. optional desktop Zipformer/wake fallback if included in the target release;
11. Windows NSIS install/startup/staged-helper behavior.

Passing automated checks means **ready to continue manual acceptance**, not release-ready.

## Relationship to existing release scripts

Existing scripts remain authoritative for source/package verification:

```text
scripts/prepare-release.ps1
scripts/verify-release.ps1
scripts/verify-public-release.ps1
```

Phase 32 complements them:

```text
source/package verification
        |
        v
build/install locally
        |
        v
release-readiness.ps1
  read-only runtime checks
        |
        v
manual Windows + Android + Tailscale acceptance
        |
        v
release decision
```

## Validation policy

This phase only implements the runner and documentation in the repository. The runner itself is **not executed remotely** during development. The user runs it locally against the actual Windows installation and phone environment.
