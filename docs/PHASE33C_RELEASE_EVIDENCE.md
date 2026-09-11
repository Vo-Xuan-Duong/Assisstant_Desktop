# Phase 33C — Privacy-bounded Release Evidence Export

## Goal

Make the existing 58-item local acceptance gate produce a portable evidence artifact without adding native authority or exporting machine secrets.

The evidence file is intended to accompany a local release candidate after real Windows + Android validation. It records what the operator marked, the current runtime readiness levels, and the combined gate at export time. It does **not** prove that a physical check was performed correctly.

## User flow

```text
Quick -> Release
  -> complete / update acceptance checks
  -> refresh runtime readiness
  -> Xuất evidence JSON
  -> assistant-release-evidence-<UTC timestamp>.json
```

Export is disabled when the acceptance store or runtime readiness cannot be read successfully. A malformed local acceptance store therefore remains fail-closed and is not silently rewritten or exported as valid evidence.

## Evidence schema

Schema version `1` records:

- UTC export timestamp;
- combined `pending | blocked | ready` gate;
- checklist totals and all 58 item statuses/timestamps;
- runtime overall level and Ready/Optional/Blocking counts;
- runtime check IDs, labels and levels;
- bounded Satellite state: enabled/paired, credential storage class, environment override presence, managed-remote state/port, trusted/revoked counts and warning count;
- an explicit list of fields intentionally redacted.

The JSON is formatted for human inspection and tooling, with a trailing newline.

## Privacy/security boundary

The exported report intentionally omits:

```text
pairing credential/token
satellite device IDs
satellite device names
runtime filesystem paths
runtime check detail strings
```

No Tauri command or Rust/native capability is introduced. Export is implemented entirely in the existing frontend using an in-memory `Blob` and a user-initiated download.

The export operation cannot:

- read arbitrary files;
- execute a process or shell command;
- mutate Firewall or Tailscale;
- change pairing/trust state;
- approve Sensitive MCP actions;
- claim that automated readiness substitutes for the physical 58-check matrix.

## Release interpretation

A `ready` evidence report means only that, at the moment of export:

```text
manual checklist: 58/58 Passed
runtime readiness: not Blocking
local acceptance storage/readiness: readable
```

Final release still requires installed NSIS/startup validation and any signed-public policy required for publication.
