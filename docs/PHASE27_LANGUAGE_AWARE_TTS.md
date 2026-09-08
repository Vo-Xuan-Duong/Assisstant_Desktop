# Phase 27 — Language-aware Windows TTS

## Goal

Make desktop spoken responses use a Windows SAPI voice matching Vietnamese or English whenever the corresponding voice is installed, while preserving the existing cancellable SAPI worker and falling back safely when a matching voice is unavailable.

## Current implementation

`voice-runtime::tts` now exposes:

```rust
pub enum TtsLanguage {
    Auto,
    Vietnamese,
    English,
}
```

and a backwards-compatible language-aware trait method:

```rust
TextToSpeech::speak_with_language(text, language)
```

Existing callers that only use `speak(text)` continue to work. `WindowsSapiTts::speak()` resolves `Auto` from the generated response text.

## SAPI language selection

The SAPI worker wraps text in SAPI XML before speaking:

```xml
<lang langid="42A">...</lang>
```

for Vietnamese (`vi-VN`), or:

```xml
<lang langid="409">...</lang>
```

for U.S. English (`en-US`).

The speak call includes `SVSFIsXML` together with the existing asynchronous and purge-before-speak flags.

Microsoft SAPI defines the `Lang` tag as voice selection based solely on the installed voice's `Language` attribute. If no installed voice matches the requested language, SAPI leaves the current voice unchanged. This gives the desktop a natural fallback to the configured/default voice rather than failing merely because a locale-specific voice is absent.

## XML safety

Assistant response text is escaped before entering SAPI XML. At minimum these characters are encoded:

- `&` -> `&amp;`
- `<` -> `&lt;`
- `>` -> `&gt;`
- `"` -> `&quot;`
- `'` -> `&apos;`

Model output therefore cannot accidentally become SAPI markup through normal response text.

## Auto mode

For the current VI/EN product scope, `Auto` uses a conservative text heuristic:

- Vietnamese-specific letters/diacritics -> `vi-VN`;
- otherwise -> `en-US`.

This already improves all existing `WindowsSapiTts::speak()` callers without forcing a broad desktop refactor.

The explicit `speak_with_language()` API exists so the satellite's `VI / EN / Auto` selection can be wired directly in a follow-up source step. Until that explicit hint is threaded through the satellite adapter, normal Vietnamese model responses are expected to contain Vietnamese diacritics and therefore resolve to `vi-VN` automatically.

## Existing cancellation behavior

This phase does not replace the persistent SAPI COM worker or barge-in/cancellation path.

The sequence remains:

```text
Speak
  -> async SAPI stream
  -> WaitUntilDone polling
  -> Cancel command
  -> purge pending/current SAPI speech
  -> sentence Skip fallback if purge fails
```

## Local verification checklist

No remote build/test/Actions execution is implied by this source phase.

On the user's Windows machine:

1. Build the desktop/voice runtime locally.
2. Confirm an English SAPI voice is installed.
3. Install/enable a Vietnamese Windows speech voice if one is available on the target Windows edition.
4. Trigger an English response and confirm an English-capable voice is selected.
5. Trigger a Vietnamese response containing Vietnamese diacritics and confirm SAPI requests `vi-VN`.
6. Test XML-sensitive text such as `A & B < C` and verify it is spoken as text rather than parsed as markup.
7. Test a machine without a Vietnamese SAPI voice and confirm speech still falls back instead of failing solely due to missing locale voice.
8. Test Stop/barge-in during both Vietnamese and English speech.
9. Verify rate and volume behavior remain unchanged.

## Follow-up

The next incremental improvement is to pass the Android satellite's explicit `response_language` value into `speak_with_language()` so `VI`, `EN`, and `Auto` no longer need to rely on response-text inference at the final TTS boundary.
