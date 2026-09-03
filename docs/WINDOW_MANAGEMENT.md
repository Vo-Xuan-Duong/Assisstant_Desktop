# Phase 11B — Safe top-level window management

## Purpose

Phase 11B adds practical desktop window management now that the permission gateway and explicit source-window context are established.

The new operations are semantic Win32 actions, not pixel automation:

```text
window_set_state
window_close
```

## Stable target identity

Every mutating window operation requires both:

```text
window_handle
expected_process_id
```

The Context Engine already provides the source application's:

```text
active_window_handle
active_window_title
active_process_id
active_executable
```

The native layer reads the current owner of the HWND immediately before acting. If it no longer matches `expected_process_id`, the operation fails as a stale target.

This protects against Windows reusing an HWND for a different process between context capture and execution.

## `window_set_state`

Supported states:

```text
minimize
maximize
restore
```

Risk:

```text
Moderate
```

Native implementation uses `ShowWindow` with the corresponding Win32 visual-state command.

`ShowWindow`'s BOOL return value describes the window's previous visibility state and is not interpreted as a normal success/failure code.

Example input:

```json
{
  "window_handle": 123456,
  "expected_process_id": 9876,
  "state": "maximize"
}
```

The Moderate runtime policy panel can configure Default / Allow / Ask / Deny.

## `window_close`

Risk:

```text
Sensitive
```

The operation uses:

```text
PostMessageW(hwnd, WM_CLOSE, ...)
```

This is a graceful close request, not process termination.

The application can still:

- show a save/unsaved-changes dialog;
- cancel closing;
- perform normal shutdown cleanup;
- keep running if its own close policy requires it.

No `TerminateProcess`, taskkill, shell command, or process kill primitive is introduced.

Because `window_close` is Sensitive, it always follows the Sensitive permission policy and cannot be downgraded by the Moderate runtime override file.

## Recommended agent flow

```text
Desktop Context
  active_window_handle
  active_process_id
        |
        v
choose explicit target
        |
        +----------------------+
        |                      |
        v                      v
window_set_state          window_close
Moderate                  Sensitive
        |                      |
runtime policy           confirmation broker
        |                      |
        v                      v
native stale-PID check -> Win32 action
```

## MCP router

Window management has a dedicated router:

```text
server/window_tools.rs
```

The server composition is now:

```text
system_tool_router
+ ui_tool_router
+ virtualized_tool_router
+ window_tool_router
```

All routers share the same `McpPermissionGateway`.

## Result contract

Both actions return metadata captured before the native action:

```json
{
  "ok": true,
  "action": "maximize",
  "window_before_action": {
    "title": "...",
    "process_id": 9876,
    "executable": "..."
  }
}
```

For close:

```text
action = close_requested
```

The result means the standard close request was posted; it does not promise that the application actually exited.

## Safety boundaries

Phase 11B deliberately excludes:

- force process termination;
- arbitrary process kill by name/PID;
- shell commands;
- raw keyboard shortcuts such as Alt+F4;
- raw mouse interaction;
- moving/resizing windows by arbitrary coordinates;
- closing a window using HWND alone without PID verification.

## Local verification checklist

1. capture an external app's HWND + PID through normal desktop context;
2. minimize it and confirm the correct window changes state;
3. restore it;
4. maximize it;
5. intentionally pass a wrong expected PID and confirm no action occurs;
6. set `window_set_state` to Ask and confirm the Moderate permission override path works;
7. call `window_close` and confirm the Sensitive confirmation modal appears;
8. deny close and confirm no WM_CLOSE is posted;
9. approve close on an app with unsaved content and confirm its native save prompt can still appear;
10. confirm no process termination primitive is used.

## Verification policy

No GitHub Actions or runtime tests are executed during remote development. Win32 behavior remains intentionally verified locally on Windows.
