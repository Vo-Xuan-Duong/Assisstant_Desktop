# Windows System Control

## Scope

Phase 17 expands the Windows MCP surface with semantic system controls while preserving the existing permission gateway. It deliberately does **not** add an arbitrary shell/PowerShell command tool.

## New MCP commands

### Power and session

- `system_lock` — lock the interactive workstation.
- `system_logoff` — log off the current user.
- `system_shutdown` — immediate shutdown request.
- `system_restart` — immediate restart request.
- `display_turn_off` — ask Windows to power off displays until the next wake/input event.

`system_lock`, `system_logoff`, `system_shutdown`, and `system_restart` are Sensitive and require explicit desktop confirmation. Shutdown/restart/logoff call the Windows-owned `shutdown.exe` directly with fixed code-owned arguments; no user-controlled command line is accepted. Workstation locking uses the native Windows shutdown/session API.

### Process

- `process_terminate`

Termination requires both a process id and the executable name observed during recent discovery. Native code re-enumerates the process before opening it and refuses the action if the PID has been recycled to another executable. The assistant process cannot terminate itself.

### Filesystem

- `file_info`
- `file_list`
- `file_create_directory`
- `file_copy`
- `file_move`
- `file_delete`

Safety constraints:

- all paths must be absolute;
- list results are bounded;
- copy/move never overwrite an existing destination;
- copy accepts regular files only;
- symlink/junction move/delete is rejected;
- move/delete cannot target a filesystem root;
- delete is non-recursive;
- file mutations are Sensitive and require confirmation.

### Keyboard input

- `input_send_hotkey`
- `input_type_text`

Hotkeys accept at most five keys from a bounded vocabulary: modifiers/navigation keys, A-Z, 0-9 and F1-F12. Unicode typing is size-bounded. Both are Sensitive because keyboard injection can trigger consequential actions. UI Automation remains preferred when a semantic pattern is available.

## Permission contract

Every MCP method invokes `McpPermissionGateway::authorize` before touching Windows state. The public `windows-tools::TOOL_CATALOG` remains the source of risk classification.

Unknown tools fail closed. Sensitive tools cannot be downgraded through runtime Moderate overrides. Catalog tests assert that destructive system/session, process, filesystem and keyboard-input operations remain Sensitive.

## Not included yet

The following remain hardware/platform-dependent follow-up work rather than being implemented through shell hacks:

- monitor brightness via physical-monitor/DDC APIs;
- Wi-Fi radio control;
- Bluetooth radio control;
- sleep/hibernate policy and privilege handling;
- mouse coordinate fallback.

These should use official Windows APIs and explicit capability detection before being added to the public MCP surface.
