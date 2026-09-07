# Architecture

## Dependency direction

The project follows a strict inward dependency rule:

```text
UI / Voice / Platform adapters
            |
            v
      assistant-core
            |
            v
      common domain
```

External integrations remain adapters. The Android Voice Satellite does not become part of Assistant Core and cannot call Windows tools directly.

```text
Android SpeechRecognizer
        |
   final text
        |
Satellite adapter
        |
        v
 Assistant Core
        |
 Antigravity / MCP
        |
 Windows Tools
```

The desktop-local `voice-runtime` remains a fallback adapter for microphone/VAD/Zipformer/TTS/wake functionality.

## Distributed process architecture

The user-facing system now has an Android input device plus the existing Windows processes:

```text
+----------------------------+
| Android Voice Satellite    |
| Kotlin / Compose           |
| SpeechRecognizer           |
+-------------+--------------+
              |
              | authenticated WebSocket
              | final text commands only
              v
+----------------------------+
| assisstant-desktop.exe     |
| Tauri + Rust core          |
| Quick / context / TTS      |
+-------------+--------------+
              |
              | spawn + NDJSON stdin/stdout
              v
+----------------------------+
| agy                        |
| Antigravity CLI            |
+-------------+--------------+
              |
              | MCP stdio
              v
+----------------------------+
| assistant-mcp.exe          |
| Rust MCP server            |
+-------------+--------------+
              |
              v
         Windows APIs
```

The MCP server remains a separate process so Antigravity owns its stdio transport and tool execution is isolated from the desktop UI process.

## Voice input architecture

### Preferred path — Android

```text
Phone microphone
      |
Android SpeechRecognizer
      |
      +-- partial hypotheses -> phone UI only
      |
      `-- final result
              |
       protocol-v1 command
              |
       WebSocket / trusted LAN
              |
              v
       satellite adapter
              |
       Quick final transcript
              |
       Assistant Core
```

Only final recognized text may create a turn. The phone sends no microphone PCM/audio to the Windows runtime.

The Android app supports initial `vi-VN` and `en-US` modes. It prefers Android's on-device recognizer when the platform reports it available, otherwise it may use the device's normal system recognition service.

### Fallback path — desktop local voice

```text
Windows microphone
      |
CPAL / WASAPI
      |
VAD / hysteresis
      |
Zipformer offline recognizer
      |
final text
      |
Assistant Core
```

The current Zipformer partial transcript is a UI preview produced from bounded snapshots; only final recognition enters Assistant Core.

## Satellite transport boundary

The desktop satellite listener uses RFC 6455 WebSocket framing over Tokio and a small JSON application protocol.

It is intentionally a narrow input boundary, not a remote-control API.

Properties of protocol v1:

- listener disabled unless a pairing token is configured;
- trusted-LAN `ws://` transport for the MVP;
- first application message authenticates and declares protocol version;
- authentication has a bounded timeout;
- client frames must be masked;
- frame payloads are bounded;
- only text commands/ping are accepted at the application layer;
- one satellite command/TTS turn at a time;
- requests are rejected while Assistant Core is already busy;
- the Android client receives assistant state/result messages but no MCP credentials or desktop-management secrets.

`ws://` is not considered suitable for Internet exposure. Productized pairing, WSS/private-overlay transport, device credentials, and revocation are later phases.

## Assistant request path

Satellite requests currently use:

```text
final recognized text
        |
response-language policy
        |
on-demand desktop context
        |
Assistant Core
        |
Antigravity
        |
MCP / permission gateway
        |
Windows tools
```

The response-language policy is `VI`, `EN`, or `Auto` and asks for friendly, natural, concise wording. Windows SAPI then speaks the returned text.

The satellite adapter intentionally does not expose a second tool-routing stack. All mutating actions still rely on Antigravity/MCP and the existing permission gateway.

## Antigravity protocol

The bridge uses Antigravity's continuous streaming mode:

```text
agy --input-format stream-json --output-format stream-json
```

Input is one JSON object per line:

```json
{"event":"user","message":{"content":"..."}}
```

Output is parsed as NDJSON events. A turn is complete only when a result event is received. Unknown future event types are ignored rather than crashing the desktop runtime.

## Assistant state machine

Core states:

```text
Idle
Listening
Processing
Executing
Speaking
Confirming
Error
```

Text/satellite turns are single-flight at Assistant Core. Satellite commands additionally use a transport-level command gate so multiple connected devices cannot create overlapping desktop TTS turns in the MVP.

The desktop-local voice path uses `Listening`; a satellite command arrives already recognized as text and enters `Processing` directly.

## Spoken response architecture

The phone does not own TTS.

```text
Assistant result
      |
VI / EN / Auto response text
      |
Windows SAPI
      |
Desktop speakers
```

Current `VI`/`EN`/`Auto` controls generated response text. Locale-aware enumeration and selection of installed SAPI voices is planned separately; the current TTS worker still uses its configured/default voice.

## Windows tool rule

AI-visible tools must remain explicit, narrow operations.

Good:

```text
audio.set_volume(value)
apps.open(name)
window.get_active()
```

Not exposed as a normal assistant tool:

```text
shell.execute(command)
```

The Android satellite never receives direct access to these native implementations.

## Context rule

Context collection is demand-driven. The assistant does not continuously upload the screen.

Examples:

- `Mở Chrome` -> usually no screenshot is necessary;
- `Lỗi trên màn hình này là gì?` -> active-window metadata + screenshot may be collected.

A satellite request may still use the current desktop source-window/context snapshot because reasoning and screen access remain on Windows.

UI Automation is preferred over OCR/pixel clicking when Windows exposes an accessible UI tree.

## UI rule

The system-level Windows UI is an adapter over assistant state and never owns business logic.

```text
EdgeOverlayManager
  |- top edge
  |- right edge
  |- bottom edge
  |- left edge
  `- Quick assistant surface
```

A final satellite transcript is emitted to the existing Quick transcript event so the user can see what the desktop is processing without creating a second desktop UI stack.

## Security boundary

There are several independent boundaries:

1. Android pairing/transport authentication.
2. Assistant Core request lifecycle.
3. Antigravity permissions.
4. Assistant-owned MCP/tool policy.
5. Sensitive desktop confirmation broker.

The Assistant-owned tool policy and Sensitive confirmation remain authoritative for Windows side effects.

Tool categories are:

```text
Safe
Moderate
Sensitive
Blocked
```

The Android device cannot approve a Sensitive operation on behalf of the user in the current MVP.

## Management boundary

`assistant.exe` uses a separate authenticated management endpoint bound to loopback with a per-runtime secret. The Android satellite connection is not that endpoint and must never receive or reuse the management secret.

Future `assistant satellite ...` commands should manage satellite settings without exposing generic desktop-management privileges to the phone.

## Persistence

Current desktop durable settings include Antigravity/wake/permission/runtime data. Productized satellite device/pairing persistence is planned for the next phase.

Google credentials are never copied into project storage; authentication remains owned by Antigravity CLI.

The Android MVP stores its endpoint/token/preferences in app-local SharedPreferences. A later hardening phase should use an appropriate Android secure credential storage strategy for long-lived device credentials.

## Failure model

The runtime must distinguish at least:

- satellite pairing/authentication failure;
- satellite protocol/version failure;
- phone disconnect/reconnect;
- assistant busy rejection;
- Android speech-recognition failure;
- authentication/quota failure in Antigravity;
- Antigravity process exit;
- malformed protocol event;
- MCP/tool failure;
- permission denial;
- desktop network failure;
- desktop local voice subsystem failure;
- desktop TTS failure.

Loss of the Android satellite must not terminate the Windows assistant. Keyboard input and the desktop-local voice fallback remain separate input adapters.
