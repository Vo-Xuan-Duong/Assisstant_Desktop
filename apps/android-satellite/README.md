# Assistant Voice Satellite for Android

This Android app is the preferred speech-input surface for Assisstant Desktop.

It does three things only:

1. listen to the user through Android `SpeechRecognizer`;
2. show partial recognition locally on the phone;
3. send only the final recognized text to the paired Windows desktop.

The phone does not execute Windows tools, run MCP, call Antigravity, or speak the assistant response. Those responsibilities remain on the desktop.

## Current MVP

- Kotlin + Jetpack Compose;
- Android 8.0+ (`minSdk 26`);
- Vietnamese `vi-VN` and English `en-US` recognition;
- prefers Android on-device speech recognition when available on Android 12+;
- falls back to the device/system `SpeechRecognizer` service;
- push-to-talk interaction;
- authenticated WebSocket connection to the desktop;
- response language selector: `VI`, `EN`, or `Auto`;
- desktop response text/status display;
- desktop remains responsible for TTS.

## Build locally

Open this directory in Android Studio:

```text
apps/android-satellite/
```

Sync the Gradle project and build/install it on the Android device from Android Studio.

This source-first phase does not commit the Gradle wrapper JAR/binary. If command-line builds are required, generate the Gradle wrapper locally from a compatible Gradle/Android Studio installation.

No remote build or GitHub Actions validation is required by this phase.

## Desktop setup

Before starting the desktop application, configure a pairing token:

```powershell
$env:ASSISTANT_VOICE_SATELLITE_TOKEN = "replace-with-a-random-32-plus-character-token"
$env:ASSISTANT_VOICE_SATELLITE_BIND = "0.0.0.0:8765"
pnpm desktop:dev
```

Find the PC's LAN IPv4 address, then enter on Android:

```text
ws://<PC-LAN-IP>:8765
```

Use exactly the same pairing token on both devices.

The current MVP uses unencrypted `ws://` and is intended only for a trusted LAN. Do not expose port 8765 directly to the public Internet.

## Using the app

1. Connect the phone and PC to the same trusted Wi-Fi/LAN.
2. Enter the desktop WebSocket address.
3. Enter the pairing token.
4. Tap **Kết nối**.
5. Choose recognition language: **Tiếng Việt** or **English**.
6. Choose desktop response language: **VI**, **EN**, or **Auto**.
7. Leave **Ưu tiên nhận dạng on-device** enabled if desired.
8. Tap **Nói với Assistant** and speak.
9. Partial text is shown on the phone while recognition is in progress.
10. Only the final recognized text is sent to the desktop.
11. The desktop processes the command and speaks the answer through Windows TTS.

## Recognition behavior

When `Ưu tiên nhận dạng on-device` is enabled, the app uses Android's on-device recognizer only when the platform reports that it is available. Otherwise it falls back to the normal system recognizer.

The normal system recognizer is device/vendor-dependent and may use an online service. The project does not require a paid speech API key for this path.

`SpeechRecognizer` is deliberately not kept listening continuously. Always-on voice activation will be implemented later with an appropriate wake-word/hardware-trigger mechanism.

## Response language

- `VI`: desktop generates a friendly Vietnamese response.
- `EN`: desktop generates a friendly English response.
- `Auto`: desktop responds in the language used in the command.

Simple commands are intentionally answered with short natural confirmations.

## Troubleshooting

If the phone cannot connect:

- confirm the PC and phone are on the same LAN;
- verify the PC LAN IP;
- verify the pairing token matches exactly;
- confirm the desktop runtime has `ASSISTANT_VOICE_SATELLITE_TOKEN` configured;
- check Windows Firewall for TCP port 8765 on the trusted/private network profile;
- do not use `127.0.0.1` or `localhost` on the phone, because those point to the phone itself.

See [`../../docs/VOICE_SATELLITE.md`](../../docs/VOICE_SATELLITE.md) for protocol and security details.
