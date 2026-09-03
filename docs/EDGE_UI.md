# Edge Assistant UI

Phase 6 implements the system-level visual surface used when Assisstant Desktop is activated.

## Goal

The UI should feel integrated with Windows rather than like a normal chat window. When the Assistant is invoked, a lightweight glow appears around the monitor the user was working on and changes behavior with the Assistant state.

The implementation is inspired by the interaction pattern of mobile system assistants, but it uses its own visual identity and does not copy proprietary Gemini assets.

## Architecture

```text
Foreground application
        |
        | Alt + Space / tray
        v
remember WindowHandle
        |
        v
monitor_bounds(HWND)
        |
        v
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

## Why four windows

The project deliberately does not use one transparent fullscreen window.

Each edge is a separate 24 physical-pixel Tauri WebviewWindow. This gives the overlay a very small desktop footprint and avoids placing a large transparent input surface above the user's applications.

All four windows are configured as:

- undecorated;
- transparent;
- non-resizable;
- non-focusable;
- always on top;
- omitted from the taskbar;
- hidden until activation;
- cursor-event/click-through.

`set_ignore_cursor_events(true)` is applied after creation, so the underlying application continues receiving mouse input.

## Multi-monitor behavior

Before the Assistant window takes focus, the desktop runtime stores only the foreground `WindowHandle`.

When the edge effect is activated:

1. `MonitorFromWindow(..., MONITOR_DEFAULTTONEAREST)` resolves the source display.
2. `GetMonitorInfoW` returns the physical monitor rectangle.
3. The four edge windows are positioned in physical pixels around that rectangle.
4. If the source handle is no longer valid, the primary monitor is used as a fallback.

This means the glow follows the application the user was working in rather than blindly following the Assistant chat window.

## Frontend isolation

The normal chat window and edge windows use the same Vite bundle but render different roots.

```text
index.html
   |
   +-- normal URL -----------------> MainSurface -> App
   |
   +-- ?surface=edge&edge=top -----> EdgeOverlay
```

`MainSurface` owns `styles.css`.

`EdgeOverlay` owns `edge.css`.

This is important because the chat UI has an opaque desktop background while the edge windows must keep `html`, `body`, and `#root` fully transparent.

## Visual states

The edge renderer responds to the existing Assistant Core state events.

### Activated

Short bloom/flow animation when the user invokes the Assistant.

### Listening

The glow reacts to the RMS microphone level emitted by `voice:level`.

```text
CPAL microphone
     |
     v
AudioLevel.rms
     |
     v
voice:level event
     |
     v
EdgeOverlay intensity / thickness
```

### Processing

Faster continuous gradient movement to indicate model reasoning.

### Executing

Higher-intensity and faster movement for tool execution.

### Speaking

Breathing animation while Windows SAPI is speaking the final response.

### Confirming

Amber state reserved for permission confirmation flows.

### Error

Red pulse state.

### Idle

Opacity transitions to zero. The windows remain click-through while the Assistant UI is open so the next state can render immediately without recreating WebView2 instances.

When the Assistant is hidden or closed to tray, the four edge windows are hidden entirely.

## Reduced motion

The renderer respects `prefers-reduced-motion` and reduces animations to a single near-instant frame.

## Security boundary

The edge webviews receive only the normal Tauri core capability and global runtime events.

They do not receive Windows tool commands, Antigravity credentials, arbitrary shell access, or direct access to the desktop context collector.

The capability uses the label glob:

```json
"windows": ["main", "edge-*"]
```

## Local verification checklist

No GitHub Action is required for this phase. Verify on the Windows development machine:

1. Launch the desktop app.
2. Put another application on the primary monitor and press `Alt + Space`.
3. Confirm all four edges appear around that monitor.
4. Confirm clicking through the glow still interacts with the underlying application.
5. With a second monitor, invoke the Assistant from an application on that display and confirm the edge moves there.
6. Start a voice turn and confirm the Listening glow responds to microphone amplitude.
7. Confirm Processing and Speaking have visibly different animations.
8. Hide the Assistant from the tray and confirm edge windows disappear.
9. Confirm the edge windows never appear in the Windows taskbar.
10. Confirm the edge windows never steal keyboard focus.

## Deferred UI work

The phase intentionally does not implement:

- wake word activation;
- a floating bottom assistant bubble;
- full-screen computer-use annotations;
- screen-selection/highlight surfaces;
- custom user themes.

Those remain independent layers on top of this edge-window foundation.
