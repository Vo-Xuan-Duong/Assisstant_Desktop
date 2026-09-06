# Quick Assistant + Edge UI

Assisstant Desktop is a background Windows assistant. Normal AI interaction is intentionally limited to a compact **Quick Assistant** near the bottom of the active monitor plus a click-through **Edge Glow** around that monitor.

The former full chat/settings React surface has been removed from the frontend source tree. Management belongs to `assistant.exe`; the remaining `main` WebView is a hidden permission-only host for Sensitive confirmations.

## Invocation behavior

```text
External application is active
        |
        | Alt + Space / wake word
        v
capture external WindowHandle
        |
        +----------------------+
        |                      |
        v                      v
  Edge Glow                Quick Assistant
(click-through)          (compact + interactive)
        |                      |
        |                      +-- text input
        |                      +-- microphone
        |                      +-- short response preview
        |                      +-- dismiss
        |
        +---------- assistant:event / voice:level
```

- `Alt + Space` toggles the Quick Assistant.
- Wake detection shows the Quick Assistant and the Quick WebView starts the existing `assistant_voice_turn` after the established 180 ms wake-to-command delay.
- `Esc`, focus loss, or `Alt + Space` again dismisses the Quick Assistant.
- There is no expand-to-full-management button; management is terminal-first.
- Sensitive permission requests hide the Quick surface and show the dedicated permission host.
- The main permission host starts hidden.

## Why the source window is captured first

The Quick Assistant is focusable because its text field needs keyboard input. Before showing it, Rust stores the foreground application's `WindowHandle`.

That captured handle is used for:

- monitor selection;
- contextual screen/window collection;
- UI Automation targeting;
- deterministic `window_get_active` Safe requests.

The Assistant therefore does not accidentally treat its own overlay as the application the user was asking about.

## Quick Assistant window

`quick_panel.rs` creates a separate Tauri `WebviewWindow`:

- max width `760` physical px;
- min target width `420` physical px when space allows;
- height `206` physical px;
- bottom-centered on the source monitor;
- undecorated and transparent;
- fixed size and always-on-top;
- skipped from taskbar;
- focusable;
- hidden until invocation.

Only the compact rectangle receives keyboard/pointer input. The rest of the desktop remains usable.

## Quick Assistant frontend

`QuickOverlay.tsx` + `quick.css` contains only immediate interaction controls:

- assistant state/activity;
- short response preview;
- text composer;
- microphone action;
- send action;
- dismiss action.

It reuses the existing runtime:

```text
QuickOverlay
   |
   +-- assistant_submit ------------> Assistant Core
   +-- assistant_voice_turn --------> Zipformer STT -> Assistant Core -> TTS
   +-- assistant:event <------------- Core state / response events
   +-- voice:level <----------------- microphone RMS
```

If STT is unavailable, the overlay points the user to the terminal resource command rather than a graphical settings panel:

```powershell
assistant resources install stt_zipformer_vi
```

## Wake-word ownership

The Quick WebView is created during background runtime setup even while hidden. It is therefore a stable lifecycle owner for wake-triggered voice turns.

On:

```text
quick:shown { reason: "wake" }
```

QuickOverlay:

1. clears the previous compact response state;
2. refreshes voice capabilities;
3. waits 180 ms so the wake-phrase tail is not captured;
4. calls the same `assistant_voice_turn` used by the Mic button.

There is no hidden full-management React listener, so wake detection cannot start a second microphone pipeline through the retired UI path.

## Sensitive permission surface

The existing Tauri window label `main` is retained so the native permission broker does not need a new lifecycle contract, but its content is now `PermissionSurface`, not the old management application.

Default properties:

- hidden at startup;
- `560 x 520`;
- fixed size;
- centered;
- shown when the native broker calls the existing `show_main_window` path.

PermissionSurface:

- listens only to `permission:request`;
- displays tool name, request id and arguments;
- keeps the existing ~29 second UI countdown under the native 30 second broker timeout;
- supports Allow Once / Deny;
- treats `Esc` as Deny;
- supports queued permission requests;
- hides itself after the final received request is resolved;
- never exposes settings/chat panels.

The native security boundary is unchanged: Sensitive tools still require the permission broker and cannot be approved through the management IPC.

## Edge Glow architecture

The perimeter still uses four transparent click-through windows:

```text
Edge manager
  +-- edge-top
  +-- edge-right
  +-- edge-bottom
  +-- edge-left
          |
          +-- assistant:event
          +-- voice:level
```

Each edge window is transparent, non-focusable, always-on-top, skipped from the taskbar, and configured with `set_ignore_cursor_events(true)`.

Visual modes remain:

- activated: short bright bloom;
- ready: subtle persistent halo;
- listening: RMS-reactive glow;
- processing/executing: faster spectrum movement;
- speaking: breathing response state;
- confirming: distinct permission state;
- error: red/coral state.

Both Quick and edge visuals respect `prefers-reduced-motion`.

## Surface routing

All surfaces share one Vite build but mount different roots:

```text
index.html
   |
   +-- normal URL -----------------> PermissionSurface
   |
   +-- ?surface=quick -------------> QuickOverlay
   |
   +-- ?surface=edge&edge=... -----> EdgeOverlay
```

The frontend source tree now contains only the three mounted surfaces plus their shared API/types files.

## Tauri capability boundary

The shared core capability still targets only application-owned UI windows:

```json
"windows": ["main", "quick", "edge-*"]
```

This frontend change does not bypass MCP or permission policy. Management IPC does not expose Windows tools directly; Sensitive actions still traverse the normal Assistant -> MCP -> permission-gateway path.

## Local verification checklist

On Windows, verify:

1. Start Assisstant Desktop normally; no full management window should appear.
2. Focus another app and press `Alt + Space`; Quick appears on the same monitor.
3. Edge glow is visible and does not block clicks outside Quick.
4. Send a text request and receive the short response without another window.
5. Mic voice turn uses local Vietnamese STT and updates the glow/response.
6. Trigger wake; Quick appears and exactly one voice turn starts after the wake delay.
7. Ask which window is active; it should refer to the source app, not Quick.
8. Trigger a Sensitive tool; Quick hides and the compact permission window appears.
9. Deny with `Esc`; verify the tool is denied.
10. Trigger again and Allow Once; verify the request proceeds and the permission window hides afterward.
11. Queue more than one confirmation and verify the permission surface advances through the queue.
12. Confirm no old chat/settings management UI exists in the frontend source tree or mounted routes.
13. Use `assistant` / `assistant status` / `assistant logs -f` for management and diagnostics.

## Remaining UI work

The remaining lifecycle issue is explicit: tray click and a normal second-instance launch still route through the historical `show_main_window` path. Because `main` is now permission-only, that path must be changed to surface Quick (or launch the terminal manager) before the terminal-first UX is considered complete.

Other useful follow-ups:

- make Quick height dynamically expand for longer responses;
- add an explicit Stop/Cancel action for microphone/long turns;
- use exact Windows work-area geometry rather than a fixed bottom margin;
- optionally add contextual chips and configurable auto-dismiss.
