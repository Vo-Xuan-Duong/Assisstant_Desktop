# Android Voice Satellite

## Purpose

The Android phone is the preferred speech-input surface for Assisstant Desktop.

The phone owns microphone capture and speech recognition. The Windows desktop receives only the final recognized text, keeps all reasoning/tool execution on the desktop, and speaks the response through desktop TTS.

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
  voice-satellite adapter
      |
Assistant Core
      |
Antigravity + MCP
      |
Windows tools
      |
friendly VI / EN / Auto response
      |
Windows SAPI TTS
```

The existing CPAL/VAD/Zipformer desktop voice path is retained as a fallback. It is no longer the preferred speech-recognition path.

## Android recognition policy

The Android app supports:

- `vi-VN` recognition;
- `en-US` recognition;
- Android on-device speech recognition when the device exposes it;
- fallback to the system `SpeechRecognizer` service;
- partial transcript display on the phone;
- final-transcript-only submission to the desktop.

The system recognizer is implementation-dependent. On Android phones with Google services it may be backed by Google's speech-recognition service; other vendors may provide a different implementation. Selecting `prefer on-device` asks the app to use Android's on-device recognizer when the platform reports it available.

The MVP is push-to-talk. `SpeechRecognizer` is not used as an always-listening loop. A future wake/hardware-trigger phase should activate recognition only when needed.

## Persistent pairing configuration

The preferred pairing source is:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\settings\satellite.json
```

Example shape:

```json
{
  "enabled": true,
  "bind": "0.0.0.0:8765",
  "token": "<64-character-random-token>"
}
```

Do not hand-author the token unless necessary. Use the pairing helper:

```powershell
assistant-satellite pair
```

Useful commands:

```powershell
assistant-satellite show
assistant-satellite pair
assistant-satellite pair --bind 0.0.0.0:8765
assistant-satellite enable
assistant-satellite disable
assistant-satellite bind 0.0.0.0:8765
assistant-satellite revoke
```

`pair` creates a new 64-character random token, stores it, enables the receiver, and prints the full token for entry into the Android app. `show` masks the token. `revoke` clears it and disables the listener.

The desktop watches this file while running. Pairing, enable/disable, bind-address, and revoke changes are normally applied within about one second. A token rotation/revoke aborts the current satellite server task, which also drops the active phone connection; the previous authenticated session therefore cannot continue after the old token has been invalidated.

### Legacy environment override

For development/backwards compatibility these environment variables are still accepted:

```text
ASSISTANT_VOICE_SATELLITE_TOKEN
ASSISTANT_VOICE_SATELLITE_BIND
```

When `ASSISTANT_VOICE_SATELLITE_TOKEN` is present, the environment configuration takes precedence over `satellite.json`. The token must contain at least 16 characters. The default bind remains:

```text
0.0.0.0:8765
```

Because environment variables are fixed for a running process, changing/removing this legacy override requires restarting the desktop process before the persistent settings file becomes authoritative. New setups should prefer the persistent pairing file/helper.

## Desktop listener lifecycle

The listener is fail-secure and supervised:

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
                     config changes?
                        /      \
                      no        yes
                      |          |
                   continue   abort old server
                              + drop active phone
                              + start/stop new server
```

The supervisor polls the persisted configuration roughly once per second. It also detects a finished listener task and retries from the desired configuration.

The listener uses WebSocket protocol RFC 6455 and the satellite JSON protocol below. The WebSocket implementation is built on the existing Tokio runtime so this path does not add a new Rust transport dependency or require a Cargo.lock regeneration.

## Security boundary

The phone never receives direct MCP, Win32, shell, permission-broker, or Antigravity access.

It can only submit a bounded text command through the satellite adapter. The command then enters the same desktop Assistant Core, Antigravity/MCP, permission, and tool boundaries as other assistant requests.

Current security properties:

- no active pairing token -> no LAN listener;
- first WebSocket application message must authenticate;
- authentication timeout is 5 seconds;
- protocol version is checked;
- client WebSocket frames must be masked;
- frame payload is bounded to 64 KiB;
- one active satellite phone connection is accepted at a time in this phase;
- one satellite command is processed at a time;
- commands are refused while another assistant turn is active;
- Sensitive actions still require the desktop permission path;
- the full pairing token is not shown by `assistant-satellite show`;
- `assistant-satellite revoke` invalidates the persisted pairing secret and disconnects the active satellite after hot reload.

The pairing token is a local credential. The current bootstrap stores it in the user's application-data directory and in Android app preferences; do not publish the file/token in logs, screenshots, bug reports, or repositories. Windows DPAPI/Android Keystore-backed secret storage and per-device credentials remain later hardening work.

The current LAN transport is `ws://`, not encrypted `wss://`. Use it only on a trusted LAN during this phase. Do not expose port 8765 directly to the public Internet. WSS/Tailscale and stronger per-device trust are later phases.

## Protocol v1

### 1. Authenticate

The first application message after WebSocket upgrade:

```json
{
  "type": "hello",
  "token": "replace-with-pairing-token",
  "device_name": "Google Pixel",
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

### 2. Submit final recognized text

```json
{
  "type": "command",
  "id": "2f765145-e6fd-4bb6-ae19-e1da2df383b9",
  "text": "Mở Visual Studio Code",
  "response_language": "vi"
}
```

`response_language` accepts:

- `vi` — generate the desktop response in Vietnamese;
- `en` — generate it in English;
- `auto` — respond in the language used by the user.

The response policy asks the assistant to be friendly, natural, and concise. Simple successful commands should normally produce one short confirmation sentence.

### 3. Runtime state

```json
{
  "type": "state",
  "id": "2f765145-e6fd-4bb6-ae19-e1da2df383b9",
  "state": "processing"
}
```

Then:

```json
{
  "type": "state",
  "id": "2f765145-e6fd-4bb6-ae19-e1da2df383b9",
  "state": "speaking"
}
```

### 4. Response

```json
{
  "type": "response",
  "id": "2f765145-e6fd-4bb6-ae19-e1da2df383b9",
  "text": "Được, mình đã mở Visual Studio Code.",
  "tts_error": null
}
```

The phone may display this text, but desktop SAPI is responsible for speaking it.

### Error example

```json
{
  "type": "error",
  "id": "2f765145-e6fd-4bb6-ae19-e1da2df383b9",
  "code": "assistant_busy",
  "message": "desktop assistant is busy with another turn"
}
```

## Desktop TTS language note

`VI`, `EN`, and `Auto` currently control the language of the generated response text.

The desktop still uses its current Windows SAPI voice instance. Automatic enumeration/selection of an installed Vietnamese or English SAPI voice is a follow-up phase. Therefore this phase does not claim locale-perfect TTS voice selection yet.

## Android source

Android source lives at:

```text
apps/android-satellite/
```

The current source is a Kotlin + Jetpack Compose push-to-talk client using Android `SpeechRecognizer` and an OkHttp WebSocket client.

No Gradle wrapper binary is committed in this source-first phase. Open the directory in Android Studio for local sync/build, or generate a wrapper locally if command-line Android builds are desired.

## Phase 26 status

Implemented in the pairing bootstrap:

- durable `satellite.json` settings;
- generated 64-character pairing secret;
- `assistant-satellite show/pair/enable/disable/bind/revoke`;
- token masking in status output;
- live settings polling/hot reload;
- active-session drop on token rotation/revoke;
- legacy environment override compatibility;
- one active phone at a time.

Still planned in Phase 26:

- fold the helper under the main `assistant satellite ...` CLI surface;
- QR pairing so the user does not type endpoint/token manually;
- per-device trusted-device registry;
- connected-device/last-seen diagnostics;
- Windows Firewall/private-network setup guidance or safe automation;
- stronger secret-at-rest storage.

## Local verification checklist

Do this on the user's own Windows/Android devices; no remote Actions/build/test run is implied by the source work.

1. Put phone and PC on the same trusted LAN.
2. Build `assistant-satellite` locally and run `assistant-satellite pair` while the desktop runtime is running.
3. Confirm the desktop log reports the listener within roughly one second without restarting the app.
4. Allow Windows Firewall access only for the intended trusted/private network profile.
5. Enter `ws://<PC-LAN-IP>:8765` and the printed token on Android.
6. Grant microphone permission.
7. Test `vi-VN`: `Mở Visual Studio Code`.
8. Confirm partial recognition appears only on Android.
9. Confirm exactly one final command reaches Quick/Assistant Core.
10. Confirm the Windows action executes through the existing MCP/permission path.
11. Confirm desktop SAPI speaks the response.
12. Switch response language among `VI`, `EN`, and `Auto`.
13. Test `en-US` recognition.
14. Test invalid pairing token, disconnected Wi-Fi, desktop busy state, and reconnect.
15. Run `assistant-satellite revoke` while the phone is connected; confirm the connection is dropped within roughly one second and cannot reconnect with the old token.
16. Run `assistant-satellite pair` again and confirm the new token works without restarting the desktop.
17. Confirm local desktop microphone/Zipformer still works as fallback when built with the voice feature.
