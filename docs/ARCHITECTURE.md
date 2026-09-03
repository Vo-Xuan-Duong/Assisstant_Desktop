# Architecture

## Dependency direction

The project follows a strict inward dependency rule:

```text
UI / Voice / Platform adapters
            |
            v
      assistant-core
            |
            v
      common domain
```

External integrations implement core interfaces rather than being imported by the core.

Current relationship:

```text
assistant-common
      ^
      |
assistant-core
      ^
      |
antigravity-bridge
```

Later modules will follow the same pattern:

```text
windows-tools  <- windows-mcp
context-engine <- desktop app adapter
voice-runtime  <- desktop app adapter
```

## Process architecture

The intended production runtime contains three relevant processes:

```text
+-------------------------+
| desktop-assistant.exe   |
| Tauri + Rust core       |
+-----------+-------------+
            |
            | spawn + NDJSON stdin/stdout
            v
+-------------------------+
| agy                     |
| Antigravity CLI         |
+-----------+-------------+
            |
            | MCP stdio
            v
+-------------------------+
| assistant-mcp.exe       |
| Rust MCP server         |
+-----------+-------------+
            |
            v
       Windows APIs
```

The MCP server is a separate process so Antigravity can own its stdio transport and so tool execution can be isolated from the desktop UI process.

## Antigravity protocol

The bridge uses Antigravity's documented continuous streaming mode:

```text
agy --input-format stream-json --output-format stream-json
```

Input is one JSON object per line:

```json
{"event":"user","message":{"content":"..."}}
```

Output is parsed as NDJSON events. A turn is complete only when a `result` event is received. The bridge treats a `result.status` other than `SUCCESS` as an agent error.

The bridge intentionally ignores unknown future event types so protocol additions do not crash the desktop runtime.

## Assistant state machine

Initial states:

```text
Idle
Listening
Processing
Executing
Speaking
Confirming
Error
```

The current text path is deliberately single-flight:

```text
Idle -> Processing -> Idle
                    \
                     -> Error
```

Later voice and tool phases extend existing transitions rather than replacing the state model.

## Windows tool rule

AI-visible tools must be explicit, narrow operations.

Good:

```text
audio.set_volume(value)
apps.open(name)
window.get_active()
```

Not allowed as a normal assistant tool:

```text
shell.execute(command)
```

Native implementation is kept below the MCP layer so it can be tested and reused independently of MCP.

## Context rule

Context collection is demand-driven. The assistant does not continuously upload the screen.

Examples:

- `Mở Chrome` -> no screenshot.
- `Lỗi trên màn hình này là gì?` -> active-window metadata + screenshot may be collected.

UI Automation should be preferred over OCR/pixel clicking when Windows exposes an accessible UI tree.

## UI rule

The system-level UI is an adapter over assistant state. It never owns business logic.

Target edge effect architecture:

```text
EdgeOverlayManager
  |- top edge
  |- right edge
  |- bottom edge
  |- left edge
  `- assistant bubble
```

The overlay must support click-through, no-activate behavior, active-monitor placement, and state-driven animations.

## Security boundary

There are two permission layers:

1. Antigravity permissions.
2. Assistant-owned tool policy.

The second layer remains authoritative for product safety. Tool categories are `Safe`, `Moderate`, `Sensitive`, and `Blocked`.

Sensitive operations require user confirmation in the desktop application. Blocked operations are not exposed to the model.

## Persistence

SQLite is planned for settings/history/permission decisions. Google credentials are never copied into project storage; authentication remains owned by Antigravity CLI.

## Failure model

The runtime must distinguish at least:

- authentication failure;
- quota exhaustion;
- Antigravity process exit;
- malformed protocol event;
- MCP/tool failure;
- permission denial;
- network failure;
- voice subsystem failure.

The agent backend is considered optional infrastructure: loss of cloud reasoning must not force the entire desktop process to exit.
