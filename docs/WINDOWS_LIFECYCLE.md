# Windows lifecycle: single instance and startup

Assisstant Desktop runs as a background Windows assistant. Normal explicit activation surfaces the compact Quick Assistant; the `main` WebView is reserved for Sensitive permission confirmation.

## Goals

- keep only one Assisstant Desktop process active per signed-in Windows session;
- surface the existing Quick Assistant when the user explicitly launches the app again;
- surface Quick from the tray rather than exposing the permission-only `main` window;
- manage Windows logon startup through the authenticated terminal management path;
- start quietly when Windows launches the app automatically;
- keep the permission host hidden unless the native permission broker needs it.

## Single-instance behavior

The Tauri single-instance plugin is registered before the remaining lifecycle plugins. When another process attempts to launch the desktop app, that process exits and the callback runs in the existing process.

For a normal second launch, the existing process calls:

```text
show_quick_window(app, "launch")
```

This captures the external source window before Quick takes focus, activates the edge glow, positions Quick on the source monitor, and focuses the compact interaction surface.

An incoming launch containing:

```text
--background
```

does not steal focus. This prevents a duplicate Windows startup invocation from unexpectedly surfacing the assistant if an instance is already active.

## Tray behavior

The current tray menu keeps:

```text
Mở Assistant
Ẩn cửa sổ
Khởi động cùng Windows
Thoát
```

`Mở Assistant` and a left click on the tray icon call:

```text
show_quick_window(..., "tray")
```

They do not show `main`. `Ẩn cửa sổ` hides Quick, Edge, and the permission host if it happens to be visible.

The permission-only `main` window is surfaced by the native permission broker through `show_main_window()` when a Sensitive tool requires explicit confirmation. It is not a general activation surface.

The tray autostart checkbox is now a legacy duplicate control. Terminal-first management uses:

```powershell
assistant startup show
assistant startup enable
assistant startup disable
```

The tray checkmark is initialized when the desktop process starts and currently does not live-refresh if autostart is changed through the CLI. Remove the duplicate tray control after local Windows verification of the new terminal path.

## Quick failure behavior

If Quick cannot be shown, the runtime logs the failure and hides Edge. It does not fall back to `main`, because displaying a permission host without a permission request would be misleading and would violate the terminal-first UI contract.

## Windows startup

The Tauri autostart plugin registers the packaged executable with one fixed argument:

```text
--background
```

The argument is controlled by the application and is not model-supplied.

The terminal manager reaches the running runtime over authenticated management IPC:

```text
assistant.exe
    |
    +-- startup.get
    +-- startup.set
            |
            v
Tauri autostart manager
```

There is no separate registry/task-scheduler implementation in the CLI.

Autostart is opt-in. The application does not silently enable itself during installation or first launch.

## Background startup

During an autostart launch the normal Tauri runtime is initialized so tray, Quick/Edge hosts, wake word, permission broker, MCP configuration, management IPC, and runtime resources remain available. Graphical surfaces start hidden.

The assistant can then be surfaced through:

- left-clicking the tray icon;
- tray **Mở Assistant**;
- `Alt + Space`;
- wake-word detection when enabled and ready;
- launching Assisstant Desktop again from Windows;
- `assistant overlay show` through authenticated local management IPC.

All of those normal activation paths lead to Quick. Sensitive confirmation remains the only reason to surface `main`.

## Security and privacy

Lifecycle management does not add any model-callable tool and does not change permission policy. Startup state is changed only through local user management surfaces.

`startup.get` and `startup.set` are authenticated management commands. They do not expose MCP execution or Sensitive approval capability.

The `--background` argument only changes initial visibility. It does not bypass permission checks, readiness checks, wake settings, or the MCP permission gateway.

`show_main_window()` remains reachable from the permission service but is no longer exposed as a normal UI activation command. The obsolete `assistant_quick_expand` Tauri command was removed.

## Local Windows verification

Do not manually dispatch GitHub Actions for native lifecycle verification. After pulling `main`, verify locally on Windows:

1. Start the app normally and confirm no full management or empty permission window appears.
2. Press `Alt + Space`; confirm Quick + Edge appear on the source monitor.
3. Left-click the tray icon; confirm Quick appears, not the permission window.
4. Use tray **Mở Assistant**; confirm the same Quick behavior.
5. Start the executable a second time; confirm no second process remains and the existing process shows Quick.
6. Start a second instance with `--background`; confirm it does not steal focus or surface Quick.
7. Run `assistant startup show` and record the current state.
8. Run `assistant startup enable`; confirm Windows registration is present and points to the packaged app with `--background`.
9. Sign out/in or invoke the registered `--background` command; confirm the runtime starts quietly with the tray available.
10. Run `assistant startup disable`; confirm the registration is removed.
11. Trigger a Sensitive tool; confirm only then does the permission-only `main` window appear.
12. Resolve/deny the request and confirm the permission window hides again.
13. Until the legacy tray toggle is removed, restart the desktop process after a CLI startup change and confirm its tray checkmark matches the actual registration state.

Any compiler/runtime failure found by the local verifier takes precedence over further release-hardening work.
