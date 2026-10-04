# SBC AUT-1 — Event-Driven Shark Local Executor Completion Relay Contract

Date: 18 September 2026  
Status: AUT-1A CONTRACT FROZEN FOR IMPLEMENTATION  
Parent authority: `SBC_STANDING_OWNER_AUTOMATION_AUTHORITY_AND_WORKFLOW_PLAN_2026-09-18.md`

## Purpose

Remove the owner from routine `done` / `check the local agent result` transport.

AUT-1 observes only terminal SLE evidence already published under the existing Dropbox queue root and emits a small metadata-only terminal-event envelope. It does not inspect or transmit source code, prompts, diffs, receipts, bookkeeping data, credentials, secrets, or arbitrary evidence content.

## Scope and trust boundaries

AUT-1 is a sidecar to SLE v1.9.0. It does not modify the v1.9.0 executor and does not change any SLE task capability.

The relay:
- may read terminal task directory names and the terminal JSON artifact needed to bind a task ID;
- may hash the complete terminal artifact bytes;
- may write a canonical metadata envelope to a relay outbox;
- may maintain local replay/deduplication state;
- must not write SharkBooks source;
- must not commit, push, merge, fetch, pull or reset Git;
- must not invoke shell/PowerShell/subprocesses;
- must not make network requests in AUT-1A/B/C;
- must not read credentials, tokens or secret stores;
- must not change `mtdshark.co.uk`, deployment, DNS or hosting configuration.

## Queue layout

Default queue root:

`%USERPROFILE%\\Dropbox\\SHARK_LOCAL_AGENT`

Observed terminal roots:
- `completed/<task_id>/`
- `failed/<task_id>/`

Relay-owned roots:
- `relay_outbox/`
- local state file under `%LOCALAPPDATA%\\SharkLocalExecutorRelay\\state.json`

The root may be overridden only by explicit CLI argument or `SHARK_LOCAL_AGENT_ROOT` environment variable. No other environment values are consumed.

## Terminal artifact discovery

For a safe terminal task directory the relay checks these names in order:
1. `result.json`
2. `error.json`
3. `failure.json`

The first existing regular file is the terminal artifact.

The task directory name must match:

`^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`

If the terminal JSON contains a `task_id` field, it must equal the directory task ID exactly. Mismatch fails closed and emits no completion event.

## Event schema

Schema file: `tools/automation/sle_completion_event_schema_v1.json`

Required event fields:
- `schema_version` = 1
- `event_type` = `SLE_TASK_TERMINAL`
- `event_id`
- `task_id`
- `terminal_state` = `COMPLETED` or `FAILED`
- `terminal_artifact_name`
- `terminal_artifact_sha256`
- `terminal_artifact_size_bytes`
- `evidence_relpath`
- `relay_version`
- `emitted_at_utc`

Optional safe fields copied from the terminal result only when scalar and bounded:
- `executor_version`
- `passed`

No other terminal-result field is copied to the event.

## Canonical event identity

`event_id` is SHA-256 over the UTF-8 canonical identity string:

`schema_version|event_type|task_id|terminal_state|terminal_artifact_name|terminal_artifact_sha256|terminal_artifact_size_bytes`

The event file name is:

`<task_id>.<event_id>.json`

This means a changed terminal artifact necessarily produces a different event identity.

## Replay and duplicate policy

The relay maintains a local map of `task_id -> event_id`.

- Same task ID + same event ID: duplicate; do not emit again.
- Same task ID + different event ID after prior emission: fail closed as terminal-artifact mutation; do not emit a replacement automatically.
- Existing outbox file with non-identical bytes: fail closed.

## Metadata minimisation

The event must never include:
- source code;
- diffs/patches;
- prompts/model replies;
- arbitrary stdout/stderr;
- file contents;
- database/bookkeeping data;
- receipt/OCR contents;
- credentials/tokens/passwords/API keys;
- local absolute source-repository paths.

The only path value is the Dropbox-relative evidence path, for example:

`completed/SLE-V1-P7-PROMOTE-027/result.json`

## AUT-1A transport decision

The local event producer is transport-neutral.

AUT-1A/B/C implement only the local canonical event outbox. This requires no new network authority and is permitted under the current privacy/security standing authority.

An immediate ChatGPT wake requires a cloud event carrier. Candidate carriers considered:
- Dropbox polling: safe but not event-driven; retained as hourly fallback only.
- Browser/UI automation: rejected as brittle and over-privileged.
- SMTP/email: rejected as unnecessary credential/network expansion.
- Git commit/push as signal: rejected because relay itself must have no Git publication authority.
- metadata-only GitHub issue/PR comment or equivalent authenticated event: technically suitable, but activation adds outbound network/credential authority and therefore is intentionally withheld until explicit `OWNER_EXCEPTION_PRIVACY_SECURITY` approval.

AUT-1D/E therefore split local live event proof from cloud wake activation. No network sink may be silently added to this contract.

## Adversarial requirements

AUT-1C must prove rejection or containment for:
- duplicate event;
- same-task terminal mutation;
- task-ID/path traversal attempts;
- task-ID mismatch between directory and JSON;
- malformed JSON;
- oversized terminal artifact;
- symlink/non-regular terminal artifact;
- pre-existing tampered outbox event;
- extra sensitive fields in result not copied to event;
- reordered JSON producing the correct whole-file hash without changing metadata extraction rules.

## Size ceilings

- terminal artifact maximum: 16 MiB;
- event envelope maximum: 8 KiB;
- task ID maximum: 128 characters.

## Exit criteria

AUT-1A passes when this contract and the exact schema are committed to the automation branch.
AUT-1B passes when the deterministic sidecar implementation conforms to this contract.
AUT-1C passes when its offline/adversarial test suite passes with zero network and zero source mutation.
