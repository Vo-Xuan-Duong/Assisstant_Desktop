# Assisstant Desktop — Unified Project Plan

## 1. Product goal

Build a Windows-first desktop AI assistant with a system-assistant experience similar in spirit to Gemini on Android, while keeping the implementation modular and local-first.

The assistant must eventually support:

- hotkey and wake-word activation;
- voice input and spoken responses;
- screen and active-window context;
- safe Windows control through explicit tools;
- multi-turn conversations;
- a lightweight edge-glow overlay UI;
- extensibility through MCP;
- Gemini reasoning through Google Antigravity CLI using the user's existing Antigravity entitlement/quota rather than a dedicated Gemini API integration.

## 2. Non-goals

The project will not:

- reverse-engineer Google credentials, private Gemini endpoints, or consumer-app cookies;
- expose unrestricted shell access to the model by default;
- depend on Python services in the production runtime;
- start with wake word, full computer-use, or long-term memory before the core agent/tool chain is stable.

## 3. Locked technology stack

### Primary languages

- Rust: backend, assistant runtime, Antigravity bridge, MCP server, Windows integration, voice/runtime integration.
- TypeScript: Tauri/React frontend only.

### Runtime and platform

- Windows first.
- Tauri 2 + React + TypeScript for desktop UI.
- Tokio for asynchronous Rust runtime.
- Antigravity CLI Headless as the AI/agent runtime.
- MCP as the tool protocol.
- Official Rust MCP SDK (`rmcp`) for the Windows MCP server.
- `windows-rs` for Win32/WinRT/COM APIs.
- CPAL/WASAPI for microphone/audio input.
- whisper.cpp for local STT after the core is stable.
- Windows SpeechSynthesizer for MVP TTS; optional Piper/sherpa-onnx later.
- Windows Graphics Capture for screen context.
- Windows UI Automation for structured desktop automation.
- SQLite for local persistent state.
- TOML for configuration.

## 4. Architectural rule

The system is developed as independent, testable parts and integrated only after each part has a stable interface.

```text
User
  |
  v
Desktop UI / Voice
  |
  v
Assistant Core
  |
  +--> Context Engine
  |
  v
Antigravity Bridge
  |
  v
Antigravity CLI / Gemini
  |
  v
MCP
  |
  v
Windows MCP Server
  |
  v
Windows Tools
  |
  v
Windows APIs
```

No UI module is allowed to call Win32 directly. No model-facing layer is allowed to bypass the permission/tool layer.

## 5. Development phases

### Phase 0 — Foundation

Deliverables:

- repository structure;
- Rust workspace;
- common protocol/state types;
- assistant core state machine;
- Antigravity CLI bridge protocol/process abstraction;
- architecture and roadmap documentation.

Exit criteria:

- interfaces are stable enough for later modules;
- no circular dependencies;
- Antigravity streaming protocol is represented explicitly;
- no Windows-specific implementation leaks into assistant-core.

### Phase 1 — Antigravity integration

Deliverables:

- CLI discovery;
- authentication-status detection via normal CLI behavior;
- long-running `stream-json` process;
- stdin prompt submission;
- stdout NDJSON event parsing;
- session lifecycle and restart handling;
- quota/auth/model error classification.

Exit criteria:

- desktop-side Rust code can maintain a multi-turn Antigravity session without spawning a new process for each request.

### Phase 2 — Windows MCP tools

Start with deterministic safe tools:

- audio.get_volume;
- audio.set_volume;
- audio.mute / audio.unmute;
- apps.list;
- apps.open;
- apps.get_active;
- media.play_pause;
- media.next / media.previous;
- system.get_info;
- clipboard.read / clipboard.write.

Exit criteria:

- Antigravity can call a local stdio MCP server;
- tool results are structured;
- permission class is attached to each tool;
- raw arbitrary shell execution is not exposed.

### Phase 3 — Text desktop MVP

Deliverables:

- Tauri app;
- system tray;
- assistant overlay;
- text conversation;
- assistant status indicator;
- permission confirmation UI;
- local settings.

Exit criteria:

A user can type natural language such as `Đặt âm lượng xuống 30%`, Antigravity selects the MCP tool, Windows executes it, and the assistant shows the result.

### Phase 4 — Screen/context engine

Deliverables:

- foreground window metadata;
- active monitor detection;
- screenshots on demand;
- clipboard context;
- UI Automation tree access where applicable.

Rule: screenshots are contextual and on-demand, not continuously uploaded.

### Phase 5 — Voice

Deliverables:

- microphone input via CPAL/WASAPI;
- VAD;
- local STT via whisper.cpp;
- Windows-native TTS abstraction;
- interruption-safe state transitions.

Start with global hotkey activation; wake word is intentionally deferred.

### Phase 6 — System-assistant UI

Deliverables:

- four-edge transparent glow windows or equivalent native overlay architecture;
- listening/thinking/executing/speaking states;
- active-monitor targeting;
- click-through/no-activate behavior;
- bottom assistant bubble;
- audio-reactive animation.

### Phase 7 — Continuous assistant

Deliverables:

- wake word;
- continuous conversation;
- barge-in;
- short-term session memory;
- local deterministic intent fast-path for commands that do not need AI.

### Phase 8 — Computer use and skills

Deliverables:

- Windows UI Automation actions;
- browser/workflow tools;
- skills/routines;
- optional persistent preference memory.

## 6. Security policy

Tools are categorized as:

- SAFE: read-only and harmless state inspection;
- MODERATE: reversible user-facing actions such as volume changes or app launch;
- SENSITIVE: shutdown, restart, process kill, file mutation;
- BLOCKED: credential extraction, disabling security controls, unrestricted administrator shell.

Sensitive tools require explicit user confirmation at the assistant layer even if Antigravity itself has a permission mechanism.

## 7. Cost/quota policy

The primary AI backend is Antigravity CLI authenticated through the user's normal Google/Antigravity session. The project does not require a Gemini API key for its primary architecture.

The assistant must treat quota exhaustion as a recoverable service state. Local commands may continue to work through a future deterministic intent router even while Antigravity reasoning is unavailable.

## 8. Integration strategy

Each phase is developed on its own branch:

```text
phase/00-foundation
phase/01-antigravity
phase/02-windows-mcp
phase/03-desktop-mvp
...
```

A phase is merged to `main` only after its interfaces and implementation are complete enough that the next phase can depend on it without redesigning the previous phase.

## 9. MVP acceptance scenarios

The first usable desktop MVP must correctly handle natural-language variants of:

- `Mở Chrome`;
- `Mở Visual Studio Code`;
- `Âm lượng hiện tại bao nhiêu?`;
- `Đặt âm lượng 30%`;
- `Tắt tiếng`;
- `Bật lại tiếng`;
- `Pause nhạc`;
- `Chuyển bài`;
- `Ứng dụng nào đang active?`;
- `Máy đang dùng bao nhiêu RAM?`.

The user must not need to memorize command syntax.

## 10. Definition of project completion

The project is considered feature-complete when the assistant can be launched with Windows, activated by hotkey/wake word, understand Vietnamese voice commands, selectively use screen context, invoke safe MCP tools, provide natural spoken feedback, render the system-level edge UI, and remain stable across Antigravity/network/tool failures.
