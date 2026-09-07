# Quick Read Aloud

Quick response actions expose the existing native `assistant_speak` command as a compact **Đọc** action beside Copy. Recent Responses reuses the same path for each stored item.

## Runtime path

```text
QuickResponseActions / Recent Responses
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

## Interruptible SAPI worker

`WindowsSapiTts` owns a dedicated COM worker thread. `ISpeechVoice` is created and used only on that thread.

Speech starts asynchronously with SAPI and the worker polls completion in short intervals. While the Assistant is `Speaking`, Quick shows the same explicit Stop control used for long AI processing.

```text
Stop
  -> quick:cancel_request
  -> WindowsSapiTts::cancel
  -> COM worker
  -> purge active SAPI utterance
  -> assistant_speak finishes Speaking
  -> Idle
```

A successful user-requested purge is normal completion, not an error. The response remains visible and history remains intact.

## UI behavior

- `Đọc` appears only when a completed response is available;
- it is enabled only while the Assistant is `Idle`;
- while native TTS is active, the action is disabled and shows `Đang đọc`;
- Quick header shows Stop while state is `Speaking`;
- pressing Stop changes the header to `Đang dừng đọc…` until native runtime returns to `Idle`;
- a real TTS command/purge failure displays a short local `Lỗi` state without discarding the response;
- auto-dismiss pauses during `Speaking` because it follows Assistant state;
- clicking/hovering response actions also pauses auto-dismiss through the existing Quick pointer policy.

## Responsive layout

Copy and Read share one compact action row. On narrow Quick windows (`<= 520px`) labels are hidden and controls become icon-only. The recent-response History toggle moves to keep clear separation from the action row.

## Verification

Verify locally on Windows:

1. Complete a text turn and click `Đọc`; Windows SAPI should read the response once.
2. Confirm Assistant state becomes `Speaking` and returns to `Idle` afterward.
3. While it is speaking, press Stop and confirm audio stops promptly without showing a false TTS error.
4. Confirm the response remains visible/copyable after Stop.
5. Confirm wake detection is suspended during playback and resumes after the existing cooldown, including after Stop.
6. Confirm the action cannot start another playback while the Assistant is already busy.
7. Open Recent Responses, read one stored item, then press Stop; only audio should stop and the history item should remain.
8. Press Stop immediately after `Speaking` first appears and confirm the registration-race retry still reaches the SAPI worker.
9. Force a genuine TTS failure and confirm the response remains visible while the action briefly shows an error state.
10. Test both wide and minimum-width Quick layouts for overlap between History, Copy, Read, Stop, and Close controls.
11. Confirm auto-dismiss does not hide Quick during native TTS playback.
12. Press `Esc` during speech and confirm it hides Quick without stopping the audio; Stop remains the explicit cancellation action.

No build/test/action was run as part of this source-only phase.
