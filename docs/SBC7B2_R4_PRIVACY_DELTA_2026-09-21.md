# SBC-7B2 R4 Privacy Delta Audit

Date: 21 September 2026

R4 changes metadata/proof orchestration only. The five already-existing local owner read/view bridges become controller-eligible in the Action Registry; no new underlying data access path is added.

**New data collected:** none.
**New storage:** none.
**New network/off-device transfer:** none.
**New third-party recipient:** none.
**New credential/secret:** none.
**New filesystem/database/shell/camera/clipboard/notification permission:** none.
**New dependency:** none.
**Telemetry/crash reporting change:** none.
**Public/staging deployment change:** none.

The affected data is data SharkBooks could already read through the bounded R1/R2 bridges: owner-safe Money records, owner-safe bank-activity detail, registered-document metadata and bounded registered-document open/view. The reconciliation does not expose raw ledger rows, arbitrary filesystem paths, document storage roots, passphrases or database authority to the webview.

R4 also makes Windows/Apple SG3-SG5 proof mandatory; proof artefacts are synthetic/test evidence and must not contain real bookkeeping secrets or personal document content.
