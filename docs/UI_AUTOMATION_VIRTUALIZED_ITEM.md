# Phase 11A — UI Automation VirtualizedItem support

## Purpose

Large Windows lists, tables and grids often virtualize items so only a subset is fully materialized. Phase 11A adds a narrow semantic path for checking and realizing such an item without introducing raw input or unbounded item-container search.

## Native module

A separate native module is used:

```text
windows-tools::virtualized
```

It intentionally does not rewrite the stable general-purpose `automation.rs` inspection engine.

### Read-only status

```text
status(window_handle, path)
```

returns:

```json
{
  "supported": true
}
```

The function resolves the exact child-index path under the explicit HWND and checks whether the element exposes `IUIAutomationVirtualizedItemPattern`.

### Realize

```text
realize(window_handle, path)
```

calls:

```text
IUIAutomationVirtualizedItemPattern::Realize()
```

The operation does not explicitly:

- focus the item;
- select it;
- invoke it;
- click it;
- type into it.

The provider may change its accessibility tree while materializing the item, so callers must re-inspect after success.

## Public MCP tools

### `ui_virtualized_item_status`

Risk:

```text
Safe
```

Input:

```json
{
  "window_handle": 123456,
  "path": [1, 8, 2]
}
```

This is a read-only capability check.

### `ui_realize`

Risk:

```text
Moderate
```

Input is the same explicit HWND/path pair.

The default Moderate policy applies, so the existing runtime permissions UI can set:

```text
Default
Allow
Ask
Deny
```

When configured as Ask, the authenticated desktop confirmation broker is reused.

## Recommended agent flow

```text
ui_inspect(source HWND)
      |
      v
choose exact element path
      |
      v
ui_virtualized_item_status(HWND, path)
      |
      +-- supported=false -> stop / use another semantic route
      |
      v
ui_realize(HWND, path)
      |
      v
RE-INSPECT REQUIRED
      |
      v
new tree + new path
      |
      v
next semantic action
```

The old path must not be reused after a successful realize operation.

## Why no ItemContainerPattern search yet

Windows UI Automation also provides ItemContainerPattern for finding items in virtualized containers. Phase 11A deliberately does not expose it because a provider-driven search API could bypass the assistant's current bounded tree inspection contract.

Current inspection remains bounded by depth/node limits. Introducing targeted container search should be a separate design decision with explicit query limits and result identity semantics.

## Router structure

Following Phase 10E, VirtualizedItem tools live in their own router:

```text
server/system_tools.rs
server/ui_tools.rs
server/virtualized_tools.rs
```

The composition root combines:

```text
system_tool_router
+ ui_tool_router
+ virtualized_tool_router
```

All routers share the same `McpPermissionGateway` stored on `WindowsMcpServer`.

## Safety properties

- explicit non-zero HWND required;
- explicit child-index path required;
- path resolved immediately before capability/action call;
- stale path fails;
- no automatic fallback to keyboard/mouse;
- no ItemContainer search;
- no editable text extraction;
- permission check occurs before `Realize()`;
- successful realization requires re-inspection.

## Local verification checklist

On a Windows application that uses UI virtualization:

1. inspect a virtualized list/grid and obtain a path to a candidate provider element;
2. call `ui_virtualized_item_status` and confirm supported state;
3. call `ui_realize` for a supported item;
4. confirm the provider materializes the item;
5. verify the result marks re-inspection required;
6. re-run `ui_inspect` and use the new path, not the old one;
7. verify an unsupported element returns a semantic Unsupported error;
8. configure `ui_realize` as Ask and verify permission confirmation;
9. configure it as Deny and confirm `Realize()` is never reached;
10. verify no mouse/keyboard input is generated.

## Verification policy

GitHub Actions and runtime tests are not run as part of remote development. Native verification remains local on Windows.
