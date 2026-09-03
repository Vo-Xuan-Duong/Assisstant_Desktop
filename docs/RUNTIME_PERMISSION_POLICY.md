# Phase 9D — Runtime Permission Policy Overrides

## Purpose

Phase 9D allows the user to change the execution policy of **Moderate** Windows tools at runtime without restarting Antigravity.

The available choices are:

```text
Default
Allow
Ask
Deny
```

`Default` currently resolves to `Allow` for Moderate tools.

## Security boundary

Runtime override scope is intentionally narrow:

```text
Safe       -> product default only
Moderate   -> runtime override allowed
Sensitive  -> product default only (Ask)
Blocked    -> product default only (Deny)
```

The desktop UI only displays Moderate tools, and the Rust backend validates the native `TOOL_CATALOG` before accepting an update.

A manually forged frontend event cannot downgrade Sensitive or Blocked policy.

## Architecture

```text
Permission Settings UI
        |
        | permission:policy_set
        v
Desktop Permission Service
        |
        | validate exact tool name + Moderate risk
        v
<AppLocalData>/permissions/policy.json
        |
        | path inherited by agy/MCP through
        | ASSISTANT_PERMISSION_POLICY_PATH
        v
assistant-mcp
        |
        | read snapshot before each Moderate tool call
        v
Allow / Ask / Deny
```

The file path is stable for the desktop session, while file contents may change at runtime. Therefore the Antigravity process does not need to restart when a policy is edited.

## Snapshot schema

Shared Rust schema:

```text
PermissionOverrideSnapshot {
    revision,
    tools: {
        tool_name -> allow | ask | deny
    }
}
```

Only Moderate entries are honored by MCP.

Example:

```json
{
  "revision": 3,
  "tools": {
    "audio_set_volume": "ask",
    "apps_open": "deny"
  }
}
```

## Fail-safe file behavior

MCP policy loading behavior for Moderate tools:

```text
policy path unavailable -> default Moderate policy
policy file missing      -> default Moderate policy
valid file               -> apply exact tool override
read error               -> Deny current Moderate call
malformed JSON           -> Deny current Moderate call
```

A partially written/corrupt policy can therefore reduce capability, but cannot turn a denied action into an allowed action.

## Desktop write behavior

The desktop creates:

```text
<AppLocalData>/permissions/policy.json
```

only when the user first changes or resets an override.

The in-memory snapshot is updated only after the file write succeeds.

If an existing file is malformed, the panel exposes the load error. Saving any valid setting repairs the snapshot.

## UI

The main Assistant surface mounts a separate `PermissionPolicyPanel`.

It is deliberately isolated from the chat component and displays:

- Moderate tool name;
- tool description;
- effective decision;
- Default / Allow / Ask / Deny selector;
- current policy revision;
- policy load/write errors.

The panel sits below the Sensitive confirmation modal in z-order, so an open Settings panel cannot cover an active confirmation request.

## Ask behavior for Moderate tools

When a Moderate override is set to `Ask`, it uses the existing authenticated permission broker:

```text
Moderate tool
   |
   | override = Ask
   v
permission broker
   |
   v
Desktop confirmation modal
   |
   +-- Allow once
   +-- Deny
```

The same timeout, one-shot approval and audit behavior used by Sensitive tools applies.

## Examples

### Require confirmation for opening applications

```text
apps_open -> Ask
```

Result:

```text
"Mở Chrome"
   |
   v
confirmation modal
```

### Disable clipboard reads

```text
clipboard_read_text -> Deny
```

The tool call is rejected before Windows clipboard APIs run.

### Restore default

```text
clipboard_read_text -> Default
```

The explicit override is removed from the snapshot and the Moderate default (`Allow`) applies again.

## Local verification checklist

No automated tests or GitHub Actions are run in this phase.

Verify locally on Windows:

1. open the Permissions panel;
2. confirm only Moderate tools are listed;
3. set `audio_set_volume` to Ask;
4. request a volume change and confirm the modal appears;
5. Allow once and verify the volume changes;
6. set the same tool to Deny and confirm it no longer executes;
7. set it back to Default and confirm normal Moderate behavior returns;
8. edit `policy.json` into malformed JSON and confirm Moderate actions fail closed;
9. verify Sensitive tools remain Ask regardless of policy-file contents;
10. verify a forged policy event targeting a Sensitive tool is rejected by the desktop backend.
