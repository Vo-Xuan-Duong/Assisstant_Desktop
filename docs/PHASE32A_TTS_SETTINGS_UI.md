# Phase 32A — Desktop TTS Settings UI

## Goal

Expose the existing Windows SAPI Vietnamese/English voice preferences in the desktop Quick UI without creating a second settings format or weakening the desktop authority boundary.

The existing terminal commands remain supported:

```powershell
assistant tts voices
assistant tts show
assistant tts set vi <voice>
assistant tts set en <voice>
assistant tts clear <vi|en|all>
```

Phase 32A adds a bounded graphical path for the common voice-selection operation.

## Architecture

```text
Quick Voice panel
  |- Satellite tab -> existing read-only readiness snapshot
  `- TTS tab
       |
       | assistant_tts_settings
       | assistant_tts_set_voice
       v
bounded Tauri TTS settings service
       |
       |- blocking worker -> enumerate installed SAPI voice tokens
       `- load/save existing tts.conf
                 |
                 v
          WindowsSapiTts
          reloads preferences
          on next utterance
```

The desktop UI and `WindowsSapiTts` runtime resolve preferences through the same `default_voice_preferences_path()` helper, so they operate on the same runtime `tts.conf`. The `assistant tts ...` CLI has an equivalent default `%LOCALAPPDATA%` resolver and therefore points to that same file during normal use, while intentionally retaining its explicit `--data-dir` override for management workflows.

SAPI enumeration is executed on a Tokio blocking worker instead of the Tauri/WebView command thread. This keeps COM initialization on a dedicated worker thread and avoids inheriting an incompatible COM apartment from the UI host.

## Desktop UX

The former standalone **Satellite** trigger becomes a **Voice** trigger with two tabs:

- **Satellite** — preserves the existing read-only connection/device diagnostics;
- **TTS** — selects the preferred Vietnamese and English SAPI voices.

Each language can be set to:

- **Tự động theo locale** — clears the explicit preference and returns to locale/default fallback;
- any SAPI voice currently installed for the Windows user.

Voices reporting the expected locale token are ordered first for convenience, but the UI does not hide other installed voices. This preserves parity with the CLI and avoids making assumptions about vendor-specific SAPI metadata.

If a previously selected voice has been removed from Windows, the UI reports the stale preference. The speech runtime already falls back safely when the preferred token cannot be resolved.

Changes apply from the next spoken response because `WindowsSapiTts` reloads voice preferences per utterance.

## Security boundary

The WebView cannot provide an arbitrary settings path, registry path, command, executable, or shell argument.

The mutating command accepts only:

```text
language = vi | en
voice_id = null | exact id from the currently enumerated installed SAPI voices
```

A non-installed token ID is rejected before persistence.

Phase 32A does not:

- expose the Android Satellite pairing token;
- add Satellite pairing/revoke/firewall/Tailscale write authority to the WebView;
- change MCP permissions or Sensitive-action confirmation behavior;
- execute `assistant-tts.exe` through a shell;
- write directly to the Windows registry.

The existing Satellite tab remains read-only.

## Failure behavior

- missing/unreadable SAPI enumeration -> TTS tab shows an error;
- malformed `tts.conf` -> error is surfaced instead of silently overwriting it;
- a requested non-installed voice -> rejected;
- a previously saved voice later removed from Windows -> UI warns and runtime uses normal fallback;
- refresh and save failures do not affect Satellite diagnostics or Assistant text operation.

## Local acceptance

Validate on the target Windows installation using the normal/default application-data path unless explicitly testing a CLI `--data-dir` override:

1. **Voice -> TTS** lists the same installed voices as `assistant tts voices`.
2. Existing `assistant tts show` preferences are reflected in the selectors.
3. Selecting a Vietnamese voice updates `assistant tts show` and is used on the next Vietnamese response.
4. Selecting an English voice updates `assistant tts show` and is used on the next English response.
5. Selecting **Tự động theo locale** clears that explicit preference.
6. Removing a selected Windows voice produces the stale-selection warning and speech falls back safely.
7. A malformed `tts.conf` surfaces an error without crashing the Quick surface or overwriting the file.
8. Opening/interacting with the Voice panel holds Quick auto-dismiss until the panel is closed.
9. Satellite diagnostics still never exposes the pairing token.

## Status

**COMPLETE IN SOURCE; LOCAL WINDOWS VALIDATION REQUIRED.**

No native build, SAPI execution, installer run, or GitHub Action is required as part of the repository implementation step. Target-machine validation remains local according to the project validation policy.
