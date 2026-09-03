# Window Discovery and Activation

Phase 11C extends the explicit top-level window contract introduced in Phase 11B.

The goal is to let the assistant discover an application window without relying on the current foreground window, then request activation using a stable `HWND + process_id` identity pair.

## Public MCP tools

### `window_list`

Risk: `Safe`

Returns a bounded set of visible, titled top-level windows.

Each record contains:

```text
window_handle
process_id
title
executable
minimized
foreground
```

The native default is 80 records and the hard maximum is 200.

The payload is bounded, but Win32 enumeration itself is allowed to finish normally. The callback intentionally keeps returning `TRUE` after the payload cap is reached because returning `FALSE` would make `EnumWindows` report early termination/failure.

### `window_activate`

Risk: `Moderate`

Input:

```text
window_handle
expected_process_id
```

Native flow:

```text
window_activate
      |
      v
resolve HWND metadata
      |
      v
compare current PID with expected_process_id
      |
      +-- mismatch --> reject stale/recycled target
      |
      v
IsIconic?
      |
      +-- yes --> ShowWindow(SW_RESTORE)
      |
      v
SetForegroundWindow
      |
      +-- FALSE --> report Windows focus-policy refusal
      |
      v
success
```

Windows can refuse `SetForegroundWindow` because foreground focus changes are controlled by OS anti-focus-stealing rules. The assistant does not bypass that refusal with synthetic keyboard or mouse input.

## Target safety

Window mutation tools use the same identity contract:

```text
HWND + expected_process_id
```

A bare HWND is insufficient for mutation because Windows can recycle window handles. Native code verifies the current owning process immediately before the action.

## Relationship to Context Engine

When the request already refers to the app the user was viewing before the Assistant appeared, Desktop Context provides:

```text
active_window_handle
active_window_title
active_process_id
active_executable
```

`window_list` is intended for requests where the user refers to another open window or application and the target must be discovered.

Example:

```text
User: "Chuyển sang cửa sổ Notepad"

window_list
   -> find Notepad record
   -> HWND + PID
window_activate(HWND, PID)
```

## Permission behavior

```text
window_list     Safe
window_activate Moderate
window_set_state Moderate
window_close    Sensitive
```

Moderate actions remain subject to runtime `Default / Allow / Ask / Deny` policy overrides.

`window_close` remains Sensitive and therefore cannot be downgraded through the Moderate override file.

## Non-goals

Phase 11C does not add:

- raw mouse input;
- raw keyboard input;
- Alt+Tab simulation;
- force process termination;
- focus-stealing bypasses;
- arbitrary HWND mutation without PID validation;
- hidden/untitled window enumeration in the AI-facing list.

## Local verification checklist

Native/runtime verification is deferred to a local Windows machine.

Recommended checks:

1. Open several apps with visible top-level windows.
2. Call `window_list` and verify HWND/title/PID/executable fields.
3. Verify the foreground record has `foreground=true`.
4. Minimize a window and verify `minimized=true`.
5. Activate a normal visible window using its returned HWND/PID.
6. Activate a minimized window and verify it restores first.
7. Supply an incorrect `expected_process_id` and verify the action is rejected.
8. Verify policy `Ask/Deny` for `window_activate` works as configured.
9. Confirm no raw input is emitted when Windows refuses foreground activation.

No GitHub Actions or runtime tests are executed during the remote development phase.
