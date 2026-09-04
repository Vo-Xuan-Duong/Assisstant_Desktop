# Local Safe Intent Fast Path

## Goal

Provide a deterministic path for simple read-only desktop questions that do not require Gemini/Antigravity reasoning.

This path is intentionally narrow. It must never become a shortcut around the permission model.

## Phase 16A scope

The core exposes `match_local_safe_intent` and currently recognizes only MCP tools classified as `Safe`:

- `audio_get_volume`
- `apps_list`
- `window_get_active`
- `system_get_info`

Supported requests include Vietnamese and basic English variants such as:

- `Âm lượng hiện tại bao nhiêu?`
- `Ứng dụng nào đang chạy?`
- `Cửa sổ active hiện tại là gì?`
- `Máy đang dùng bao nhiêu RAM?`
- `What is the current volume?`
- `Show running apps`
- `What is the active window?`
- `System info`

## Safety rules

The matcher must return `None` when a request mutates state or is not deterministic enough.

Examples that must continue through Antigravity + MCP + permission handling:

- `Đặt âm lượng xuống 30%`
- `Tắt tiếng`
- `Mở Chrome`
- window move/resize/state changes
- clipboard writes
- UI Automation mutations
- sensitive operations

Unknown or ambiguous language also falls back to the agent instead of guessing.

## Core lifecycle

`AssistantCore::handle_local_safe_tool` provides the state/event contract for the future desktop executor:

```text
Idle
  -> Processing
  -> ToolStarted
  -> Executing
  -> ToolFinished
  -> ResponseCompleted
  -> Idle
```

A local tool failure emits `ToolFinished { success: false }`, moves the assistant to `Error`, and emits `local_tool_error`.

This keeps the UI event model consistent with agent-driven tool turns.

## Phase 16B

The desktop integration should:

1. call `match_local_safe_intent` before desktop context collection and Antigravity submission;
2. verify the matched tool still exists in `windows_tools::TOOL_CATALOG` and is still classified `Safe`;
3. execute only the corresponding read-only `windows-tools` operation;
4. format a concise local response;
5. run it through `AssistantCore::handle_local_safe_tool` so UI state/events remain consistent;
6. fall back to the existing Antigravity path for every unmatched request.

The implementation must not add Moderate/Sensitive tools to the direct path until a reusable permission gateway is shared by both MCP and desktop execution.

## Why this exists

Antigravity quota exhaustion, authentication problems, or transient backend failures should not prevent deterministic local read-only questions from being answered when Windows itself can provide the result directly.
