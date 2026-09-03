# Phase 7C — Wake to Conversation

## Goal

Turn a successful local wake-word detection into a complete assistant interaction without requiring the user to click the Mic button.

```text
Wake phrase
    ↓
Sherpa KWS
    ↓
wake:event detected
    ↓
Assistant window + Edge Glow
    ↓
automatic full voice turn
    ↓
Whisper STT
    ↓
Antigravity / Gemini
    ↓
MCP / Windows tools
    ↓
SAPI TTS
    ↓
Wake runtime resumes
```

## Reuse the existing voice path

Phase 7C deliberately does not create a separate backend command for wake-triggered speech.

The hidden main WebView remains alive while the application is in the system tray. It already receives `wake:event` events. On `detected`, it invokes the same Tauri command as the manual Mic button:

```text
assistant_voice_turn
```

This means manual and wake-triggered turns share:

- Whisper model loading;
- VAD and microphone capture;
- Antigravity request handling;
- context collection;
- TTS;
- wake suspend/resume behavior;
- AssistantCore state transitions;
- edge UI state events.

There is only one full voice pipeline to maintain.

## Activation sequence

Backend wake handling still owns system activation:

```text
WakeRuntimeEvent::Detected
      ↓
wake:event
      ↓
show_main_window()
      ↓
remember source application
      ↓
position edge windows on source monitor
      ↓
edge activated
```

Frontend then starts the full voice turn.

When `AssistantCore` enters `Listening`, the edge effect naturally changes from activation bloom to listening animation. No special edge-wake state is required.

## Wake-to-command delay

The frontend uses a short delay before opening the full command microphone:

```text
180 ms
```

Purpose:

- let the wake detector finish releasing its input stream;
- avoid treating the final acoustic tail of the wake phrase as the beginning of the command;
- give the activation glow a perceptible transition into Listening.

The backend microphone suspend barrier remains authoritative. The delay is UX/acoustic separation, not the synchronization mechanism.

## Recommended speech pattern for this phase

Use two beats:

```text
"Hey Assistant"
(short pause)
"Mở Chrome"
```

Phase 7C does not yet implement a shared continuous audio ring buffer between KWS and Whisper. Therefore a single uninterrupted phrase such as:

```text
"Hey Assistant mở Chrome"
```

may lose the first portion of the command while the wake stream is released and the full Whisper stream starts.

A future continuous-audio phase can remove this limitation.

## Single-flight behavior

The frontend keeps a synchronous `busyRef` in addition to React state.

This prevents two wake detections or a wake + manual Mic click from queueing overlapping voice turns before React has rendered its next state.

Policy:

```text
Assistant idle + Whisper ready
    → accept wake-triggered turn

Assistant busy
    → ignore additional wake detection

Whisper unavailable
    → keep Assistant open and report that local voice is unavailable
```

The backend still retains its own voice `turn_gate`, so frontend single-flight is an additional UX guard rather than the only concurrency control.

## Conversation result

The automatic turn returns the same `VoiceTurnResult` as manual voice:

```text
transcript
response
tts_error
```

The frontend appends:

1. transcript as a user message;
2. Antigravity response as an assistant message;
3. optional TTS error as a system message.

Therefore wake interactions become normal conversation history and subsequent prompts can continue using the same Antigravity conversation.

## Failure behavior

### Whisper not compiled/model missing

Wake detection still opens the Assistant, but no automatic voice turn starts. The conversation shows a local runtime message.

### Assistant already busy

The extra detection is ignored. It is not queued.

### No command speech after wake

The existing voice capture timeout applies. The turn fails cleanly and wake runtime resumes after the normal delayed resume path.

### Antigravity failure

AssistantCore enters its normal Error state and the frontend reports the failed voice turn. No wake-specific backend recovery path is introduced.

## Privacy

The wake phrase remains local to sherpa-onnx.

After wake activation, command audio is processed by local Whisper. Only the resulting text and any explicitly requested desktop context are passed to Antigravity.

```text
wake audio       → local KWS
command audio    → local Whisper
transcript       → Antigravity
```

## Local verification checklist

Do not run GitHub Actions for this phase. Verify locally on Windows:

1. Build desktop with both `wake-word` and `voice-whisper`.
2. Provide sherpa wake resources and Whisper model.
3. Enable Wake in the UI.
4. Hide the Assistant to tray.
5. Focus an application on monitor 1 or monitor 2.
6. Say configured wake phrase.
7. Confirm Assistant opens on detection and edge glow surrounds the source monitor.
8. Confirm state moves into Listening automatically without clicking Mic.
9. Say a command after the wake phrase pause.
10. Confirm transcript appears in conversation.
11. Confirm Antigravity response appears and SAPI speaks it.
12. Confirm wake returns to listening after the TTS cooldown.
13. Trigger wake while a manual request is already processing; verify a second voice turn is not queued.
14. Test with Whisper model removed; wake should open UI but not crash the application.
15. Test wake while main window is hidden; event handling must still work.

## Deferred

- continuous shared microphone ring buffer from KWS into Whisper;
- command audio pre-roll covering the wake/command boundary;
- barge-in while TTS is speaking;
- multi-turn hands-free follow-up window;
- explicit cancel phrase;
- configurable automatic-listening duration;
- persistent wake/auto-turn settings.
