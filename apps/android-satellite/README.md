# Assistant Voice Satellite for Android

This Android app is the preferred speech-input surface for Assisstant Desktop.

It listens through Android `SpeechRecognizer`, shows partial recognition locally, and sends only final recognized text to Windows. The phone does not execute Windows tools, run MCP, call Antigravity directly, approve Sensitive actions, or speak the Assistant response.

## Current capabilities

- Kotlin + Jetpack Compose;
- Android 8.0+ (`minSdk 26`);
- `vi-VN` and `en-US` speech recognition;
- Android on-device recognizer when available;
- system `SpeechRecognizer` fallback;
- push-to-talk;
- Quick Settings **Assistant Voice** tile;
- static launcher shortcut **Nói AI**;
- manual interrupt-to-talk while desktop is Processing/Speaking;
- optional **Hội thoại liên tục** turn-taking mode;
- automatic WebSocket reconnect with bounded exponential backoff;
- one bounded recognizer-busy retry and on-device -> system recognizer fallback;
- recognition diagnostics: engine, elapsed time, optional confidence, and up to two alternatives;
- completed desktop-turn timing through AI/tool/TTS response completion;
- authenticated WebSocket over trusted LAN or a Tailscale tailnet;
- QR/deep-link pairing;
- pairing token protected with Android Keystore AES-GCM;
- stable per-installation `device_id`;
- desktop trusted-device registry and per-device revoke;
- response modes `VI`, `EN`, `Auto`;
- desktop owns reasoning, permissions, Windows tools, and TTS.

## Build locally

Open:

```text
apps/android-satellite/
```

in Android Studio, sync Gradle, then build/install on the target phone.

The source-first project does not commit the Gradle wrapper JAR/binary. Generate a wrapper locally if command-line Android builds are required. Per project policy, native Android builds/tests are validated locally rather than through remote GitHub Actions during these phases.

## Pair on a trusted LAN

Recommended on Windows:

```powershell
assistant satellite pair --qr
```

If the PC has multiple network adapters:

```powershell
assistant satellite pair --qr --host 192.168.1.20
```

The QR is generated locally and contains a compact deep link:

```text
assd://p?h=<host>&p=<port>&t=<token>
```

No QR web service is used. Scanning imports the desktop endpoint/token but **does not connect automatically**; the user must still tap **Kết nối**.

Manual fallback:

```powershell
assistant satellite pair
```

then enter the printed token and:

```text
ws://<PC-LAN-IP>:8765
```

in the Android app.

## Pair for remote use through Tailscale

Do not port-forward the raw satellite port to the Internet.

Install/connect the normal Tailscale client on both Windows and Android and join the intended tailnet. On Windows:

```powershell
assistant satellite remote tailscale enable
assistant satellite remote tailscale pair --qr
```

The desktop backend is rebound to loopback and exposed through **Tailscale Serve**, not Funnel:

```text
Android + Tailscale
      |
 encrypted tailnet
      |
Tailscale Serve TCP
      |
127.0.0.1:<satellite-port>
      |
Assisstant Desktop
```

The Android app needs no Tailscale SDK. The remote QR simply imports the PC's Tailscale IPv4 endpoint plus the normal Assistant pairing token.

Inspect/disable remote mode with:

```powershell
assistant satellite remote tailscale show
assistant satellite remote tailscale disable
```

## Trusted-device identity

Android generates a UUID once per installation and sends it in the authenticated `hello` message with the pairing token and device name.

Windows records known devices in:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\settings\satellite-devices.json
```

Manage devices with:

```powershell
assistant satellite devices
assistant satellite revoke-device <device-id>
assistant satellite allow-device <device-id>
```

A connected revoked phone is rechecked by Windows and disconnected. Reinstalling/clearing app storage can create a new installation identity.

## Pairing-token storage

New/imported pairing tokens are not intentionally persisted as plaintext in the normal app preferences.

The app:

1. creates an AES key through `AndroidKeyStore`;
2. prefers AES-256 and falls back to AES-128 where necessary;
3. encrypts with `AES/GCM/NoPadding`;
4. stores IV + ciphertext in private preferences;
5. migrates legacy plaintext conservatively;
6. deletes legacy plaintext only after encrypted persistence succeeds.

If the Keystore state becomes unreadable after restore/reset/invalidation, the app reports the issue and the phone can be paired again.

Keystore protects persistence at rest; the token still exists in process memory while the app uses it.

## Normal voice flow

1. Pair and connect the phone.
2. Choose recognition language **Tiếng Việt** or **English**.
3. Choose desktop response mode **VI**, **EN**, or **Auto**.
4. Optionally keep **Ưu tiên nhận dạng on-device** enabled.
5. Start speech from the app, Quick Settings tile, or launcher shortcut.
6. Partial recognition stays on Android.
7. One final transcript is sent to Windows.
8. Windows processes it through Assistant Core -> Antigravity -> MCP -> permission gates.
9. Windows speaks the response through SAPI.

If Windows is already Processing/Speaking, the primary button becomes **Nói ngắt Assistant**. It opens an authenticated control connection, requests cancellation, waits for the desktop acknowledgement, and only then starts a fresh recognition turn. Executing/Confirming remain intentionally non-cancellable.

## Activation surfaces

### In-app

Tap **Nói với Assistant**.

### Quick Settings

Add the **Assistant Voice** tile from Android's Quick Settings editor.

Tapping it:

1. opens `MainActivity` through the platform-supported tile launch API;
2. never bypasses microphone permission;
3. reconnects to the paired desktop when needed;
4. safely cancels Processing/Speaking first when necessary;
5. starts recognition only when the desktop is ready.

Android 14+ uses the required `PendingIntent` launch API; Android 8-13 use the older Intent overload. Microphone permission must have been granted in the app first.

### Launcher shortcut

Long-press the **Assistant Voice Satellite** launcher icon and select **Nói AI**. Supported launchers may also allow dragging/pinning that shortcut to the home screen.

The static shortcut uses:

```text
shortcuts.xml
  -> VoiceShortcutActivity
  -> MainActivity + EXTRA_START_VOICE=true
```

`VoiceShortcutActivity` is a NoDisplay trampoline with empty task affinity. It contains no pairing/network/AI/tool logic and immediately forwards into the existing voice activation path.

The launcher shortcut therefore keeps the same pairing, microphone-permission, reconnect and safe-cancel requirements as the app and Quick Settings tile. No desktop endpoint or pairing token is stored in shortcut metadata.

See [`../../docs/PHASE28B_ANDROID_LAUNCHER_SHORTCUT.md`](../../docs/PHASE28B_ANDROID_LAUNCHER_SHORTCUT.md).

If **Hội thoại liên tục** is enabled, Quick Settings or launcher activation can also become the first turn of a conversational session.

## Voice diagnostics

Final recognition results expose read-only diagnostics on the phone:

```text
Bạn nói
Mở Visual Studio Code

STT: on-device · 842 ms · confidence 91%
Phương án khác: Mở Visual Studio Cốt · Mở VS Code
```

The app records:

- whether the final result came from the on-device or system recognizer;
- recognition elapsed time;
- the first confidence value only when Android supplies a valid score;
- up to two additional final recognition candidates.

The **first recognition candidate remains the exact command submitted to Windows**. Confidence does not auto-approve/reject a command, and alternatives are display-only. Android recognizers are allowed to omit confidence data.

The response area also displays a completed desktop-turn duration measured from successful command submission until the desktop `response` arrives after AI/tool/TTS processing:

```text
Desktop turn: 2380 ms (AI/tool/TTS đến khi response hoàn tất)
```

This is not a pure network RTT metric.

See [`../../docs/PHASE29B_VOICE_DIAGNOSTICS.md`](../../docs/PHASE29B_VOICE_DIAGNOSTICS.md).

## Conversational mode

Enable **Hội thoại liên tục** when you want turn-taking without tapping the microphone button after every response.

The mode does **not** immediately turn on the microphone. It becomes active when you start a normal voice turn.

Flow:

```text
Android listens
  -> final transcript
  -> desktop processes
  -> desktop TTS speaks
  -> TTS completes
  -> short 450 ms guard delay
  -> Android opens one follow-up recognition window
```

The desktop keeps using its existing Assistant session, so follow-up utterances naturally continue the same conversation context unless the desktop conversation is reset separately.

While active, Android shows **Kết thúc hội thoại**. Ending the session cancels the current/pending Android listening window but does not cancel a Windows task merely because no further follow-up is desired. Use **Dừng Assistant** for explicit safe desktop cancellation.

Conversation mode is bounded:

- speech timeout / `NO_MATCH` ends the conversation instead of looping forever;
- terminal audio/network/permission/recognizer errors end the conversation;
- losing the desktop connection ends the conversation;
- a TTS failure ends the conversation;
- on-device -> system recognizer fallback remains a non-terminal status and can continue normally.

Automatic follow-up recognition reuses in-memory preferences; it does not rewrite Android settings/Keystore on every follow-up window.

`SpeechRecognizer` is therefore still **not** used as a permanent always-listening loop.

## Connection resilience

Transient WebSocket losses reconnect with bounded backoff:

```text
1s -> 2s -> 4s -> 8s -> 15s max
```

Connection generations prevent callbacks from stale sockets from overwriting current state.

Authentication failure and device revocation stop automatic reconnect. Commands are **not automatically resent** after transport failure because a Windows side effect may already have executed even if its response was lost. Desktop request-ID deduplication remains an additional replay boundary.

An active conversational session is deliberately stopped on disconnect; it is not silently resumed later with the microphone automatically reopening after a delayed reconnect.

## Speech-recognition behavior

When on-device recognition is preferred, Android uses it only when the platform reports support. Otherwise it uses the system recognizer.

If an on-device engine becomes busy/disconnected/unavailable, the controller can perform one bounded fallback to the system recognizer. A busy recognizer can be recreated/retried once.

Recovery/fallback notifications are status events. Final failures such as `NO_MATCH`, speech timeout, microphone/network/permission/server errors are terminal for the current recognition window and stop an active conversational session.

The normal recognizer is vendor-dependent. On Google-enabled phones it may be backed by Google Speech Services and may require Internet. This project does not require a dedicated paid STT API key.

## Barge-in scope

Current mid-response barge-in is **explicit/manual**:

```text
Nói ngắt Assistant / Quick Settings / launcher activation
  -> cancel Processing or Speaking
  -> wait for desktop cancellation acknowledgement
  -> start a fresh SpeechRecognizer turn
```

Conversational follow-up is different: Android waits until desktop TTS finishes before listening again.

Automatic acoustic full-duplex barge-in is **not** claimed. Because the preferred microphone is on the phone while the desktop speaker produces the response, opening SpeechRecognizer during TTS can cause the phone to transcribe the Assistant itself. There is currently no synchronized far-end reference audio on the phone for a real echo canceller.

Do not replace that missing AEC architecture with RMS/VAD thresholds and call it full duplex.

## Useful Windows commands

```powershell
assistant satellite show
assistant satellite doctor
assistant satellite pair --qr
assistant satellite enable
assistant satellite disable
assistant satellite revoke
assistant satellite devices
assistant satellite revoke-device <device-id>
assistant satellite allow-device <device-id>
assistant satellite firewall show
assistant satellite firewall install
assistant satellite firewall remove
assistant satellite remote tailscale show
assistant satellite remote tailscale enable
assistant satellite remote tailscale pair --qr
assistant satellite remote tailscale disable
```

## Security notes

LAN mode is intended for trusted/private networks. The managed Windows firewall rule is restricted to the configured TCP port, Private profile, and LocalSubnet.

Remote mode uses Tailscale's encrypted tailnet and Tailscale Serve. The project does not configure Tailscale Funnel and should not expose raw port `8765` directly to the public Internet.

The pairing QR contains a credential. Treat QR/URI/token material as secret and do not publish it in screenshots or logs.

The phone remains low-authority: Windows permissions and Sensitive confirmations remain authoritative.

Development compatibility environment overrides remain available, but managed remote mode requires persisted settings to be authoritative.

See:

- [`../../docs/VOICE_SATELLITE.md`](../../docs/VOICE_SATELLITE.md)
- [`../../docs/PHASE28B_ANDROID_LAUNCHER_SHORTCUT.md`](../../docs/PHASE28B_ANDROID_LAUNCHER_SHORTCUT.md)
- [`../../docs/PHASE29B_VOICE_DIAGNOSTICS.md`](../../docs/PHASE29B_VOICE_DIAGNOSTICS.md)
- [`../../docs/PHASE30_TAILSCALE_REMOTE.md`](../../docs/PHASE30_TAILSCALE_REMOTE.md)
- [`../../docs/PHASE31_CONVERSATIONAL_VOICE.md`](../../docs/PHASE31_CONVERSATIONAL_VOICE.md)
