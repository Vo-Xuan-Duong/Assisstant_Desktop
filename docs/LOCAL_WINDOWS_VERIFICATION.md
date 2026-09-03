# Local Windows Verification

Phase 12C prepares the project for the first full local Windows verification without adding CI or automatically running tests.

## Verification harness

From the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-local.ps1
```

For machine-readable output:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-local.ps1 -Json
```

The script is read-only except for commands it invokes to query versions. It does **not** build the project, run tests, launch GitHub Actions, start Antigravity sessions, modify runtime policy, or download models.

## What it checks

### Blocking prerequisites

- Windows host;
- repository root structure;
- `rustc`;
- `cargo`;
- `pnpm`;
- `agy`;
- Rust host target is Windows MSVC;
- existing runtime permission policy parses correctly.

### Informational checks

- `cl.exe` visible in PATH;
- WebView2 filesystem probe;
- debug/release `assistant-mcp.exe` build output;
- Tauri target-triple staged sidecar;
- generated app-local-data MCP config;
- context app-local-data directory;
- baseline permission policy state.

### Optional checks

- Whisper model;
- wake-word model directory.

Missing optional resources do not cause a blocking exit code.

## Exit code

```text
0 → no blocking checks
1 → at least one blocking check
```

`info` and `optional` results do not make the script fail.

## Standard local sequence

The verifier only recommends these commands; it does not run them:

```powershell
pnpm install
pnpm --dir apps/desktop sidecar:stage:dev
pnpm --dir apps/desktop tauri dev
```

Because Tauri `beforeDevCommand` already stages the debug MCP sidecar, the explicit staging command is mainly useful when isolating sidecar build problems.

## First verification workflow

1. Run `verify-local.ps1` before building.
2. Resolve blocking prerequisite results.
3. Run `pnpm install` if dependencies are not installed.
4. Start `pnpm --dir apps/desktop tauri dev`.
5. Open the desktop **Readiness** panel.
6. Compare the preflight script with runtime readiness.
7. Verify generated MCP config under app-local-data.
8. Verify text conversation before enabling optional voice/wake features.
9. Verify safe MCP tools first: system info, window list, active window, volume reads.
10. Verify Moderate tools with policy controls.
11. Verify Sensitive UIA/window-close actions display confirmation and require Allow once.
12. Verify app startup from a non-repository working directory/shortcut.
13. Only after dev runtime is stable, run the release Tauri bundle locally.

## App-local-data paths

Default Windows root:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop
```

Expected structure after startup:

```text
com.voduong.assisstantdesktop\
├── context\
├── models\
├── permissions\
├── audit\
└── runtime\
    └── .agents\
        └── mcp_config.json
```

The script reports these paths but does not create them. The desktop application creates them as needed.

## Voice verification

Voice remains optional during initial integration.

Expected Whisper model default:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\models\whisper\ggml-base.bin
```

Without that file:

```text
Text assistant → usable
TTS            → usable
Whisper STT    → optional missing
Wake-to-voice  → not fully usable
```

## Wake verification

Wake resources live below:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\models\wake
```

The exact sherpa model/keywords paths are still reported by the desktop Wake status/readiness UI.

## What to report after local verification

When a local failure occurs, capture:

- failing command;
- full compiler/runtime error text;
- Readiness panel states;
- verifier output;
- whether build is debug or release;
- Rust host target triple;
- whether the app was launched from repo root, shortcut, or installed package.

Do **not** include Antigravity credentials, permission broker secret, private clipboard contents, screenshots with sensitive information, or unrelated personal files.

## Remote development rule

This phase adds the verification harness and documentation only. The remote development workflow still does not execute tests, builds, GitHub Actions, or native Windows runtime verification.
