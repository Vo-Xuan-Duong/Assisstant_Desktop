# Voice Runtime — Desktop Integration

## Scope

The desktop voice path connects CPAL microphone capture, local VAD, Vietnamese Zipformer STT, Assistant Core, and Windows SAPI TTS.

The normal voice turn is still bounded to one utterance at a time; wake can trigger that turn automatically, but the microphone is not left continuously recording between turns.

```text
Mic / Wake
   |
Listening
   |
CPAL / WASAPI
   |
local VAD
   |
   +--> active snapshots --> throttled Zipformer partial decode --> Quick UI only
   |
complete utterance
   |
final Zipformer decode
   |
final text request
   |
Context Engine / local Safe path
   |
Antigravity / Gemini + MCP
   |
text response
   |
Windows SAPI TTS
   |
Speaking -> Idle
```

## Build behavior

The preferred desktop feature is:

```toml
voice-stt
```

The historical feature name remains as a compatibility alias:

```toml
voice-whisper
```

In the current desktop crate, `voice-whisper` enables the Zipformer STT path rather than the legacy `whisper-rs` backend. This keeps existing build/release configuration valid while runtime naming is migrated incrementally.

Text Assistant, Antigravity/MCP, and TTS continue to work when voice STT resources are unavailable.

## Vietnamese STT resource

Resource id:

```text
stt_zipformer_vi
```

Default model directory:

```text
<Windows app local data>/models/stt/
  sherpa-onnx-zipformer-vi-30M-int8-2026-02-09/
```

Required runtime files:

```text
encoder.int8.onnx
decoder.onnx
joiner.int8.onnx
tokens.txt
```

Optional/preparation file:

```text
bpe.model
```

Install through the shared runtime resource installer:

```powershell
assistant resources install stt_zipformer_vi
```

For diagnostics, the model directory can be overridden with an absolute path:

```text
ASSISTANT_ZIPFORMER_MODEL_DIR
```

The UI queries `assistant_voice_capabilities` and treats microphone voice turns as ready only when the voice STT feature is compiled and the complete runtime bundle is present.

## Voice lifecycle

A voice turn is single-flight and uses the Assistant Core state machine:

```text
Idle
 |
(load recognizer if needed)
 |
Listening
 |
partial voice:transcript events (UI only)
 |
VAD finds complete utterance
 |
final Zipformer transcript
 |
Processing / Executing / Confirming as needed
 |
Assistant response
 |
Speaking
 |
Windows SAPI finishes or is explicitly interrupted
 |
Idle
```

The recognizer is loaded before entering `Listening`, so the first voice turn cannot display a listening state while the model is still being initialized.

All capture/final-STT failure paths cancel `Listening` before returning an error to the UI. A failed voice turn must not leave the Assistant stuck in the listening state.

A 25-second safety timeout bounds microphone capture when no complete utterance is detected.

## Partial transcript UI

While VAD has an active utterance, the backend can emit:

```text
voice:transcript
```

Payload:

```text
text
is_final
```

Behavior:

- `is_final=false`: throttled preview generated from an active VAD snapshot;
- `is_final=true`: authoritative full-utterance recognition result;
- partial events never call Assistant Core and never create Antigravity requests;
- Quick clears previous transcript state when a new invocation or request begins;
- Quick uses the final transcript as `Bạn nói: ...` while the response continues processing/speaking.

The current model is offline, so these partial previews are simulated streaming rather than native frame/token streaming. See `VOICE_STT.md` for the decode/throttle contract.

## Audio level UI signal

While the microphone is active, the backend also emits:

```text
voice:level
```

with:

```text
rms
peak
```

Quick and edge effects can consume this event without coupling visual code directly to CPAL.

## Local TTS

`WindowsSapiTts` uses the Windows `SpVoice` COM component through `windows-rs`.

SAPI now runs on a dedicated COM worker thread. Speech is queued asynchronously with `SVSFlagsAsync`, and the worker polls `WaitUntilDone` in short intervals. This lets the same worker receive an out-of-band Cancel command and purge active/pending speech with `SVSFPurgeBeforeSpeak`.

```text
Assistant -> Speaking
        |
WindowsSapiTts worker
        |
SpVoice async speech
        |
        +-- WaitUntilDone(40 ms)
        |
Quick Stop / Mic
        |
SapiCommand::Cancel
        |
purge active stream
        |
Assistant -> Idle
```

The TTS path:

- does not use Antigravity quota;
- does not require a network request;
- uses voices installed/available to Windows;
- has configurable SAPI rate and output volume in the Rust abstraction;
- can be interrupted without aborting or moving the COM voice across threads.

Voice turns automatically read the Assistant response aloud. Quick response actions and Recent Responses reuse the same native `assistant_speak` path for read-aloud. An intentional SAPI cancellation is treated as expected control flow rather than a read-aloud failure.

## Explicit barge-in

While the Assistant is in `Speaking`, the Quick microphone button remains available. Pressing it performs explicit click-to-barge-in:

```text
Speaking
   |
Mic click
   |
quick:cancel_request
   |
SAPI purge
   |
Idle
   |
new assistant_voice_turn
   |
Listening
```

Quick assigns a generation to each voice UI operation. Starting barge-in invalidates the interrupted generation so stale result/finally handlers from the previous voice promise cannot overwrite or clear the new turn.

This phase does **not** keep the microphone open while TTS is playing. Automatic speech-over-TTS interruption requires echo/AEC safeguards; otherwise the current RMS VAD could interpret the Assistant's own speaker output as user speech.

## Wake integration

Wake detection shows Quick and starts the existing voice turn after the configured wake-to-command gap. Wake is suspended while capture/TTS is active and resumes afterward, reducing self-trigger risk.

Wake suspension is reference-counted. Rapid consecutive turns or explicit barge-in can temporarily overlap old cooldown timers with a new suspension; wake resumes only when the final outstanding suspension is released. This prevents an older `resume_after(...)` timer from re-enabling wake detection while a newer voice turn is listening.

Wake detection does not bypass the normal voice lifecycle, Assistant Core, MCP permission gateway, or Sensitive confirmation path.

## Failure isolation

Voice remains an optional enhancement around the text Assistant.

Failures such as:

- no microphone;
- microphone disconnect;
- incomplete Zipformer resource bundle;
- invalid native model;
- partial STT decode failure;
- final STT failure;
- SAPI/TTS failure;

do not remove the text path.

Partial decode errors are logged and ignored; final recognition remains authoritative. TTS failure after a successful voice request is returned separately as `tts_error`, so the transcript and Assistant response remain available. Intentional TTS cancellation is suppressed by Quick rather than presented as a red failure.

## Remaining voice work

Separate follow-up work includes:

- microphone/VAD cancellation while `Listening`;
- local STT decode cancellation where practical;
- one shared turn-level voice cancellation controller;
- automatic acoustic barge-in with echo/AEC safeguards;
- true sherpa-onnx `OnlineRecognizer` streaming with a compatible Vietnamese model;
- stronger/neural VAD such as Silero VAD;
- contextual biasing / application-name hotwords;
- commercial-friendly STT model selection if required;
- measured latency/accuracy tuning on target Windows hardware.

## Verification policy

No tests, GitHub Actions, workflow runs, native builds, model downloads, or microphone tests are executed by the remote development process. Native microphone, Zipformer, SAPI and Tauri behavior is intended to be verified on the target Windows machine.
