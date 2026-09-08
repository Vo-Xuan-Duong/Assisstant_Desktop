# Phase 27C — Selectable Windows SAPI Voices

## Goal

Allow the user to inspect installed Windows SAPI voices and choose separate preferred voices for Vietnamese and English without restarting Assisstant Desktop.

## Runtime behavior

`voice-runtime` enumerates SAPI through the existing `SpVoice` COM object:

```text
ISpeechVoice::GetVoices
  -> ISpeechObjectTokens::Count / Item
  -> ISpeechObjectToken::Id
  -> GetDescription
  -> GetAttribute("Language")
```

The stable SAPI token `Id` is persisted. Display names and list indexes are only UI/CLI conveniences.

Before every utterance, `WindowsSapiTts` loads:

```text
%LOCALAPPDATA%\com.voduong.assisstantdesktop\settings\tts.json
```

(or `ASSISTANT_APP_DATA/settings/tts.json`, or explicit `ASSISTANT_TTS_SETTINGS_PATH`).

This makes preference changes effective on the next spoken response without a runtime restart or management IPC mutation.

Example schema:

```json
{
  "vietnamese_voice_id": "<stable SAPI token id>",
  "english_voice_id": "<stable SAPI token id>"
}
```

## CLI

Canonical commands:

```powershell
assistant tts voices
assistant tts voices --json
assistant tts show
assistant tts set vi <voice-index-or-id>
assistant tts set en <voice-index-or-id>
assistant tts clear vi
assistant tts clear en
assistant tts clear all
```

`assistant tts set vi 2` resolves index `2` immediately and persists the voice token ID. A later reordering of the installed voice list therefore does not silently change the configured voice.

## Fallback

For each utterance:

1. resolve `VI` / `EN` / `Auto` to a concrete language;
2. if a configured preferred token is installed, set it with `ISpeechVoice::putref_Voice`;
3. if the token disappeared or cannot be selected, restore the initial/default SAPI voice;
4. still wrap the utterance in SAPI `<lang>` markup so locale-based selection can choose an installed matching voice.

A malformed/missing `tts.json` does not make speech fail closed; the runtime logs a warning and uses the existing locale/default behavior for that utterance.

## Packaging

The Windows bundle now contains `assistant-tts` next to the canonical router. `assistant.exe` routes only the `tts` namespace to that helper, just as the `satellite` namespace routes to `assistant-satellite`.

## Local validation gate

No remote build/test/Actions/installer/SAPI execution is performed.

Validate locally on Windows:

1. `assistant tts voices` enumerates installed voices;
2. `assistant tts set vi <index>` writes a stable token id;
3. Vietnamese satellite response uses that voice where SAPI supports it;
4. `assistant tts set en <index>` similarly selects English;
5. removing/renaming a configured voice does not break TTS;
6. `assistant tts clear all` restores automatic locale/default behavior;
7. existing Quick read-aloud and desktop fallback voice paths still use the same preference file.
