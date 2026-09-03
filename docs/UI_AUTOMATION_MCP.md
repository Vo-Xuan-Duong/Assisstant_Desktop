# Phase 8B — UI Automation MCP Contract

## Public tools

```text
ui_inspect
ui_focus
ui_invoke
ui_set_value
```

The MCP layer is an adapter over `windows-tools::automation`; it does not contain COM/UI Automation implementation details.

## Targeting contract

### Inspection

`ui_inspect` accepts:

```json
{
  "window_handle": 123456,
  "max_depth": 4,
  "max_nodes": 160
}
```

`window_handle` is optional for inspection only. When Desktop Context provides `active_window_handle`, that value should be passed explicitly.

The result contains:

```text
window metadata
+
tree.root_window_handle
+
structural UIA nodes
```

### Actions

The action tools require an explicit handle:

```json
{
  "window_handle": 123456,
  "path": [0, 2, 1]
}
```

`ui_set_value` additionally requires `value`.

The model must use a path from a recent `ui_inspect` of the same root HWND. Paths are transient and must not be stored as durable identifiers.

## Source-window handoff

The standalone MCP process cannot know which application the user was looking at before the Assistant window took focus.

The Desktop Context Engine already has that source HWND. For requests that refer to screen/current app/UI interaction it now includes:

```text
active_window_handle
active_window_title
active_process_id
active_executable
```

in the Antigravity request context.

This gives the model a deterministic bridge:

```text
Tauri source HWND
  → prompt context
  → Antigravity
  → ui_inspect(window_handle)
  → ui action with same handle
```

No persistent shared HWND registry is required.

## Context intent

The context engine recognizes direct UI-manipulation phrases such as:

```text
nhấn nút
bấm nút
click vào
điền vào
nhập vào ô
chọn mục
focus vào
```

as requiring source-window metadata even if the user does not literally say “cửa sổ này”.

## Blocking policy

UI Automation providers are synchronous COM components and can be slow or hung independently of the assistant.

Every UIA MCP handler runs the native operation through:

```rust
tokio::task::spawn_blocking(...)
```

The MCP async runtime therefore remains responsive while the blocking thread owns the COM client.

## Risk baseline

```text
ui_inspect    Safe
ui_focus      Moderate
ui_invoke     Sensitive
ui_set_value  Sensitive
```

`ui_invoke` is conservative because InvokePattern does not reveal whether a button means “Next”, “Buy”, “Delete”, or “Confirm”.

`ui_set_value` is conservative because it mutates application state and may target forms.

A later semantic permission gateway can make a more specific decision using element/application context.

## Privacy

`ui_inspect` does not read `ValuePattern.CurrentValue`.

The model receives structural metadata only:

```text
name
id/class/control type
bounds
focus/enabled/offscreen
supports_invoke
supports_value
```

`ui_set_value` writes only a value that already exists in the user/agent action context. Tool descriptions explicitly prohibit inventing passwords or secrets.

## Stale paths

The native layer resolves every path again just before the action.

If a dynamic UI changes:

```text
ui_inspect
  ↓
UI changes
  ↓
ui_invoke(old path)
```

then resolution may return `NotFound`. Correct behavior is to inspect again, not guess a new child index.

## Action result semantics

Window metadata is captured before an action executes.

This matters because a valid `ui_invoke` can close its own dialog/window. The tool still reports success even if the original HWND disappears immediately after the action.

## Deferred

- toggle/check-box pattern;
- selection/list pattern;
- expand/collapse pattern;
- scroll pattern;
- semantic risk classification by element/application;
- user confirmation UI for sensitive tool calls;
- vision-coordinate fallback.
