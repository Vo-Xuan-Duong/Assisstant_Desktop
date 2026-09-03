# Phase 8C — Richer Windows UI Automation Patterns

## Purpose

Phase 8C expands deterministic computer-use beyond `InvokePattern` and `ValuePattern` without introducing coordinate clicking.

The assistant can now reason over common Windows accessibility state and perform bounded actions through UI Automation.

## Added pattern support

### TogglePattern

Snapshot fields:

```text
toggle_state: number | null
```

Current native UIA values:

```text
0 = off
1 = on
2 = indeterminate
null = TogglePattern unavailable
```

Action:

```text
ui_toggle(window_handle, path)
```

Typical controls:

- checkboxes;
- switches;
- toggle buttons.

The model should inspect `toggle_state` before calling the action. `ui_toggle` changes state rather than setting an absolute desired value.

Risk classification: **Sensitive**.

A toggle can enable settings or options with consequences, so it is intentionally conservative until the Permission Gateway can apply semantic rules.

### SelectionItemPattern

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

Selection itself may alter application state or choose a consequential workflow option.

### ExpandCollapsePattern

Snapshot field:

```text
expand_collapse_state: number | null
```

Current native UIA values:

```text
0 = collapsed
1 = expanded
2 = partially_expanded
3 = leaf_node
null = ExpandCollapsePattern unavailable
```

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

### ScrollPattern

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

## Interaction contract

Every action follows the existing UIA safety contract:

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
UI action(handle, path, ...)
```

Mutating actions do not default to the current foreground window. They require the exact root HWND returned by the inspection/context path.

If the accessibility tree changes and the child-index path is stale, the action fails. The assistant must inspect again rather than guessing a new target.

## Privacy boundary

Inspection still does **not** read editable field contents.

Pattern state is limited to structural/action state such as:

- selected/not selected;
- toggle state;
- expanded/collapsed;
- scrollability and scroll percentages.

This is sufficient for deterministic control decisions without automatically extracting text values that may contain private data.

## Why no coordinate clicking yet

The priority order remains:

```text
Windows API
   > UI Automation
   > application/browser API
   > vision + coordinate input fallback
```

UI Automation provides semantic targets, native capability discovery and explicit failure when a control cannot support an operation. Coordinate clicking lacks those guarantees.

## Phase 8C acceptance scenarios

Local Windows verification should include at least:

1. inspect a checkbox and observe `toggle_state`;
2. toggle it and inspect again to confirm the state changed;
3. inspect a selectable list/tab item and confirm `is_selected`;
4. select another item and inspect again;
5. expand and collapse a tree/combo element;
6. inspect a scroll container and perform a small vertical scroll;
7. verify a stale path fails rather than hitting another control;
8. verify the main Assistant window taking focus does not retarget actions away from the source HWND.

## Deferred

Phase 8C deliberately does not add:

- RangeValuePattern;
- Grid/Table patterns;
- TextPattern text extraction;
- drag-and-drop;
- raw keyboard typing;
- raw mouse movement/clicking;
- pixel-coordinate vision fallback.

Those should only be added after the Permission Gateway is active.
