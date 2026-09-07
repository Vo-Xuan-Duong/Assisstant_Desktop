# Assistant Voice Satellite for Android

This Android app is the preferred speech-input surface for Assisstant Desktop.

It listens through Android `SpeechRecognizer`, shows partial recognition locally, and sends only the final recognized text to Windows. The phone does not execute Windows tools, run MCP, call Antigravity directly, or speak the assistant response.

## Current capabilities

- Kotlin + Jetpack Compose;
- Android 8.0+ (`minSdk 26`);
- `vi-VN` and `en-US` speech recognition;
- Android on-device recognizer when available;
- system `SpeechRecognizer` fallback;
- push-to-talk;
- authenticated WebSocket connection over trusted LAN;
- stable per-installation `device_id`;
- desktop trusted-device registry and per-device revoke;
- response modes `VI`, `EN`, `Auto`;
- desktop owns reasoning, Windows tools, permissions, and TTS.

## Build locally

Open:

```text
apps/android-satellite/
```

in Android Studio, sync Gradle, then build/install on the target phone.

This source-first phase does not commit the Gradle wrapper JAR/binary. Generate a wrapper locally if command-line builds are required. No remote build or GitHub Actions validation is required for this phase.

## Pair with Windows

On the PC:

```powershell
assistant-satellite pair
```

This generates a random 64-character shared pairing token and enables the LAN receiver. Persistent configuration is stored at:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\settings\satellite.json
```

Enter on Android:

```text
ws://<PC-LAN-IP>:8765
```

and the token printed by `assistant-satellite pair`.

The Windows runtime watches pairing settings and normally applies changes within about one second without a restart.

Useful commands:

```powershell
assistant-satellite show
assistant-satellite pair
assistant-satellite enable
assistant-satellite disable
assistant-satellite bind 0.0.0.0:8765
assistant-satellite revoke
assistant-satellite devices
assistant-satellite revoke-device <device-id>
assistant-satellite allow-device <device-id>
```

## Trusted-device identity

The Android app generates a UUID once per installation and stores it in private `SharedPreferences`. It sends that `device_id` in the WebSocket `hello` message together with the pairing token and device name.

After the token is verified, Windows records the device in:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\settings\satellite-devices.json
```

Run:

```powershell
assistant-satellite devices
```

to see registered devices and their first/last-seen Unix timestamps.

To block only one phone:

```powershell
assistant-satellite revoke-device <device-id>
```

A connected revoked phone should be disconnected by Windows within roughly one second and cannot reconnect even if it still knows the current shared token.

To allow it again:

```powershell
assistant-satellite allow-device <device-id>
```

Reinstalling/clearing app storage can create a new installation identity. Device-specific QR credentials are planned next.

## Using the app

1. Put the phone and PC on the same trusted Wi-Fi/LAN.
2. Pair from Windows if necessary.
3. Enter the desktop WebSocket address and pairing token.
4. Tap **Kết nối**.
5. Choose **Tiếng Việt** or **English** recognition.
6. Choose desktop response language **VI**, **EN**, or **Auto**.
7. Leave **Ưu tiên nhận dạng on-device** enabled if desired.
8. Tap **Nói với Assistant** and speak.
9. Partial recognition stays on the phone.
10. Only the final text is submitted to Windows.
11. Windows processes the command through Assistant Core/Antigravity/MCP/permissions and speaks the answer through desktop TTS.

## Speech-recognition behavior

When on-device recognition is preferred, the app uses Android's on-device recognizer only when the platform reports it available. Otherwise it falls back to the normal system recognizer.

The normal recognizer is vendor-dependent. On phones using Google services it may be backed by Google's speech-recognition service and may require Internet. No dedicated paid STT API key is required by this project.

`SpeechRecognizer` is not kept running continuously; always-on wake/hardware activation is a later phase.

## Security notes

The current transport is unencrypted `ws://` and is intended only for a trusted/private LAN. Do not expose port `8765` directly to the public Internet.

The phone is low-authority: it cannot bypass desktop permission checks or approve Sensitive actions. The shared token is still a local credential and should not be published in logs/screenshots.

For development compatibility, `ASSISTANT_VOICE_SATELLITE_TOKEN` and `ASSISTANT_VOICE_SATELLITE_BIND` remain supported as environment overrides.

See [`../../docs/VOICE_SATELLITE.md`](../../docs/VOICE_SATELLITE.md) for protocol and security details.
