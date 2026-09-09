# Phase 33A — Local Validation Dashboard

Status: **COMPLETE IN SOURCE; LOCAL WINDOWS VALIDATION REQUIRED**

## Goal

Expose the desktop runtime's existing readiness checks in the product UI so local acceptance work can start from one consistent, non-secret status surface instead of requiring the user to infer state from logs or multiple CLI commands.

This phase does **not** introduce a second health engine. The existing `assistant_readiness` report remains authoritative for automated runtime checks.

## Product surface

The former Quick **Voice** trigger becomes **Control** and now contains three tabs:

```text
Control
  |- Satellite
  |- TTS
  `- System
```

The Satellite and TTS behavior from Phase 32A/32B is preserved.

The new **System** tab displays:

- overall readiness level;
- count of `ready` checks;
- count of `optional_missing` checks;
- count of `blocking` checks;
- all runtime readiness checks, sorted with blocking issues first;
- each check label and detail;
- the relevant local path when the readiness payload already exposes one.

Current readiness sources include the existing checks for:

- Antigravity CLI;
- Windows MCP runtime/config;
- Permission Broker;
- context storage;
- Windows TTS;
- Android Voice Satellite;
- local STT resource;
- wake-word resource/runtime.

## Architecture

```text
existing native readiness collectors
        |
        v
assistant_readiness
        |
        v
RuntimeReadinessReport
        |
        +--> Satellite tab
        |
        `--> System self-check dashboard
```

No new shell execution, process launch, network probe, firewall mutation, Tailscale mutation, microphone access, model download, or installer execution is added by this dashboard.

## Meaning of readiness levels

```text
ready
  automated runtime condition is currently satisfied

optional_missing
  optional subsystem or configuration is unavailable / incomplete

blocking
  an automated condition required by the core desktop runtime is not satisfied
```

The UI deliberately says **runtime self-check**, not "release passed".

`RuntimeReadinessReport.overall` is a runtime aggregation. It is not a release certification and should not be reinterpreted as one.

## What the dashboard cannot verify

The following remain real-device/local acceptance items rather than automated readiness claims:

- QR pairing scans correctly on the target Android phone;
- Android recognizer quality for `vi-VN` / `en-US`;
- microphone permission/device behavior;
- real SAPI voice quality on the target Windows user profile;
- connected-device revoke timing under real traffic;
- Tailscale operation over mobile data with LAN unavailable;
- Windows Firewall rule behavior on the target machine;
- Quick Settings tile and launcher shortcut behavior on the target launcher;
- Windows installer/NSIS/startup behavior;
- conversational follow-up timing and acoustic behavior.

These stay in the local acceptance matrix.

## Security boundary

The System tab is read-only.

```text
native runtime state
      |
      v
bounded readiness DTO
      |
      v
System tab

pairing token ----------------X
raw secret values ------------X
arbitrary filesystem read ----X
shell/process execution -------X
firewall mutation ------------X
Tailscale mutation ------------X
microphone execution ----------X
```

Paths shown by the dashboard are only paths already included in the existing readiness DTO. Phase 33A does not add a generic path-reading capability.

## Local acceptance for Phase 33A

On the target Windows installation:

1. Open Quick -> **Control -> System**.
2. Confirm all readiness rows render without crashing when some subsystems are missing.
3. Confirm blocking rows appear before optional/ready rows.
4. Confirm summary counts match the visible rows.
5. Confirm **Làm mới** updates the report after a real local configuration change.
6. Confirm path text is truncated safely in the compact panel but remains inspectable via its native title tooltip.
7. Confirm no pairing credential or other secret value appears in the System payload/UI.
8. Confirm Satellite and TTS tabs keep their Phase 32A/32B behavior after the trigger is renamed to **Control**.

## Validation policy

No GitHub Actions, native build/tests, installer execution, microphone execution, firewall mutation, Tailscale mutation, model download, or Android device run is performed as part of source implementation.

Target Windows/Android validation remains local.
