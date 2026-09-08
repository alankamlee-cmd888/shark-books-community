# Shark Books Community — SBC-17 Browser SQLite/OPFS Durability Backlog

**Date:** 8 September 2026  
**Created by:** SBC-1F browser adapter interface smoke  
**Status:** DEFERRED TO SBC-17 — NOT A PARITY CLAIM

SBC-1F proves only that a browser implementation can sit behind a bounded Shark application-adapter interface without importing the native SQLCipher/Tauri stack or duplicating accounting rules.

Before browser/PWA parity can be claimed in SBC-17, the browser persistence implementation must be tested and adjudicated for at least the following:

- SQLite WASM runtime and exact build/version selection.
- OPFS support matrix across Chrome/Edge, Firefox and Safari/iOS PWA.
- Worker/threading model, including any SharedArrayBuffer/cross-origin-isolation requirements.
- Single-tab and multi-tab locking/concurrency behaviour.
- Transaction atomicity and crash consistency under abrupt tab/process/device termination.
- WAL/journal/checkpoint behaviour in the selected WASM/OPFS stack.
- Schema migration atomicity, rollback and interrupted-migration recovery.
- Browser storage quota, persistence requests, eviction risk and low-storage behaviour.
- Private/incognito mode behaviour and explicit user warnings.
- iOS/Safari/PWA suspension and lifecycle behaviour.
- Backup/export/restore design independent of OPFS internals.
- Corruption detection, recovery and user-controlled rescue/export path.
- Large-book performance, startup latency, import throughput and memory ceilings.
- Attachment/document storage strategy and consistency with ledger references.
- Browser encryption-at-rest/security model; native SQLCipher guarantees must not be implied.
- Cross-version compatibility and reproducible test fixtures.
- No-egress/privacy tests proving no mandatory Shark cloud or telemetry path.
- Accessibility and recovery UX for unsupported/evicted browser storage.

**Gate rule:** none of the above is satisfied merely because the SBC-1F interface crate compiles for `wasm32-unknown-unknown`. Browser feature parity remains unproven until SBC-17 closes these items with implementation evidence.
