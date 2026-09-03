# Voice Runtime — Phase 5C Desktop Integration

## Scope

Phase 5C connects the local audio/VAD/STT modules to the desktop Assistant and adds native Windows text-to-speech.

This phase implements **one-shot voice turns**, not an always-listening assistant yet.

```text
Mic button
   |
Listening
   |
CPAL / WASAPI
   |
local VAD
   |
Whisper STT
   |
text request
   |
Context Engine
   |
Antigravity / Gemini
   |
text response
   |
Windows SAPI TTS
   |
Speaking -> Idle
```

## Default build behavior

The normal desktop build keeps Whisper disabled.

- text Assistant works;
- Antigravity/MCP works;
- Windows SAPI TTS is available;
- microphone voice turns are disabled with an explicit UI explanation.

This preserves a lightweight default build and avoids requiring the Whisper native C/C++ toolchain for developers who are working on non-voice parts of the project.

## Enabling local Whisper

The desktop Rust crate exposes:

```toml
voice-whisper = ["voice-runtime/whisper"]
```

Enable the `voice-whisper` Cargo feature when building/running the desktop binary locally.

The exact local command can be chosen to match the developer workflow; the important contract is that the `assisstant-desktop` crate is compiled with the `voice-whisper` feature.

No GitHub Action is required or configured for this verification.

## Whisper model location

The desktop runtime lazy-loads the model on the first voice turn.

Default local-data path:

```text
<Windows app local data>/models/whisper/ggml-base.bin
```

The path can be overridden before launch:

```text
ASSISTANT_WHISPER_MODEL=<absolute model path>
```

The repository does not commit or automatically download model files in this phase.

The UI queries `assistant_voice_capabilities` and keeps the microphone button disabled unless:

1. the binary was compiled with `voice-whisper`;
2. the configured model file exists.

Text mode continues to work when either condition is missing.

## One-shot voice lifecycle

A voice turn is single-flight and uses the Assistant Core state machine:

```text
Idle
 |
Listening
 |
VAD finds complete utterance
 |
Whisper transcribes locally
 |
Processing
 |
Antigravity returns response
 |
Idle
 |
Speaking
 |
Windows SAPI finishes
 |
Idle
```

All capture/model/STT failure paths cancel `Listening` before returning an error to the UI. A failed voice turn must not leave the Assistant stuck in listening state.

A 25-second safety timeout bounds microphone capture when no complete utterance is detected.

## Local TTS

`WindowsSapiTts` uses the Windows `SpVoice` COM component through `windows-rs`.

The COM voice object is created and used entirely inside a blocking worker because SAPI COM interfaces are not moved into the async runtime.

The TTS path:

- does not use Antigravity quota;
- does not require a network request;
- uses voices installed/available to Windows;
- has configurable SAPI rate and output volume in the Rust abstraction.

Voice turns automatically read the AI response aloud. The text UI also exposes a `Đọc lại phản hồi cuối` action.

## Realtime UI signal

While the microphone is active, the backend emits:

```text
voice:level
```

with:

```text
rms
peak
```

The Text Desktop UI currently renders a small level meter from this event.

This event is intentionally reusable: the next Gemini-like Edge UI phase can drive border intensity from the same signal without coupling the visual layer to CPAL.

## Failure isolation

Voice is an optional enhancement around the already-working text Assistant.

Failures such as:

- no microphone;
- microphone disconnect;
- missing Whisper model;
- invalid model;
- STT failure;
- SAPI/TTS failure;

do not remove the text path.

TTS failure after a successful voice request is returned separately as `tts_error`, so the transcript and Gemini response can still be shown in the conversation.

## Not included yet

The following remain separate phases:

- wake word;
- always-on listening;
- barge-in / interrupting TTS by speaking;
- streaming partial Whisper transcripts;
- neural VAD;
- model download/update management;
- custom voice model;
- Gemini-like full-screen edge glow.

The next UI phase can now use stable Assistant states (`Listening`, `Processing`, `Speaking`) plus `voice:level` to implement the screen-edge visual experience independently of the voice engine.

## Verification policy

No tests, GitHub Actions, or workflow runs are executed by this development process. Native microphone, Whisper, SAPI and Tauri behavior is intended to be verified on the local Windows machine.
