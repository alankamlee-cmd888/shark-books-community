# SBC AUT-1D — V3 Failure Root Cause and V4 Recovery Repair

Date: 19 September 2026  
Status: V3 SUPERSEDED / V4 READY FOR LIVE PROOF

V3 failed closed with exact installed hashes intact and no SharkBooks source, Git publication, deployment, DNS or public-release change.

The live startup log proved SLE itself raised `PermissionError: [Errno 13] Permission denied` while reading its own `executor.lock` during a restart attempt. At the same time, Dropbox heartbeat evidence showed an existing SLE v1.9.0 watcher at PID 29836 was continuing to publish an IDLE heartbeat.

Root cause: V3 attempted to probe SLE's internal lock file from the recovery process with `msvcrt.locking(..., LK_NBLCK, 1)`. That was unnecessary cross-process coupling to a runtime-internal byte-range lock and produced a false external conclusion.

V4 repair:
- never opens, reads, writes, probes, locks or unlocks `executor.lock` or `relay.lock`;
- watcher identity is exact installed SHA -> Dropbox heartbeat -> heartbeat PID -> one-PID Win32_Process exact script identity;
- no broad Python process enumeration;
- only a stale heartbeat's exact recorded Shark PID may be stopped, and only with an empty SLE running queue;
- SLE restart uses the exact shipped/hash-pinned `START_SHARK_LOCAL_EXECUTOR.cmd`;
- relay restart uses the installed `START_SHARK_LOCAL_EXECUTOR_RELAY.cmd`;
- V4 then queues the exact frozen read-only AUT-1D task and requires exactly one fresh relay event bound by SHA-256 and byte length to the exact SLE terminal result.

V4 bundle SHA-256:
`e3ac3b06eb99fb2b1326f7acdf0bebe7bed05d9d86206e991c98b6348197603e`

V4 helper SHA-256:
`3f5025df4127ce6cb1438b6e01e8f357d758d79a2a6266728d7ebd418b5831e2`

Offline gates passed:
- Python compile;
- forbidden lock-probe token scan;
- frozen AUT-1D task SHA;
- ZIP CRC;
- re-extracted manifest hashes.

V1, V2 and V3 recovery helpers are superseded and must not be used again.
