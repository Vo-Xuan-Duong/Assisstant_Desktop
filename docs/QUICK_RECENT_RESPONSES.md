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
- duplicate completion events with identical text inside 500 ms are ignored.

## Window geometry

The history component does not call a Tauri window mutation API directly.

When the drawer opens it emits the existing:

```text
quick:resize_request
```

with a requested height of 380 px. Native `quick_panel.rs` remains authoritative for clamping and work-area-aware positioning. The component remembers the previous `window.innerHeight` and requests that height again when the drawer closes.

This preserves the existing capability boundary: frontend can request geometry, but cannot mutate the native window itself.

## Auto-dismiss interaction

The history drawer lives inside the same Quick WebView. Existing `useQuickAutoDismiss` pointer tracking therefore pauses dismissal while the user is interacting with History or Copy controls. If the user leaves the Quick surface, the normal guarded auto-dismiss policy may resume.

## Verification

Windows local verification should cover:

1. Complete 6+ turns and confirm only the newest 5 responses remain.
2. Hide/reopen Quick and confirm recent responses remain while the runtime stays alive.
3. Restart the runtime and confirm history is empty.
4. Open History from a 206 px Quick window and confirm native resize reaches a usable height without leaving the work area.
5. Close History and confirm the previous Quick height is restored.
6. Copy each history item and confirm the clipboard receives the complete text, not the clamped preview.
7. Confirm hover/click in the history drawer pauses auto-dismiss.
8. Confirm `Xóa` removes all in-memory history immediately.

No build/test/action was run as part of this source-only phase.
