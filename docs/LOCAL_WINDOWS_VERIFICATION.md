# Local Windows Verification

This document defines the first full local Windows verification path without adding CI or automatically running tests.

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
- invalid relative `ASSISTANT_ZIPFORMER_MODEL_DIR` override;
- existing runtime permission policy parses correctly.

### Informational checks

- `cl.exe` visible in PATH;
- WebView2 filesystem probe;
- debug/release `assistant-mcp.exe` build output;
- Tauri target-triple staged sidecar;
- generated app-local-data MCP config;
- context app-local-data directory;
- baseline permission policy state;
- Zipformer `bpe.model` preparation file.

### Optional checks

- Vietnamese Zipformer STT runtime bundle;
- each required Zipformer runtime file;
- wake-word model directory and wake resources.

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
assistant resources install stt_zipformer_vi
```

Because Tauri `beforeDevCommand` already stages the debug MCP sidecar, the explicit staging command is mainly useful when isolating sidecar build problems.

## First verification workflow

1. Run `verify-local.ps1` before building.
2. Resolve blocking prerequisite results.
3. Run `pnpm install` if dependencies are not installed.
4. Start `pnpm --dir apps/desktop tauri dev`.
5. Run `assistant status` and `assistant doctor`.
6. Compare the preflight script with runtime readiness/status output.
7. Verify generated MCP config under app-local-data.
8. Verify text conversation before enabling optional voice/wake features.
9. Install `stt_zipformer_vi` through `assistant resources install stt_zipformer_vi` when voice testing is needed.
10. Verify safe MCP tools first: system info, window list, active window, volume reads.
11. Verify Moderate tools with policy controls.
12. Verify Sensitive UIA/window-close actions display confirmation and require Allow once.
13. Verify app startup from a non-repository working directory/shortcut.
14. Only after dev runtime is stable, run the release Tauri bundle locally.

## App-local-data paths

Default Windows root:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop
```

Expected structure after startup/resource installation:

```text
com.voduong.assisstantdesktop\
├── context\
├── models\
│   ├── stt\
│   │   └── sherpa-onnx-zipformer-vi-30M-int8-2026-02-09\
│   └── wake\
├── permissions\
├── audit\
└── runtime\
    └── .agents\
        └── mcp_config.json
```

The script reports these paths but does not create them. The desktop application/resource installer creates them as needed.

## Voice verification

Voice remains optional during initial integration. The primary recognizer is sherpa-onnx Vietnamese Zipformer, not Whisper.

Default STT directory:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\models\stt\sherpa-onnx-zipformer-vi-30M-int8-2026-02-09
```

Required runtime files:

```text
encoder.int8.onnx
decoder.onnx
joiner.int8.onnx
tokens.txt
```

`bpe.model` is retained for preparation/context work but is not required by the current offline recognition path.

The directory can be overridden for diagnostics with an **absolute** path:

```text
ASSISTANT_ZIPFORMER_MODEL_DIR=C:\absolute\path\to\zipformer-model
```

Without a complete runtime bundle:

```text
Text assistant      → usable
TTS                 → usable
Zipformer STT       → optional missing/incomplete
Wake-to-voice       → not fully usable
```

Install the pinned STT resource through the same installer used by the desktop runtime:

```powershell
assistant resources install stt_zipformer_vi
```

## Wake verification

Wake resources live below:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\models\wake
```

The exact sherpa model/keywords paths are reported by the desktop wake status/readiness path.

## What to report after local verification

When a local failure occurs, capture:

- failing command;
- full compiler/runtime error text;
- readiness/status output;
- verifier output;
- whether build is debug or release;
- Rust host target triple;
- whether the app was launched from repo root, shortcut, or installed package.

Do **not** include Antigravity credentials, permission broker secret, private clipboard contents, screenshots with sensitive information, or unrelated personal files.

## Remote development rule

The remote development workflow does not execute tests, builds, GitHub Actions, model downloads, installers, or native Windows runtime verification.
