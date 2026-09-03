# Windows lifecycle: single instance and startup

Phase 14A hardens the desktop application lifecycle without expanding the MCP or computer-use surface.

## Goals

- keep only one Assisstant Desktop process active per signed-in Windows session;
- bring the existing main window forward when the user launches the app again;
- allow the user to opt in or out of Windows logon startup from the tray menu;
- start quietly in the tray when Windows launches the app automatically;
- keep explicit user launches visible and focused.

## Single-instance behavior

The Tauri single-instance plugin is registered before every other lifecycle plugin. When another process attempts to launch the desktop app, that process exits and the callback runs in the existing process.

For a normal launch, the existing process calls the same `show_main_window` path used by the tray icon and global shortcut. This preserves source-window capture and edge-overlay activation.

An incoming launch containing `--background` does not steal focus. This prevents a duplicate Windows startup invocation from unexpectedly opening the assistant if an instance is already active.

## Windows startup

The Tauri autostart plugin registers the packaged executable with one fixed argument:

```text
--background
```

The argument is controlled by the application and is not model-supplied.

The tray menu contains a checked item:

```text
Khởi động cùng Windows
```

Toggling it calls the native autostart manager. If registration fails, the menu check state is reverted and the failure is logged without terminating the assistant.

Autostart is opt-in. The application does not silently enable itself during installation or first launch.

## Background startup

During an autostart launch the normal Tauri runtime is initialized so tray, wake word, permission broker, MCP configuration and runtime resources remain available. After setup, the main window and edge overlay are hidden.

The assistant can then be surfaced through:

- left-clicking the tray icon;
- `Alt + Space`;
- a wake-word detection when that runtime is enabled and ready;
- launching Assisstant Desktop again from Windows.

## Security and privacy

This phase does not add any model-callable tool and does not grant frontend plugin permissions. Startup state is changed only by the local tray interaction.

The `--background` argument only changes initial visibility. It does not bypass permission checks, readiness checks, wake settings, or the MCP permission gateway.

## Local Windows verification

Remote development policy still forbids running native builds or GitHub Actions. Verify this phase locally on Windows after pulling `main`:

1. Start the app normally and confirm the main window appears.
2. Start the executable a second time and confirm no second process remains; the first window should become visible/focused.
3. Enable **Khởi động cùng Windows** from the tray and confirm the item remains checked after reopening the tray.
4. Inspect Windows Startup Apps / the user startup registration and confirm Assisstant Desktop is present.
5. Sign out/in or launch the registered command with `--background`; confirm the process starts with the main window hidden and the tray icon available.
6. While the background instance is active, launch the app normally and confirm the existing window is shown.
7. Disable **Khởi động cùng Windows** and confirm the registration is removed.

Any compiler/runtime failure found by the local verifier takes precedence over further release-hardening work.
