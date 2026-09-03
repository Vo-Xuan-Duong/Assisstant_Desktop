# Phase 9C — Permission UX State and Audit

## Purpose

Phase 9C makes permission confirmation a real Assistant Core state and adds a privacy-preserving audit trail.

It deliberately does **not** add persistent `Always allow` rules. Runtime policy overrides are separated into the next phase because the policy engine lives inside the MCP process while confirmation belongs to the desktop broker.

## Confirmation lifecycle

The backend AI turn normally runs as:

```text
Processing
    |
    v
MCP tool request
```

When the MCP permission policy returns `Ask`:

```text
Processing
    |
    v
Confirming
    |
    +---- user Allow once ----+
    |                         |
    +---- user Deny ----------+----> Processing
    |                         |
    +---- timeout ------------+
```

`AssistantCore` now exposes only two narrow lifecycle methods:

```text
begin_confirming()
finish_confirming()
```

The desktop broker cannot perform arbitrary state transitions.

This means existing edge UI state rendering receives `Confirming` through the normal `assistant:event` stream instead of fabricating a frontend-only state.

## Multiple pending requests

The desktop service keeps an in-memory pending map keyed by broker request UUID.

If multiple confirmations are ever outstanding, the Assistant remains in `Confirming` until the last pending request is resolved. A single completed request cannot incorrectly return the Assistant to `Processing` while another confirmation is still open.

## Timeout behavior

The broker remains fail-closed.

A desktop timer mirrors the broker timeout so a missing or crashed UI cannot leave the Assistant permanently in `Confirming`.

Timeout outcome:

```text
permission request
      |
      v
30 second timeout
      |
      +--> broker returns Deny
      |
      +--> desktop removes pending request
      |
      +--> audit = timeout_deny
      |
      +--> Core resumes Processing
```

The frontend still auto-denies slightly before the backend timeout when it is alive.

## Permission audit

Audit location:

```text
<AppLocalData>/audit/permissions.jsonl
```

The file is append-only JSON Lines.

The desktop also keeps the most recent 100 audit entries in RAM for diagnostics/UI without rereading the file.

Each record contains only:

```text
request_id
tool_name
risk
decision
timestamp_unix_ms
duration_ms
```

Example shape:

```json
{
  "request_id": "...",
  "tool_name": "ui_set_value",
  "risk": "sensitive",
  "decision": "allow_once",
  "timestamp_unix_ms": 0,
  "duration_ms": 1250
}
```

## Data intentionally excluded

The audit record does **not** contain:

- tool arguments;
- values written into form fields;
- clipboard content;
- screen content;
- prompt text;
- Gemini response text;
- Google/Antigravity credentials;
- permission broker secret.

This distinction is intentional: the confirmation modal may need to display exact arguments to the user, but those arguments must not become durable audit data.

## Decision labels

Current audit decisions include:

```text
allow_once
deny
timeout_deny
ui_unavailable_deny
state_rejected_deny
stale_or_failed_deny
```

They distinguish explicit user decisions from fail-closed infrastructure outcomes without storing sensitive payloads.

## Local verification checklist

No automated tests or GitHub Actions are run for this project phase.

Verify locally on Windows:

1. request a Sensitive UI action;
2. confirm edge/UI state changes from Processing to Confirming;
3. press Allow once and verify the action executes;
4. confirm state returns to Processing and then Idle;
5. repeat with Deny and verify no mutation occurs;
6. leave a confirmation unanswered and verify timeout returns to Processing;
7. inspect `<AppLocalData>/audit/permissions.jsonl`;
8. confirm audit records contain metadata only and no tool arguments;
9. queue more than one confirmation if possible and verify Core remains Confirming until all are resolved.

## Deferred to next phase

Runtime Moderate-tool overrides are intentionally deferred.

They require a policy-control channel between desktop Settings and the MCP permission engine. That channel must be authenticated/separated from the one-shot confirmation broker so policy administration cannot be confused with an individual approval request.
