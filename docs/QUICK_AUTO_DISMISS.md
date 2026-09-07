# Quick Auto-Dismiss

Quick is a transient interaction surface, not a persistent chat window. After a completed response it may hide itself, but only when doing so cannot interrupt active work or user interaction.

## Policy

Auto-dismiss is owned by `quickAutoDismiss.ts` and mounted only through `QuickSurface`.

The timer starts only when all of the following are true:

- the Assistant state is `idle`;
- a `response_completed` event has been observed for the current Quick invocation;
- no error has replaced that response;
- no feature currently owns an auto-dismiss hold lease;
- the pointer is not inside the Quick surface;
- the composer textarea is empty.

Timeouts:

```text
response <= 320 chars  -> 9 seconds
response >  320 chars  -> 15 seconds
```

A state transition away from `idle` immediately cancels the timer. This means voice output does not disappear while SAPI is speaking: the temporary `idle -> speaking` transition cancels the first timer, and a fresh timer is scheduled only after speaking returns to `idle`.

## Interaction guards

The timer pauses while the pointer is inside Quick so the user can read/select/copy the answer. It also stays disabled while the composer contains text. When the pointer leaves or the composer becomes empty, eligibility is recalculated from live DOM state and the timer starts again if the response is still current.

Longer-lived Quick features use the DOM-local hold lease contract in `quickLifecycle.ts`:

```text
quick:auto-dismiss-hold
  { source, held }
```

`quickAutoDismiss` tracks active sources in a set rather than a single boolean. This prevents one feature from accidentally releasing another feature's hold. The recent-response drawer owns the `recent-responses` lease for the entire time the drawer is open.

`quick:shown` clears all stale leases and resets response eligibility. Reopening Quick therefore never inherits an old hold or response timer from the previous invocation.

Errors never auto-dismiss. Cancellation without a completed response never auto-dismisses. Sensitive confirmation is a separate surface and is not affected by this controller.

## Architecture

```text
main.tsx
   |
   +-- QuickSurface
          |
          +-- QuickOverlay          presentation / input / voice / Stop
          +-- Quick feature UIs     may acquire named hold leases
          +-- useQuickAutoDismiss   transient-window lifecycle policy
```

The controller consumes only existing Assistant/Tauri events, DOM-local hold events, and the existing `assistant_quick_hide` command through `hideQuickAssistant()`. It adds no Tauri capability, native command, MCP permission, or management IPC method.

## Windows verification

1. Send a short text request and do not touch Quick; after the response returns to Idle, verify Quick hides after about 9 seconds.
2. Generate a response longer than 320 characters; verify the delay is about 15 seconds.
3. Keep the mouse over Quick after the response; verify it remains visible.
4. Move the pointer out; verify a fresh full timeout starts before hiding.
5. Type text into the composer after a response; verify Quick remains visible.
6. Clear the composer; verify auto-dismiss becomes eligible again.
7. Run a voice turn; verify Quick does not hide while the Assistant is Listening, Processing, or Speaking, and starts its timer only after final Idle.
8. Trigger an error; verify Quick stays visible.
9. Trigger Stop before a response completes; verify no auto-dismiss timer is created from cancellation alone.
10. Open Recent Responses, move the pointer outside Quick for longer than 15 seconds, and verify the drawer remains visible because its hold lease is active.
11. Close Recent Responses and verify a fresh normal timeout can start again.
12. Hide Quick manually and reopen it; verify previous response timers and hold leases cannot affect the new invocation.
