# Quick Read Aloud

Quick response actions expose the existing native `assistant_speak` command as a compact **Đọc** action beside Copy.

## Runtime path

```text
QuickResponseActions
      |
      | speakResponse(text)
      v
assistant_speak
      |
      +-- require AssistantState::Idle
      +-- suspend WakeService
      +-- AssistantCore -> Speaking
      +-- WindowsSapiTts::speak
      +-- AssistantCore -> Idle
      +-- resume wake after cooldown
```

The frontend does not use the browser Web Speech API and does not create a second TTS implementation.

## UI behavior

- the action appears only when a completed response is available;
- it is enabled only while the Assistant is `Idle`;
- while native TTS is active, the action is disabled and shows `Đang đọc`;
- a command failure displays a short local `Lỗi` state without discarding the response;
- auto-dismiss already pauses during `Speaking` because it follows Assistant state;
- clicking/hovering response actions also pauses auto-dismiss through the existing Quick pointer policy.

## Responsive layout

Copy and Read share one compact action row. On narrow Quick windows (`<= 520px`) labels are hidden and both controls become icon-only. The recent-response History toggle moves to keep clear separation from the expanded action row.

## Verification

Verify locally on Windows:

1. Complete a text turn and click `Đọc`; Windows SAPI should read the response once.
2. Confirm Assistant state becomes `Speaking` and returns to `Idle` afterward.
3. Confirm wake detection is suspended during playback and resumes after the existing cooldown.
4. Confirm the action cannot start another playback while the Assistant is already busy.
5. Force a TTS failure and confirm the response remains visible while the action briefly shows an error state.
6. Test both wide and minimum-width Quick layouts for overlap between History, Copy, Read, Stop, and Close controls.
7. Confirm auto-dismiss does not hide Quick during native TTS playback.

No build/test/action was run as part of this source-only phase.
