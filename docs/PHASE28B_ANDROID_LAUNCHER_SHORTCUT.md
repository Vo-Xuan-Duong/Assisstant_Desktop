# Phase 28B — Android Launcher Voice Shortcut

## Goal

Add another low-friction activation surface without introducing a foreground service, notification permission, or an always-listening recognizer.

Android publishes one static launcher shortcut:

```text
Nói AI
```

Long-press the **Assistant Voice Satellite** app icon and select the shortcut. Supported launchers may also let the user drag/pin that shortcut onto the home screen.

## Flow

```text
Launcher shortcut
  -> VoiceShortcutActivity (NoDisplay trampoline)
  -> MainActivity + EXTRA_START_VOICE=true
  -> existing pending voice activation flow
  -> reconnect if necessary
  -> safe cancel if desktop is Processing/Speaking
  -> microphone permission check
  -> SpeechRecognizer
```

The trampoline does not perform pairing, networking, cancellation, recognition, AI, MCP, or Windows actions itself.

## Why a trampoline activity

Android static shortcuts launch their first intent with task-clearing behavior. The dedicated `VoiceShortcutActivity` uses `android:taskAffinity=""`, immediately forwards to the existing singleTop `MainActivity`, and finishes. This preserves the app's normal activity lifecycle instead of giving shortcut launch its own voice implementation.

## Security / privacy

- no pairing token is embedded in shortcut metadata;
- no desktop endpoint is embedded in shortcut metadata;
- no extra Android permission is added;
- the shortcut cannot bypass microphone permission;
- the shortcut cannot bypass pairing/trusted-device checks;
- Sensitive Windows actions remain desktop-confirmation gated;
- if the desktop is already Processing/Speaking, existing safe cancellation acknowledgement is reused before a new speech turn begins.

## Files

```text
apps/android-satellite/app/src/main/AndroidManifest.xml
apps/android-satellite/app/src/main/java/com/assisstant/voicesatellite/VoiceShortcutActivity.kt
apps/android-satellite/app/src/main/res/xml/shortcuts.xml
apps/android-satellite/app/src/main/res/values/strings.xml
```

## Local validation

1. Install the Android app.
2. Long-press the launcher icon and confirm **Nói AI** appears.
3. Trigger it with the desktop disconnected and verify the normal reconnect path is used.
4. Trigger it while desktop is idle and verify recognition starts only with microphone permission.
5. Trigger it while desktop is Processing/Speaking and verify safe cancellation happens first.
6. Confirm no token/endpoint appears in launcher shortcut metadata.
7. Confirm returning/back-stack behavior does not create duplicate MainActivity instances.

No remote Android build/test or device execution was performed for this source phase.
