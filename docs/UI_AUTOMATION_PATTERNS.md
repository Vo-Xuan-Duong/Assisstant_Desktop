# Windows UI Automation Pattern Contract

## Purpose

This document describes the AI-facing structural state used by the deterministic Windows UI Automation layer. The assistant should inspect first, reason over semantic state, and then call one exact action using the same root HWND and child-index path.

## TogglePattern

Snapshot field:

```text
toggle_state: "off" | "on" | "indeterminate" | "unknown" | null
```

`null` means the element does not expose `TogglePattern`. `unknown` is reserved for a future/native value that is not recognized by the current normalized schema.

Action:

```text
ui_toggle(window_handle, path)
```

Typical controls:

- checkboxes;
- switches;
- toggle buttons.

The model must inspect `toggle_state` before calling the action because `ui_toggle` cycles state rather than setting an absolute desired value.

Risk classification: **Sensitive**.

## SelectionItemPattern

Snapshot field:

```text
is_selected: boolean | null
```

`null` means the element does not expose `SelectionItemPattern`.

Action:

```text
ui_select(window_handle, path)
```

Typical controls:

- list items;
- tabs;
- selectable options;
- tree/list choices.

Risk classification: **Sensitive**.

## ExpandCollapsePattern

Snapshot field:

```text
expand_collapse_state:
  "collapsed"
  | "expanded"
  | "partially_expanded"
  | "leaf"
  | "unknown"
  | null
```

`null` means the element does not expose `ExpandCollapsePattern`.

Action:

```text
ui_set_expanded(window_handle, path, expanded)
```

Typical controls:

- combo boxes;
- disclosure controls;
- tree nodes;
- menus;
- expandable panels.

Risk classification: **Moderate**.

## RangeValuePattern

Added in Phase 10A.

Snapshot field:

```text
range_value: {
  value,
  minimum,
  maximum,
  small_change,
  large_change,
  read_only
} | null
```

Action:

```text
ui_set_range_value(window_handle, path, value)
```

Risk classification: **Sensitive**.

See `UI_AUTOMATION_RANGE_VALUE.md` for the complete RangeValue contract.

## ScrollPattern

Snapshot field:

```text
scroll: {
  horizontally_scrollable,
  vertically_scrollable,
  horizontal_percent,
  vertical_percent
} | null
```

Windows UI Automation can return `-1` for a scroll percentage when that axis is not scrollable.

Action:

```text
ui_scroll(
  window_handle,
  path,
  horizontal,
  vertical
)
```

Supported relative amounts:

```text
large_decrement
small_decrement
none
large_increment
small_increment
```

Risk classification: **Moderate**.

The tool exposes bounded relative scrolling only. Arbitrary wheel-event synthesis and coordinate scrolling remain out of scope.

## Stable state serialization

Native `windows-rs` returns typed UI Automation state wrappers such as `ToggleState` and `ExpandCollapseState`. Those native types are normalized inside `windows-tools` before serialization.

The MCP/model contract must therefore depend on semantic strings, not Windows integer values:

```text
Windows wrapper enum
       |
       v
windows-tools normalization
       |
       v
stable snake_case JSON enum
```

This prevents model prompts and downstream consumers from depending on native numeric representation details.

## Interaction contract

Every action follows the same safety contract:

```text
Desktop Context
      |
      v
active_window_handle
      |
      v
ui_inspect(handle)
      |
      v
structural snapshot + paths + pattern state
      |
      v
choose one exact path
      |
      v
Permission Gateway
      |
      v
UI action(handle, path, ...)
```

Mutating actions do not default to the current foreground window. They require the exact root HWND returned by inspection/context.

If the accessibility tree changes and the child-index path is stale, the action fails. The assistant must inspect again rather than guessing a new target.

## Privacy boundary

Inspection still does **not** read editable `ValuePattern` text.

Pattern state is limited to structural/action state such as:

- selected/not selected;
- toggle state;
- expanded/collapsed state;
- bounded numeric range state;
- scrollability and scroll percentages.

This is sufficient for deterministic control decisions without automatically extracting arbitrary field contents.

## Priority over coordinate automation

The priority order remains:

```text
Windows API
   > UI Automation
   > application/browser API
   > vision + coordinate input fallback
```

UI Automation provides semantic targets, native capability discovery, explicit bounds and deterministic failure when a control cannot support an operation.

## Local verification checklist

No GitHub Actions or runtime tests are run for these phases.

Verify locally on Windows:

1. inspect a checkbox and confirm `toggle_state` is a semantic string;
2. verify `off`, `on` and `indeterminate` map correctly;
3. inspect expandable elements and confirm semantic expand/collapse strings;
4. verify leaf elements serialize as `leaf`;
5. inspect/select list or tab items and verify `is_selected`;
6. inspect a RangeValue control and verify bounded numeric metadata;
7. inspect a scroll container and perform a bounded scroll;
8. verify stale paths fail rather than hitting another control;
9. verify the Assistant window taking focus does not retarget actions away from the source HWND.

## Deferred

Still deferred:

- Grid/Table semantic metadata;
- TextPattern content extraction;
- drag-and-drop;
- raw keyboard typing;
- raw mouse movement/clicking;
- pixel-coordinate vision fallback.
