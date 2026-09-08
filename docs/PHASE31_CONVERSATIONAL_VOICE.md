# Phase 31A — Safe Conversational Voice

## Goal

Make repeated Android voice turns feel conversational without keeping Android `SpeechRecognizer` active while the desktop speaker is talking.

The phone remains an input satellite. Windows remains the only reasoning/tool authority.

## Turn-taking model

```text
Android listens
  -> final transcript
  -> desktop Assistant turn
  -> desktop TTS speaks response
  -> response event is returned after TTS finishes
  -> 450 ms guard delay
  -> Android opens one new SpeechRecognizer window
```

This reuses the existing desktop session, so follow-up utterances continue through the same Assistant Core/Antigravity conversation context unless the desktop session is explicitly reset elsewhere.

## Android UI

A persisted **Hội thoại liên tục** switch enables conversational follow-ups.

The switch does not immediately activate the microphone. A session becomes active only after the user starts a normal voice turn (main button or Quick Settings activation).

While a conversation session is active, the UI exposes **Kết thúc hội thoại**. Ending the conversation:

- cancels a currently open Android recognition window;
- cancels any scheduled follow-up listen;
- prevents later desktop response callbacks from reopening the microphone;
- does not cancel a Windows action already Processing/Executing merely because the user no longer wants another follow-up.

The existing **Dừng Assistant** action remains the explicit control for cancelling safe desktop phases.

## Failure behavior

Conversation mode is intentionally bounded and fail-closed.

A conversation session stops when:

- the desktop connection is lost;
- the pairing/desktop protocol reports a terminal error;
- TTS reports an error;
- microphone permission is unavailable;
- Android recognition reaches a terminal `NO_MATCH`, speech timeout, network, audio, permission, server, or client error;
- a follow-up is due but the desktop is not idle/ready.

On-device recognizer recovery messages are status events rather than terminal failures. The existing bounded on-device -> system fallback and one busy retry can therefore complete without incorrectly ending an active conversation.

There is no automatic infinite retry after silence or `NO_MATCH`.

## Relationship to barge-in

Manual interrupt-to-talk remains available:

```text
Desktop Processing/Speaking
  -> Android Nói ngắt Assistant / Quick Settings
  -> authenticated cancel control session
  -> desktop acknowledges cancellation
  -> fresh SpeechRecognizer turn
```

Conversation mode complements this; it does not replace it.

## Why this is not full-duplex AEC

Automatic acoustic barge-in is deliberately not claimed here.

The preferred microphone is physically on the Android phone while response audio is produced by the Windows speaker. If the phone microphone listens during desktop TTS, it can hear and transcribe the Assistant itself. The current system has no synchronized far-end audio reference on the phone from which a real echo canceller could remove that playback.

Therefore the source does **not** turn on `SpeechRecognizer` during desktop TTS and does not use RMS/VAD thresholds to pretend the echo problem is solved.

A future Phase 31B implementation would need a real reference-aware audio architecture (for example, routing response audio/reference to the capture endpoint or moving both capture/playback into a single AEC-capable endpoint) before automatic acoustic full duplex can be considered reliable.

## Local validation

Validate on the target phone and Windows machine:

1. enable **Hội thoại liên tục**;
2. start one voice turn;
3. verify Android does not listen while desktop TTS is speaking;
4. verify a new recognition window opens shortly after TTS completes;
5. verify a follow-up utterance uses the existing desktop conversation/session;
6. verify `NO_MATCH`/speech timeout stops the conversation instead of looping forever;
7. verify **Kết thúc hội thoại** prevents any pending follow-up from reopening the microphone;
8. verify disconnect during a session stops conversational auto-listening;
9. verify on-device -> system fallback status does not incorrectly terminate the session;
10. verify manual interrupt-to-talk still works while Processing/Speaking.

No remote build/test/device run is required by repository policy; this phase requires local Android/Windows validation before release readiness is claimed.
