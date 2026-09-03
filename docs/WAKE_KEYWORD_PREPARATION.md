# Wake Keyword Preparation

Phase 13C adds a local, validated way to generate `keywords.txt` for the current sherpa-onnx GigaSpeech KWS model without downloading or modifying the wake model package itself.

## Scope

Phase 13C does **not** enable automatic installation of the wake model archive. Model redistribution/license and pinned archive checksum remain unresolved for automatic install, so the archive stays manual/fail-closed.

What Phase 13C does provide:

```text
local wake model
├── bpe.model
├── tokens.txt
└── runtime model files
       ↓
user wake phrase
       ↓
SentencePiece tokenizer
       ↓
validate pieces against tokens.txt
       ↓
keywords.txt
```

## Required files for keyword preparation

The preparation step needs:

```text
bpe.model
tokens.txt
```

`bpe.model` is a **preparation file**, not a runtime readiness requirement. Once a valid `keywords.txt` exists, the wake detector does not need `bpe.model` to keep running.

Runtime wake readiness still depends on:

```text
encoder
 decoder
 joiner
 tokens.txt
 keywords.txt
```

## Phrase constraints

The current GigaSpeech model contract accepts an English wake phrase using:

- ASCII letters;
- spaces;
- apostrophes.

The phrase is normalized to uppercase, repeated whitespace is collapsed, and the maximum length is 64 characters.

Examples:

```text
HEY ASSISTANT
HELLO COMPUTER
DUONG'S ASSISTANT
```

## Token validation

The generator uses the model's own `bpe.model` through SentencePiece and then checks every produced BPE piece against the model's own `tokens.txt`.

The generator rejects:

```text
<unk>
unknown SentencePiece IDs
pieces absent from tokens.txt
invalid tokens.txt structure
empty phrases
unsupported characters
```

A tokenization result is never written merely because SentencePiece returned something.

## Output format

For a normalized phrase such as:

```text
HEY ASSISTANT
```

the output uses the sherpa keyword file form:

```text
<token-1> <token-2> ... @HEY_ASSISTANT
```

The `@...` label gives the runtime a stable detection label independent of the exact BPE pieces.

Global keyword score/threshold remain part of `SherpaWakeConfig`; they are not embedded in the generated file.

## No-overwrite rule

Phase 13C deliberately refuses to overwrite an existing `keywords.txt`.

Generation flow:

```text
validate phrase/resources
       ↓
prepare exact keyword line
       ↓
create unique .part in target directory
       ↓
write + flush + sync_all
       ↓
re-check destination does not exist
       ↓
rename .part → keywords.txt
```

If another file appears during generation, the operation fails instead of replacing it.

Changing an existing wake phrase and hot-reloading the detector belong to the next phase.

## Resource UI

Open:

```text
Readiness → Resources → Wake Word Resources
```

The panel shows preparation status for:

```text
bpe.model
tokens.txt
keywords.txt
```

When the current build has `wake-word`, `bpe.model` and `tokens.txt` exist, and `keywords.txt` does not exist, the UI enables:

```text
[ HEY ASSISTANT ]
[ Tạo keywords.txt ]
```

After successful creation, Registry and Readiness refresh. A restart notice is shown because the current WakeService is initialized during app startup.

## Backend trust boundary

Wake keyword preparation is a local resource action. It does not:

- accept an arbitrary URL;
- download a wake archive;
- run shell/PowerShell;
- call Gemini/Antigravity;
- expose an MCP tool;
- replace an existing keywords file;
- send the phrase to cloud services.

## Build feature

The SentencePiece Rust dependency is attached to the existing wake feature:

```text
wake-sherpa
```

Default/text builds do not compile the tokenizer dependency.

## Local verification checklist

Run these manually on Windows after dependencies are installed:

1. Build without `wake-word`; confirm normal text assistant behavior is unaffected.
2. Build with `wake-word` but without `bpe.model`; confirm keyword generation is disabled.
3. Add `bpe.model` and `tokens.txt`; confirm the generation control becomes available.
4. Generate `HEY ASSISTANT` and inspect the resulting `keywords.txt`.
5. Confirm an unsupported/non-English phrase is rejected rather than written.
6. Confirm a second generation attempt refuses to overwrite the existing file.
7. Restart the desktop and confirm WakeService sees the generated file.
8. Enable wake and verify detection on the configured phrase.
9. Confirm `bpe.model` can be treated as preparation-only after `keywords.txt` exists.

Remote development does not execute builds, native runtime tests, GitHub Actions or model downloads.
