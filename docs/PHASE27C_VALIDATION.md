# Phase 27C local validation

Run on the target Windows machine only. No remote build/test/action is required by this phase.

```powershell
assistant tts voices
assistant tts show
assistant tts set vi 0
assistant tts set en 1
assistant tts show
assistant tts clear all
```

Confirm:

- installed SAPI voices are enumerated with stable token IDs;
- `settings\tts.conf` uses `version=1` and stores token IDs, not indexes;
- a Vietnamese satellite response uses the selected VI voice where supported;
- an English response uses the selected EN voice;
- removing a selected voice falls back without breaking TTS;
- Quick read-aloud and desktop fallback voice use the same preferences;
- cancellation still purges the active SAPI utterance.
