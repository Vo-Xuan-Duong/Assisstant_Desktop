# Windows MCP Server

## Purpose

`assistant-mcp.exe` is the explicit boundary between Antigravity/Gemini and Windows actions.

```text
Antigravity CLI
      |
      | MCP stdio
      v
assistant-mcp.exe
      |
      v
 windows-tools
      |
      v
 Windows APIs
```

The MCP process contains protocol/schema adaptation only. Native Windows behavior lives in the reusable `windows-tools` crate.

## Why it is a separate process

Antigravity supports local MCP servers through stdio. Keeping the MCP server separate gives us:

- protocol isolation from the desktop UI process;
- a clean stdout channel owned by MCP;
- explicit tool boundaries;
- independent lifecycle/restart behavior;
- a place to apply assistant-owned permission rules before sensitive tools execute.

`assistant-mcp.exe` writes diagnostics to **stderr only**. Writing logs to stdout would corrupt MCP protocol frames.

## Current tool set

### Audio

- `audio_get_volume` — read master volume and mute state.
- `audio_set_volume` — set master output volume from 0 to 100 percent.
- `audio_set_mute` — mute or unmute the default output endpoint.

Implementation: Windows Core Audio / `IAudioEndpointVolume`.

### Applications

- `apps_open` — open a Windows Shell target such as an application, document, file path, or URI.
- `apps_list` — list running process executable names and process ids through Tool Help APIs.

`apps_open` is deliberately **not** a general-purpose command shell. The tool passes one target to Windows Shell and does not expose PowerShell/cmd command execution as a primitive.

### Window context

- `window_get_active` — return the current foreground-window title, process id, and executable name when it can be resolved.

### System

- `system_get_info` — return logical CPU count and physical-memory state.

### Media

- `media_play_pause`
- `media_next`
- `media_previous`

These send the corresponding Windows media keys.

### Clipboard

- `clipboard_read_text` — read Unicode text from the clipboard.
- `clipboard_write_text` — replace clipboard text.

Clipboard reads are classified as **Moderate** rather than Safe because clipboard content may contain sensitive user data even though the operation itself is read-only.

### Windows UI Automation

Phase 8B exposes the native UIA foundation through four MCP tools:

- `ui_inspect`
- `ui_focus`
- `ui_invoke`
- `ui_set_value`

`ui_inspect` returns a bounded structural Control View snapshot. It does not read editable field values. Each node has a child-index `path` plus metadata/capability flags.

Typical flow:

```text
user asks to interact with current app
        ↓
Desktop Context Engine captures source HWND
        ↓
active_window_handle is included in request context
        ↓
Antigravity calls ui_inspect(window_handle=...)
        ↓
model selects an inspected element path
        ↓
ui_focus / ui_invoke / ui_set_value
        ↓
native layer resolves path again immediately before action
```

### Why action tools require an explicit HWND

Only `ui_inspect` can omit `window_handle` and fall back to the current foreground window.

All modifying UIA actions require the exact `root_window_handle` returned by `ui_inspect`.

This prevents a race such as:

```text
inspect Notepad
    ↓
foreground changes to Assistant
    ↓
invoke path [0,2]
```

from silently targeting the new foreground application.

If the application's UI tree changes and a path is stale, the action fails. Antigravity should call `ui_inspect` again rather than guessing a replacement path.

### Source-window targeting from Desktop Context

The desktop window itself often has focus while the user is typing or after wake activation. Therefore MCP cannot reliably infer the user's intended application from `GetForegroundWindow()` alone.

The desktop already remembers the application that was foreground **before** it opened. For requests that need window/screen/UI interaction, Context Engine adds:

```text
active_window_handle
active_window_title
active_process_id
active_executable
```

to the on-demand `<desktop_context>` block.

The context explicitly instructs the agent to pass `active_window_handle` to UI Automation tools when that referenced application is the target.

No extra shared-state file, local socket, or named pipe is needed between the desktop and `assistant-mcp.exe`.

### Blocking isolation

UI Automation uses synchronous COM calls. The MCP handlers for UIA tools are async but execute the native operation through:

```text
tokio::task::spawn_blocking
```

This prevents a slow accessibility provider from blocking the MCP Tokio worker while keeping the COM client lifetime inside one blocking thread.

## Risk catalogue

The native tool crate assigns product-level risk independent of Antigravity permissions:

```text
Safe
Moderate
Sensitive
Blocked
```

The catalogue stores the exact public MCP name (`audio_set_volume`, `ui_invoke`, etc.). This lets the permission gateway map an incoming MCP tool call to one risk classification without aliases or fuzzy matching.

Current UIA baseline:

```text
ui_inspect    Safe
ui_focus      Moderate
ui_invoke     Sensitive
ui_set_value  Sensitive
```

`ui_invoke` is Sensitive because the same InvokePattern primitive can represent both an ordinary button and a consequential confirmation button. `ui_set_value` is Sensitive because it mutates an application's data and could target forms or other important fields.

A later semantic permission phase can downgrade/upgrade decisions based on application, element metadata, and action context. Until that exists, the baseline remains conservative.

The project still does not expose shutdown, restart, file deletion, process termination, administrator actions, or raw shell execution as generic MCP primitives.

## Antigravity configuration

Antigravity CLI supports workspace MCP configuration at:

```text
.agents/mcp_config.json
```

A sample is committed as:

```text
.agents/mcp_config.example.json
```

After building `assistant-mcp.exe`, copy/adapt the example to `mcp_config.json` and point `command` to the built binary.

Example:

```json
{
  "mcpServers": {
    "assistant-windows": {
      "command": "target\\release\\assistant-mcp.exe",
      "cwd": ".",
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

The repository ignores `.agents/mcp_config.json` because it is machine-local configuration, while the example remains version controlled.

The production desktop application will later manage this integration more automatically; the workspace file remains sufficient for manual/local development.

## Build target

The Rust binary target is named:

```text
assistant-mcp
```

On Windows release builds the expected executable path is:

```text
target\release\assistant-mcp.exe
```

## Core integration examples

### Basic system tool

```text
User: "Đặt âm lượng 30%"
   |
   v
Antigravity
   |
   v
audio_set_volume
   |
   v
assistant-mcp.exe
   |
   v
windows-tools::audio::set_volume
```

### UI Automation tool chain

```text
User: "Bấm nút Retry giúp tôi"
   |
   v
Context Engine stores source HWND
   |
   v
Antigravity receives active_window_handle
   |
   v
ui_inspect(handle)
   |
   v
find Retry element + supports_invoke=true
   |
   v
ui_invoke(same handle, path)
   |
   v
IUIAutomationInvokePattern::Invoke
```

Native verification is intentionally performed on the Windows development machine rather than by introducing a GitHub Action at this stage.
