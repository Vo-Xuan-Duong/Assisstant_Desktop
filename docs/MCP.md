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
- a place to apply assistant-owned permission rules before sensitive tools are added.

`assistant-mcp.exe` writes diagnostics to **stderr only**. Writing logs to stdout would corrupt MCP protocol frames.

## Current tool set

### Audio

- `audio_get_volume` — read master volume and mute state.
- `audio_set_volume` — set master output volume from 0 to 100 percent.
- `audio_set_mute` — mute or unmute the default output endpoint.

Implementation: Windows Core Audio / `IAudioEndpointVolume`.

### Applications

- `apps_open` — open a Windows Shell target such as an application, document, file path, or URI.

This is deliberately **not** a general-purpose command shell. The tool passes one target to Windows Shell and does not accept arbitrary PowerShell/cmd command lines as an execution primitive.

### Window context

- `window_get_active` — return the current foreground-window title and process id.

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

Clipboard reads are classified as **Moderate** rather than Safe because clipboard content may contain credentials or other sensitive user data even though the operation itself is read-only.

## Risk catalogue

The native tool crate assigns product-level risk independent of Antigravity permissions:

```text
Safe
Moderate
Sensitive
Blocked
```

Current tools are Safe or Moderate only. Phase 2 intentionally does not expose shutdown, restart, file deletion, process termination, administrator actions, or raw shell execution.

Later, the desktop permission gateway will use this catalogue to decide whether a tool can execute automatically or requires confirmation.

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

The production desktop application will later manage this integration more automatically; the workspace file is sufficient for Phase 2 development and manual verification.

## Build target

The Rust binary target is named:

```text
assistant-mcp
```

On Windows release builds the expected executable path is:

```text
target\release\assistant-mcp.exe
```

## Phase 2 acceptance flow

The key integration to verify locally is:

```text
User prompt
   |
   v
Antigravity
   |
   | chooses MCP tool
   v
assistant-windows/audio_set_volume
   |
   v
assistant-mcp.exe
   |
   v
windows-tools::audio::set_volume
   |
   v
IAudioEndpointVolume
   |
   v
Windows volume changes
```

No GitHub Action is required for this verification; it is intentionally designed to be run locally on the Windows development machine.
