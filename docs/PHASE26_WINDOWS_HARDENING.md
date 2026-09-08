# Phase 26 — Windows Satellite Hardening

## Scope

This phase closes the Windows-side security and diagnostics gaps left after Android Voice Satellite pairing became functional.

## DPAPI credential storage

New and rewritten `satellite.json` files no longer intentionally persist the bootstrap token as plaintext.

The helper keeps the existing JSON field for backwards compatibility, but writes a DPAPI envelope:

```json
{
  "enabled": true,
  "bind": "0.0.0.0:8765",
  "token": "dpapi:<hex-ciphertext>"
}
```

Implementation details:

- `CryptProtectData` / `CryptUnprotectData` through `windows-rs`;
- current-user scope (no `CRYPTPROTECT_LOCAL_MACHINE`);
- `CRYPTPROTECT_UI_FORBIDDEN` so background/runtime reads never trigger Windows UI;
- DPAPI output is copied and released with `LocalFree`;
- ciphertext is encoded locally as lowercase hex; no additional crypto/encoding dependency is added;
- runtime decrypts before building `SatelliteConfig`, so Android still authenticates with the original 64-character bootstrap token;
- supervisor compares the decrypted token, so randomized DPAPI ciphertext changes do not cause unnecessary listener restarts.

### Migration

Legacy settings containing a plaintext token remain readable. The next mutating command that rewrites settings (for example `enable`, `bind`, or `pair`) persists the token as a DPAPI envelope.

`assistant satellite doctor` reports `legacy-plaintext` until migration occurs.

The development override `ASSISTANT_VOICE_SATELLITE_TOKEN` still takes precedence and is intentionally not written to disk by this feature.

## Diagnostics

Canonical command:

```powershell
assistant satellite doctor
```

It reports:

- settings path;
- enabled/paired state;
- credential storage (`not-configured`, `legacy-plaintext`, `dpapi-current-user`);
- bind address and TCP port;
- trusted/revoked device counts;
- whether the listener uses a wildcard interface;
- whether a legacy token environment override is active;
- firewall and remote-access guidance.

## Windows Firewall

Commands:

```powershell
assistant satellite firewall show
assistant satellite firewall install
assistant satellite firewall remove
```

`install` creates only the named Assistant rule with these bounds:

```text
Direction   inbound
Protocol    TCP
LocalPort   configured satellite port
Profile     Private
RemoteIP    LocalSubnet
Action      Allow
```

It does not create a Public-profile rule and does not use an unrestricted Internet remote scope.

The CLI never self-elevates. `install` and `remove` must be run from an Administrator terminal when Windows requires it.

## Security boundary

DPAPI protects the persisted bootstrap token against simple plaintext theft by another Windows account or offline file inspection. It does not protect a token already available to a compromised process running as the same Windows user.

The shared token is still a bootstrap credential; trusted-device identity and per-device revocation remain additional controls. Stronger device-specific credentials remain a future hardening option.

The raw `ws://` listener is still LAN/private-network transport. Do not port-forward it to the public Internet. Phase 30 uses a private Tailscale tailnet for remote access.

## Local validation gate

No remote build/test/Actions/installer/DPAPI/netsh execution is performed.

Validate locally on Windows:

1. `assistant satellite pair --qr` writes `token: "dpapi:..."` to `satellite.json`.
2. The QR still contains the original plaintext bootstrap token and Android authenticates normally.
3. Restarting Assisstant Desktop decrypts the DPAPI value and re-enables the listener.
4. A legacy plaintext settings file is readable and becomes DPAPI-protected after a mutating satellite command.
5. `assistant satellite doctor` reports the expected storage/bind/device state.
6. `assistant satellite firewall install` creates only a Private + LocalSubnet TCP rule for the configured port.
7. `firewall remove` deletes only the named Assistant rule.
8. Token rotation/revoke still closes active Android sessions through the existing hot-reload lifecycle.
