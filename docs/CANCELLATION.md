# Assistant cancellation contract

Assisstant Desktop now has an explicit **Stop** path for an active Antigravity turn. This is deliberately separate from hiding the Quick overlay.

## User-facing behavior

While the Assistant is in `processing`, Quick shows a Stop control in the header.

```text
Quick Stop
   |
   +-- emit quick:cancel_request
            |
            v
      quick_panel.rs
            |
            v
AntigravityClient::cancel_active_turn()
            |
            v
watch cancellation generation
            |
            v
active AntigravitySession
   start_kill() child process
            |
            v
BridgeError::Cancelled
            |
            v
CoreError::Cancelled
            |
            v
AssistantCore -> Idle
```

Cancellation is intentionally out-of-band from the Antigravity session mutex. Calling `reset()` is not sufficient for Stop because `ask()` owns that mutex for the duration of a turn; a concurrent reset would wait for the turn instead of interrupting it.

## State/error semantics

`CoreError::Cancelled` is not treated as a backend failure.

On cancellation:

- the active Antigravity process is terminated;
- the current session is invalidated and discarded;
- `AssistantCore` transitions back to `Idle`;
- no `backend_error` Assistant event is emitted;
- Quick clears partial streaming text and suppresses the expected cancelled promise rejection;
- the next request starts a fresh Antigravity session automatically.

The Stop button is disabled after the first request until the runtime returns to `Idle`, preventing repeated cancel signals from the same UI action.

## Scope and safety boundary

Stop currently means **stop the AI turn**, not "undo everything that happened during the turn".

It does **not** guarantee rollback of a Windows/MCP tool side effect that already started before cancellation. For example, if a tool has already written a file or changed system state, killing the Antigravity process does not reverse that action.

The UI therefore labels the action as stopping the current AI turn and does not claim transaction rollback.

Current support:

| Phase | Cancel support | Notes |
| --- | --- | --- |
| Context collection | No explicit Stop | Usually short and occurs before `Processing`. |
| Antigravity reasoning / stream | Yes | Terminates the active Antigravity process. |
| MCP/tool activity inside the agent turn | Stops the agent session | Already-started side effects may continue or already be committed. |
| Sensitive confirmation | Use Allow/Deny | Permission surface remains the authority. |
| Microphone capture (`Listening`) | Not yet | Requires a capture cancellation primitive. |
| Local STT decode | Not yet | Requires recognizer/task cancellation design. |
| Windows SAPI TTS (`Speaking`) | Not yet | Current `Speak()` runs synchronously inside `spawn_blocking`. |

## Keyboard/window behavior

`Esc` continues to mean **hide Quick**, not cancel the backend turn. This distinction is intentional: users may hide the overlay while allowing a long task to continue.

Stop is therefore an explicit action rather than an overloaded dismiss shortcut.

## Local verification checklist

On Windows, verify:

1. Send a prompt that keeps Antigravity in `processing` long enough to observe the Stop button.
2. Press Stop once; the header should show `Đang dừng lượt AI…` until the state returns to `Idle`.
3. Verify the partial streamed response stops and no red backend-error state is shown.
4. Immediately send another prompt; a fresh Antigravity session should start normally.
5. Trigger Stop during the processing phase of a voice turn; the turn should return without starting TTS.
6. Press `Esc` during a long turn; Quick should hide without cancelling the turn.
7. Reopen Quick and verify the runtime remains usable after the hidden turn completes.
8. Verify Stop is not offered while `Listening`, `Speaking`, or on the Sensitive permission surface.
9. If a Windows/MCP action has already started, do not treat Stop as rollback; validate the actual tool outcome separately.

## Next cancellation phase

The next phase should add independent cancellation primitives for:

1. microphone capture / VAD;
2. local STT decode where practical;
3. SAPI speech output.

Those should feed a shared turn-level cancellation controller only after each subsystem has a real interrupt mechanism. Task abortion alone is not sufficient for the current blocking SAPI implementation.
