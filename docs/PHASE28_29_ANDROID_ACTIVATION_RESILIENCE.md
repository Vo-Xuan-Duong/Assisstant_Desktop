# Phase 28/29 — Android activation and resilience

## Scope

This phase improves the Android Voice Satellite without moving reasoning or tool authority off the desktop.

Implemented source behavior:

- Quick Settings **Assistant Voice** tile;
- one-tap activation after pairing and microphone permission have already been established;
- manual interrupt-to-talk while the desktop is Processing/Speaking;
- transient WebSocket reconnect with bounded exponential backoff;
- connection-generation protection against stale callbacks;
- terminal handling for authentication failure/device revoke;
- one bounded SpeechRecognizer busy retry;
- on-device recognizer fallback to the system recognizer when the local engine becomes unavailable;
- no automatic command replay after transport failure.

## Quick Settings flow

```text
Quick Settings tile
  -> MainActivity(EXTRA_START_VOICE)
  -> paired? + microphone permission already granted?
  -> connect/reconnect desktop if needed
  -> if Processing/Speaking: send authenticated cancel
  -> wait for cancelled acknowledgement
  -> SpeechRecognizer.startListening()
```

The tile does not own microphone capture itself and does not bypass Android runtime permissions.

Android 14+ uses `TileService.startActivityAndCollapse(PendingIntent)`. Android 8–13 uses the older Intent overload.

## Reconnect policy

```text
1s -> 2s -> 4s -> 8s -> 15s maximum
```

A connection generation is incremented on an explicit connect/close. WebSocket callbacks check that generation so an old socket cannot restore stale state after the user has paired/reconnected elsewhere.

`authentication_failed` and `device_revoked` are terminal and disable automatic reconnect until the user explicitly connects again after fixing trust/pairing.

## Command-delivery semantics

The phone never automatically resends a command after a transport failure.

Reason: a command can reach the desktop and execute a side effect before the network drops. Blind retry could execute that action twice. Desktop command-id replay protection remains the duplicate boundary for a command that is deliberately submitted again with the same id.

## SpeechRecognizer recovery

When on-device recognition is requested:

1. use it only when Android reports it available;
2. on busy/server-disconnected/server-engine failure, fall back once to the system recognizer;
3. permit one recognizer recreation/retry for a busy engine;
4. never create an unbounded recognition loop.

No final transcript is retried or resent automatically.

## Barge-in boundary

This phase provides explicit interrupt-to-talk only. It is not automatic acoustic barge-in.

Automatic full-duplex behavior while Windows speakers are playing requires AEC/reference-audio handling so assistant TTS is not interpreted as user speech. That remains a later, separately validated phase.

## Local validation gate

Validate on the target Android/Windows devices:

1. add the Assistant Voice Quick Settings tile;
2. grant microphone permission once in the app;
3. tile tap opens the app and begins recognition after the desktop is ready;
4. Android 14+ tile launch does not throw `UnsupportedOperationException`;
5. disconnect Wi-Fi briefly and confirm reconnect backoff/status;
6. revoke the phone and confirm reconnect stops;
7. re-allow/pair and confirm explicit reconnect succeeds;
8. interrupt desktop Processing/Speaking and confirm a new recognition turn starts only after cancellation acknowledgement;
9. force an on-device recognizer failure/busy condition where possible and confirm bounded system fallback;
10. confirm no command is automatically duplicated after a network drop.

No remote build, tests, GitHub Actions, installer, or device run is performed for this phase.
