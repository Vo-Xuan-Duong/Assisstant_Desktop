# Phase 30 — Secure Remote Android Voice Satellite with Tailscale

## Goal

Allow the Android Voice Satellite to reach Assisstant Desktop away from the local Wi-Fi/LAN without exposing the raw satellite WebSocket port to the public Internet.

## Architecture

```text
Android Voice Satellite
  + Tailscale Android client
          |
     encrypted tailnet
          |
          v
Windows Tailscale Serve TCP
  tailnet TCP :8765
          |
          v
127.0.0.1:8765
          |
          v
Assisstant Desktop satellite listener
```

The Assistant application protocol remains unchanged:

- RFC 6455 WebSocket;
- bootstrap pairing token;
- trusted Android `device_id`;
- per-device revoke;
- request-id deduplication;
- desktop-owned permission checks;
- desktop-owned cancellation/TTS.

Tailscale adds a private encrypted network transport and tailnet access policy; it does not replace application authentication.

## Why Tailscale Serve

Phase 30 deliberately uses **Tailscale Serve**, not Funnel.

Serve is accessible only inside the configured tailnet and obeys Tailscale access-control policy. Funnel is intended for public Internet exposure and is not used by this project.

The integration uses Tailscale's raw TCP forwarder:

```powershell
tailscale serve --bg --tcp=8765 tcp://127.0.0.1:8765
```

The desktop satellite backend is moved to loopback before Serve is enabled. This avoids leaving the raw listener directly reachable on normal LAN interfaces while remote mode is managed.

On Windows, Tailscale documents running Serve commands from an Administrator terminal where required; the Assistant helper itself does not self-elevate.

## Canonical commands

```powershell
assistant satellite remote tailscale show
assistant satellite remote tailscale enable
assistant satellite remote tailscale pair --qr
assistant satellite remote tailscale disable
```

### Enable

`enable`:

1. refuses to proceed while the legacy `ASSISTANT_VOICE_SATELLITE_TOKEN` environment override is active;
2. verifies that `tailscale ip -4` returns a connected Tailscale IPv4 address;
3. requires an existing valid Assistant satellite pairing token;
4. records the current satellite bind **and enabled state** in `settings/satellite-tailscale.json`;
5. moves/enables the backend on `127.0.0.1:<port>`;
6. starts persistent tailnet-only raw TCP Serve with `--bg`;
7. rolls the backend settings back if Serve setup fails;
8. rolls Serve/settings back if managed-state persistence fails.

Re-running enable is idempotent only while the managed backend state still matches. If the user manually changes the bind while remote state exists, the helper fails closed and asks for `disable` before reconfiguration so it cannot leave a stale Serve endpoint behind.

### Pair

`pair --qr`:

1. requires managed remote mode to be enabled;
2. gets this PC's Tailscale IPv4 using `tailscale ip -4`;
3. decrypts the existing Windows DPAPI-protected bootstrap token in memory;
4. creates the existing `assd://p?...` pairing URI with the Tailscale IPv4;
5. renders the QR locally in the terminal;
6. does not upload the token/QR anywhere.

Android still imports the QR without automatically connecting. The user explicitly taps **Kết nối** and the normal trusted-device authentication remains in effect.

### Disable

`disable`:

1. reads only the Assistant-managed state file;
2. disables only the Serve TCP port created by this integration using the same `--tcp=<port>` selector plus `off`;
3. does **not** use `tailscale serve reset`, so unrelated Serve configuration is left intact;
4. restores the previous satellite bind only if the current bind still matches the managed loopback endpoint;
5. restores the previous enabled state if the listener was still enabled by remote mode;
6. if the user explicitly disabled the satellite while remote mode was active, keeps it disabled;
7. leaves a manually changed bind untouched rather than overwriting it.

## State file

Non-secret managed remote state:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\settings\satellite-tailscale.json
```

Example:

```json
{
  "version": 1,
  "previous_bind": "0.0.0.0:8765",
  "previous_enabled": true,
  "port": 8765
}
```

The pairing token remains in `satellite.json` protected with Windows DPAPI.

## Tailscale IP

The helper uses `tailscale ip -4` and currently validates the normal Tailscale CGNAT IPv4 range `100.64.0.0/10` before generating the QR.

IPv6-only tailnets are not handled by this first remote flow; add IPv6/MagicDNS as a follow-up if local validation requires it.

## Android requirements

The Android phone must:

- have the Tailscale Android client installed;
- be connected to the same tailnet (or otherwise granted access by tailnet policy);
- keep the existing Assistant Voice Satellite app installed and paired;
- use the QR produced by the remote pairing command.

The Android application does not need a Tailscale SDK dependency. Its existing OkHttp WebSocket connection simply targets the PC's tailnet IP.

## Security properties

- no public port-forward is created;
- no Tailscale Funnel command is used;
- desktop backend is loopback-only while managed remote mode is active;
- Tailscale tailnet encryption/access control is additive to Assistant pairing/device auth;
- DPAPI still protects the persisted Windows bootstrap credential;
- Android Keystore still protects the phone-side credential;
- per-device revoke remains authoritative;
- Sensitive Windows actions still require the desktop permission model;
- command replay protection remains unchanged.

Raw `ws://` is still used at the application layer. In remote mode those packets travel inside the encrypted Tailscale tunnel. This phase does not claim end-to-end WebSocket TLS independent of Tailscale.

## Local validation gate

No remote Tailscale installation, login, Serve mutation, build, test, GitHub Action, installer, Android run, or Windows run is performed.

Validate locally:

1. Install/sign in to Tailscale on Windows and Android using the intended tailnet.
2. Create normal satellite pairing if one does not exist.
3. Run `assistant satellite remote tailscale enable`.
4. Confirm `assistant satellite show` reports a loopback backend.
5. Run `assistant satellite remote tailscale show` and verify the expected Serve mapping.
6. Run `assistant satellite remote tailscale pair --qr` and scan it on Android.
7. Turn off local Wi-Fi on the phone (use mobile data) while Tailscale remains connected.
8. Verify a voice command reaches the desktop and is still permission-gated.
9. Revoke the device on Windows and verify it cannot reconnect remotely.
10. Run `assistant satellite remote tailscale disable` and confirm the prior bind/enabled state is restored as expected.
11. Verify unrelated Tailscale Serve configuration, if any, remains intact.
