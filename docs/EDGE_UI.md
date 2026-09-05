# Quick Assistant + Edge UI

Assisstant Desktop uses two lightweight system-level surfaces for normal AI invocation: a compact interactive **Quick Assistant** near the bottom of the active monitor and a click-through **Edge Glow** around that monitor.

The interaction pattern is inspired by modern mobile system assistants such as Gemini on Android: invoking AI should not force the user to leave the application they are currently using. The project uses its own React/CSS implementation and does not copy proprietary Gemini assets.

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
        |                      +-- expand / dismiss
        |
        +---------- assistant:event / voice:level
```

Normal invocation no longer opens the full 620x760 management window.

- `Alt + Space` toggles the Quick Assistant.
- Wake-word detection shows the Quick Assistant while the existing hidden main surface continues to run the established voice-turn pipeline.
- `Esc`, clicking outside the Quick Assistant, or pressing `Alt + Space` again dismisses the compact surface.
- The tray icon/menu and explicitly launching the app still open the full application.
- The expand button in the Quick Assistant promotes the interaction to the full application.
- Sensitive permission requests promote to the full application so the complete confirmation UI remains available.

## Why the source window is captured first

The Quick Assistant is intentionally focusable because its text field needs keyboard input. Therefore it becomes the Windows foreground window after activation.

Before showing it, the Rust desktop runtime stores the foreground application's `WindowHandle`. That handle is then used for:

- choosing the correct monitor;
- contextual screen/window collection;
- UI Automation targeting;
- the deterministic `window_get_active` local Safe command.

This prevents the Assistant from accidentally treating its own Quick window as the application the user was asking about.

## Quick Assistant window

The compact surface is created dynamically by `quick_panel.rs` as a separate Tauri `WebviewWindow`.

Current geometry:

- maximum width: `760` physical pixels;
- minimum target width: `420` physical pixels when the monitor has enough space;
- height: `206` physical pixels;
- horizontally centered on the source monitor;
- positioned close to the bottom edge with space for the Windows taskbar area.

Window properties:

- undecorated;
- transparent;
- fixed size;
- always on top;
- omitted from the taskbar;
- focusable;
- hidden until invocation.

Unlike a fullscreen transparent overlay, only this compact rectangle receives pointer/keyboard input. The rest of the desktop remains directly usable.

## Quick Assistant frontend

The surface is rendered by `QuickOverlay.tsx` and `quick.css`.

It contains only the controls needed for an immediate interaction:

- assistant state / activity label;
- a short two-line response preview;
- one compact text composer;
- microphone action;
- send action;
- expand-to-full-app action;
- dismiss action.

The Quick Assistant reuses the existing backend commands and events rather than introducing a separate AI runtime:

```text
QuickOverlay
   |
   +-- assistant_submit ------------> Assistant Core
   +-- assistant_voice_turn --------> Whisper -> Assistant Core -> TTS
   +-- assistant:event <------------- Core state / response events
   +-- voice:level <----------------- microphone RMS
```

## Wake-word behavior

Wake detection does not create a second STT/AI pipeline.

The hidden `MainSurface` remains alive while the application is in the tray. Its existing wake listener starts the same `assistant_voice_turn` used by the normal Mic button. The Quick Assistant is shown as the lightweight presentation surface and reflects the shared Assistant Core state/events.

This keeps one authoritative voice pipeline and avoids duplicate microphone capture.

## Edge Glow architecture

The perimeter glow continues to use four separate transparent windows:

```text
Edge Overlay Manager (Rust/Tauri)
        |
        +---- edge-top
        +---- edge-right
        +---- edge-bottom
        +---- edge-left
                  |
                  v
          React EdgeOverlay
                  |
        +---------+----------+
        |                    |
 assistant:event         voice:level
        |                    |
        v                    v
 state animation       listening intensity
```

Each edge window is:

- undecorated;
- transparent;
- non-resizable;
- non-focusable;
- always on top;
- omitted from the taskbar;
- cursor-event/click-through.

`set_ignore_cursor_events(true)` ensures the glow never blocks the application underneath it.

## Visual behavior

The current UI deliberately uses a smoother blue/cyan/violet/pink spectrum and less dot texture than the earlier implementation.

### Activated

A bright short bloom appears immediately when AI is invoked.

### Ready

After the activation surge, the border settles into a subtle persistent halo while the Quick Assistant remains open. This avoids the old behavior where the visual effect disappeared after roughly one second even though the Assistant surface was still active.

### Listening

The perimeter and microphone glow react to RMS microphone amplitude from `voice:level`.

### Processing / Executing

The spectrum movement becomes faster to indicate reasoning or tool execution.

### Speaking

The edge uses the existing breathing state while Windows SAPI speaks the response.

### Confirming

Permission confirmation remains visually distinct with an amber state.

### Error

Errors retain a red/coral state rather than using the normal AI spectrum.

## Multi-monitor behavior

Before either the edge effect or Quick Assistant takes focus, the runtime captures the external foreground `WindowHandle`.

1. `MonitorFromWindow(..., MONITOR_DEFAULTTONEAREST)` resolves the source display.
2. `GetMonitorInfoW` returns that monitor rectangle.
3. The four edge windows are positioned around the same rectangle.
4. The Quick Assistant is centered near the bottom of that rectangle.
5. If the source handle is no longer valid, the primary monitor is used as fallback.

The AI surface therefore follows the monitor where the user was actually working.

## Frontend surface isolation

All surfaces share one Vite bundle but render separate React roots:

```text
index.html
   |
   +-- normal URL -----------------> MainSurface -> App
   |
   +-- ?surface=quick -------------> QuickOverlay
   |
   +-- ?surface=edge&edge=top -----> EdgeOverlay
```

Styles remain isolated by surface:

- `MainSurface` -> `styles.css`;
- `QuickOverlay` -> `quick.css`;
- `EdgeOverlay` -> `edge.css` + `edge-gemini.css`.

The quick and edge roots explicitly keep `html`, `body`, and `#root` transparent.

## Tauri capability boundary

The shared core capability explicitly includes only the application's UI windows:

```json
"windows": ["main", "quick", "edge-*"]
```

The Quick Assistant can call the same normal Tauri commands as the main frontend, but this does not bypass the MCP permission architecture. Sensitive Windows tools still require confirmation through the existing permission broker and desktop confirmation service.

The edge surfaces remain presentation-only and click-through.

## Reduced motion

Both the Quick Assistant and edge refinement respect `prefers-reduced-motion`. When enabled, decorative continuous animations are disabled while state colors and basic visibility remain available.

## Local verification checklist

Verify this UI on the Windows development machine:

1. Start Assisstant Desktop and leave the full window hidden in the tray.
2. Focus another application and press `Alt + Space`.
3. Confirm the full application does **not** open.
4. Confirm the Quick Assistant appears near the bottom-center of the same monitor and receives keyboard focus.
5. Confirm the perimeter glow appears around that monitor without blocking clicks outside the compact Quick window.
6. Enter a text request and confirm the response preview updates without opening the full application.
7. Press `Alt + Space` again and confirm both Quick Assistant and perimeter glow hide.
8. Reopen it and press `Esc`; confirm it hides.
9. Reopen it and click another application; confirm focus-loss dismisses the Quick Assistant.
10. Use the microphone and confirm both the mic halo and edge glow react while listening.
11. Trigger the wake word; confirm the Quick Assistant appears instead of the full application and the existing voice-turn pipeline runs once.
12. On a second monitor, invoke AI from an application on that display and confirm both the Quick Assistant and edge glow follow it.
13. Ask which window is active and confirm the response refers to the external application captured before overlay focus, not the Quick Assistant itself.
14. Trigger a Sensitive tool and confirm the full permission UI is surfaced for confirmation.
15. Use the expand button and confirm the full management application opens.
16. Open the Assistant from the tray and confirm the tray still opens the full application.
17. Confirm `main`, `quick`, and all `edge-*` windows do not create unexpected taskbar entries beyond the intended main application behavior.

## Follow-up UI work

The compact invocation layer is now implemented. Useful next iterations are independent of this first overlay version:

- share/persist visible chat history between Quick Assistant and the full application;
- dynamically expand the response panel for longer answers;
- add contextual chips such as “Ask about this screen”;
- add an explicit cancel/stop control for microphone or long-running AI turns;
- make bottom positioning aware of the exact Windows work area/taskbar geometry rather than relying on a fixed bottom margin;
- configurable automatic dismissal after a completed response;
- optional user-selectable visual themes.
