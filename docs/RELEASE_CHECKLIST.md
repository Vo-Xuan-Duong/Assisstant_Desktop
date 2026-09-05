# Release checklist

This document defines the Windows release gate for Assisstant Desktop. The release path remains local-first; GitHub Actions are not used.

## Release shape

Supported release target:

```text
Windows x64/MSVC
Tauri full features: voice-whisper + wake-word
NSIS current-user installer
assistant-mcp external sidecar
```

`tauri.windows.conf.json` is loaded automatically on Windows. Public signed builds add `tauri.windows.signed.conf.json` through Tauri's `--config` merge option.

## Required local environment

- Windows 10/11 x64.
- Rust `1.98.1` as pinned by `rust-toolchain.toml`.
- Node.js `20.19+` or `22.12+`.
- pnpm.
- Microsoft Edge WebView2 prerequisites.
- Antigravity CLI (`agy`) for end-to-end verification.
- Visual Studio C++ Build Tools / Windows SDK when native linking or signing requires them.

## First local release preparation

Run:

```powershell
pnpm desktop:release:prepare
```

This performs only deterministic preparation work:

1. materializes `apps/desktop/src-tauri/icons/icon.ico` from the tracked Base64 payload;
2. verifies the generated ICO against its pinned SHA-256;
3. runs `cargo generate-lockfile`;
4. runs `pnpm install --lockfile-only --ignore-scripts`.

The generated ICO is intentionally ignored by Git because its reproducible source is committed as:

```text
apps/desktop/src-tauri/icons/app-icon.svg
apps/desktop/src-tauri/icons/icon.ico.b64
```

Review and commit the generated dependency lockfiles:

```text
Cargo.lock
pnpm-lock.yaml
```

Do not fabricate or hand-edit resolved dependency entries.

## Repository gates

Before a release candidate is built, all of these must be true:

- `LICENSE` matches the workspace MIT declaration;
- Rust toolchain remains pinned to `1.98.1` for the current baseline;
- `Cargo.lock` and `pnpm-lock.yaml` are committed;
- tracked icon SVG/Base64 payload exists;
- materialized `icons/icon.ico` matches the pinned SHA-256;
- `tauri.windows.conf.json` enables `voice-whisper` and `wake-word`;
- Windows bundle target is exactly `nsis`;
- NSIS install mode is `currentUser`;
- Windows bundle explicitly uses `icons/icon.ico`;
- `binaries/assistant-mcp` remains in Tauri `externalBin`;
- desktop/Tauri versions match;
- working tree is clean.

Read-only verification:

```powershell
pnpm desktop:release:verify
```

The verifier never builds, installs, signs, downloads models, or invokes GitHub Actions.

## Release icon contract

Human-editable source:

```text
apps/desktop/src-tauri/icons/app-icon.svg
```

Reproducible Windows ICO payload:

```text
apps/desktop/src-tauri/icons/icon.ico.b64
```

Materialized build asset:

```text
apps/desktop/src-tauri/icons/icon.ico
```

Materialize only assets with:

```powershell
pnpm desktop:assets:prepare
```

This command is safe to run before dev/build and does not touch dependency lockfiles.

## Local unsigned release candidate

After the two lockfiles are committed:

```powershell
pnpm install --frozen-lockfile
pnpm desktop:release:build
```

`desktop:release:build` materializes the icon, runs the read-only release verifier, then invokes the normal Tauri Windows release build.

The Tauri build hooks then:

1. build `windows-mcp` / `assistant-mcp.exe` in release mode;
2. stage the target-triple sidecar under `src-tauri/binaries/`;
3. build the React frontend;
4. compile the desktop with Whisper + wake-word features;
5. produce the NSIS installer.

Unsigned builds are development/release-candidate artifacts only.

## Public Windows signing

The repository contains a generic signing overlay and script but no certificate identity or secret:

```text
apps/desktop/src-tauri/tauri.windows.signed.conf.json
apps/desktop/src-tauri/scripts/sign-windows.ps1
```

The signed overlay configures Tauri `bundle.windows.signCommand`. Tauri replaces `%1` with each file that must be signed; the script uses Windows `signtool.exe`, signs with SHA-256, timestamps the artifact, and immediately verifies the resulting Authenticode signature.

Set these local environment variables before public verification/build:

```powershell
$env:ASSISTANT_WINDOWS_CERT_SHA1 = "<40-character certificate thumbprint>"
$env:ASSISTANT_WINDOWS_TIMESTAMP_URL = "<your certificate provider RFC3161 timestamp URL>"
```

The certificate must be installed in:

```text
Cert:\CurrentUser\My
```

and expose its private key.

Do not commit PFX files, private keys, passwords, signing tokens, or cloud signing secrets.

Public gate:

```powershell
pnpm desktop:release:verify:public
```

It additionally verifies:

- the signed config calls the reviewed signing script and preserves `%1`;
- thumbprint syntax is valid;
- timestamp URL is absolute HTTP(S);
- the matching CurrentUser certificate exists and has a private key;
- the certificate is not expired;
- `signtool.exe` is available.

Public signed build:

```powershell
pnpm desktop:release:build:public
```

This merges `tauri.windows.signed.conf.json` on top of the normal Windows config and therefore keeps the same features, installer target and icon contract while adding signing.

## Local functional verification

Run the prerequisite/runtime verifier:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-local.ps1
```

Then verify at minimum:

- normal startup and second-launch single-instance behavior;
- tray show/hide and opt-in Windows startup;
- `--background` startup;
- global shortcut;
- text → Antigravity/Gemini response;
- permission Ask / Allow once / Deny paths;
- permission-gated MCP mutations;
- microphone → VAD → Whisper → Gemini → SAPI;
- wake detection, phrase generation and hot reload;
- readiness/resource panels;
- runtime paths under app-local-data.

## Installed-package verification

Test the NSIS installer on a clean/disposable Windows user profile.

Verify that:

- current-user installation does not require Administrator privileges;
- installed app starts without repository-relative paths;
- bundled `assistant-mcp.exe` is present;
- generated runtime MCP config points to the installed sidecar;
- app-local-data directories are created correctly;
- optional Whisper/wake resources can be installed after installation;
- startup enable/disable survives reinstall/update flows correctly;
- uninstall does not unexpectedly delete user-owned runtime data.

For a public artifact also verify Authenticode on the final installer/executables with SignTool before publication.

## Updater policy

Automatic updater artifacts remain disabled. Do not enable the updater until all of these exist together:

- trusted update endpoint;
- updater signing-key lifecycle;
- version publication process;
- rollback/recovery policy;
- installer/update test matrix.

Manual signed releases remain the initial distribution model.

## Versioning

For each release:

1. choose a semantic version;
2. update workspace/Tauri/frontend versions consistently;
3. commit reviewed dependency lockfiles;
4. run release verification;
5. build and smoke-test the installed package;
6. for public distribution, sign and verify the exact final artifacts;
7. create the Git tag/release only after those exact artifacts pass verification.

Never reuse a version/tag for different binaries.

## Remaining machine-dependent gates

After Phase 15, repository-side release automation is complete. The remaining gates are intentionally machine/identity dependent:

1. generate and review `Cargo.lock` using the real resolver;
2. generate and review `pnpm-lock.yaml` using the real resolver;
3. perform native Windows compile/install/runtime verification;
4. for public distribution, provide a real trusted Windows code-signing certificate/private key.

Compiler/runtime/install findings override this checklist and must be fixed before publication.
