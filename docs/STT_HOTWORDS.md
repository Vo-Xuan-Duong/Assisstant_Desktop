# STT Contextual Hotwords

## Purpose

The Vietnamese Zipformer path supports contextual biasing for rare application names, project names, product names, and technical vocabulary without replacing the baseline recognizer.

The feature is intentionally opt-in. When no hotword file exists, speech recognition remains on the same low-overhead `greedy_search` path used before this feature.

## Runtime file

Create this file inside the installed Vietnamese Zipformer directory:

```text
hotwords.txt
```

Default location:

```text
%LOCALAPPDATA%/<Assisstant Desktop app-data>/models/stt/
  sherpa-onnx-zipformer-vi-30M-int8-2026-02-09/
    hotwords.txt
```

The model directory can still be overridden through `ASSISTANT_ZIPFORMER_MODEL_DIR`.

Use one phrase per line. Empty lines and lines beginning with `#` are ignored.

Example:

```text
# Applications and infrastructure
Visual Studio Code
DataGrip
SQL Server
Antigravity
Gemini
Cloudflare

# Project-specific vocabulary can be added here as well
```

These are examples only. The engine does not hard-code application or project names.

## Decode behavior

```text
hotwords.txt missing / empty / no valid phrases
        |
        v
existing greedy_search recognizer

hotwords.txt has valid phrases
        |
        v
validate with local bpe.model + tokens.txt
        |
        v
lazy contextual recognizer
modified_beam_search
modeling_unit=bpe
        |
        v
per-stream hotwords
```

The contextual recognizer is created lazily only after a valid hotword is observed. Its defaults are:

```text
hotwords_score   1.5
max_active_paths 4
```

The baseline greedy recognizer remains loaded and is used whenever contextual preparation is unavailable.

## BPE vocabulary

Sherpa's BPE hotword encoder requires `bpe.vocab`, while the pinned Vietnamese package provides `bpe.model` instead.

On the first contextual decode the runtime derives:

```text
bpe.vocab.generated
```

from the pinned local `bpe.model`. The export reads only the SentencePiece protobuf vocabulary entries (`piece`, `score`) and writes the two-column vocabulary expected by Sherpa. No Python process, network access, or additional model download is required.

The generated file is not part of baseline model readiness. If generation fails, the current utterance falls back to greedy decoding.

## Validation and limits

The hotword surface is bounded:

```text
maximum hotwords.txt size  16 KiB
maximum phrases            128
maximum phrase length      128 Unicode characters
```

Before a phrase reaches the native recognizer, the runtime:

1. normalizes repeated whitespace;
2. rejects `/` and NUL because `/` separates per-stream Sherpa hotwords;
3. rejects words beginning with Sherpa metadata prefixes `:`, `#`, or `@`;
4. tokenizes the phrase with the same local SentencePiece model;
5. rejects `<unk>`;
6. verifies every generated BPE piece exists in the model's `tokens.txt`.

One unsupported phrase is skipped without disabling other valid phrases. A file/resource/tokenizer-level failure disables contextual biasing for that utterance and falls back to greedy decoding.

## Hot reload

`hotwords.txt` is read for each utterance. Editing the file therefore does not require restarting Assisstant Desktop.

The native contextual recognizer itself is reused after its first successful creation; only the per-stream hotword text changes between utterances.

## Performance boundary

`modified_beam_search` has a larger CPU/memory cost than `greedy_search`, and the contextual path keeps a second native recognizer after first use. That cost is paid only when at least one valid hotword is configured.

For command-style assistant speech, accuracy on application/project names should be measured by command success rate in addition to raw WER/CER.

## Current limitations

- No CLI editor for `hotwords.txt` yet; the file is user-managed in this phase.
- No automatic discovery of installed application names or current project names yet.
- Per-phrase custom scores are intentionally not exposed yet; the runtime uses one global score.
- Hotwords improve contextual biasing but do not make the offline recognizer streaming.

## Local verification

On Windows, compare the same recorded commands with and without `hotwords.txt`, including names that the baseline recognizer commonly confuses. Measure:

- proper-noun accuracy;
- command success rate;
- decode latency;
- CPU peak;
- memory after the contextual recognizer is first loaded.

This repository phase is static-reviewed only; native build and microphone verification remain local.
