# Phase 10D — ScrollItem MCP exposure

## Purpose

Phase 10D exposes the native `ScrollItemPattern` capability from Phase 10C through the Windows MCP server while preserving the existing explicit target and permission boundaries.

## Public tool

```text
ui_scroll_into_view
```

Input:

```json
{
  "window_handle": 123456,
  "path": [2, 4, 1]
}
```

The input uses the same `UiElementActionInput` contract as other semantic UI Automation actions.

## Required discovery flow

The model should use:

```text
Desktop Context active_window_handle
      |
      v
ui_inspect(window_handle)
      |
      v
find target element
      |
      +-- supports_scroll_item = true
      |
      v
ui_scroll_into_view(window_handle, path)
```

The tool must not be used when `supports_scroll_item` is false. The native layer returns `Unsupported` rather than falling back to raw wheel or mouse input.

## Permission classification

`ui_scroll_into_view` is classified as:

```text
Moderate
```

Reason: the action changes viewport/navigation state but does not submit a form, write a value, select a consequential option, or execute an arbitrary command.

Because it is Moderate, the existing runtime policy panel can configure:

```text
Default
Allow
Ask
Deny
```

The default Moderate policy remains Allow unless the user changes it.

## Runtime path

```text
Antigravity / Gemini
      |
      v
assistant-mcp
      |
      v
McpPermissionGateway.authorize(
  "ui_scroll_into_view",
  exact arguments
)
      |
      v
spawn_blocking
      |
      v
windows-tools::automation::scroll_into_view
      |
      v
IUIAutomationScrollItemPattern::ScrollIntoView
```

The permission check occurs before native UI Automation mutation.

## Target safety

Like the existing UIA actions, this tool requires:

- explicit non-zero HWND;
- an exact path from a recent bounded inspection;
- path resolution immediately before the action.

If the UI tree changed, the action fails instead of guessing another element.

## Returned result

The MCP result follows the existing `UiActionResult` contract:

```json
{
  "ok": true,
  "action": "scroll_into_view",
  "window_before_action": { "...": "..." },
  "path": [2, 4, 1]
}
```

The window metadata is captured before the action because a UI operation may change the visible application state.

## Grid/GridItem metadata

Phase 10C GridPattern and GridItemPattern data is automatically available through the existing `ui_inspect` result. There is no separate mutation tool for grids in Phase 10D.

This avoids creating unnecessary tool surface and keeps table/grid reasoning read-only unless the model calls an existing semantic element action.

## Privacy boundary

This phase does not add:

- cell-value extraction;
- TextPattern extraction;
- OCR;
- screenshots;
- arbitrary grid traversal through `GetItem`;
- raw mouse input;
- wheel-event synthesis;
- raw keyboard input.

## Local verification checklist

1. inspect a scrollable list or grid;
2. locate an element with `supports_scroll_item=true`;
3. call `ui_scroll_into_view` with the exact HWND/path;
4. confirm the item becomes visible;
5. change the UI tree and confirm a stale path fails;
6. call on an element without ScrollItemPattern and confirm `Unsupported`;
7. set `ui_scroll_into_view` to Ask in runtime permission settings and confirm the desktop confirmation broker is used;
8. set it to Deny and confirm no native action occurs;
9. confirm no wheel/mouse input is generated.

## Deferred

The MCP server is now large enough that the next structural work should separate tool groups into multiple router modules before adding many more patterns.
