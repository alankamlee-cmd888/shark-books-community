# SBC AUT-1A/B/C — SLE Completion Relay Offline PASS

Date: 18 September 2026  
Status: **PASS — AUT-1A/B/C COMPLETE / AUT-1D LIVE INSTALL + EVENT PROOF NEXT**

## Repository boundary

This work is on the docs/automation branch `sbc-automation-workflow-v1-20260918` under draft PR #30.

It does **not** modify:
- PR #29;
- SharkBooks product/runtime source;
- the current R2 entry SHA `1ad86cc7715aa38a8162952f02c8debe3d743736`;
- SLE v1.9.0 runtime bytes;
- dependencies or lockfiles;
- deployment/DNS/hosting;
- `mtdshark.co.uk`.

## Frozen AUT-1 contract

The relay is a separate standard-library-only sidecar. It observes already-published SLE terminal evidence and emits a metadata-only canonical completion event.

The relay has:
- no network client;
- no subprocess/shell capability;
- no Git capability;
- no source-repository access;
- no credential/token reads;
- no public-release authority.

Historical terminal tasks are baselined on first activation so they are not emitted as fresh events.

## Exact reviewed implementation identities

`tools/automation/sle_completion_relay.py`
- SHA-256: `56e6abd6adbf86eacd6e674ccf3ec5a46a87f3da14eb4b91a82b7ef4bf945929`
- Git blob: `ed9355257a122ba32a5cfc21e998a81ef8a2f0a3`

`tools/automation/test_sle_completion_relay.py`
- SHA-256: `3080934a6a120bb3a292e35ccd4f02548b003435ed47f56959bf8359caca0f25`
- Git blob: `a498b48b602f9b511870bd2738700c4e3e61f57a`

`tools/automation/sle_completion_event_schema_v1.json`
- SHA-256: `32e3b54efa9034dcc9cd8bc3463b100634a5ab49034949e6a2d30e35032c7fc0`

## Offline/adversarial proof

Final suite:
- tests: **17**
- passed: **17**
- failed: **0**

It proves:
- metadata-only event output;
- completion and failure terminal states;
- duplicate suppression;
- same-task terminal mutation fail-closed;
- task-ID mismatch rejection;
- malformed JSON rejection;
- unsafe task directory containment;
- actual terminal-byte hashing;
- tampered outbox rejection;
- symlink/non-regular rejection where supported;
- oversized artifact rejection;
- outbox containment under the queue root;
- invalid replay state rejection;
- first-install historical baseline without event emission;
- no historical re-baseline over existing state;
- metadata-only relay heartbeat with `network_authority=false`;
- single-instance relay locking.

## Finished installer package

A standalone one-click Windows installer package was built and independently re-extracted/tested.

Outer bundle:
`SHARK_LOCAL_EXECUTOR_RELAY_V1_0_0_INSTALL_BUNDLE.zip`

SHA-256:
`83d741fb231214739467fc3efdee546cd48ef585f3583376bdc1ca7f1a93a08b`

Inner relay candidate ZIP:
SHA-256:
`40a16f907530de0fe1a4f05b22a3a2021f43c3fe332caa41ed610975ed883a1a`

Finished package verification:
- outer CRC: PASS;
- inner CRC: PASS;
- all manifest file hashes: PASS;
- packaged 17-test suite: PASS;
- forbidden network/Git/deployment-token scan: zero hits;
- SLE install directory is never removed or replaced;
- relay installs separately;
- existing SLE v1.9.0 exact runtime SHA is a precondition;
- first start baselines historical terminal evidence;
- user-login startup entry is installed;
- relay heartbeat must prove `WATCHING` and `network_authority=false`.

## Live SLE state checked before packaging

Dropbox heartbeat reported:
- executor: `1.9.0`;
- host: `DESKTOP-HVGG9T5`;
- state: `idle`;
- stage: `IDLE`;
- current task: null.

The exact SLE v1.9.0 release bundle was recovered and its runtime identity matched the existing authority.

## AUT-1D

Next bounded gate:
1. install the separate relay package on the Windows host;
2. verify relay heartbeat and historical baseline;
3. submit one safe non-product SLE proof task;
4. prove exactly one fresh metadata event is emitted;
5. prove no historical events are emitted;
6. independently reconcile the event with terminal SLE evidence.

No cloud wake/network sink is authorised by AUT-1D.

## AUT-1E owner-exception boundary

Immediate ChatGPT wake-up requires a narrow outbound event carrier. Adding any new network/credential authority is governed by the standing privacy/security owner exception.

Therefore no metadata event sink may be activated silently. The local AUT-1 relay is intentionally transport-neutral until that specific authority is granted.
