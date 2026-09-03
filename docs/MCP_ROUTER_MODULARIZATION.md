# Phase 10E — Modular MCP tool routers

## Purpose

The Windows MCP server accumulated system/device tools and Windows UI Automation tools in one large `server.rs`. Phase 10E is a structural refactor only: it separates those tool groups while keeping the public MCP contract unchanged.

No new capability is introduced in this phase.

## Structure

```text
crates/windows-mcp/src/
├── main.rs
├── permissions.rs
└── server.rs
    └── server/
        ├── system_tools.rs
        └── ui_tools.rs
```

In Rust module terms, `server.rs` declares two child modules:

```text
server::system_tools
server::ui_tools
```

## Composition root

`server.rs` now owns only:

- `WindowsMcpServer` state;
- `McpPermissionGateway`;
- the combined `ToolRouter<WindowsMcpServer>`;
- shared result serialization/error helpers;
- the `ServerHandler` implementation.

Conceptually:

```text
system_tool_router()
        \
         +--> combined ToolRouter --> ServerHandler
        /
ui_tool_router()
```

The implementation uses the official rmcp multiple-router mechanism:

```rust
Self::system_tool_router() + Self::ui_tool_router()
```

and:

```rust
#[tool_handler(router = self.tool_router)]
impl ServerHandler for WindowsMcpServer {}
```

## System router

`system_tools.rs` owns the existing non-UIA tools:

```text
audio_get_volume
audio_set_volume
audio_set_mute
apps_open
apps_list
window_get_active
system_get_info
media_play_pause
media_next
media_previous
clipboard_read_text
clipboard_write_text
```

Count: 12.

## UI Automation router

`ui_tools.rs` owns:

```text
ui_inspect
ui_focus
ui_invoke
ui_set_value
ui_set_range_value
ui_toggle
ui_select
ui_set_expanded
ui_scroll
ui_scroll_into_view
```

Count: 10.

Total public tools after refactor: 22.

## Contract preservation

Phase 10E intentionally preserves:

- exact public MCP tool names;
- input JSON schemas;
- tool descriptions except formatting/location in source;
- permission-gateway calls and exact argument payloads;
- risk lookup through `windows-tools::TOOL_CATALOG`;
- `spawn_blocking` for UI Automation COM calls;
- explicit HWND/path requirements;
- tool result JSON formats;
- stdio transport;
- `WindowsMcpServer::default()` as the construction API used by `main.rs`.

Antigravity configuration does not change.

## Why store the combined router

The combined `ToolRouter<Self>` is stored on `WindowsMcpServer` so `#[tool_handler]` has one stable router expression while each feature area can generate its own router independently.

`ToolRouter` supports `Clone` and router addition/merge, so the server remains cloneable as required by the existing service path.

## Permission boundary

Tool modules do not own permission policy. Both routers call the same shared:

```text
McpPermissionGateway
```

Therefore splitting the router cannot bypass the permission layer.

The call order remains:

```text
MCP request
  -> tool handler
  -> permissions.authorize(...)
  -> native Windows operation
```

## Future extension rule

New MCP capabilities should no longer be appended blindly to `server.rs`.

Prefer the appropriate router module, and create another tool group only when the domain is distinct enough to justify it. Examples:

```text
server/browser_tools.rs
server/file_tools.rs
```

if those domains are introduced later.

## Static review checklist

Before merging Phase 10E:

1. count tool names in both routers and compare with `TOOL_CATALOG`;
2. verify no duplicate public names;
3. verify every mutating tool still calls `permissions.authorize` before native work;
4. verify UIA actions still use exact HWND/path;
5. verify `main.rs` can continue constructing `WindowsMcpServer::default()`;
6. verify stdout remains reserved for MCP protocol;
7. compare branch against main and ensure changes are structural only.

## Verification policy

Per project policy, GitHub Actions and runtime tests are not run here. Final compile/native verification remains local on Windows.
