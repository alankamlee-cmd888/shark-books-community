# SBC AUT-1D — Live Event Proof and Relay v1.0.1 Dropbox-Contention Repair

Date: 19 September 2026  
Status: **AUT-1D FUNCTIONAL EVENT PROOF SATISFIED / RELAY v1.0.1 LIVE STABILITY UPDATE NEXT**

## Live evidence already achieved

The frozen read-only task `SLE-AUT1D-RELAY-PROOF-001` completed successfully under SLE v1.9.0.

Observed terminal result:
- status: `completed`
- executor: `1.9.0`
- source HEAD: `6233a44c802c1205eb5958c5037b3df294c97d99`
- passed: `true`
- source_status_unchanged: `true`
- terminal result bytes: `1002`

The relay emitted exactly one metadata-only event:
- event ID: `d666844e2f78d63e7255fa94706f4ed5e6ec8a3fd567adef4872547a89bccad5`
- task ID: `SLE-AUT1D-RELAY-PROOF-001`
- terminal state: `COMPLETED`
- evidence path: `completed/SLE-AUT1D-RELAY-PROOF-001/result.json`
- terminal artifact SHA-256: `06984e643591ee12d09d55c7c55d089c2a87bebb71926abef1a69e04b5504ae3`
- terminal artifact bytes: `1002`
- relay version that emitted the event: `1.0.0`

No historical relay event replay was observed before this event; the outbox contained this one AUT-1D event.

## V4 failure root cause

The relay itself restarted and produced a valid WATCHING heartbeat at PID 36236, but then crashed while refreshing `relay_heartbeat.json`.

Exact live exception:

`PermissionError: [WinError 5] Access is denied`

at the atomic `os.replace(temp_path, relay_heartbeat.json)` operation.

This is the same Windows/Dropbox contention class already encountered and architecturally repaired in SLE v1.2: Dropbox-visible file operations require bounded retry/verification rather than assuming a single atomic replacement will always succeed.

The event was emitted before the heartbeat crash. Therefore:
- terminal observation works;
- metadata minimisation works;
- exact event identity works;
- deduplication state works;
- event publication works;
- the defect is confined to transient Dropbox destination contention during atomic replacement.

## Relay v1.0.1 repair

v1.0.1 preserves the event schema and all v1.0.0 policy/security boundaries.

It adds bounded retry/backoff around atomic replacement:
- default retry window: 15 seconds;
- initial delay: 50 ms;
- maximum delay: 500 ms;
- retry class: `PermissionError`, Windows access-denied `WinError 5`, Windows sharing violation `WinError 32`;
- persistent failure still raises `RelayError` and fails closed;
- temporary files are cleaned on both success and terminal failure.

The same atomic-write primitive protects heartbeat and relay-outbox publication.

## Exact v1.0.1 identities

Relay:
- SHA-256: `97647552e70466fbdbd863e9c178bdd6bec3064f3ef41f3144b115798eec121a`
- Git blob: `0f75971084f35fddd1639825bb93944334632272`

Tests:
- SHA-256: `ef7afc40e21a8c6a17095f814c4cb7aa964582755037fdb43fe31beef6d44258`
- Git blob: `2fa02989e4cd6da6c0845dea777febb5f8dc157d`

Schema unchanged:
- SHA-256: `32e3b54efa9034dcc9cd8bc3463b100634a5ab49034949e6a2d30e35032c7fc0`

Candidate ZIP:
- SHA-256: `e163c2b58126d357c8484138ff9200a5e4dd80f6c80ecd1e7f407bd4a209df9f`

Bounded update bundle:
- SHA-256: `2797e157047a613becc3c9d2a474776a9478b697f26dd586352efa49d25bcc75`

## Offline proof

v1.0.1 suite:
- tests: **19**
- passed: **19**
- failed: **0**

Two new explicit contention tests prove:
1. three transient `PermissionError` replacement failures are retried and the fourth replacement succeeds with exact bytes;
2. persistent contention exhausts the bounded window, raises `RelayError`, leaves no destination file and cleans temporary files.

## Live update acceptance

The bounded updater:
1. preserves the existing local replay state;
2. runs the 19-test suite against the candidate before mutation;
3. stops at most the one exact relay PID identified by its heartbeat;
4. updates only relay files;
5. reruns the 19-test suite after installation;
6. restarts the existing relay launcher;
7. requires four distinct advancing v1.0.1 WATCHING heartbeat timestamps at one stable PID;
8. proves no historical-event replay after restart;
9. cryptographically reconciles the existing AUT-1D event to the exact SLE terminal result;
10. writes live PASS evidence.

No new network, Git, source-write, deployment, DNS or public-release authority is introduced.

## Boundaries

No change to:
- PR #29;
- SharkBooks runtime/product source;
- frozen R2 authority;
- SLE v1.9.0;
- DNS/hosting/deployment;
- `mtdshark.co.uk` holding-page rule;
- public-release prohibition.
