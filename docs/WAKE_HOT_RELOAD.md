# Wake Phrase Lifecycle and Hot Reload

Phase 13D removes the application-restart requirement introduced by Phase 13C and gives wake configuration a persistent lifecycle.

## Goals

- create a wake phrase when the app started without `keywords.txt`;
- replace an existing wake phrase without restarting the desktop;
- keep the existing `wake:event` subscription alive across detector creation/reload;
- preserve enabled/disabled state;
- persist user-facing wake settings;
- roll back `keywords.txt` when native detector validation/hot reload fails.

## Stable service architecture

The desktop owns one `WakeService` for the whole process lifetime.

```text
Tauri / App.tsx
      ↓
WakeService stable event relay
      ↓
Optional WakeRuntimeHandle
      ↓
WakeRuntime worker
      ↓
Sherpa detector
```

The stable relay exists even when the application starts with missing wake resources. Therefore the main WebView may subscribe once during startup and still receive events when a runtime is created later in the same session.

## Runtime detector reload

`WakeRuntimeHandle` now supports:

```text
reload(Box<dyn WakeWordDetector>)
```

Internally the worker receives a `Reload` command with a one-shot acknowledgement.

```text
new detector
    ↓
worker receives Reload
    ↓
replacement.reset()
    ↓
reset fails ──→ keep old detector / return error
    ↓ success
replace detector
    ↓
acknowledge
    ↓
drop/reopen microphone when active
```

The handle and event broadcaster do not change, so no frontend/MCP/Tauri re-subscription is required.

## First-time runtime creation

If the desktop started before the wake resources were complete, `WakeService` initially has no runtime handle but its stable event relay still exists.

After the user generates a valid keyword file:

```text
complete model + new keywords
       ↓
load SherpaWakeWordDetector
       ↓
WakeService.reload_or_start()
       ↓
no handle exists
       ↓
spawn WakeRuntime
       ↓
relay its events into stable service broadcaster
```

This makes the wake capability available in the same application session.

## Transactional keyword replacement

Phase 13D permits replacing an existing `keywords.txt`.

The operation is intentionally transactional:

```text
prepare + tokenize phrase
       ↓
write unique .part
       ↓
flush + sync_all
       ↓
old keywords → unique .bak   (if present)
       ↓
.part → keywords.txt
       ↓
load native Sherpa detector from final keywords.txt
       ↓
load fails → remove new file + restore .bak
       ↓
WakeRuntime hot reload
       ↓
reload fails → remove new file + restore .bak
       ↓ success
remove .bak
```

The old in-memory detector continues operating until the replacement detector is accepted by the runtime worker.

## Persisted settings

Wake preferences are stored at:

```text
<app-local-data>/settings/wake.json
```

Schema:

```json
{
  "enabled": false,
  "phrase": "HEY ASSISTANT"
}
```

`ASSISTANT_WAKE_ENABLED`, when explicitly present, remains a startup override for the persisted `enabled` value.

The UI-facing `WakeStatus` also exposes the persisted phrase so the Resources panel can show the current configuration rather than a hard-coded default.

## Settings failure semantics

Runtime success is more important than settings persistence.

If a wake toggle or detector reload succeeds but writing `wake.json` fails:

- the runtime change remains active;
- the in-memory preference remains current for the process lifetime;
- the command still returns success;
- `WakeStatus.detail` reports a persistence warning.

This avoids rolling back a valid detector/file merely because a non-critical settings write failed.

## Enabled state

`set_enabled()` now persists successful user toggle changes.

On the next startup:

```text
ASSISTANT_WAKE_ENABLED set?
      ├── yes → environment value wins
      └── no  → persisted wake.json.enabled
```

Delayed resume still cannot re-enable a runtime that the user disabled.

## UI lifecycle

`Readiness → Resources → Wake Word Resources` now supports both:

```text
Tạo + nạp ngay
Cập nhật + nạp ngay
```

Hot loading is enabled only when:

- the build contains `wake-word`;
- encoder/decoder/joiner/tokens are present;
- `bpe.model` is present;
- the phrase is non-empty;
- no keyword update is currently running.

After success, Resource Registry, Readiness and the main wake status are refreshed. No desktop restart is required.

## Security and privacy

Wake phrase replacement remains local-only.

It does not:

- expose an MCP tool;
- call Gemini/Antigravity;
- accept a network URL;
- execute shell/PowerShell;
- auto-download the wake model archive;
- expose microphone data outside local wake processing.

## Local Windows verification checklist

1. Start with the wake feature compiled but without `keywords.txt`.
2. Open Resources and generate `HEY ASSISTANT`.
3. Confirm Wake becomes available without restarting the app.
4. Enable wake and confirm the persisted `enabled` value appears in `settings/wake.json`.
5. Change the phrase to another supported English phrase.
6. Confirm `keywords.txt` changes and the runtime stays alive.
7. Confirm the old phrase no longer triggers and the new phrase does.
8. Restart the app and verify phrase + enabled state are restored.
9. Set `ASSISTANT_WAKE_ENABLED=0` and confirm the environment override wins at startup.
10. Force a settings-write failure and confirm runtime operation succeeds while `WakeStatus.detail` reports the persistence issue.
11. Force a detector-load/reload failure and verify the previous keyword file is restored.
12. Confirm no `.part` or `.bak` files remain after successful updates.

Remote development does not execute native builds, runtime tests, model downloads or GitHub Actions.
