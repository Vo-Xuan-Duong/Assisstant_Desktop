# Phase 10A — Windows UI Automation RangeValue

## Purpose

Phase 10A adds deterministic control of bounded numeric Windows UI controls through UI Automation `RangeValuePattern`.

Typical controls include:

- sliders;
- numeric spinners;
- bounded numeric inputs;
- application-specific controls that expose `RangeValuePattern`.

The implementation remains semantic UI Automation. It does **not** synthesize mouse movement, drag coordinates, wheel events, or keyboard input.

## Inspection state

`ui_inspect` now returns a `range_value` object when an element exposes `RangeValuePattern`:

```text
range_value = {
  value,
  minimum,
  maximum,
  small_change,
  large_change,
  read_only
}
```

Numeric range state is intentionally included in structural context because it is bounded control state rather than arbitrary user-entered text.

Editable `ValuePattern` text is still not automatically read.

## Native action

The Windows tools layer exposes:

```text
set_range_value(window_handle, path, value)
```

Before calling Windows UI Automation it validates:

1. `value` is finite;
2. the element still resolves under the exact inspected root HWND;
3. the element exposes `RangeValuePattern`;
4. the range is not read-only;
5. `minimum <= value <= maximum`.

Only then does the native layer call `IUIAutomationRangeValuePattern::SetValue`.

## MCP tool

Public tool:

```text
ui_set_range_value
```

Arguments:

```json
{
  "window_handle": 0,
  "path": [0, 2, 1],
  "value": 50.0
}
```

The exact `window_handle` and child-index `path` must come from the relevant `ui_inspect` snapshot.

If the tree changed and the path is stale, the action fails. The assistant must inspect again instead of guessing.

## Permission classification

`ui_set_range_value` is classified:

```text
Sensitive
```

Therefore the default execution path is:

```text
ui_inspect
    |
    v
choose range element/value
    |
    v
ui_set_range_value
    |
    v
Permission Gateway
    |
    v
Desktop confirmation
    |
    +-- Allow once -> native SetValue
    +-- Deny       -> no Windows mutation
```

Runtime Moderate overrides cannot downgrade this requirement because the tool is not Moderate.

## Example

For a slider with:

```text
range_value.value   = 30
range_value.minimum = 0
range_value.maximum = 100
range_value.read_only = false
```

A request such as:

```text
"Đặt slider này thành 60"
```

can map to:

```text
ui_set_range_value(
  inspected_window_handle,
  inspected_path,
  60
)
```

The user still sees the Sensitive confirmation modal before execution.

## Local verification checklist

No GitHub Actions or runtime tests are run in this project phase.

Verify locally on Windows:

1. inspect an application with a slider/range control;
2. verify `range_value` includes current value, bounds, increments and read-only state;
3. request an in-range value and verify the confirmation modal appears;
4. Allow once and verify the value changes;
5. Deny and verify no value changes;
6. request a value below minimum and verify native validation rejects it;
7. request a value above maximum and verify native validation rejects it;
8. verify a read-only range is rejected;
9. change the UI tree after inspection and verify a stale path fails rather than targeting a different element;
10. confirm permission audit records tool metadata but not arguments.

## Deferred

Phase 10A does not add:

- raw mouse drag;
- raw wheel input;
- raw keyboard typing;
- coordinate click;
- vision-based target coordinates;
- unrestricted generic UI Automation pattern invocation.

Those remain outside the semantic computer-use boundary.
