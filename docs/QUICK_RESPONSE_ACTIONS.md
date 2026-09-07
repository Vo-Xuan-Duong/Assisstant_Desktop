# Quick Response Actions

Quick remains a compact interaction surface, but completed answers need a few lightweight actions that do not require opening another application.

## Current actions

After `assistant:event -> response_completed`, Quick shows compact response actions near the window controls:

```text
response_completed
      |
      v
QuickResponseActions
      |
      +-- Copy
      |     +-- navigator.clipboard.writeText
      |     `-- DOM copy fallback
      |
      `-- Đọc
            `-- assistant_speak -> Windows SAPI
```

The actions are intentionally owned by `QuickSurface`, not `QuickOverlay`, so the main input/voice/cancellation component does not accumulate unrelated response lifecycle code.

## Copy lifecycle

The Copy action:

- appears only after a completed response;
- resets on every `quick:shown` event;
- disappears when an assistant error replaces the response;
- never renders for partial `text_delta` output;
- reports `Đã sao chép` or `Lỗi` for about 1.4 seconds;
- does not introduce a new Tauri capability or native clipboard command.

The primary clipboard path is the browser/WebView Clipboard API. If the WebView exposes the API but denies the write, the action falls back to a temporary hidden textarea and `document.execCommand("copy")`.

## Read-aloud lifecycle

The `Đọc` action reuses the existing native `assistant_speak` command and Windows SAPI implementation.

Playback is enabled only while Assistant state is `idle`. When invoked, native runtime owns the transition:

```text
Idle -> Speaking -> Idle
```

and suspends wake detection around SAPI playback using the existing wake lifecycle. Duplicate playback is disabled while a speak call is pending or Assistant state is `speaking`.

A TTS failure is local to the action: it briefly shows `Lỗi` without discarding the completed response.

Recent Responses uses this same `speakResponse()` API for per-history-item read-aloud. There is no second TTS implementation.

## Auto-dismiss interaction

`useQuickAutoDismiss` observes pointer activity across the whole Quick WebView. Because `QuickResponseActions` is mounted inside that same surface, hovering or clicking Copy/Đọc pauses the normal auto-dismiss timer while the pointer remains inside Quick.

A state transition to `speaking` also cancels the timer independently. Auto-dismiss becomes eligible again only after native TTS returns the Assistant to `idle` and the other normal guards allow dismissal.

## Windows verification

Verify locally:

1. submit a text request and wait for a completed answer;
2. confirm Copy/Đọc are not shown during streaming/processing;
3. click Copy and paste into Notepad;
4. confirm the label briefly changes to `Đã sao chép`;
5. click `Đọc` while Idle and confirm Windows SAPI reads the complete response;
6. confirm duplicate `Đọc` invocation is disabled while Speaking;
7. confirm wake detection does not immediately retrigger from Assistant speech;
8. force a TTS failure and confirm the response remains visible while the action briefly reports `Lỗi`;
9. keep the pointer over the response actions beyond the normal 9/15-second auto-dismiss period and confirm Quick stays visible;
10. move the pointer away after Idle and confirm normal auto-dismiss resumes;
11. trigger a new Quick invocation and confirm the previous response actions are cleared;
12. trigger an error and confirm no stale response action remains.

No automated build/runtime verification is claimed for this phase.
