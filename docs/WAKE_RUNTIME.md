# Phase 7B — Background Wake Runtime

## Goal

Run wake-word detection as an optional local background capability without breaking text mode, TTS, or full Whisper voice turns.

The wake runtime is deliberately separate from `AssistantCore` and `DesktopState`.

```text
CPAL microphone
      ↓
WakeRuntime
      ↓
SherpaWakeWordDetector
      ↓
wake:event
      ↓
Tauri desktop / Edge UI
```

## Feature flags

Default desktop builds do not include sherpa-onnx.

```toml
voice-whisper = ["voice-runtime/whisper"]
wake-word = ["voice-runtime/wake-sherpa"]
```

For the full local voice build, enable both desktop features locally.

## Runtime states

```text
Disabled
Starting
Listening
Suspended
Cooldown
Error
Stopped
```

`Suspended` is used when another voice path needs the microphone. `Cooldown` prevents immediate wake retrigger after a successful detection.

## Microphone ownership rule

Only one assistant subsystem should own the Windows input stream at a time.

Normal wake state:

```text
WakeRuntime
   ↓
CPAL/WASAPI microphone
```

Before a full voice turn or TTS operation:

```text
WakeRuntime.suspend()
       ↓
worker drops CPAL stream
       ↓
Suspended state confirmed
       ↓
Whisper voice turn / SAPI operation
       ↓
delayed Resume
```

`WakeRuntimeHandle::suspend()` is a resource barrier and waits for the worker to publish a state that guarantees the wake microphone is no longer owned.

After wake detection the worker also drops its microphone before publishing the `Detected` event, allowing a following full voice turn to acquire the input endpoint immediately.

## Delayed resume safety

A delayed `Resume` command never enables wake from the `Disabled` state. This prevents the following race:

```text
TTS schedules Resume
       ↓
user disables wake
       ↓
old delayed Resume arrives
       ↓
MUST remain disabled
```

## Model resources

Default local-data layout:

```text
<app-local-data>/
└── models/
    └── wake/
        └── sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01/
            ├── encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx
            ├── decoder-epoch-12-avg-2-chunk-16-left-64.onnx
            ├── joiner-epoch-12-avg-2-chunk-16-left-64.int8.onnx
            ├── tokens.txt
            └── keywords.txt
```

`keywords.txt` must be generated using the tokenizer belonging to the chosen sherpa model. Do not hand-write model token IDs.

## Environment overrides

```text
ASSISTANT_WAKE_MODEL_DIR
ASSISTANT_WAKE_KEYWORDS
ASSISTANT_WAKE_ENABLED
```

Examples:

```powershell
$env:ASSISTANT_WAKE_ENABLED="1"
$env:ASSISTANT_WAKE_MODEL_DIR="D:\AI\wake-model"
$env:ASSISTANT_WAKE_KEYWORDS="D:\AI\wake-model\keywords.txt"
```

`ASSISTANT_WAKE_ENABLED` accepts values such as `1/0`, `true/false`, `yes/no`, and `on/off`.

## Safe startup behavior

Wake word is disabled by default.

If the desktop build contains `wake-word` but model resources are missing:

- application startup continues;
- text assistant continues to work;
- TTS continues to work;
- Whisper voice can continue to work independently;
- wake status becomes `unavailable`;
- UI explains which resource is missing.

There is no automatic model download in Phase 7B.

## Desktop commands

Tauri exposes:

```text
assistant_wake_status
assistant_wake_set_enabled
```

Frontend API:

```text
getWakeStatus()
setWakeEnabled(enabled)
onWakeEvent(...)
```

The UI shows wake readiness/state and only enables the toggle when the runtime is compiled and resources are available.

## Wake events

The runtime emits:

```text
state_changed
detected
error
```

The desktop forwards them as:

```text
wake:event
```

In Phase 7B a successful detection opens/focuses the Assistant and activates the edge glow. It does **not** automatically start the full Whisper command capture yet; that belongs to Phase 7C.

## Privacy boundary

Wake audio remains local. The wake detector processes microphone chunks locally and does not send them to Antigravity/Gemini.

Antigravity only receives text after a later full STT turn.

Always-on wake is an explicit opt-in setting. It is not enabled merely because the application supports the feature.

## Local verification checklist

No CI/runtime tests are run by the repository development workflow for this phase. Verify on Windows locally:

1. Build without `wake-word`; application must still open normally.
2. Build with `wake-word` but without model resources; UI reports unavailable.
3. Install model + `keywords.txt`; toggle Wake ON.
4. Confirm state reaches `listening`.
5. Speak the configured phrase and confirm the Assistant window + edge effect appears.
6. Start a manual Whisper voice turn while Wake is ON; there should be no microphone-in-use conflict.
7. Use TTS while Wake is ON and verify wake detection does not trigger from the Assistant's own voice.
8. Disable Wake during/after a voice turn and confirm delayed resume does not re-enable it.
9. Verify tray/hotkey activation still works with Wake OFF.
10. Verify a missing or disconnected microphone moves wake into an error/retry state without crashing the desktop app.

## Deferred to Phase 7C

- wake detection automatically transitions to full Listening;
- automatic Whisper command capture after the wake phrase;
- response result delivery to the main conversation without a button click;
- wake-to-voice single-flight orchestration;
- cancellation and timeout UX for automatic turns.
