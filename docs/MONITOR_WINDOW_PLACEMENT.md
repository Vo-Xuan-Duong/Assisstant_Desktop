# Monitor Discovery and Window Placement

Phase 11D adds semantic monitor geometry and explicit top-level window placement without synthetic mouse dragging.

## Public MCP tools

### `display_list`

Risk: `Safe`

Returns monitor records with:

```text
monitor_handle
bounds
work_area
primary
```

`bounds` is the full monitor rectangle. `work_area` excludes desktop-reserved areas such as taskbars and should normally be preferred for assistant-managed window placement.

Monitor enumeration is capped at 32 records while the Win32 callback continues successfully after the cap.

### `window_set_bounds`

Risk: `Moderate`

Input:

```text
window_handle
expected_process_id
x
y
width
height
```

The coordinates use the Windows virtual-desktop coordinate space, so monitors left/above the primary display can have negative coordinates.

Native flow:

```text
window_set_bounds
      |
      v
validate dimensions / coordinate bounds
      |
      v
resolve HWND metadata
      |
      v
compare current PID to expected_process_id
      |
      +-- mismatch --> reject stale/recycled target
      |
      v
SetWindowPos(
  SWP_NOZORDER |
  SWP_NOACTIVATE
)
```

The operation intentionally does not activate the target and does not modify its Z-order. `window_activate` is a separate explicit action.

## Validation

Native validation rejects:

- width or height <= 0;
- width or height above 32768 pixels;
- x/y coordinates outside ±100000 pixels;
- `expected_process_id = 0`;
- a target HWND whose current PID no longer matches the expected PID.

These limits prevent accidental extreme placement values while retaining practical multi-monitor layouts.

## Example

User intent:

```text
"Đưa Notepad sang nửa trái màn hình thứ hai"
```

Semantic sequence:

```text
window_list
   ↓
select Notepad HWND + PID

display_list
   ↓
select monitor work_area
   ↓
calculate left-half rectangle

window_set_bounds(HWND, PID, x, y, width/2, height)
```

No mouse drag or coordinate click is synthesized.

## Permission behavior

```text
display_list       Safe
window_set_bounds  Moderate
window_activate    Moderate
window_set_state   Moderate
window_close       Sensitive
```

Moderate actions remain configurable with runtime `Default / Allow / Ask / Deny` overrides.

## Non-goals

Phase 11D does not add:

- raw mouse drag;
- keyboard shortcuts for snapping;
- automatic focus changes;
- Z-order changes;
- always-on-top manipulation;
- force-moving inaccessible/system windows;
- display mode/resolution changes.

## Local verification checklist

1. Verify `display_list` returns every connected monitor and exactly one primary monitor in a normal setup.
2. Compare `bounds` vs `work_area` where a taskbar is present.
3. Place a normal window using its existing HWND/PID.
4. Place a window on a monitor with negative virtual-desktop coordinates if available.
5. Verify placement does not activate a background window.
6. Verify placement does not change Z-order unexpectedly.
7. Pass width=0 or negative dimensions and confirm rejection.
8. Pass an incorrect expected PID and confirm stale-target rejection.
9. Configure `window_set_bounds` to Ask/Deny in runtime Moderate policy and verify the policy path locally.

No GitHub Actions or runtime tests are executed during the remote development phase.
