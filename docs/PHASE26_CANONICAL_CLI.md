# Phase 26 — Canonical Satellite CLI

## Goal

Make `assistant satellite ...` the normal user-facing management surface without duplicating or rewriting the existing satellite pairing/trust implementation.

## Runtime layout

Windows packaging now stages four management/tool binaries:

```text
assistant.exe
assistant-core.exe
assistant-satellite.exe
assistant-mcp.exe
```

Responsibilities:

- `assistant.exe` — lightweight canonical command router;
- `assistant-core.exe` — existing Assisstant Desktop management CLI implementation;
- `assistant-satellite.exe` — Android Voice Satellite pairing/trusted-device implementation;
- `assistant-mcp.exe` — Windows MCP server sidecar.

The router is intentionally small. It does not reimplement pairing, QR encoding, trusted-device storage, revoke logic, runtime management, or resource management.

## Dispatch rules

Normal commands are forwarded unchanged:

```text
assistant status
assistant doctor
assistant ai show
assistant wake show
assistant resources list
assistant permissions list
```

become calls to `assistant-core` with the same arguments.

Satellite commands:

```text
assistant satellite show
assistant satellite pair --qr
assistant satellite devices
assistant satellite revoke-device <device-id>
```

remove only the `satellite` namespace token and forward the remaining arguments to `assistant-satellite`.

Global `--data-dir` is preserved, including:

```text
assistant --data-dir C:\AssistantData satellite devices
```

The compatibility binary remains usable:

```text
assistant-satellite devices
```

but new documentation should prefer `assistant satellite ...`.

## Sidecar resolution

The canonical router looks for the delegated binaries beside itself. It supports both installed names and target-triple-staged names, for example:

```text
assistant-core.exe
assistant-satellite.exe
```

or:

```text
assistant-core-x86_64-pc-windows-msvc.exe
assistant-satellite-x86_64-pc-windows-msvc.exe
```

Development overrides are available when needed:

```text
ASSISTANT_CORE_CLI
ASSISTANT_SATELLITE_CLI
```

A missing/ambiguous sibling fails closed with a clear error rather than silently executing an unrelated binary from `PATH`.

## Packaging

`apps/desktop/scripts/stage-sidecar.mjs` now:

1. builds the existing `assistant` management binary;
2. builds `assistant-root` as the canonical router;
3. builds `assistant-satellite`;
4. stages `assistant-root` as `binaries/assistant-<target>.exe`;
5. stages the old management binary as `binaries/assistant-core-<target>.exe`;
6. stages satellite as `binaries/assistant-satellite-<target>.exe`.

Tauri `externalBin` includes all three management binaries plus `assistant-mcp`.

## Local verification checklist

Do this locally on Windows; no remote build/test/Actions execution is implied.

1. Run the normal sidecar staging command.
2. Confirm `binaries/` contains target-triple variants of `assistant`, `assistant-core`, `assistant-satellite`, and `assistant-mcp`.
3. Run staged `assistant ... status` and confirm behavior matches the previous management CLI.
4. Run `assistant ... help` and confirm the satellite namespace hint is appended.
5. Run `assistant satellite show`.
6. Run `assistant satellite pair --qr --host <PC-LAN-IP>`.
7. Run `assistant satellite devices` after an Android connection.
8. Confirm the older `assistant-satellite devices` command still works.
9. Package/install locally and repeat steps 3–8 from the installed binary layout.
