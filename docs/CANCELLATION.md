# Assistant cancellation contract

Assisstant Desktop has explicit **Stop** paths for active Antigravity processing and Windows SAPI speech. Hiding the Quick overlay remains a separate operation and does not imply cancellation.

## User-facing behavior

Quick currently exposes Stop while the Assistant is in `processing` or `speaking`.

### Processing cancellation

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

### Speaking cancellation

Windows SAPI no longer runs as one uncancellable synchronous `spawn_blocking` call. `WindowsSapiTts` owns a dedicated COM worker thread for its lifetime.

```text
AssistantCore -> Speaking
          |
          v
WindowsSapiTts worker
          |
SpVoice.Speak(SVSFlagsAsync | SVSFPurgeBeforeSpeak)
          |
          +---- poll WaitUntilDone(40 ms)
          |
Quick Stop / Mic barge-in
          |
          v
SapiCommand::Cancel
          |
          v
SpVoice.Speak("", SVSFlagsAsync | SVSFPurgeBeforeSpeak)
          |
          v
purge active speech -> TtsError::Cancelled
          |
          v
AssistantCore -> Idle
```

The COM voice stays on the worker thread that initialized it. Cancellation therefore does not depend on aborting a Tokio blocking task and normally reaches SAPI within one short `WaitUntilDone` polling interval.

## Explicit mic barge-in

Quick supports an explicit click-to-barge-in path while TTS is speaking:

```text
Assistant Speaking
      |
 user presses Mic
      |
      +-- invalidate stale voice UI operation
      +-- emit quick:cancel_request
      |
 SAPI speech purged
      |
 Assistant -> Idle
      |
      v
new assistant_voice_turn
      |
 Listening -> STT -> Processing -> Speaking
```

The previous voice promise is tagged with an operation generation in Quick. Once barge-in starts, stale result/finally handlers from the interrupted turn cannot clear or overwrite the new voice turn UI.

This is **explicit barge-in**, not acoustic hands-free interruption. The microphone is not kept open during TTS, because a naive VAD listener would also hear the Assistant's own speaker output. Automatic speech-over-TTS interruption requires an echo-aware capture design and remains a later phase.

## Wake suspension during rapid voice turns

Wake suspension is reference-counted. Every `WakeService::suspend()` acquires one suspend depth and the paired `resume_after(...)` releases one after cooldown. Wake resumes only when the final outstanding suspend is released.

This prevents a delayed resume timer from an interrupted/previous voice turn from re-enabling wake-word detection while a newer voice turn is already listening.

## State/error semantics

`CoreError::Cancelled` is not treated as a backend failure.

On Antigravity cancellation:

- the active Antigravity process is terminated;
- the current session is invalidated and discarded;
- `AssistantCore` transitions back to `Idle`;
- no `backend_error` Assistant event is emitted;
- Quick clears partial streaming text and suppresses the expected cancelled promise rejection;
- the next request starts a fresh Antigravity session automatically.

On SAPI cancellation:

- pending/current SAPI speech is purged;
- `TtsError::Cancelled` is expected control flow rather than a backend failure;
- the caller still runs `finish_speaking()`, returning Assistant Core to `Idle`;
- Quick suppresses cancellation-only TTS feedback;
- read-aloud does not display a false error when the user intentionally stops it.

The Stop button is disabled after the first request until the runtime leaves the cancellable phase, preventing repeated cancel signals from the same UI action.

## Scope and safety boundary

Stop means **stop the active operation where a real interruption primitive exists**, not "undo everything that happened during the turn".

It does **not** guarantee rollback of a Windows/MCP tool side effect that already started before cancellation. For example, if a tool has already written a file or changed system state, killing the Antigravity process does not reverse that action.

Current support:

| Phase | Cancel support | Notes |
| --- | --- | --- |
| Context collection | No explicit Stop | Usually short and occurs before `Processing`. |
| Antigravity reasoning / stream | Yes | Terminates the active Antigravity process. |
| MCP/tool activity inside the agent turn | Stops the agent session | Already-started side effects may continue or already be committed. |
| Sensitive confirmation | Use Allow/Deny | Permission surface remains the authority. |
| Microphone capture (`Listening`) | Not yet | Requires a capture cancellation primitive. |
| Local STT decode | Not yet | Requires recognizer/task cancellation design. |
| Windows SAPI TTS (`Speaking`) | Yes | Dedicated COM worker accepts out-of-band purge commands. |
| Mic while `Speaking` | Yes | Explicit barge-in: stop TTS, wait for Idle, start a new voice turn. |

## Keyboard/window behavior

`Esc` continues to mean **hide Quick**, not cancel the backend turn. This distinction is intentional: users may hide the overlay while allowing a long task to continue.

Stop is therefore an explicit action rather than an overloaded dismiss shortcut.

## Local verification checklist

On Windows, verify:

1. Send a prompt that keeps Antigravity in `processing` long enough to observe Stop.
2. Press Stop once; the header should show `Đang dừng…` until the state returns to `Idle`.
3. Verify the partial streamed response stops and no red backend-error state is shown.
4. Immediately send another prompt; a fresh Antigravity session should start normally.
5. Trigger a voice turn and let SAPI begin speaking; press Stop and verify audio stops promptly and state returns to `Idle`.
6. Start read-aloud from a recent response, press Stop, and verify no false "Không thể đọc" error appears.
7. During voice-response TTS, press Mic. Verify TTS stops and Quick enters `Listening` for exactly one new voice turn.
8. Repeat barge-in quickly and verify wake-word detection does not resume during the new microphone capture.
9. Press `Esc` during a long turn; Quick should hide without cancelling the turn.
10. Verify Stop is not offered during `Executing`, `Confirming`, or `Listening`.
11. If a Windows/MCP action has already started, do not treat Stop as rollback; validate the actual tool outcome separately.

## Next cancellation phase

The next phase should add a turn-level cancellation primitive for:

1. microphone capture / VAD;
2. local partial/final STT decode where practical;
3. propagation of the same cancellation token across the entire voice turn.

Automatic acoustic barge-in should be handled separately after the capture path has echo/AEC safeguards; simply opening the existing RMS VAD during speaker TTS would create self-trigger risk.
