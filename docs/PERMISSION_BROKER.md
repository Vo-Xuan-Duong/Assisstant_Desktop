# Phase 9B — Desktop Permission Confirmation Broker

## Purpose

Phase 9A made the MCP permission layer fail closed: Sensitive tools returned `confirmation_required` and did not execute.

Phase 9B connects those `Ask` decisions to the desktop application so a user can approve **one exact tool call once**.

```text
Antigravity / Gemini
       |
       v
assistant-mcp.exe
       |
       v
PermissionEngine
       |
       +-- Allow --> native Windows tool
       |
       +-- Deny  --> rejected
       |
       +-- Ask
            |
            v
   Permission Broker Client
            |
       loopback TCP
            |
            v
   Desktop Permission Broker
            |
            v
      Tauri / React modal

       [Deny] [Allow once]
```

## Local-only transport

The desktop broker binds:

```text
127.0.0.1:0
```

Port `0` asks Windows for an ephemeral available port. There is no fixed listening port in configuration.

Connections from non-loopback peers are rejected.

## Ephemeral session secret

At desktop startup the broker creates a random session credential in memory.

The address and secret are exposed to the Antigravity process tree through two environment variables:

```text
ASSISTANT_PERMISSION_BROKER_ADDR
ASSISTANT_PERMISSION_BROKER_SECRET
```

The desktop does **not** write the secret into:

- `.agents/mcp_config.json`;
- TOML settings;
- SQLite;
- logs;
- frontend state;
- files on disk.

`AntigravityConfig` has explicit child-environment support and its `Debug` implementation prints environment **keys only**, never values.

The expected process inheritance is:

```text
Desktop
  |
  | environment
  v
agy
  |
  | inherited environment
  v
assistant-mcp.exe
```

If the installed Antigravity runtime does not pass these environment values to local MCP children, the MCP broker client is unavailable and Sensitive tools remain fail-closed. This inheritance must be verified on the user's Windows machine.

## Exact request identity

Every confirmation request contains:

```text
request_id
exact tool_name
ToolRisk
exact JSON arguments
```

Example:

```json
{
  "request_id": "...",
  "tool_name": "ui_set_value",
  "risk": "sensitive",
  "arguments": {
    "window_handle": 123456,
    "path": [2, 4, 1],
    "value": "example"
  }
}
```

The user approves the request currently waiting with that exact request id. There is no reusable approval token for another argument set.

## One-shot decisions

Phase 9B supports only:

```text
AllowOnce
Deny
```

There is deliberately no `Always allow` button for Sensitive tools.

When the user chooses `Allow once`, the broker consumes the pending one-shot channel and the waiting MCP handler continues that one tool call.

A later tool call requires a new request and a new user decision.

## Timeout behavior

The broker waits up to approximately 30 seconds for the desktop decision.

No decision means:

```text
Deny
```

The frontend starts its auto-deny slightly before the backend timeout so it normally sends an explicit Deny while the broker request is still pending.

If the response arrives after the backend timeout, the request is already removed and the late UI response is rejected as no longer pending.

That is safe: a timing race never turns into an Allow.

## Frontend confirmation UI

When a request arrives:

1. the desktop window is shown and focused even if it was hidden in the tray;
2. the backend emits `permission:request`;
3. React queues the request;
4. the first request is displayed in a modal;
5. exact arguments are visible for review;
6. the user chooses `Từ chối` or `Cho phép một lần`;
7. the Tauri command returns the decision to the broker.

Permission arguments are **not copied into the normal chat transcript**. They are only rendered in the confirmation modal because they can contain data the user must inspect but may not want persisted as conversation text.

## Request queue

The frontend can queue multiple pending requests, although the current Antigravity/tool flow is generally sequential.

Only the first request is actionable. Once it is resolved, the next one becomes visible.

The local broker itself uses bounded channels and a bounded message size to avoid unbounded local memory use.

## Message limit

Permission broker request/response JSON is capped at 64 KiB.

This is sufficient for current Sensitive tools. `ui_set_value` already has a native value-size limit below that broker message ceiling.

## Fail-closed cases

A Sensitive action is denied or remains unexecuted when:

- broker environment is missing;
- broker address is invalid or non-loopback;
- session secret is wrong;
- broker connection fails;
- request JSON is malformed;
- request exceeds the message limit;
- request id is duplicated;
- desktop request receiver is unavailable;
- desktop cannot emit the UI event;
- the user clicks Deny;
- the user closes/hides the UI and waits for timeout;
- the user does not answer in time;
- the broker response id does not match;
- the UI sends a stale/late response.

No listed failure mode falls back to automatic execution.

## Current Sensitive tools

At Phase 9B:

```text
ui_invoke
ui_set_value
ui_toggle
ui_select
```

These now become usable only after an `Allow once` decision.

Safe and Moderate tools continue to follow their Phase 9A default policy and do not contact the broker unless a future policy override changes them to `Ask`.

## Security scope

The broker protects against accidental or unauthorized model-side execution and against unauthenticated connections to the loopback port.

It is not intended as a hardened privilege boundary against malware already executing as the same Windows user. A future hardened version could use Windows named pipes with explicit ACLs and process identity checks.

The assistant itself must continue to run without administrator privileges by default.

## Local verification

Do not run GitHub Actions for this project.

On the Windows development machine verify:

1. start the desktop app and confirm no broker secret appears in logs;
2. verify `agy` starts normally with the injected environment;
3. verify `assistant-mcp.exe` inherits broker address/secret;
4. call a Safe tool and verify no confirmation appears;
5. call a Moderate tool and verify baseline behavior remains unchanged;
6. trigger `ui_invoke` and verify the confirmation modal appears before the native action;
7. click Deny and verify the target control is not invoked;
8. trigger again and click Allow once; verify exactly one invocation occurs;
9. trigger another Sensitive call and verify the previous approval is not reused;
10. let a request time out and verify no action executes;
11. close/hide the Assistant during a pending request and verify timeout denies it;
12. modify/restart Antigravity without broker environment and verify Sensitive tools fail closed;
13. verify exact UIA arguments shown in the modal match the inspected HWND/path/action payload.

## Deferred to Phase 9C

Phase 9B intentionally does not yet add:

- `AssistantState::Confirming` lifecycle integration;
- a dedicated confirming edge animation transition from the broker;
- durable permission audit log;
- user-configurable Moderate-tool overrides;
- permanent approvals for Sensitive tools;
- Windows named-pipe ACL hardening.
