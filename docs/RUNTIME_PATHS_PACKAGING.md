# Runtime Paths & Packaging Hardening

Phase 12B removes assumptions that the desktop application is launched from the repository root.

## Runtime layout

The desktop resolves Tauri app-local-data first, then prepares:

```text
<app-local-data>/
├── context/
├── models/
│   ├── whisper/
│   └── wake/
├── permissions/
├── audit/
└── runtime/
    └── .agents/
        └── mcp_config.json
```

`ASSISTANT_RUNTIME_DIR` can override only the Antigravity/MCP runtime directory. Context, permission and default model storage remain under application local data.

## Antigravity working directory

`AntigravityConfig.working_directory` is set to:

```text
<app-local-data>/runtime
```

Antigravity therefore discovers:

```text
.agents/mcp_config.json
```

from a deterministic directory regardless of whether the desktop app was started from Explorer, a shortcut, Windows startup, a terminal or an installer-created entry.

## Generated MCP config

The desktop creates the runtime MCP config before Antigravity is constructed.

It contains an absolute path to `assistant-mcp.exe` plus the runtime directory as MCP cwd. The file contains no permission broker secret or Antigravity credential.

The repository keeps `.agents/mcp_config.example.json` for documentation only. Runtime code does not depend on a tracked `.agents/mcp_config.json`.

## MCP binary resolution

Resolution order:

```text
1. ASSISTANT_MCP_BINARY
2. Tauri resource directory / assistant-mcp.exe
3. debug workspace target fallback (debug build only)
4. release workspace target fallback (debug build only)
5. expected bundled sidecar path, even if missing
```

Step 5 allows Readiness Diagnostics to report the exact expected packaged location.

## Tauri sidecar bundle

Tauri 2 `bundle.externalBin` is configured with:

```json
"externalBin": [
  "binaries/assistant-mcp"
]
```

Tauri requires the source binary to include its target triple suffix. The repository does not commit generated executable files.

Before Tauri dev/build, the Node staging script:

```text
apps/desktop/scripts/stage-sidecar.mjs
```

performs:

```text
cargo build -p windows-mcp --bin assistant-mcp
        ↓
read rustc host target triple
        ↓
copy binary
        ↓
src-tauri/binaries/assistant-mcp-<target-triple>.exe
        ↓
Tauri externalBin bundling
```

Release bundling uses `cargo build --release` for the sidecar.

The script respects `CARGO_TARGET_DIR` when locating Cargo output.

## Current build scope

The staging script intentionally supports native Windows hosts only. Cross-compilation is not silently guessed. If cross-target packaging is needed later, the target triple must become an explicit build input and both Cargo and Tauri must use the same target.

## Context storage migration

Before Phase 12B:

```text
.assistant/context
```

was relative to the process working directory.

After Phase 12B:

```text
<app-local-data>/context
```

is injected into `ContextEngine` through `ContextConfig` at desktop setup.

The existing one-artifact replacement behavior remains unchanged: context capture does not accumulate screenshots indefinitely.

## Environment overrides

Supported runtime overrides relevant to paths:

```text
ASSISTANT_RUNTIME_DIR
ASSISTANT_MCP_BINARY
ASSISTANT_WHISPER_MODEL
ASSISTANT_WAKE_MODEL_DIR
ASSISTANT_WAKE_KEYWORDS
```

Overrides are intended for local development, diagnostics and recovery. Normal packaged operation should resolve the bundled MCP sidecar and standard app-local-data directories without overrides.

## Security properties

- generated MCP config contains no broker secret;
- broker address/secret continue to flow only through the Antigravity process environment;
- MCP binary path is explicit rather than shell-resolved;
- app data is not stored next to the executable;
- context screenshots are no longer affected by arbitrary working directories;
- missing sidecar remains fail-visible through readiness rather than causing an implicit shell fallback.

## Local verification checklist

1. `pnpm tauri dev` stages a debug sidecar before Vite starts.
2. `pnpm tauri build` stages a release sidecar before frontend build/bundle.
3. The target-triple sidecar file is generated under `src-tauri/binaries/` and remains untracked.
4. Packaged Windows output contains `assistant-mcp.exe` as the Tauri external binary.
5. Starting the app from a non-repository working directory still generates app-local-data runtime config.
6. Generated MCP config contains an absolute sidecar command path.
7. Antigravity runs with `<app-local-data>/runtime` as cwd.
8. Context screenshot artifacts are written under `<app-local-data>/context`.
9. Removing the packaged sidecar makes Windows MCP readiness Blocking with the expected path.
10. `ASSISTANT_MCP_BINARY` can temporarily point to a developer binary without editing runtime config manually.

Remote development does not execute these build/runtime checks; they are reserved for local Windows verification.
