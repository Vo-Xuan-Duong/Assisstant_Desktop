# Assistant cancellation contract

Assisstant Desktop has an explicit **Stop** path for active Antigravity processing and Windows SAPI speech. This is deliberately separate from hiding the Quick overlay.

## User-facing behavior

Quick shows Stop while the Assistant is in `processing` or `speaking`.

### Processing

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

Antigravity cancellation is intentionally out-of-band from the session mutex. Calling `reset()` is not sufficient because `ask()` owns that mutex for the duration of a turn; a concurrent reset would wait instead of interrupting it.

### Speaking

```text
Quick Stop
   |
   +-- emit quick:cancel_request
            |
            v
      quick_panel.rs
            |
            v
WindowsSapiTts::cancel()
            |
            v
COM-owning SAPI worker thread
            |
            +-- SVSFPurgeBeforeSpeak
            |
            v
assistant_speak / voice turn unwinds normally
            |
            v
AssistantCore -> Idle
```

`ISpeechVoice` remains entirely on the worker thread that created it. The runtime does not move or call the COM voice from a Tokio thread.

## SAPI worker design

The old TTS implementation called synchronous `ISpeechVoice::Speak()` inside `spawn_blocking`, which could not be interrupted correctly by aborting the Rust future.

The interruptible worker now:

1. owns COM initialization and one `ISpeechVoice` on a dedicated named thread;
2. starts speech with `SVSFlagsAsync`;
3. polls `WaitUntilDone(40)` so it can process worker commands with bounded latency;
4. receives Stop through a standard channel;
5. purges the active utterance on that same COM thread with `SVSFPurgeBeforeSpeak`;
6. completes a successful user-initiated purge as normal completion rather than a TTS error.

A second Speak command received while the worker is busy is rejected instead of being silently queued behind the active utterance. Normal Assistant state gating should prevent this path in ordinary UI use.

## Registration-race handling

`Processing` and `Speaking` are published just before their respective backend cancellation receivers may exist.

`quick_panel.rs` therefore routes cancellation by the current Assistant state, tries once immediately, then retries exactly once after 20 ms only if the same phase is still active.

This closes the tiny human-clickable registration gap without making cancellation sticky enough to affect a later AI turn or later TTS utterance.

## State/error semantics

### AI turn cancellation

`CoreError::Cancelled` is not treated as a backend failure.

On cancellation:

- the active Antigravity process is terminated;
- the current session is invalidated and discarded;
- `AssistantCore` transitions back to `Idle`;
- no `backend_error` Assistant event is emitted;
- Quick clears partial streaming text and suppresses the expected cancelled promise rejection;
- the next request starts a fresh Antigravity session automatically.

### TTS cancellation

A successful SAPI purge is treated as normal completion:

- speech stops;
- `assistant_speak` still executes `finish_speaking()`;
- wake resumes through the existing cooldown path;
- response text remains available;
- current-response/history `Đọc` promises resolve without showing a false error state.

Only an actual SAPI purge failure is returned as a TTS backend error.

The Stop button is disabled after the first request until the runtime returns to `Idle`, preventing repeated cancel signals from the same UI action.

## Scope and safety boundary

Stop during `processing` means **stop the AI turn**, not "undo everything that happened during the turn".

It does **not** guarantee rollback of a Windows/MCP tool side effect that already started before cancellation. For example, if a tool has already written a file or changed system state, killing the Antigravity process does not reverse that action.

Stop during `speaking` only stops audio output. It does not delete or invalidate the completed response.

Current support:

| Phase | Cancel support | Notes |
| --- | --- | --- |
| Context collection | No explicit Stop | Usually short and occurs before `Processing`. |
| Antigravity reasoning / stream | Yes | Terminates the active Antigravity process. |
| MCP/tool activity inside the agent turn | Stops the agent session | Already-started side effects may continue or already be committed. |
| Sensitive confirmation | Use Allow/Deny | Permission surface remains the authority. |
| Microphone capture (`Listening`) | Not yet | Needs a clean state boundary with local STT decode. |
| Local STT decode | Not yet | Current blocking decode does not have a real interrupt primitive. |
| Windows SAPI TTS (`Speaking`) | Yes | Async SAPI worker purges the active utterance on its COM-owning thread. |

## Keyboard/window behavior

`Esc` continues to mean **hide Quick**, not cancel backend work or TTS. This distinction is intentional: users may hide the overlay while allowing a long task or speech output to continue.

Stop is therefore an explicit action rather than an overloaded dismiss shortcut.

## Local verification checklist

On Windows, verify:

1. Send a prompt that keeps Antigravity in `processing` long enough to observe Stop.
2. Press Stop once; the header should show `Đang dừng lượt AI…` until the state returns to `Idle`.
3. Verify partial streamed response stops and no red backend-error state is shown.
4. Immediately send another prompt; a fresh Antigravity session should start normally.
5. Trigger Stop during the processing phase of a voice turn; the turn should return without starting TTS.
6. Complete a response, click `Đọc`, then press Stop while state is `Speaking`; audio should stop promptly and state should return to `Idle` without a TTS error.
7. Repeat from one item in Recent Responses; Stop should stop only playback and keep the stored text available.
8. Trigger a voice turn that reaches automatic TTS, then Stop during `Speaking`; response text should remain and wake should resume after the existing cooldown.
9. Press Stop immediately as `Speaking` first appears; the 20 ms registration retry should still stop the utterance rather than leaving Stop pending indefinitely.
10. Press `Esc` during processing/speaking; Quick should hide without cancellation.
11. Reopen Quick and verify the runtime remains usable after the hidden operation completes.
12. If a Windows/MCP action has already started, do not treat Stop as rollback; validate the actual tool outcome separately.

## Remaining cancellation work

The remaining voice cancellation work is limited to the input side:

1. microphone capture / VAD;
2. a clean state boundary around local STT decode, with a real recognizer cancellation mechanism where practical.

Task abortion alone is not sufficient for blocking native work, so those phases should only expose Stop once their subsystem has a genuine interrupt contract.
