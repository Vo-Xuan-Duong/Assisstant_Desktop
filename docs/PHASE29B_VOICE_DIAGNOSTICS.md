# Phase 29B — Android Voice Diagnostics

## Goal

Expose enough recognition/turn metadata on the Android Voice Satellite to evaluate real-device speech quality and latency without changing what command is executed on Windows.

Diagnostics are deliberately **read-only**. They do not rewrite, rank, auto-correct, or block a command based on confidence.

## Recognition result

For each final Android `SpeechRecognizer` result, the app records:

- the first recognition candidate — this remains the exact text submitted to the desktop;
- up to two additional recognition alternatives for display only;
- the first confidence score when Android provides a valid `0..1` value;
- elapsed recognition time measured with `SystemClock.elapsedRealtime()`;
- whether the final result came from the on-device recognizer or the system recognizer.

Android does not guarantee confidence data for every recognizer/result. Missing confidence is displayed as unavailable rather than treated as low confidence.

The UI can therefore show information such as:

```text
Bạn nói
Mở Visual Studio Code

STT: on-device · 842 ms · confidence 91%
Phương án khác: Mở Visual Studio Cốt · Mở VS Code
```

The alternatives are never submitted automatically.

## Desktop-turn latency

After the final recognized text is successfully submitted, Android records the start timestamp. When the desktop `response` event arrives, the app displays elapsed time:

```text
Desktop turn: 2380 ms (AI/tool/TTS đến khi response hoàn tất)
```

This is intentionally **not** described as network RTT. It includes the full accepted desktop turn from successful command submission through Assistant processing, possible tools, and desktop TTS completion before the response callback.

Cancellation, connection loss, and terminal errors clear the in-flight timing marker so stale timing is not attached to a later response.

## Conversational-mode interaction

Automatic conversational follow-up turns reuse current in-memory preferences and no longer rewrite settings/Android Keystore on every follow-up window.

Each new recognition window clears only the previous STT diagnostics. The completed desktop response/turn timing can remain visible while the next conversational recognition starts.

## Safety invariants

- the top `RESULTS_RECOGNITION` candidate remains the command text;
- confidence never auto-approves or auto-rejects a Windows action;
- alternatives never trigger additional commands;
- diagnostics do not weaken desktop permissions or confirmation gates;
- no automatic command normalization is introduced in this phase;
- missing confidence is valid and does not count as an error.

## Local validation

On the target Android device:

1. verify the top recognized phrase still matches the text sent to Windows;
2. verify alternatives appear only when the recognizer returns them;
3. verify confidence appears only when the recognizer supplies a valid value;
4. compare on-device vs system recognition engine labels;
5. compare STT elapsed time across Vietnamese and English turns;
6. verify desktop-turn elapsed time is cleared on cancellation/error;
7. verify conversational follow-up does not repeatedly rewrite the pairing secret/settings;
8. verify diagnostics remain useful over both LAN and Tailscale transport.

No remote Android build/test or device execution is performed by repository policy; validate these metrics locally on real hardware.
