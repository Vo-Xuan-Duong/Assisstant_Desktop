# CI and Release Automation

This document describes the repository-side validation and release automation for Assisstant Desktop.

## Windows CI

Workflow:

```text
.github/workflows/windows-ci.yml
```

It runs on pull requests to `main`, pushes to `main`, and manual dispatch. The current gate validates:

- locked frontend dependencies;
- deterministic release assets;
- native build dependencies;
- MCP/helper sidecar staging;
- Rust formatting;
- Rust tests;
- frontend production build;
- full desktop feature compilation;
- release contract verification.

This CI is source/build validation only. It does not replace the 58-item Windows + Android target-device acceptance matrix.

## Android CI

Workflow:

```text
.github/workflows/android-ci.yml
```

The Android project currently uses Android Gradle Plugin 9.4.0 and API 37. CI therefore pins:

```text
JDK 17
Gradle 9.6.0
Android platform 37
Android Build Tools 36.0.0
```

The workflow runs for Android source changes and can also be dispatched manually. It performs:

```text
lintDebug
  -> testDebugUnitTest
  -> assembleDebug
  -> upload app-debug.apk as a short-lived CI artifact
```

The Android unit-test suite includes pure pairing-payload validation so CI verifies security-sensitive QR/deep-link inputs without requiring an emulator.

Real `SpeechRecognizer`, Keystore, Quick Settings, launcher lifecycle, mobile-data/Tailscale, and microphone behavior still require a physical Android device.

## Manual Windows release workflow

Workflow:

```text
.github/workflows/windows-release.yml
```

It is `workflow_dispatch` only. A release is never created automatically from a normal push or merge.

Inputs:

```text
release_tag          must exactly match v<apps/desktop package version>
mode                 unsigned-candidate | signed-public
create_draft_release false by default
```

### Unsigned candidate

Use:

```text
mode = unsigned-candidate
create_draft_release = false
```

The workflow runs the existing reviewed release verifier/build path, generates `SHA256SUMS.txt`, and uploads the Tauri bundle as a GitHub Actions artifact for 14 days.

Unsigned candidates are not eligible to create a GitHub Release.

### Signed public candidate

Required repository Actions secrets:

```text
ASSISTANT_WINDOWS_CERT_PFX_BASE64
ASSISTANT_WINDOWS_CERT_PASSWORD
ASSISTANT_WINDOWS_CERT_SHA1
ASSISTANT_WINDOWS_TIMESTAMP_URL
```

The workflow imports the PFX only into the ephemeral runner user's `CurrentUser\My` store, verifies that its thumbprint matches `ASSISTANT_WINDOWS_CERT_SHA1`, runs the existing `desktop:release:build:public` path, then removes the imported certificate and temporary PFX file in an `always()` cleanup step.

The Tauri signing script still performs the authoritative `signtool` SHA-256 signing, RFC3161 timestamping and signature verification.

### Draft GitHub Release

A draft release may be requested only when:

```text
mode = signed-public
workflow ref = main
release_tag = v<current desktop version>
```

The build job itself keeps `contents: read`. A separate `draft-release` job receives `contents: write` only when draft creation is requested.

The release job refuses to overwrite an existing release/tag of the same release name. It creates a draft, uploads the generated `.exe` / `.msi` installers and `SHA256SUMS.txt`, and leaves final publication as an explicit human action after target-device acceptance.

## Release boundary

Repository automation can establish:

```text
Windows CI green
Android lint/unit/build green
release contract valid
candidate installers produced
signatures valid for signed-public mode
SHA-256 manifest produced
```

It cannot establish:

```text
58/58 physical acceptance checks
real Android SpeechRecognizer quality
Keystore persistence on the target phone
DPAPI persistence across the target Windows restart
real microphone/wake/SAPI cancellation behavior
Firewall/Tailscale mobile-data behavior
installed NSIS startup behavior on the target machine
```

The final release condition remains:

```text
Windows CI: green
Android CI: green for the release source
local release preflight: no Blocking
Quick -> Control -> System: no Blocking
Quick -> Release: 58/58 Passed
installed NSIS candidate: validated
signed-public policy: validated when publishing publicly
```

## Main-branch protection

`main` should be protected after the Android workflow is merged and has produced its first stable check name.

Recommended repository rules:

```text
Require a pull request before merging
Require status checks to pass
  - Windows test and compile
  - Android lint, test and assemble
Require branches to be up to date before merging
Block force pushes
Block branch deletion
```

Repository branch protection is a GitHub Administration setting rather than source code. The repository connector used for source changes does not expose Administration mutations, so this setting must be enabled in GitHub repository Settings after the workflows are merged.

Do not make the manual Windows Release workflow a required PR status check; it is intentionally dispatch-only and release-oriented.
