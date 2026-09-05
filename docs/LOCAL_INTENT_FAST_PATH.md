# Local Safe Intent Fast Path

## Goal

Provide a deterministic path for simple read-only desktop questions that do not require Gemini/Antigravity reasoning.

This path is intentionally narrow. It must never become a shortcut around the permission model.

## Implemented scope

The core exposes `match_local_safe_intent` and currently recognizes only MCP tools classified as `Safe`:

- `audio_get_volume`
- `apps_list`
- `window_get_active`
- `system_get_info`

The Tauri desktop checks the matcher before collecting desktop context or submitting the request to Antigravity. A match is re-validated against `windows_tools::TOOL_CATALOG`; execution is refused unless the catalog still marks that exact tool as `Safe`.

Supported requests include Vietnamese and basic English variants such as:

- `Âm lượng hiện tại bao nhiêu?`
- `Ứng dụng nào đang chạy?`
- `Cửa sổ active hiện tại là gì?`
- `Máy đang dùng bao nhiêu RAM?`
- `What is the current volume?`
- `Show running apps`
- `What is the active window?`
- `System info`

Local answers are produced directly from the existing `windows-tools` implementations. Text and voice turns share the same `complete_prompt` path, so both can use the fast-path.

## Safety rules

The matcher returns `None` when a request mutates state or is not deterministic enough.

Examples that continue through Antigravity + MCP + permission handling:

- `Đặt âm lượng xuống 30%`
- `Tắt tiếng`
- `Mở Chrome`
- window move/resize/state changes
- clipboard writes
- UI Automation mutations
- sensitive operations

Unknown or ambiguous language also falls back to the agent instead of guessing.

The desktop performs a second safety check immediately before execution. If a mapped tool is missing from the catalog or no longer has `ToolRisk::Safe`, the local path fails closed instead of executing it.

## Core lifecycle

`AssistantCore::handle_local_safe_tool` keeps local execution on the same state/event model as agent-driven tool turns:

```text
Idle / Listening
  -> Processing
  -> ToolStarted
  -> Executing
  -> ToolFinished
  -> ResponseCompleted
  -> Idle
```

A local tool failure emits `ToolFinished { success: false }`, moves the assistant to `Error`, and emits `local_tool_error`.

## Request flow

```text
User text / Whisper transcript
        |
        v
match_local_safe_intent
   |               |
 match           no match
   |               |
   v               v
catalog Safe?   context collection
   |               |
   v               v
windows-tools   Antigravity / Gemini
   |               |
   v               v
local response   MCP as needed
```

The fast-path avoids screenshot/clipboard/context collection for matched read-only commands because those inputs are unnecessary for deterministic execution.

## Next extension

Moderate/Sensitive tools must not be added to the direct path until the permission gateway is reusable outside the MCP server. The next extension should extract/share the authorization gateway first, then add deterministic mutating intents only through that same authorization contract.

## Why this exists

Antigravity quota exhaustion, authentication problems, or transient backend failures should not prevent deterministic local read-only questions from being answered when Windows itself can provide the result directly.
