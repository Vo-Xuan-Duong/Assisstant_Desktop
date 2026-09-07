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

The system recognizer is implementation-dependent. On some devices it may use a vendor/cloud recognition service. Selecting `prefer on-device` asks the app to use Android's on-device recognizer when the platform reports it available.

The MVP is push-to-talk. `SpeechRecognizer` is not used as an always-listening loop. A future wake/hardware-trigger phase should activate recognition only when needed.

## Desktop listener

The desktop listener is disabled unless a pairing token is configured.

Environment variables:

```text
ASSISTANT_VOICE_SATELLITE_TOKEN
ASSISTANT_VOICE_SATELLITE_BIND
```

`ASSISTANT_VOICE_SATELLITE_TOKEN` is required and must contain at least 16 characters. Use a random value of 32 characters or more for real use.

`ASSISTANT_VOICE_SATELLITE_BIND` is optional. The default is:

```text
0.0.0.0:8765
```

Example PowerShell development session:

```powershell
$env:ASSISTANT_VOICE_SATELLITE_TOKEN = "replace-with-a-random-32-plus-character-token"
$env:ASSISTANT_VOICE_SATELLITE_BIND = "0.0.0.0:8765"
pnpm desktop:dev
```

The listener uses WebSocket protocol RFC 6455 and the satellite JSON protocol below. The WebSocket implementation is built on the existing Tokio runtime so this phase does not add a new Rust transport dependency or require a Cargo.lock regeneration.

## Security boundary

The phone never receives direct MCP, Win32, shell, permission-broker, or Antigravity access.

It can only submit a bounded text command through the satellite adapter. The command then enters the same desktop Assistant Core, Antigravity/MCP, permission, and tool boundaries as other assistant requests.

Current MVP security properties:

- no pairing token -> no LAN listener;
- first WebSocket application message must authenticate;
- authentication timeout is 5 seconds;
- protocol version is checked;
- client WebSocket frames must be masked;
- frame payload is bounded to 64 KiB;
- one satellite command is processed at a time;
- commands are refused while another assistant turn is active;
- Sensitive actions still require the desktop permission path.

The current LAN transport is `ws://`, not encrypted `wss://`. Use it only on a trusted LAN during this phase. Remote networking, certificate-based WSS, Tailscale, QR pairing, and trusted-device revocation are planned follow-up work.

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

The response policy also asks the assistant to be friendly, natural, and concise. Simple successful commands should normally produce one short confirmation sentence.

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

## Local verification checklist

Do this on the user's own Windows/Android devices; no remote Actions/build/test run is implied by the source work.

1. Put phone and PC on the same trusted LAN.
2. Set `ASSISTANT_VOICE_SATELLITE_TOKEN` on the desktop.
3. Start the desktop runtime and confirm the log says the listener is bound.
4. Allow Windows Firewall access only for the intended trusted network profile.
5. Enter `ws://<PC-LAN-IP>:8765` and the same token on Android.
6. Grant microphone permission.
7. Test `vi-VN`: `Mở Visual Studio Code`.
8. Confirm partial recognition appears only on Android.
9. Confirm exactly one final command reaches Quick/Assistant Core.
10. Confirm the Windows action executes through the existing MCP/permission path.
11. Confirm desktop SAPI speaks the response.
12. Switch response language among `VI`, `EN`, and `Auto`.
13. Test `en-US` recognition.
14. Test invalid pairing token, disconnected Wi-Fi, desktop busy state, and reconnect.
15. Confirm local desktop microphone/Zipformer still works as fallback when built with the voice feature.
