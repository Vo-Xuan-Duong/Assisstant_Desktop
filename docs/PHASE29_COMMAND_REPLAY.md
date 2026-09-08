# Phase 29 — Satellite Command Replay Protection

The Android satellite now assigns a UUID to every command and the desktop keeps a bounded in-memory set of the 128 most recently accepted command ids.

A command id is stored only after the desktop is ready to accept the turn. Re-sending an accepted id returns `duplicate_command` and never executes the Windows action twice.

This closes the common reconnect/double-tap retry case. It is intentionally process-local: a full desktop restart clears the cache. Durable exactly-once execution would require persistent idempotency records and is not necessary for the current personal desktop-assistant threat model.
