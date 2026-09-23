# SBC-7B2 R4 Cumulative Privacy / Data-Handling Audit — FT3/FT4 Surface

Date: 21 September 2026

This is the cumulative truth for the SBC-7B2 owner UI + finite command-text surface at the R4 pre-candidate boundary.

- Bookkeeping records live behind the existing encrypted/local SharkBooks books boundary; Vue receives bounded owner-safe DTOs rather than raw SQL/ledger authority.
- Bank data enters through explicit local CSV or OFX/QFX import. There is no Open Banking/live feed in this product stage.
- Receipts/documents use user-selected/local-or-user-controlled storage references and bounded native read/verify paths; normal UI does not receive arbitrary paths or broad filesystem authority.
- OCR remains the already-admitted local factual extraction path with manual fallback. R4 adds no remote OCR or AI endpoint.
- Typed command text is processed deterministically against the local finite Action Registry/ActionController. No LLM/model call or remote interpretation is introduced.
- The webview has only the existing explicit Tauri command/capability allow-list; R4 adds no permission.
- No product telemetry, remote analytics recipient or new diagnostic export is introduced by R4.
- Development publication remains GitHub/draft-PR evidence only. `mtdshark.co.uk` public holding-page state is untouched.
- Temporary build/test artefacts are created only for proof and must use synthetic/test data; R4 does not add retention of user bookkeeping data in CI.

Unresolved privacy expansion questions: **none introduced by R4**. Later backup, cloud-provider integration, local AI and public-release stages require their own fresh privacy audit before activation.
