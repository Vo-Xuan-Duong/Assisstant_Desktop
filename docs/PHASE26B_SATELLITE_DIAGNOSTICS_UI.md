# Phase 26B — Desktop Satellite Diagnostics UI

## Goal

Expose useful Android Voice Satellite state inside the existing desktop Quick surface without exposing pairing secrets or moving security mutations into the WebView.

## Data source

The existing `assistant_readiness` Tauri command now includes a `satellite` snapshot read from app-local state:

- `settings/satellite.json`;
- `settings/satellite-devices.json`;
- `settings/satellite-revoked/` markers;
- `settings/satellite-tailscale.json`.

The snapshot contains only non-secret operational metadata:

- listener enabled/disabled;
- paired/unpaired state;
- listener bind;
- credential storage class (`dpapi-current-user`, legacy plaintext, unpaired, or environment override in UI);
- whether managed Tailscale remote mode exists and its port;
- trusted/revoked device counts;
- device id/name plus first/last-seen timestamps and revoked state;
- warnings for malformed state, legacy storage, environment override, or remote-state drift.

The pairing token itself is never serialized to the frontend and the UI never asks DPAPI to decrypt it.

## UI behavior

`SatelliteDiagnostics` is mounted in the Quick surface as a compact read-only panel. Opening it temporarily holds Quick auto-dismiss so the user can inspect/scroll the state. The panel uses the existing Quick window bounds instead of competing with `QuickOverlay` for native resize ownership.

The panel supports only:

- open/close;
- refresh snapshot.

Device revoke/allow, pairing rotation, firewall changes, and Tailscale changes remain on the authoritative desktop CLI:

```powershell
assistant satellite ...
```

This is intentional. A read-only WebView lowers the risk of accidentally turning diagnostics UI into a second management/security boundary.

## Readiness semantics

Voice Satellite remains optional for the desktop product. A disabled/unpaired/warning state is reported as `optional_missing`, not a blocking failure for text Assistant/MCP operation.

## Validation

Source/static review only in this phase. Validate locally that:

1. Quick shows the Satellite trigger without covering native Quick actions;
2. opening the panel does not auto-dismiss while interacting with it;
3. no token value appears in the WebView/devtools payload;
4. DPAPI, legacy, env override, LAN, Tailscale, trusted and revoked states render correctly;
5. malformed JSON yields warnings instead of crashing the Quick surface;
6. existing `assistant_readiness` callers tolerate the additive `satellite` field.

No remote build/tests/GitHub Actions/installer/network mutation were run for this source phase.
