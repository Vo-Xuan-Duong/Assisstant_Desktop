# Phase 32B — Bounded Satellite Management UI

## Goal

Add a small, reversible management surface to **Quick -> Voice -> Satellite** without turning the WebView into the authority for pairing credentials, network exposure, firewall configuration, or Tailscale lifecycle.

Phase 32B intentionally supports only:

```text
listener enable / disable
known device revoke / allow
```

Everything else stays in the authoritative CLI.

## Why the scope is narrow

The existing Satellite CLI can also:

- create or rotate the shared pairing token;
- print a pairing QR containing that credential;
- change listener bind addresses;
- install/remove Windows Firewall rules;
- enable/disable managed Tailscale Serve state.

Those operations can change credentials, network exposure, or external system state. They remain terminal-only in this phase.

The graphical controls are limited to actions that are bounded, reversible, and already represented by persisted desktop state.

## Architecture

```text
Quick Voice -> Satellite
        |
        | fixed Tauri commands only
        v
satellite_management.rs
        |
        +-- listener enabled flag
        |     `-- existing satellite.json
        |
        `-- per-device trust
              `-- authoritative satellite-revoked/<device-id>.revoked marker

pairing token value --------------------X--> WebView
arbitrary bind address -----------------X--> WebView
firewall/Tailscale command arguments ---X--> WebView
```

The frontend submits only:

```text
enabled: bool
```

or:

```text
device_id: existing bounded device ID
revoked: bool
```

The backend validates the device ID again and rejects unknown devices.

## Listener management

The UI can disable the persisted listener while retaining the current pairing credential, or re-enable it when a valid persisted pairing exists.

Enabling fails closed when:

- there is no persisted pairing token;
- the persisted token cannot be decrypted for the current Windows user;
- the token is invalid.

A legacy plaintext token encountered during **enable** is migrated to current-user Windows DPAPI before the listener is enabled.

### Managed-state locks

The graphical listener switch is deliberately rejected when either of these is active:

```text
ASSISTANT_VOICE_SATELLITE_TOKEN
satellite-tailscale.json managed remote state
```

Reason:

- an environment token override makes the persisted `enabled` flag non-authoritative;
- Tailscale remote management owns restore state and listener lifecycle.

The user must use the relevant CLI workflow in those modes.

## Device revoke / allow

The runtime already treats files under:

```text
settings/satellite-revoked/<device-id>.revoked
```

as authoritative.

### Revoke

Phase 32B creates the marker and does **not** rewrite the device registry. This avoids racing an active trusted connection that may concurrently update `last_seen_unix`.

The active satellite connection rechecks trust approximately once per second and should close after the marker appears.

### Allow

For compatibility with devices previously revoked by older CLI behavior:

1. a legacy `revoked=true` registry flag is cleared first;
2. the authoritative marker remains in place until that registry write succeeds;
3. only then is the marker removed.

This ordering fails closed: a write failure does not accidentally re-enable the device.

## Quick UI behavior

The Satellite tab now provides:

- listener state and bounded **Bật listener / Tắt listener** action;
- lock explanation for environment-override and managed-Tailscale modes;
- per-device **Thu hồi / Cho phép lại** action;
- mutation progress state;
- backend errors surfaced in the panel;
- automatic readiness refresh after successful mutation.

Quick auto-dismiss remains held while the Voice panel is open.

## Security invariants

Phase 32B does not:

- return or decrypt the pairing token into frontend memory;
- create a new pairing token;
- display a pairing QR;
- change the Satellite bind address;
- install/remove firewall rules;
- execute `netsh`;
- execute Tailscale commands;
- modify Tailscale managed restore state;
- grant the phone any additional MCP/Win32 authority;
- change Sensitive-action confirmation rules.

The WebView cannot provide an arbitrary path, executable, shell argument, firewall rule, host, port, or pairing credential.

## Local acceptance

Validate on the target Windows/Android installation:

1. paired local Satellite can be disabled from **Voice -> Satellite**;
2. the listener stops after the normal hot-reload interval while the pairing remains intact;
3. it can be enabled again without re-pairing;
4. enable is unavailable/rejected when no valid persisted pairing exists;
5. environment-token override locks the graphical listener control;
6. managed Tailscale mode locks the graphical listener control;
7. revoking a connected phone changes diagnostics to `Revoked` and closes/rejects that device;
8. another trusted device remains usable;
9. allowing the device again restores access without rotating the shared token;
10. malformed/unknown device IDs are rejected;
11. no pairing token appears in frontend payloads/devtools;
12. CLI pairing, bind, firewall and Tailscale workflows remain unchanged.

## Status

**COMPLETE IN SOURCE; LOCAL WINDOWS/ANDROID VALIDATION REQUIRED.**

Repository implementation remains source-first. No GitHub Action, native build/test, installer execution, microphone run, firewall mutation, Tailscale mutation, or device connection test is required in this implementation phase.
