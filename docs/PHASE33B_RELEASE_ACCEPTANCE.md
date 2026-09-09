# Phase 33B — Persisted Local Acceptance + Release Gate

Status: **COMPLETE IN SOURCE; LOCAL WINDOWS/ANDROID VALIDATION REQUIRED**

## Goal

Turn the existing 58-item local acceptance matrix into a persistent product-side checklist without automatically executing native tests or expanding WebView authority.

Phase 33A answers:

```text
What can the running desktop verify automatically right now?
```

Phase 33B answers:

```text
Which target-device acceptance checks has the user actually completed?
```

The two signals are deliberately kept separate and combined only at the final local release gate.

## User surface

Quick now exposes a compact **Release** panel next to the existing **Control** panel.

The Release panel shows:

- local release gate: `Pending`, `Blocked`, or `Ready`;
- manual `Passed / Failed / Blocked / Pending` totals;
- current automated runtime blocker count from `assistant_readiness`;
- all 58 acceptance checks grouped by the same categories as `PROJECT_PLAN.md`;
- the last local update time for each manually changed check;
- status filtering;
- explicit reset of the checklist after user confirmation.

## Gate semantics

The gate is fail-closed.

```text
storage/readiness error                    -> Blocked
runtime readiness has any Blocking state   -> Blocked
manual check is Failed or Blocked           -> Blocked
required manual checks still Pending        -> Pending
all 58 required checks Passed
  + runtime has no Blocking                 -> Ready
```

`Ready` means the local acceptance tracker has been completed and the current runtime self-check is not blocking. It is not a remote attestation, CI result, code-signing statement, or proof that a different machine/device combination was tested.

## Persistence

Manual acceptance state is stored in the Tauri WebView's origin-scoped `localStorage` using a versioned key:

```text
assistant.acceptance.v1
```

Schema:

```json
{
  "schemaVersion": 1,
  "items": {
    "acceptance-01": {
      "status": "passed",
      "updatedUnix": 1788930000
    }
  }
}
```

Only the fixed catalog IDs `acceptance-01` through `acceptance-58` and the four status values are meaningful to the UI.

If persisted data cannot be parsed or does not match schema v1, the panel reports an error and the combined release gate becomes `Blocked`. Invalid data is not silently overwritten. The user may explicitly reset the checklist.

## Why this remains frontend-local

Manual acceptance state is not security authority and does not need a new native Tauri command.

Keeping it frontend-local means Phase 33B adds no capability to:

- read arbitrary files;
- choose arbitrary filesystem paths;
- launch a process or shell;
- run an installer;
- capture microphone audio;
- change Windows Firewall;
- change Tailscale Serve;
- pair/rotate/reveal the Satellite credential;
- approve Sensitive MCP actions.

The existing native `assistant_readiness` command remains the only automated runtime signal used by this panel.

## Checklist source

The 58 checks mirror the current local acceptance matrix:

1–20 Core / Android

21–31 Windows / Security / TTS

32–37 Remote / Tailscale

38–40 Fallback / Release

41–44 Phase 32A / TTS UI

45–52 Phase 32B / Satellite UI

53–58 Phase 33A / System

The checklist intentionally does not include Phase 31B acoustic full duplex because that capability is deferred and outside the current release claim.

## Local acceptance for Phase 33B itself

Before relying on the tracker for a release candidate, validate locally that:

1. a changed item remains changed after closing/reopening Quick and restarting the desktop app;
2. `Pending`, `Passed`, `Failed`, and `Blocked` persist correctly;
3. filter counts match visible rows;
4. reset requires confirmation and returns all 58 items to Pending;
5. a manual Failed/Blocked item blocks the gate;
6. a runtime Blocking readiness item blocks the gate even when all manual items are Passed;
7. the gate becomes Ready only after all 58 required items are Passed and runtime readiness is non-blocking;
8. malformed local checklist data is surfaced and not silently rewritten;
9. no microphone, installer, Firewall, Tailscale, pairing-token, shell or generic file operation is triggered by checklist interaction;
10. opening the Release panel holds Quick auto-dismiss while the user is interacting with it.

## Validation policy

No GitHub Actions, native build/tests, installer execution, microphone execution, Android device run, Firewall mutation, Tailscale mutation, or model download was performed while implementing this phase.

Those remain target-machine acceptance work.
