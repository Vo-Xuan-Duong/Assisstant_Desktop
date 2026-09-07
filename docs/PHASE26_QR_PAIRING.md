# Phase 26 — Local QR Pairing

This phase removes manual endpoint/token typing from the normal Android Voice Satellite pairing path without sending the pairing credential to an external QR service.

## Windows flow

```powershell
assistant-satellite pair --qr
```

Optional explicit LAN address:

```powershell
assistant-satellite pair --qr --host 192.168.1.20
```

The helper generates a fresh shared pairing token, creates a compact deep link, and renders the QR locally in an ANSI-capable terminal.

Pairing URI format:

```text
assd://p?h=<phone-reachable-host>&p=<port>&t=<64-hex-token>
```

The local QR encoder is intentionally fixed to QR Version 5, error-correction level L, byte mode. The compact pairing payload is bounded before encoding. No network QR API is called.

## Android flow

The Android app registers the `assd://p` deep-link route. Scanning the QR imports:

- desktop host;
- port;
- pairing token.

The deep-link parser validates the scheme, endpoint marker, ASCII host, port range, and 64-character hexadecimal token.

Scanning **does not connect automatically**. The imported address/token are persisted and displayed, then the user must explicitly tap **Kết nối**. A new pairing link closes an existing satellite WebSocket before replacing the stored pairing data.

## Security boundary

- Treat the QR/URI as a credential because it contains the bootstrap token.
- Do not upload or post QR screenshots.
- The current `ws://` transport is still trusted-LAN-only.
- The custom URI scheme is an MVP convenience mechanism; it is not stronger than device-specific credentials.
- Per-device trust/revocation remains enforced after the shared token is verified.
- Future hardening should move persisted secrets to Windows DPAPI / Android Keystore and issue device-specific credentials during pairing.

## Local validation

No remote build/test/Actions run is implied. On the target Windows PC and Android phone:

1. run `assistant-satellite pair --qr`;
2. confirm the terminal QR is visually intact and not line-wrapped;
3. scan with the phone camera/QR scanner;
4. confirm Android opens the Voice Satellite app;
5. confirm the imported `ws://` address is the expected PC LAN address;
6. confirm the app does not auto-connect;
7. tap **Kết nối** and verify the device appears in `assistant-satellite devices`;
8. repeat with `--host <PC-LAN-IP>` on a multi-adapter machine;
9. verify malformed/custom deep links are rejected;
10. rotate pairing and verify the previous shared token no longer authenticates.
