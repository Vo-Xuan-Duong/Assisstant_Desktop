# Quick Assistant + Edge UI

Assisstant Desktop is a background Windows assistant. Normal AI interaction is intentionally limited to a compact **Quick Assistant** near the bottom of the active monitor plus a click-through **Edge Glow** around that monitor.

The former full chat/settings React surface has been removed from the frontend source tree. Management belongs to `assistant.exe`; the remaining `main` WebView is a hidden permission-only host for Sensitive confirmations.

## Invocation behavior

```text
External application is active
        |
        | Alt + Space / wake / tray / normal second launch
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

Normal activation paths:

- `Alt + Space` toggles Quick.
- Wake detection shows Quick and starts the existing `assistant_voice_turn` after the established 180 ms wake-to-command delay.
- Tray **Mở Assistant** and tray left-click show Quick.
- A normal second application launch is redirected by the single-instance callback to Quick.
- `assistant overlay show` shows Quick through authenticated management IPC.
- `Esc`, focus loss, `Alt + Space` again, tray hide, or `assistant overlay hide` dismisses the compact surface.

There is no expand-to-full-management action. Sensitive permission requests are the only normal reason to surface the hidden `main` permission host.

## Why the source window is captured first

The Quick Assistant is focusable because its text field needs keyboard input. Before showing it, Rust stores the foreground application's `WindowHandle`.

That captured handle is used for:

- monitor/work-area selection;
- contextual screen/window collection;
- UI Automation targeting;
- deterministic `window_get_active` Safe requests.

The Assistant therefore does not accidentally treat its own overlay as the application the user was asking about.

## Quick Assistant window

`quick_panel.rs` creates a separate Tauri `WebviewWindow`:

- max width `760` physical px;
- min target width `420` physical px when space allows;
- default height `206` physical px;
- content-driven maximum height `380` physical px;
- bottom-centered inside the source monitor **work area**;
- `18` physical px bottom margin inside that work area;
- undecorated and transparent;
- user resizing disabled and always-on-top;
- skipped from taskbar;
- focusable;
- hidden until invocation.

The work area comes from Win32 `MONITORINFO.rcWork`, so Windows-reserved desktop space such as a taskbar docked to the bottom, top, left, or right is excluded before Quick is positioned.

Resolution order:

```text
source HWND monitor rcWork
        |
        +-- unavailable -> primary monitor rcWork
        |
        +-- unavailable -> Tauri primary full monitor bounds
```

The last fallback intentionally favors showing the Assistant over failing activation; it may include taskbar space if Win32 work-area lookup is unavailable.

Only the compact rectangle receives keyboard/pointer input. The rest of the desktop remains usable.

If `quick_panel::show` fails, the runtime logs the failure and hides Edge. It does not fall back to `main`, because `main` is permission-only.

## Dynamic Quick height

Quick starts at `206px` on every normal invocation. React measures the rendered card after response/error/state changes and requests only the vertical space that the current content needs.

```text
QuickOverlay DOM
     |
ResizeObserver + rendered content change
     |
requestAnimationFrame throttle
     |
quick:resize_request { height }
     |
     v
quick_panel.rs
     |
clamp 206..380 px
clamp again to current monitor work area
     |
recalculate Y so bottom edge remains anchored
     |
set_position + set_size
```

The frontend does **not** receive direct window mutation capability. `core:default` provides event emission; native Rust remains authoritative for geometry, bounds and source-monitor selection.

The request is deduplicated by the frontend so a native resize does not produce an unbounded `ResizeObserver -> set_size -> ResizeObserver` loop. Native code also treats the frontend height as untrusted input and clamps it before use.

On a new `quick:shown` event, Rust first restores the default `206px` geometry and the frontend clears the old compact response. This prevents a previous long answer from leaving the next invocation unnecessarily expanded.

## Quick Assistant frontend

`QuickOverlay.tsx` + `quick.css` contains only immediate interaction controls:

- assistant state/activity;
- bounded response preview;
- text composer;
- microphone action;
- send action;
- dismiss action.

Response behavior:

- short response -> panel normally stays near the default height;
- longer response/error -> panel expands to the measured content height;
- preview is bounded to roughly eight rendered lines;
- native window never exceeds `380px` or the current work-area height;
- a subsequent shorter state may shrink the panel again.

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

The existing Tauri window label `main` is retained so the native permission broker does not need a new lifecycle contract, but its content is `PermissionSurface`, not a management application.

Default properties:

- hidden at startup;
- `560 x 520`;
- fixed size;
- centered;
- shown by the native permission broker through `show_main_window()` when a Sensitive request needs confirmation.

PermissionSurface:

- listens only to `permission:request`;
- displays tool name, request id and arguments;
- keeps the existing ~29 second UI countdown under the native 30 second broker timeout;
- supports Allow Once / Deny;
- treats `Esc` as Deny;
- supports queued permission requests;
- hides itself after the final received request is resolved;
- never exposes settings/chat panels.

The native security boundary is unchanged: Sensitive tools still require the permission broker and cannot be approved through management IPC.

## Edge Glow architecture

The perimeter uses four transparent click-through windows:

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

Edge geometry follows the **full monitor bounds**, not `rcWork`. This is intentional: the glow represents the physical display perimeter while Quick must stay inside the usable desktop work area.

Aurora Pulse render depth:

```text
edge-top      44 px
edge-left     46 px
edge-right    46 px
edge-bottom   64 px
```

The larger transparent render strips exist only to give blur/bloom room. They remain click-through and do not reserve desktop space.

### Aurora Pulse visual stack

The upgraded perimeter intentionally avoids the previous dense rainbow-strip appearance. Default assistant states use a restrained cyan -> electric blue -> violet -> pink spectrum.

```text
soft ambient bloom
       +
legacy flow layer at reduced opacity
       +
2 offset 1 px orbit traces
       +
travelling bright pulse packet
       +
1.5-1.8 px physical-light core
       +
4 corner blooms (top/bottom windows)
```

`EdgeOverlay.tsx` keeps the existing state/event contract and only adds visual layers:

```text
edge-orbit-a
edge-orbit-b
edge-pulse
edge-corner-start
edge-corner-end
```

The two side windows intentionally do not render their own corner bloom, preventing doubled flares where edge windows overlap. Top and bottom surfaces own the four corner highlights.

Visual modes:

- **activated**: fast pulse + strong corner bloom for the invocation surge;
- **ready**: very low-intensity persistent aura and slow travelling pulse;
- **listening**: intensity follows the already precomputed `--voice-opacity` / `--voice-scale` values from microphone RMS;
- **processing**: faster orbit/pulse motion;
- **executing**: fastest normal motion and stronger pulse;
- **speaking**: slow breathing brightness;
- **confirming**: warm amber/orange spectrum including corner blooms;
- **error**: coral/red pulse and corner blooms.

The default assistant spectrum no longer uses green/yellow segments. Amber is reserved for permission confirmation; red/coral is reserved for errors.

Both Quick and edge visuals respect `prefers-reduced-motion`. Reduced-motion mode disables orbit/pulse/corner animation while retaining a low-intensity static perimeter.

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

The frontend source tree contains only the three mounted surfaces plus shared API/types files.

## Tauri capability boundary

The shared core capability targets only application-owned UI windows:

```json
"windows": ["main", "quick", "edge-*"]
```

It still contains only:

```json
"permissions": ["core:default"]
```

Dynamic height uses the default event capability; it does not add frontend `set_size` or `set_position` permission. The Aurora Pulse redesign adds no frontend capability and no new command or IPC surface.

This UI architecture does not bypass MCP or permission policy. Management IPC does not expose Windows tools directly; Sensitive actions still traverse the normal Assistant -> MCP -> permission-gateway path.

The obsolete `assistant_quick_expand` command has been removed, so the frontend cannot promote Quick into the permission-only `main` window.

## Local verification checklist

On Windows, verify:

1. Start Assisstant Desktop normally; no full management or empty permission window appears.
2. Focus another app and press `Alt + Space`; Quick appears on the same monitor at approximately `206px` high.
3. Verify the activation edge has a thin cyan/blue/violet/pink core, moving pulse and visible corner bloom rather than a thick rainbow strip.
4. Move the mouse/click through all four glow strips and corners; underlying desktop/application interaction must remain unaffected.
5. After the activation surge, verify `ready` settles to a subtle persistent aura rather than remaining visually dominant.
6. Speak into the microphone; listening intensity should react to RMS without changing window geometry.
7. Trigger processing/executing; pulse/orbit speed should increase without strobing.
8. Trigger speaking; perimeter brightness should breathe more slowly.
9. Trigger Sensitive confirmation; edge color should switch to amber/orange including the corners.
10. Trigger an error; edge color should switch to coral/red including the corners.
11. Enable Windows reduced-motion preference; verify travelling/orbit/corner animations stop while a static low-intensity edge remains.
12. Send a one-line/short request; verify the panel remains compact.
13. Produce a multi-line response; verify Quick grows smoothly enough to expose more preview lines but never beyond `380px`.
14. After a long response, send/return to shorter content and verify Quick shrinks again.
15. Hide and reopen Quick after a long response; verify the new invocation starts at default compact height.
16. While Quick expands/shrinks, verify its bottom edge stays anchored above the taskbar/work-area bottom rather than moving below it.
17. Trigger wake; Quick appears and exactly one voice turn starts after the wake delay.
18. Ask which window is active; it should refer to the source app, not Quick.
19. Left-click tray and use tray **Mở Assistant**; both should show Quick.
20. Launch the desktop executable again; the existing single instance should show Quick.
21. With the taskbar at the bottom, verify Quick stays above it with a small margin.
22. Test a secondary monitor; Aurora Pulse should follow the source application monitor while Quick follows its work area.
23. Trigger a Sensitive tool; Quick hides and the compact permission window appears.
24. Deny with `Esc`; verify the tool is denied.
25. Trigger again and Allow Once; verify the request proceeds and the permission window hides afterward.
26. Confirm no old chat/settings management UI exists in the frontend source tree or mounted routes.
27. Use `assistant` / `assistant status` / `assistant logs -f` for management and diagnostics.

## Remaining UI work

The terminal-first surface routing is source-complete. Remaining UI work is refinement rather than migration:

- add an explicit Stop/Cancel action for microphone/long turns;
- optionally add contextual chips and configurable auto-dismiss;
- remove the legacy tray autostart duplicate after local verification;
- complete target-Windows lifecycle/permission/wake verification before release.
