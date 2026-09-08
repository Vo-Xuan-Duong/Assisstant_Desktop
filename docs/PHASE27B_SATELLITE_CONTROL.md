# Phase 27B — Explicit TTS Language + Satellite Control

## Scope

This phase closes three gaps in the Android Voice Satellite path without changing the desktop permission boundary:

1. `VI` / `EN` / `Auto` is forwarded explicitly into desktop TTS;
2. accepted command ids are remembered so a reconnect/retry cannot execute the same command twice;
3. Android can request cancellation while the desktop is Processing or Speaking.

## Explicit language path

```text
Android response_language
  vi   -> TtsLanguage::Vietnamese -> SAPI vi-VN hint
  en   -> TtsLanguage::English    -> SAPI en-US hint
  auto -> TtsLanguage::Auto       -> response-text detection
```

The model response policy and the spoken-language hint now use the same `response_language` value. `Auto` deliberately keeps text-based detection because the final response language is determined by the model.

## Command replay protection

Android already sends a UUID command `id`. The desktop now validates and remembers the most recent accepted ids.

Rules:

- malformed ids are rejected;
- ids are recorded only after the desktop is available to accept a turn;
- the bounded cache keeps 128 recent ids;
- a repeated accepted id returns `duplicate_command` and is never executed again;
- an external desktop-busy rejection does not consume the id, so a caller may retry later.

This is an in-memory process-level safety cache, not durable exactly-once storage across a desktop restart.

## Phone Stop / barge-in primitive

The primary command WebSocket is synchronously waiting for the current turn. To keep the existing RFC6455 implementation simple, Android opens a short-lived second authenticated WebSocket when the user taps **Dừng Assistant**.

```text
Primary session                Control session
--------------                 ---------------
command -> Processing          hello/token/device_id
(waiting)                      ready
                               cancel { id? }
                               cancelled { accepted }
```

The listener can now serve multiple authenticated sessions concurrently, but `COMMAND_GATE` still permits only one satellite command turn at a time.

All connection tasks are owned by a Tokio `JoinSet`. When the listener supervisor aborts because pairing is disabled, the bind changes, or the shared token rotates, dropping the `JoinSet` aborts the active primary and control sessions as well. This preserves the previous token-rotation/revoke session boundary.

## Cancellation policy

Cancellation remains lifecycle-aware:

- `Listening`: cancel local microphone/STT generation and return Core to Idle;
- `Processing`: call `AntigravityClient::cancel_active_turn()` with the existing short registration-race retry;
- `Speaking`: purge the active Windows SAPI stream;
- `Executing` / `Confirming`: reject cancellation because an external side effect may already be in flight;
- `Idle` / `Error`: nothing is cancelled.

This is a Stop primitive, not rollback. It never claims to undo already executed MCP/Windows actions.

## TTS cancellation result

If speech is cancelled after a textual response has already been produced, the desktop still returns that response text to Android, then reports the turn as cancelled. The user therefore keeps the result while only the spoken playback is interrupted.

## Validation gate

No remote build, tests, GitHub Actions, installer, model download, Android run, or Windows speech run is performed for this phase.

Local validation should cover:

1. `VI` selects the Vietnamese SAPI language path;
2. `EN` selects the English SAPI language path;
3. `Auto` follows response text;
4. duplicate command id is rejected without a second Windows action;
5. Stop during Processing cancels Antigravity and returns to Idle;
6. Stop during Speaking purges SAPI but keeps response text;
7. Stop during Executing/Confirming is rejected;
8. token rotation/disable terminates both primary and control sessions;
9. a fresh connection can submit a new UUID command normally.
