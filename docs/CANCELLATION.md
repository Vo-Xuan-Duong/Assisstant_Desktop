# Assistant cancellation contract

Assisstant Desktop has explicit **Stop** paths for voice input, Antigravity processing, and Windows SAPI speech. Hiding the Quick overlay remains a separate operation and does not imply cancellation.

## User-facing behavior

Quick exposes Stop while the Assistant is in:

- `listening`;
- `processing`;
- `speaking`.

`executing` and `confirming` remain intentionally non-cancellable from this control because an external side effect may already be in progress or a Sensitive decision is pending.

## Listening / local STT cancellation

Voice capture and local recognition use a shared cancellation generation in `voice-runtime`.

```text
Quick Stop while Listening
          |
          v
quick:cancel_request
          |
          v
voice_runtime::cancellation::cancel_current()
          |
          +------------------------------+
          |                              |
          v                              v
MicrophoneStream                    Offline STT
CancellationToken                   generation snapshot
          |                              |
next_chunk() wakes/returns None      decode result becomes stale
          |                              |
          +---------------+--------------+
                          |
                          v
               no authoritative transcript
                          |
                          v
                  AssistantCore -> Idle
```

Each microphone stream subscribes to the generation that was current when it opened. `next_chunk()` selects between the CPAL audio queue and the cancellation watcher, so Stop does not need to wait for a 25-second capture timeout or for another UI action.

Offline Zipformer recognition has no safe mid-native-decode abort primitive in the current sherpa-onnx path. The recognizer therefore checks cancellation:

1. before entering native decode;
2. again after acquiring the serialized decode gate;
3. after native decode returns, before exposing the transcript.

If cancellation occurs while native decode is already executing, CPU work may finish, but its result is returned as `SttError::Cancelled` and can never become the final Assistant prompt. The legacy opt-in Whisper backend follows the same stale-result rule.

Quick invalidates the active voice UI operation generation when Stop is pressed during `listening`, so a late promise rejection/result from the cancelled turn cannot overwrite the idle/new-turn UI.

## Processing cancellation

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

## Speaking cancellation

Windows SAPI runs on a dedicated COM worker thread for its lifetime.

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
purge active speech
          |
          +-- sentence Skip fallback if purge fails
          |
          v
TtsError::Cancelled -> AssistantCore -> Idle
```

The COM voice stays on the worker thread that initialized it. Cancellation therefore does not depend on aborting a Tokio blocking task.

## Explicit mic barge-in

Quick supports click-to-barge-in while TTS is speaking:

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

This is explicit barge-in, not acoustic hands-free interruption. The microphone is not kept open during TTS because the current RMS VAD could hear the Assistant's own speaker output. Automatic speech-over-TTS interruption requires an echo-aware/AEC design.

## Wake suspension during rapid voice turns

Wake suspension is reference-counted. Every `WakeService::suspend()` acquires one suspend depth and the paired `resume_after(...)` releases one after cooldown. Wake resumes only when the final outstanding suspend is released.

This prevents a delayed resume timer from an interrupted/previous voice turn from re-enabling wake-word detection while a newer voice turn is already listening.

## State/error semantics

Cancellation is expected control flow rather than a backend failure.

For voice input/STT cancellation:

- active microphone delivery ends through its cancellation watcher;
- Assistant Core returns from `Listening` to `Idle` immediately from the Quick cancellation router;
- an STT decode already inside native code may finish internally, but its transcript is discarded;
- no cancelled final transcript is emitted to Quick;
- no cancelled transcript reaches `complete_prompt()` or Antigravity;
- stale voice promise handlers are ignored by Quick.

For Antigravity cancellation:

- the active Antigravity process is terminated;
- the current session is invalidated and discarded;
- `AssistantCore` transitions back to `Idle`;
- no `backend_error` Assistant event is emitted;
- the next request starts a fresh Antigravity session automatically.

For SAPI cancellation:

- pending/current SAPI speech is purged or skipped through the fallback;
- `TtsError::Cancelled` is expected control flow;
- the caller still runs `finish_speaking()`;
- Quick suppresses cancellation-only TTS feedback.

## Scope and safety boundary

Stop means **stop the active operation where a real interruption or stale-result primitive exists**, not "undo everything that happened during the turn".

It does not guarantee rollback of a Windows/MCP side effect that already started before cancellation.

| Phase | Cancel support | Notes |
| --- | --- | --- |
| Context collection | No explicit Stop | Usually short and occurs before `Processing`. |
| Microphone capture / VAD (`Listening`) | Yes | Shared voice generation wakes `next_chunk()` and returns Core to Idle. |
| Local Zipformer decode (`Listening`) | Stale-result cancellation | Native decode is not forcibly aborted; cancelled output is discarded before it can become a prompt. |
| Legacy Whisper decode | Stale-result cancellation | Same boundary for the optional backend. |
| Antigravity reasoning / stream | Yes | Terminates the active Antigravity process. |
| MCP/tool activity inside the agent turn | Stops the agent session | Already-started side effects may continue or already be committed. |
| Sensitive confirmation | Use Allow/Deny | Permission surface remains the authority. |
| Windows SAPI TTS (`Speaking`) | Yes | Dedicated COM worker accepts out-of-band cancellation. |
| Mic while `Speaking` | Yes | Explicit barge-in: stop TTS, wait for Idle, start a new voice turn. |

## Keyboard/window behavior

`Esc` continues to mean **hide Quick**, not cancel the backend turn. Users may hide the overlay while allowing a long task to continue.

## Local verification checklist

On Windows, verify:

1. Start a voice turn, remain silent, then press Stop while `Listening`; Quick should return to Idle without waiting for the 25-second timeout.
2. Speak a longer utterance and press Stop while partial transcripts are appearing; no final `Bạn nói:` transcript and no Assistant request should follow.
3. Press Stop immediately after speech ends while final offline STT is likely decoding; even if CPU decode completes internally, no response should start from that cancelled transcript.
4. Start a fresh voice turn immediately after cancellation and confirm it is not cancelled by the previous generation.
5. Send a long Antigravity prompt and verify Processing Stop still terminates the active turn.
6. Let SAPI begin speaking; press Stop and verify audio stops promptly and state returns to Idle.
7. During voice-response TTS, press Mic; verify explicit barge-in starts exactly one new voice turn.
8. Repeat rapid stop/barge-in cycles and verify wake-word detection does not resume during active capture.
9. Verify Stop is not offered during `Executing` or `Confirming`.
10. Verify `Esc` only hides Quick and does not cancel active work.

## Remaining cancellation work

The main remaining voice cancellation limitation is the native offline decoder itself: sherpa-onnx `OfflineRecognizer::decode` is treated as non-interruptible once entered. The current stale-result gate is sufficient for correctness but not for reclaiming CPU immediately.

Future automatic acoustic barge-in should be implemented separately with echo/AEC safeguards rather than simply opening the existing RMS VAD during speaker TTS.
