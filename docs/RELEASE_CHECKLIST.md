# Release checklist

This document defines the Windows release gate for Assisstant Desktop. It is intentionally local-first: GitHub Actions are not part of the current release path.

## Release shape

The supported release target is Windows with an NSIS current-user installer.

Windows builds load `tauri.windows.conf.json`, which enables both product voice features:

```text
voice-whisper
wake-word
```

and overrides the generic bundle target with:

```text
nsis
```

The packaged desktop must include the `assistant-mcp` external sidecar. The existing Tauri `beforeBuildCommand` stages a release build of that sidecar before the frontend bundle is produced.

## Required local environment

- Windows 10/11 x64 with the MSVC Rust toolchain.
- Rust compatible with workspace `rust-version = 1.85`.
- Node.js `20.19+` or `22.12+`.
- pnpm.
- Microsoft Edge WebView2 runtime / installer prerequisites.
- Antigravity CLI (`agy`) available for end-to-end runtime verification.
- Visual Studio C++ Build Tools if the MSVC linker/native dependencies require them.

## Repository gates

Before a release candidate is built, all of these must be true:

- `LICENSE` exists and matches the workspace MIT declaration.
- `pnpm-lock.yaml` is generated, reviewed and committed.
- a branded Windows icon is approved and committed at `apps/desktop/src-tauri/icons/icon.ico` together with the normal Tauri icon set used by packaging.
- `tauri.windows.conf.json` enables `voice-whisper` and `wake-word`.
- Windows bundle target is exactly `nsis`.
- NSIS install mode remains `currentUser` unless the security/elevation model is deliberately changed.
- `binaries/assistant-mcp` remains declared in Tauri `externalBin`.
- desktop/Tauri versions are synchronized.
- working tree is clean and the release commit is reviewed.

Run the read-only gate:

```powershell
pnpm desktop:release:verify
```

It does not build, install, sign, download resources or invoke GitHub Actions.

## Local functional verification

Run the existing prerequisite/runtime verifier first:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-local.ps1
```

Then perform the Phase 14A lifecycle checks in `docs/WINDOWS_LIFECYCLE.md`, including:

- normal startup;
- second-launch single-instance behavior;
- tray show/hide;
- opt-in Windows startup;
- `--background` startup;
- global shortcut;
- wake activation when wake resources are installed.

Also verify:

- text request → Antigravity/Gemini response;
- permission-gated MCP action;
- permission Ask/Allow once/Deny paths;
- microphone → VAD → Whisper → Gemini → SAPI voice turn;
- wake phrase generation/hot reload;
- readiness/resource panels;
- runtime paths under app-local-data after install.

## Dependency lockfile

The repository currently does not generate a lockfile remotely. Create it on the release workstation:

```powershell
pnpm install --lockfile-only
```

Review `pnpm-lock.yaml`, then commit it. Subsequent release dependency installs should use the frozen lockfile:

```powershell
pnpm install --frozen-lockfile
```

A public release must not be cut while the release verifier reports the lockfile as blocking.

## Release icon

Do not publish with a placeholder/generic executable icon. Create and review the final product artwork, then generate the Tauri icon set locally. The required Windows release gate checks:

```text
apps/desktop/src-tauri/icons/icon.ico
```

If using the Tauri icon generator, review every generated file before committing it.

## Build release candidate

After all non-signing gates pass:

```powershell
pnpm install --frozen-lockfile
pnpm desktop:release:verify
pnpm desktop:release:build
```

The build hooks should:

1. build `windows-mcp` / `assistant-mcp.exe` in release mode;
2. stage it with the Rust host target triple under `src-tauri/binaries/`;
3. build the React frontend;
4. compile the Tauri desktop with Whisper and wake-word features;
5. produce an NSIS installer.

Expected installer output is under the Tauri/Cargo release bundle `nsis` directory for the active target.

## Installed-package verification

Test the installer on a clean or disposable Windows user profile, not only from the repository checkout.

Verify that:

- installation does not require Administrator privileges in current-user mode;
- installed app starts without repository-relative paths;
- `assistant-mcp.exe` is present and the generated runtime MCP config points to the installed sidecar;
- app-local-data directories are created correctly;
- optional Whisper/wake resources can be installed/prepared after installation;
- uninstall removes application binaries while not unexpectedly deleting user-owned runtime data unless that behavior is explicitly chosen;
- install → update/reinstall → uninstall flows do not leave a broken Windows startup entry.

## Code signing policy

Local unsigned builds are acceptable for development only.

Before distributing a production installer publicly, configure and review Windows code signing. Tauri supports Windows signing through the Windows bundle signing configuration. The exact certificate/account/signing command is intentionally not committed until a real signing identity is available.

Enforce this gate with:

```powershell
pnpm desktop:release:verify:public
```

The public-release verifier fails while no reviewed Windows `signCommand` is configured.

Do not commit certificate private keys, passwords, client secrets or signing tokens to the repository.

## Updater policy

Automatic updater artifacts are intentionally disabled for now. Do not enable the updater until all of the following are defined together:

- trusted update endpoint;
- updater public/private signing key lifecycle;
- version publication process;
- rollback/recovery policy;
- installer/update test matrix.

Manual signed releases are the safer initial distribution model.

## Versioning

For each release:

1. choose a semantic version;
2. update the workspace/Tauri/frontend versions consistently;
3. re-run `verify-release.ps1`;
4. build and smoke-test the installed package;
5. sign the public installer;
6. verify the signature on the final artifact;
7. create the Git tag/release only after the exact signed artifact has passed verification.

Never reuse a version/tag for different binaries.

## Current external blockers

These are not safely inventable during remote code development and must be supplied/approved locally before the first public release:

1. final application icon/branding;
2. generated and reviewed `pnpm-lock.yaml`;
3. real Windows code-signing identity/configuration;
4. full compile/runtime/install verification on Windows.

Compiler/runtime findings discovered during these checks override this checklist and should be fixed before adding further product capability.
