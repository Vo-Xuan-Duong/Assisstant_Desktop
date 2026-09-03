# Phase 8A — Windows UI Automation Foundation

## Goal

Build the first deterministic computer-use layer on top of Microsoft UI Automation (UIA), before introducing any vision-based coordinate clicking.

Microsoft UI Automation exposes desktop UI elements through an accessibility tree and control patterns. The assistant uses this structured interface whenever an application exposes it.

```text
Windows application
      ↓
UI Automation provider
      ↓
IUIAutomationElement tree
      ↓
windows-tools::automation
      ↓
structured element snapshot / action
```

## Why UIA first

Prefer:

```text
Find button by accessibility element
      ↓
InvokePattern.Invoke()
```

instead of:

```text
vision guesses x/y
      ↓
mouse click
```

UIA is more deterministic, survives many window position/layout changes, and lets the runtime understand whether an element is enabled, focusable, offscreen, or exposes a supported control pattern.

Vision/pixel automation remains a fallback for applications that do not expose useful accessibility metadata.

## Native implementation

Phase 8A uses the existing `windows = 0.62.2` binding directly.

Required namespace:

```text
Win32_UI_Accessibility
```

The module lives at:

```text
crates/windows-tools/src/automation.rs
```

No extra Node.js/Python automation service is introduced.

## COM lifetime

Each public automation operation creates its own `IUIAutomation` client on the calling thread:

```text
CoInitializeEx(COINIT_MULTITHREADED)
      ↓
CoCreateInstance(CUIAutomation)
      ↓
ElementFromHandle(HWND)
      ↓
operation
      ↓
COM interfaces dropped
      ↓
CoUninitialize
```

UI Automation COM interfaces are not stored in shared application state and are not moved across async worker threads.

The future MCP adapter must execute these synchronous operations in a blocking worker context.

## Structural inspection

```rust
inspect(window_handle, UiInspectOptions)
```

returns a flat list of control-view nodes.

Each node contains:

```text
path
name
automation_id
class_name
localized_control_type
control_type
process_id
enabled
keyboard_focusable
has_keyboard_focus
offscreen
bounds
supports_invoke
supports_value
```

## Privacy boundary

The structural inspection deliberately does **not** read `ValuePattern.CurrentValue`.

This prevents a normal tree inspection from collecting editable field contents, including potentially sensitive text.

```text
inspect
  → structural metadata only

set_value
  → explicit action only
```

Password/text extraction is not part of Phase 8A.

## Tree limits

UIA trees can be very large for browsers, IDEs, Office applications, and complex Electron apps.

Defaults:

```text
max_depth = 4
max_nodes = 160
```

Hard limits:

```text
max_depth <= 8
max_nodes <= 500
```

The snapshot reports `truncated=true` when the requested tree exceeds the node budget.

This limit is both a performance control and an AI-context/token control.

## Element path

No COM pointer or UIA object is exposed outside the native function.

An element is represented by a path such as:

```json
[0, 2, 1]
```

Meaning:

```text
window root
  └── child 0
       └── child 2
            └── child 1
```

Actions re-open the UIA client and resolve the path against the current Control View immediately before execution.

Paths are not persistent identities. If the application's UI structure changes, resolution can fail with `NotFound`. The assistant must inspect again rather than guessing.

## Actions in Phase 8A

### Focus

```rust
focus(handle, path)
```

Calls UIA `SetFocus` on the resolved element.

### Invoke

```rust
invoke(handle, path)
```

Requires `InvokePattern` and calls:

```text
IUIAutomationInvokePattern::Invoke
```

This is suitable for many buttons/menu commands and does not synthesize a physical mouse click.

### Set value

```rust
set_value(handle, path, value)
```

Requires a writable `ValuePattern`.

The native layer checks:

- element supports ValuePattern;
- pattern is not read-only;
- value input is bounded before the COM call.

## Tool exposure

Phase 8A does **not** expose these methods to Antigravity/MCP yet.

Reason:

The native implementation must be stable before tool schema, permissions, and AI-facing semantics are frozen.

Phase 8B will add explicit tools such as:

```text
ui_inspect
ui_focus
ui_invoke
ui_set_value
```

with risk levels and argument validation.

## Security model for Phase 8B+

Expected policy:

```text
ui_inspect     → Safe / read-only structural context
ui_focus       → Moderate
ui_invoke      → Moderate or Sensitive depending on target
ui_set_value   → Moderate/Sensitive
```

A later permission gateway may elevate risk based on target application or requested operation.

For example, invoking a normal application tab and invoking a destructive confirmation button should not necessarily share the same approval policy.

## Limitations

UI Automation only works as well as the target application's accessibility provider.

Possible limitations:

- custom canvas controls may expose little/no structure;
- games may expose no usable UIA tree;
- elevated/admin applications may have access restrictions;
- browser page accessibility structure can be large;
- element paths can become stale after a dynamic UI update;
- UIA cannot cross user-session boundaries.

For those cases, later phases can combine:

```text
UI Automation
+
screen context
+
vision/computer use fallback
```

## Local verification checklist

Do not run repository GitHub Actions for this phase. Verify locally on Windows:

1. Inspect Notepad or another simple native application.
2. Confirm root + controls appear with names/control types.
3. Inspect a browser window and verify node limit/truncation behavior.
4. Inspect Visual Studio/VS Code and check that large trees remain bounded.
5. Focus a known edit/button element using its returned path.
6. Invoke a harmless button through InvokePattern.
7. Set a value on a writable edit control.
8. Change the UI after inspection and confirm a stale path returns an error rather than clicking elsewhere.
9. Confirm inspect output does not include typed edit-field values.
10. Inspect a non-accessible/canvas-heavy app and record missing-provider behavior for the later vision fallback.

## Deferred to Phase 8B

- MCP tool schemas;
- active/source-window convenience targeting;
- permission/risk mapping;
- Antigravity instructions for inspect → action sequences;
- action result normalization.

## Deferred further

- Toggle/Selection/ExpandCollapse patterns;
- scrolling patterns;
- browser-specific integration;
- text ranges;
- cache requests for high-performance bulk inspection;
- event subscriptions;
- vision-coordinate fallback;
- destructive-action confirmation based on semantic target context.
