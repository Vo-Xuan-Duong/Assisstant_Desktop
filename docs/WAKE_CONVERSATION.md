# Wake to Conversation

## Goal

Turn a successful local wake-word detection into a complete assistant interaction without requiring the user to click the Mic button.

```text
Wake phrase
    ↓
sherpa-onnx KWS
    ↓
WakeRuntimeEvent::Detected
    ↓
Quick Assistant + Edge Glow
    ↓
180 ms wake-to-command gap
    ↓
automatic assistant_voice_turn
    ↓
Vietnamese Zipformer STT
    ↓
Antigravity / Gemini
    ↓
MCP / Windows tools
    ↓
Windows SAPI TTS
    ↓
Wake runtime resumes
```

## One authoritative voice path

Wake does not use a second STT/AI pipeline.

The background Rust wake event handler shows Quick with:

```text
show_quick_window(&app, "wake")
```

which emits:

```text
quick:shown { reason: "wake" }
```

The lifetime-owned `QuickOverlay` WebView receives that event, waits the wake-to-command gap, and invokes the same Tauri command used by its Mic button:

```text
assistant_voice_turn
```

Manual and wake-triggered turns therefore share:

- microphone capture and VAD;
- Vietnamese Zipformer recognizer loading;
- AssistantCore state transitions;
- source-window/context handling;
- Antigravity conversation;
- MCP / permission handling;
- Windows SAPI TTS;
- wake suspend/resume behavior;
- assistant/edge events.

There is no hidden full-management React listener and no duplicate microphone pipeline.

## Activation sequence

```text
WakeRuntimeEvent::Detected
      ↓
emit wake:event for observers
      ↓
show_quick_window(..., "wake")
      ↓
remember external source application
      ↓
position Edge + Quick on source monitor
      ↓
quick:shown { reason: "wake" }
      ↓
QuickOverlay refreshes voice capability
      ↓
180 ms delay
      ↓
assistant_voice_turn
```

When AssistantCore enters `Listening`, Edge and Quick naturally move to the listening state using existing events. No wake-specific AssistantCore state is needed.

## Wake-to-command delay

The Quick frontend keeps the established:

```text
180 ms
```

delay before command capture. Its purpose is acoustic/UX separation:

- let the wake detector release its microphone stream;
- reduce capture of the wake phrase tail as command audio;
- provide a visible activation-to-listening transition.

The backend wake suspend/resume and voice `turn_gate` remain the authoritative synchronization boundaries.

## Recommended speech pattern

The current runtime still works best with a short separation:

```text
"Hey Assistant"
(short pause)
"Mở Chrome"
```

There is not yet a shared continuous audio ring buffer spanning KWS into command STT. A fully uninterrupted phrase may therefore lose the earliest part of the command during the handoff.

## Single-flight behavior

Quick maintains a synchronous busy guard around manual/wake voice capture, while the backend retains its own voice `turn_gate` and AssistantCore state checks.

Policy:

```text
Assistant idle + STT ready
    → accept wake-triggered turn

Assistant busy
    → do not start an overlapping turn

STT unavailable
    → show Quick and report the terminal resource command
```

The missing-STT recovery path points to:

```powershell
assistant resources install stt_zipformer_vi
```

rather than a removed graphical Resource Setup panel.

## Conversation result

Automatic wake turns return the same `VoiceTurnResult` as manual voice:

```text
transcript
response
tts_error
```

The response is displayed in Quick and remains part of the same backend Antigravity conversation. The graphical runtime no longer maintains a separate full chat-history application.

## Failure behavior

### STT not compiled/model missing

Quick remains the presentation surface and reports that local voice is unavailable. No permission/main window is used as fallback.

### Assistant already busy

An overlapping voice turn is rejected/ignored by existing frontend/backend single-flight checks; it is not intentionally queued.

### No command speech after wake

The existing 25-second utterance timeout applies. The turn fails cleanly and wake resumes through the normal delayed resume path.

### Antigravity failure

AssistantCore follows its normal Error lifecycle. Wake does not add a second recovery protocol.

### Quick cannot be shown

The runtime logs the failure and hides Edge. It does not surface the permission-only `main` window.

## Privacy

Wake detection and command speech recognition remain local:

```text
wake audio       → local sherpa-onnx KWS
command audio    → local Vietnamese Zipformer STT
transcript       → Antigravity
```

Only the resulting text and any explicitly collected desktop context are passed into the AI request path.

## Local verification checklist

Do not manually dispatch GitHub Actions for this verification. On Windows:

1. Build the desktop with STT and wake features enabled.
2. Provide required wake resources and Vietnamese Zipformer assets.
3. Enable wake using `assistant wake enable`.
4. Configure/validate the phrase using `assistant wake phrase "HEY ASSISTANT"` as needed.
5. Keep the assistant in the background and focus another application.
6. Say the configured wake phrase.
7. Confirm Quick + Edge appear on the source monitor; the permission window must not appear.
8. Confirm exactly one voice turn enters Listening automatically after the short delay.
9. Say a command after the wake pause.
10. Confirm Vietnamese Zipformer produces a transcript and Antigravity returns a response.
11. Confirm SAPI speaks the response and wake returns to listening after cooldown.
12. Trigger wake while another request is busy; verify a second overlapping voice capture does not start.
13. Remove/rename the STT model temporarily and verify wake shows a useful missing-resource error rather than crashing.
14. Confirm tray and normal second-launch activation also use Quick and do not create a second wake listener.

## Deferred

- continuous shared microphone ring buffer from KWS into command STT;
- command audio pre-roll covering the wake/command boundary;
- barge-in while TTS is speaking;
- multi-turn hands-free follow-up window;
- explicit cancel phrase / Stop control;
- configurable automatic-listening duration;
- dynamic Quick response sizing.
