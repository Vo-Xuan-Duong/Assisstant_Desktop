# Phase 26 — Trusted Android Satellite Devices

This source phase adds per-installation Android identity and per-device revocation on top of the existing shared pairing token.

Implemented:

- Android generates and persists a stable random `device_id` per installation;
- WebSocket `hello` carries `device_id` and device name;
- Windows records first/last seen metadata in `settings/satellite-devices.json`;
- shared token must validate before a device can be registered;
- `assistant-satellite devices` lists known devices;
- `assistant-satellite revoke-device <device-id>` revokes one phone;
- `assistant-satellite allow-device <device-id>` re-enables it;
- active sessions recheck device trust roughly once per second;
- revocation markers under `settings/satellite-revoked/` are authoritative, preventing a concurrent last-seen registry write from accidentally undoing a revoke.

The shared token remains the bootstrap pairing credential. QR pairing and device-specific credentials are the next Phase 26 work.

No remote build, tests, Actions, model download, or installer run was performed for this source phase. Validate on the target Windows PC and Android phone locally.
