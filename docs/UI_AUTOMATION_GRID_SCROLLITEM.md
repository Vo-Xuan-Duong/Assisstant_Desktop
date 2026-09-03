# Phase 10C — UI Automation Grid + ScrollItem native foundation

## Purpose

Phase 10C extends the native `windows-tools` UI Automation layer with structural grid metadata and semantic item scrolling without introducing raw mouse, wheel, keyboard, or coordinate input.

This phase deliberately stops at the native contract. MCP exposure of `scroll_into_view` is deferred to Phase 10D so the native API can be reviewed independently first.

## Added GridPattern snapshot

When an element exposes `IUIAutomationGridPattern`, `ui_inspect`'s native snapshot now contains:

```json
{
  "grid": {
    "row_count": 24,
    "column_count": 6
  }
}
```

`row_count` and `column_count` describe the provider's current logical grid dimensions.

The snapshot does **not** enumerate arbitrary cells through `GridPattern::GetItem` in this phase. Existing bounded tree traversal remains the only inspection traversal path, preserving the current `max_depth` / `max_nodes` limits.

## Added GridItemPattern snapshot

When an element exposes `IUIAutomationGridItemPattern`, the snapshot contains:

```json
{
  "grid_item": {
    "row": 4,
    "column": 2,
    "row_span": 1,
    "column_span": 1
  }
}
```

All row and column indexes are zero-based as defined by Windows UI Automation.

This gives the assistant structural context for tables, detail views, grids and other two-dimensional controls without reading editable cell values.

## Added ScrollItemPattern capability

Each element now includes:

```text
supports_scroll_item: boolean
```

When true, the native layer can call:

```text
scroll_into_view(window_handle, path)
```

The implementation resolves the element from the existing explicit HWND + child-index path and invokes `IUIAutomationScrollItemPattern::ScrollIntoView()`.

Windows UI Automation chooses the final position inside the owning viewport. The assistant does not synthesize wheel input and does not choose pixel coordinates.

## Targeting contract

The existing targeting contract is unchanged:

```text
Desktop source HWND
      |
      v
inspect(handle)
      |
      v
bounded structural tree
      |
      +-- grid / grid_item metadata
      +-- supports_scroll_item
      |
      v
exact element path
      |
      v
native semantic action
```

If the accessibility tree changes, the path can become stale. The native resolver must fail rather than target a different element. The caller must inspect again.

## Privacy boundary

Phase 10C adds only structural metadata:

- grid dimensions;
- grid item row/column;
- row/column spans;
- ScrollItem capability.

It does not add:

- ValuePattern text extraction;
- TextPattern document extraction;
- cell contents outside the existing element `name` metadata;
- clipboard reads;
- OCR;
- screenshots;
- raw keyboard input;
- raw mouse input.

## Why GridPattern is read-only here

`GridPattern` is primarily a structural traversal contract. Its dimensions and item coordinates improve model reasoning, but this phase intentionally avoids a new `GetItem(row, column)` traversal API because that could bypass the existing bounded tree inspection contract.

A later phase can add targeted grid lookup if there is a concrete need and an explicit result/path identity contract.

## Phase 10C local verification checklist

On Windows, verify manually after the larger system is ready for local validation:

1. inspect a simple table/data grid and confirm `grid.row_count` / `grid.column_count` appear on the container;
2. confirm visible cells expose `grid_item.row` / `column` where supported;
3. verify row/column spans are reasonable for merged/spanning cells when the provider exposes them;
4. inspect a virtualized or scrollable list and find an element with `supports_scroll_item=true`;
5. call native `scroll_into_view` on a valid path and confirm the provider brings the item into its viewport;
6. verify a stale path fails rather than scrolling another element;
7. verify controls without ScrollItemPattern return Unsupported instead of falling back to wheel events;
8. confirm no editable field contents are newly extracted by inspection.

## Deferred to Phase 10D

- MCP tool `ui_scroll_into_view`;
- permission catalogue classification;
- Antigravity tool description and schema;
- end-to-end permission gateway path.

## Still deferred

- raw mouse movement/clicking;
- raw keyboard typing;
- pixel-coordinate vision fallback;
- GridPattern `GetItem` traversal;
- TextPattern extraction;
- arbitrary computer-use primitives.
