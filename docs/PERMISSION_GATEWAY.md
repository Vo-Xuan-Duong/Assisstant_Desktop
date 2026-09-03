# Permission Gateway

## Goal

The Permission Gateway prevents Gemini/Antigravity from turning tool availability into unconditional execution authority.

The security boundary is independent of the model and independent of Antigravity's own permission system.

```text
Antigravity
    |
    v
MCP tool call
    |
    v
Assistant Permission Gateway
    |
    +---- ALLOW ----> native tool
    |
    +---- ASK ------> desktop confirmation (Phase 9B)
    |
    +---- DENY -----> no execution
```

## Phase 9A behavior

Phase 9A introduces the pure Rust `permission-engine` crate and makes `assistant-mcp.exe` fail closed.

Default policy:

| Risk | Decision |
| --- | --- |
| Safe | Allow |
| Moderate | Allow |
| Sensitive | Ask |
| Blocked | Deny |

`Ask` is intentionally not treated as `Allow` while the confirmation broker is not connected.

In Phase 9A, a Sensitive tool returns:

```text
confirmation_required: tool=<name>; ...
```

and **does not execute**.

This temporarily makes Sensitive UIA actions unavailable until Phase 9B connects the MCP process to the desktop confirmation UI. That is deliberate.

## Fail-closed rules

### Unknown tools

A public MCP tool without an exact entry in `windows-tools::TOOL_CATALOG` is denied.

No fuzzy lookup or alias fallback is permitted.

### Blocked tools

`ToolRisk::Blocked` is an absolute boundary.

Even if a runtime override says `Allow`, the permission engine returns `Deny` for a Blocked primitive.

This rule is intended for future primitives such as unrestricted shell execution or credential extraction that should never be enabled through normal user preference toggles.

### Sensitive tools

Current Sensitive tools include:

```text
ui_invoke
ui_set_value
ui_toggle
ui_select
```

They are not executed in Phase 9A because the default decision is `Ask`.

### Moderate tools

Examples:

```text
audio_set_volume
apps_open
clipboard_read_text
clipboard_write_text
ui_focus
ui_set_expanded
ui_scroll
```

The baseline policy allows these automatically. Later Settings can make selected Moderate tools require confirmation without changing the tool implementation.

## Engine separation

`permission-engine` does not depend on:

- Tauri;
- MCP;
- Windows APIs;
- Antigravity;
- UI Automation.

It only evaluates:

```text
tool name
+
ToolRisk
+
policy
```

and returns:

```text
Allow | Ask | Deny
```

This keeps policy reusable across MCP, desktop-local fast paths and future integrations.

## Enforcement location

Phase 9A enforcement occurs inside `assistant-mcp.exe` before native tool functions are called.

Example:

```text
ui_toggle(...)
    |
    v
WindowsMcpServer::authorize("ui_toggle")
    |
    v
TOOL_CATALOG -> Sensitive
    |
    v
PermissionEngine -> Ask
    |
    v
confirmation_required
    X
automation::toggle() is never called
```

All current MCP handlers, including read-only tools, pass through `authorize()`.

This matters because a newly exposed tool cannot silently bypass the policy just because it is currently considered harmless.

## Tool overrides

The policy model supports exact-name overrides for non-Blocked tools.

Example future settings:

```text
clipboard_read_text = Ask
apps_open = Allow
ui_scroll = Allow
```

Overrides are not yet exposed in Settings in Phase 9A.

A Blocked tool cannot be overridden to Allow.

## Phase 9B

Phase 9B will add a desktop confirmation broker.

Target flow:

```text
Sensitive MCP call
      |
      v
PermissionEngine -> Ask
      |
      v
MCP Permission Broker
      |
      v
Desktop Assistant
      |
      v
Confirmation overlay

  "Assistant wants to invoke
   the Save button in Notepad"

      [Deny] [Allow once]
      |
      v
signed/one-shot decision
      |
      v
MCP resumes or rejects call
```

Requirements for the broker:

- local-only;
- authenticated between desktop and MCP process;
- one request id per tool call;
- one-shot decisions;
- timeout defaults to deny;
- user closing the dialog means deny;
- arguments included in the confirmation identity so approval cannot be reused for changed parameters;
- no permanent `Always allow` for Sensitive tools in the first implementation.

## Local verification for Phase 9A

Do not run GitHub Actions for this project.

Verify locally on Windows that:

1. Safe tools such as `system_get_info` still execute;
2. Moderate tools such as `audio_set_volume` still execute;
3. `ui_inspect` still executes;
4. `ui_invoke` returns `confirmation_required` without invoking the control;
5. `ui_set_value` does not modify the field;
6. `ui_toggle` does not toggle the target;
7. `ui_select` does not alter selection;
8. an unknown/uncatalogued tool path is denied;
9. MCP stdout remains reserved for protocol frames and permission diagnostics stay in returned tool errors/stderr.
