# Release Readiness

This document defines the final boundary between **source-complete MVP** and **release-ready build** and is the canonical local validation runbook.

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

The Phase 32A, 32B, 33A and 33B source stack is merged into `main`.

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

## 1. Sync the target Windows checkout

Run from PowerShell in the repository:

```powershell
git checkout main
git pull --ff-only
```

The release candidate should normally be validated from a clean `main` checkout.

## 2. Install locked JavaScript dependencies

```powershell
corepack enable
pnpm install --frozen-lockfile
```

Required toolchain remains:

- Node.js `^20.19.0 || >=22.12.0`;
- pnpm 10;
- Rust `1.98.1` MSVC toolchain;
- Visual Studio C++ Build Tools + Windows SDK;
- CMake;
- Antigravity CLI.

## 3. Run the safe local release preflight

Recommended entrypoint:

```powershell
pnpm desktop:release:local
```

This command:

- requires Windows;
- checks that `git`, `node`, `pnpm` and `cargo` exist;
- reports current branch and dirty working-tree state;
- reuses `scripts/verify-release.ps1` in JSON mode;
- prints every Ready / Optional / Blocking preflight item;
- stops with a non-zero exit code when any Blocking item exists;
- prints the exact next local acceptance steps.

It deliberately does **not** build, install, sign, download models, run microphone tests, change Firewall/Tailscale state, or invoke GitHub Actions.

For public/signing preflight:

```powershell
pnpm desktop:release:local:public
```

That additionally enforces the public-release signing prerequisites already defined by the release verifier.

## 4. Start the desktop application locally

After static preflight has no Blocking items:

```powershell
pnpm desktop:dev
```

This starts the real desktop application on the target Windows machine. From this point the validation evidence is machine/device specific.

## 5. Clear automated runtime blockers

Open:

```text
Quick -> Control -> System
```

The System tab is the authoritative automated runtime starting point. Resolve every `Blocking` item that is relevant to the intended release configuration.

Do not treat the System tab as release certification. It cannot prove microphone quality, Android lifecycle, Tailscale mobile-data behavior, installer behavior or other real-device facts.

## 6. Execute the manual release acceptance matrix

Open:

```text
Quick -> Release
```

Run each of the 58 acceptance checks against the actual Windows/Android environment and mark each one:

```text
Pending
Passed
Failed
Blocked
```

The local release gate is eligible for `Ready` only when:

```text
runtime Blocking = 0
manual required passed = 58/58
```

A manual checkbox is user-attested local evidence only. It does not approve Sensitive MCP operations and is not remote attestation or CI certification.

## 7. Areas that require real-device validation

The 58-item matrix covers at least these release-critical areas:

```text
Android QR/deep-link pairing
vi-VN / en-US recognition
on-device -> system recognizer fallback
Android Keystore persistence/migration
Quick Settings + Nói AI launcher activation
interrupt-to-talk and conversation follow-up
Windows DPAPI persistence/restart
SAPI VI/EN selection/fallback/cancellation
Satellite listener/device revoke/allow
Firewall Private + LocalSubnet rule
Tailscale Serve and mobile-data operation
Zipformer/wake fallback
sidecar/helper resolution
NSIS install/startup behavior
System self-check and no-secret boundaries
```

When a check is `Failed` or `Blocked`, fix the underlying behavior first, then repeat the affected checks. Do not mark a check Passed solely to make the aggregate gate green.

## 8. Build the local unsigned release candidate

Only after the relevant local acceptance is satisfactory:

```powershell
pnpm desktop:release:build
```

Or use the explicit wrapper, which performs preflight first and builds only because the command explicitly requests it:

```powershell
pnpm desktop:release:local:build
```

After build completion, install the produced NSIS package on the target Windows installation and validate startup, helper resolution, settings persistence and normal Assistant operation again.

## 9. Public/signed release candidate

A public Windows release additionally requires the configured signing certificate and timestamp policy.

Preflight:

```powershell
pnpm desktop:release:local:public
```

Build:

```powershell
pnpm desktop:release:build:public
```

or the explicit combined wrapper:

```powershell
pnpm desktop:release:local:build:public
```

Do not publish an unsigned local acceptance build as though it were the final public artifact.

## 10. Final release condition

The project can move from source-complete to release-ready only after the target-machine evidence is complete:

```text
static release preflight: no Blocking
runtime System: no Blocking
manual acceptance: 58/58 Passed
final NSIS install/startup: validated
public signing policy: validated when publishing publicly
```

Only then should the release be tagged/published.

## Repository validation policy

Repository editing remains source-first. GitHub Actions, native builds/tests, installer execution, microphone execution, Android device execution, Firewall mutation, Tailscale mutation and model downloads are not run as part of repository editing. Those operations belong to the user's target-machine acceptance session.
