# Android Voice Satellite

## Purpose

The Android phone is the preferred speech-input surface for Assisstant Desktop.

The phone owns microphone capture and speech recognition. Windows receives only final recognized text, keeps reasoning/tool execution on the desktop, and speaks the response through desktop TTS.

```text
Android phone
  microphone
      |
SpeechRecognizer
  |- partial text -> phone UI only
  `- final text
      |
      | authenticated WebSocket / trusted LAN
      v
Windows desktop
  Voice Satellite adapter
      |
Assistant Core -> Antigravity -> MCP -> permissions -> Windows tools
      |
friendly VI / EN / Auto response
      |
Windows SAPI TTS
```

The CPAL/VAD/Zipformer desktop path remains available as a fallback input path.

## Android recognition policy

The Android app supports:

- `vi-VN` recognition;
- `en-US` recognition;
- Android on-device speech recognition when the platform exposes it;
- fallback to the system `SpeechRecognizer` service;
- partial transcript display on the phone;
- final-transcript-only submission to Windows.

On Android devices with Google services, the system recognizer may be backed by Google's speech-recognition service. Other vendors may provide another recognizer. The project does not require a paid STT API key.

`SpeechRecognizer` is not used as an always-listening loop. Push-to-talk remains the current activation mechanism.

## Persistent pairing

Listener configuration is stored at:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\settings\satellite.json
```

Example:

```json
{
  "enabled": true,
  "bind": "0.0.0.0:8765",
  "token": "<64-character-random-token>"
}
```

Create or rotate the pairing token with:

```powershell
assistant-satellite pair
```

Useful listener commands:

```powershell
assistant-satellite show
assistant-satellite pair
assistant-satellite pair --bind 0.0.0.0:8765
assistant-satellite enable
assistant-satellite disable
assistant-satellite bind 0.0.0.0:8765
assistant-satellite revoke
```

The desktop watches `satellite.json` while running. Pairing, enable/disable, bind-address, and shared-token revoke changes normally apply within about one second. Rotating or revoking the shared token aborts the active satellite server task and drops the current phone connection.

For development/backwards compatibility these environment variables are still accepted:

```text
ASSISTANT_VOICE_SATELLITE_TOKEN
ASSISTANT_VOICE_SATELLITE_BIND
```

An environment token takes precedence over `satellite.json`. Because process environment is fixed after launch, changing/removing the legacy override requires restarting the desktop process.

## Trusted-device registry

Each Android installation now owns a stable random `device_id`. It is generated once and stored in the app's private `SharedPreferences`.

After the shared pairing token is verified, Windows records the phone at:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\settings\satellite-devices.json
```

Each entry contains:

```json
{
  "id": "2f765145-e6fd-4bb6-ae19-e1da2df383b9",
  "name": "Google Pixel 9",
  "first_seen_unix": 1788790000,
  "last_seen_unix": 1788790200,
  "revoked": false
}
```

The shared token is the bootstrap pairing credential. The device registry is the second trust layer used to revoke one phone without rotating the pairing token for every other phone.

Manage devices with:

```powershell
assistant-satellite devices
assistant-satellite revoke-device <device-id>
assistant-satellite allow-device <device-id>
```

Behavior:

- first successful authentication with a valid shared token registers the `device_id`;
- subsequent successful authentication updates the device name and `last_seen_unix`;
- a revoked device is rejected even when it still knows the current shared token;
- an already-connected revoked device is checked approximately once per second and disconnected;
- `allow-device` permits the same installation identity to connect again;
- rotating/revoking the shared token remains the emergency way to invalidate all current pairing knowledge.

Deleting/reinstalling the Android app may create a new installation identity. QR pairing and device-specific credentials are still follow-up work.

## Listener lifecycle

```text
legacy env token present?
     /          \
   yes           no
    |             |
env config    read satellite.json
                  |
             enabled + valid token?
                /        \
              no          yes
              |            |
         no LAN bind    bind listener
                            |
                      phone connects
                            |
                    shared token valid?
                       /          \
                     no            yes
                     |              |
                  reject       validate device_id
                                    |
                            device revoked?
                              /        \
                            yes         no
                             |           |
                          reject      register/touch
                                          |
                                      ready session
```

Only one active satellite phone connection is accepted at a time in the current phase.

## Security boundary

The phone never receives direct MCP, Win32, shell, permission-broker, management-IPC, or Antigravity access.

Current security properties:

- no active pairing token -> no LAN listener;
- first WebSocket application message must authenticate;
- authentication timeout is 5 seconds;
- protocol version is checked;
- a bounded validated `device_id` is required;
- revoked device IDs are rejected after shared-token verification;
- active device trust is rechecked approximately once per second;
- client WebSocket frames must be masked;
- frame payload is bounded to 64 KiB;
- one active phone connection and one satellite command at a time;
- commands are refused while another Assistant turn is active;
- Sensitive actions still require the desktop permission path;
- `assistant-satellite show` masks the shared token.

The current transport is unencrypted `ws://` and is intended only for a trusted/private LAN. Do not expose port `8765` directly to the public Internet.

The shared token is currently persisted in application data and Android preferences. DPAPI/Android Keystore-backed secret storage, QR pairing, device-specific credentials, WSS/Tailscale, and replay hardening remain later work.

## Protocol v1

### Authenticate

The first application message after WebSocket upgrade must include the installation identity:

```json
{
  "type": "hello",
  "token": "replace-with-pairing-token",
  "device_id": "2f765145-e6fd-4bb6-ae19-e1da2df383b9",
  "device_name": "Google Pixel 9",
  "protocol": 1
}
```

Successful response:

```json
{
  "type": "ready",
  "protocol": 1,
  "response_language": "vi"
}
```

If authentication or device trust fails, the desktop returns an `authentication_failed` error when possible and closes the session.

If a connected phone is revoked, the desktop may send:

```json
{
  "type": "error",
  "id": null,
  "code": "device_revoked",
  "message": "This Android satellite device has been revoked on the desktop."
}
```

and then closes the connection.

### Submit final recognized text

```json
{
  "type": "command",
  "id": "5db576d9-a3dc-47e1-b1ef-ad9522bd3410",
  "text": "Mở Visual Studio Code",
  "response_language": "vi"
}
```

`response_language` accepts:

- `vi` — generate the response in Vietnamese;
- `en` — generate the response in English;
- `auto` — reply in the language used by the user.

Simple successful commands should normally produce one short, friendly confirmation sentence.

Runtime state messages use `processing` and `speaking`, followed by:

```json
{
  "type": "response",
  "id": "5db576d9-a3dc-47e1-b1ef-ad9522bd3410",
  "text": "Được, mình đã mở Visual Studio Code.",
  "tts_error": null
}
```

The phone displays the text; Windows SAPI owns spoken output.

## Desktop TTS language note

`VI`, `EN`, and `Auto` currently control generated response text. The desktop still uses its configured/default Windows SAPI voice. Locale-aware installed-voice selection is Phase 27.

## Phase 26 status

Completed so far:

- durable `satellite.json` settings;
- generated 64-character shared pairing secret;
- `show/pair/enable/disable/bind/revoke` helper commands;
- live listener hot reload;
- active-session drop on token rotation/revoke;
- stable Android installation `device_id`;
- `satellite-devices.json` trusted-device registry;
- first/last-seen tracking;
- per-device revoke/allow commands;
- active-session disconnect after per-device revoke;
- legacy environment compatibility.

Still planned in Phase 26:

- fold management under the canonical `assistant satellite ...` CLI surface;
- QR pairing so endpoint/token do not need manual typing;
- connection/last-seen presentation in normal desktop UI;
- Windows Firewall/private-network setup guidance or safe automation;
- stronger secret-at-rest storage;
- device-specific pairing credentials after QR flow.

## Local verification checklist

Run these checks on the user's Windows PC and Android phone; no remote build/test/Actions run is implied.

1. Put phone and PC on the same trusted LAN.
2. Run `assistant-satellite pair` while the desktop runtime is running.
3. Confirm the listener appears without restarting Windows desktop.
4. Enter `ws://<PC-LAN-IP>:8765` and the token on Android.
5. Connect and run `assistant-satellite devices`; confirm one trusted UUID is recorded.
6. Test a Vietnamese command and an English command.
7. Confirm partial recognition remains phone-side and one final text request reaches Assistant Core.
8. Confirm Windows action execution still uses MCP/permission policy.
9. Confirm desktop TTS speaks the response in the selected response-text language.
10. Run `assistant-satellite revoke-device <device-id>` while connected; confirm the session closes within roughly one second.
11. Confirm that phone cannot reconnect while revoked even with the correct shared token.
12. Run `assistant-satellite allow-device <device-id>` and confirm reconnect succeeds.
13. Rotate the shared token with `assistant-satellite pair`; confirm the old token no longer authenticates.
14. Run `assistant-satellite revoke`; confirm the listener stops and active phone disconnects.
15. Confirm desktop Zipformer voice remains usable as fallback when built with the voice feature.
