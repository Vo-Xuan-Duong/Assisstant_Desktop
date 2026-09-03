# Antigravity Runtime Integration

## Purpose

`antigravity-bridge` is the only project module that knows how to launch and speak the Antigravity CLI streaming protocol. Other modules depend on the `AgentBackend` abstraction from `assistant-core`.

## Authentication

Authentication is owned by Antigravity CLI. The desktop assistant does not read, copy, or persist Google credentials.

The expected setup is:

1. Install Antigravity CLI.
2. Run an interactive `agy` session and complete the normal Google sign-in flow once.
3. Start Assisstant Desktop.
4. The bridge launches headless Antigravity using the CLI's cached credentials.

If the CLI reports authentication errors, they are classified as `BridgeFailureKind::Authentication` and should be surfaced by the UI as an actionable sign-in requirement.

## Continuous session

Production mode uses one long-running process:

```text
agy --input-format stream-json --output-format stream-json
```

A user prompt is written as one NDJSON record:

```json
{"event":"user","message":{"content":"Mở Chrome"}}
```

The bridge waits for the corresponding `result` event before allowing the current `ask()` call to finish. The session can then receive the next prompt on the same stdin pipe, preserving conversation context and avoiding process startup cost.

## Stream events

`AntigravityClient::subscribe()` exposes parsed Antigravity events through a Tokio broadcast channel. Later desktop/UI layers can use this to render:

- initialization state;
- streamed response text;
- tool/step progress;
- usage metadata;
- completion state.

Unknown future event types are intentionally ignored rather than treated as fatal protocol errors.

## Health check

`AntigravityClient::health()` performs a local CLI probe using `agy --help`. It is designed to detect executable availability without consuming a model request.

Possible states:

- `Available`;
- `Missing`;
- `Unhealthy`.

Authentication and quota are runtime service states, not CLI-installation health states, and are classified from Antigravity diagnostics/results when a session is used.

## Failure classification

The bridge classifies failures into:

- `Authentication`;
- `Quota`;
- `Permission`;
- `Model`;
- `Transport`;
- `Process`;
- `Protocol`;
- `InvalidInput`;
- `Unknown`.

This classification exists so the UI can present the correct recovery action instead of a generic error.

Examples:

```text
Authentication -> Ask user to sign into Antigravity normally.
Quota          -> Keep local functionality available and explain AI quota state.
Model          -> Remove/change an invalid pinned model.
Process        -> Recreate the Antigravity session.
Transport      -> Recreate the session or report local process/pipe failure.
```

## Diagnostics

The streaming process captures stderr into a bounded in-memory buffer (last 32 lines). It is used for diagnostics and failure classification.

Diagnostics must not be persisted to application logs blindly because external tools may print sensitive information. The UI should expose sanitized diagnostics only in an explicit troubleshooting surface.

## Recovery behavior

The bridge does **not** automatically replay a failed prompt. Replaying an agent turn could duplicate side effects after a tool executed but before the process returned its final result.

Instead:

- process/transport/protocol failures invalidate the current session;
- the failed request returns an error;
- the next request can create a clean session automatically;
- callers may explicitly invoke `restart()`.

This is intentional at-least-once side-effect protection.

## Model selection

`AntigravityConfig` supports optional model, agent, and effort settings. The default is to let Antigravity choose its configured/default model.

Do not hard-code a model slug into the core architecture. Model availability and quota policy can change independently of the desktop assistant.
