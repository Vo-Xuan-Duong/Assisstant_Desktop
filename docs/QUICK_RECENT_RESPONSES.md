# Quick Recent Responses

`QuickSurface` keeps a small ephemeral list of recently completed Assistant responses so auto-dismiss does not make useful output difficult to recover.

## Behavior

- keeps at most 5 `response_completed` texts;
- stores only response text plus an in-memory timestamp;
- does not store prompts, desktop context, tool arguments, or permission payloads;
- does not write history to `localStorage`, files, logs, or settings;
- history disappears when the Quick WebView/background runtime exits;
- `Xóa` purges the in-memory list immediately;
- every history item can be copied with the shared Quick clipboard helper;
- every history item can be read aloud through the existing native `assistant_speak` / Windows SAPI path;
- duplicate completion events with identical text inside 500 ms are ignored.

## Read-aloud behavior

Recent Responses does not implement a second TTS engine. It calls the same `speakResponse()` frontend API used by the current-response `Đọc` action.

A history item can start playback only while Assistant state is `idle`. Once one item begins playback:

```text
Idle
  -> assistant_speak
  -> Speaking
  -> Windows SAPI
  -> Idle
```

all history `Đọc` controls remain disabled until the call completes. The active item shows `Đang đọc`; a failed invocation shows a short item-local `Lỗi` state without deleting the saved response.

The native TTS command remains responsible for wake suspend/resume and Assistant state transitions.

## Window geometry

The history component does not call a Tauri window mutation API directly.

When the drawer opens it emits the existing:

```text
quick:resize_request
```

with a requested height of 380 px. Native `quick_panel.rs` remains authoritative for clamping and work-area-aware positioning. The component remembers the previous `window.innerHeight` and requests that height again when the drawer closes.

This preserves the existing capability boundary: frontend can request geometry, but cannot mutate the native window itself.

## Auto-dismiss interaction

Opening the drawer acquires the named auto-dismiss lease:

```text
source = recent-responses
held   = true
```

through the DOM-local contract in `quickLifecycle.ts`. While that lease exists, `useQuickAutoDismiss` cannot hide Quick even if the pointer leaves the surface and the normal 9/15-second response timeout would otherwise expire.

The lease is released on every drawer exit path:

- toggle History closed;
- `Xóa`;
- a new `quick:shown` invocation;
- component unmount/runtime teardown.

After release, the normal guarded auto-dismiss policy is reevaluated. Hold leases are source-scoped, so future Quick features can independently hold the window without one feature accidentally releasing another.

## Verification

Windows local verification should cover:

1. Complete 6+ turns and confirm only the newest 5 responses remain.
2. Hide/reopen Quick and confirm recent responses remain while the runtime stays alive.
3. Restart the runtime and confirm history is empty.
4. Open History from a 206 px Quick window and confirm native resize reaches a usable height without leaving the work area.
5. Close History and confirm the previous Quick height is restored.
6. Copy each history item and confirm the clipboard receives the complete text, not the clamped preview.
7. Click `Đọc` on a history item while Idle and confirm Windows SAPI reads the full stored response.
8. While one history item is speaking, confirm the other `Đọc` buttons remain disabled and the active item shows `Đang đọc`.
9. Force a TTS failure and confirm only the selected history item briefly reports `Lỗi`; the response remains available.
10. Open History, move the pointer away for longer than 15 seconds, and confirm Quick remains visible because the `recent-responses` hold lease is active.
11. Close History and confirm normal auto-dismiss becomes eligible again.
12. Confirm `Xóa` removes all in-memory history and releases the hold immediately.
13. Hide/reopen Quick while History was previously open and confirm no stale hold survives the new invocation.

No build/test/action was run as part of this source-only phase.
