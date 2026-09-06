# Quick Response Actions

Quick remains a compact interaction surface, but completed answers need one lightweight action that does not require opening another application: copying the final response.

## Current action

After `assistant:event -> response_completed`, Quick shows a compact `Copy` pill near the window actions.

```text
response_completed
      |
      v
QuickResponseActions
      |
      +-- navigator.clipboard.writeText
      |
      `-- DOM copy fallback
```

The action is intentionally owned by `QuickSurface`, not `QuickOverlay`, so the main input/voice/cancellation component does not accumulate unrelated lifecycle code.

## Lifecycle

The Copy action:

- appears only after a completed response;
- resets on every `quick:shown` event;
- disappears when an assistant error replaces the response;
- never renders for partial `text_delta` output;
- reports `Đã sao chép` or `Lỗi` for about 1.4 seconds;
- does not introduce a new Tauri capability or native clipboard command.

The primary clipboard path is the browser/WebView Clipboard API. If the WebView exposes the API but denies the write, the action falls back to a temporary hidden textarea and `document.execCommand("copy")`.

## Auto-dismiss interaction

`useQuickAutoDismiss` observes pointer activity across the whole Quick WebView. Because `QuickResponseActions` is mounted inside that same surface, hovering or clicking Copy pauses the existing auto-dismiss timer. The user therefore does not lose the response while trying to copy it.

## Windows verification

Verify locally:

1. submit a text request and wait for a completed answer;
2. confirm Copy is not shown during streaming/processing;
3. click Copy and paste into Notepad;
4. confirm the label briefly changes to `Đã sao chép`;
5. keep the pointer over Copy beyond the normal 9/15-second auto-dismiss period and confirm Quick stays visible;
6. move the pointer away and confirm normal auto-dismiss resumes;
7. trigger a new Quick invocation and confirm the previous response action is cleared;
8. trigger an error and confirm no stale Copy action remains.

No automated build/runtime verification is claimed for this phase.
