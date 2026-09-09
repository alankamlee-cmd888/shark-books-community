# Implementation notes

The B3C implementation will reuse the existing SBC-5 opaque native document selection/root-scoped integrity boundary and the B2 factual OCR contract. Rust/Tauri owns package-internal sidecar discovery, process lifetime, timeout/kill, bounded output, exactly-one JSON parse, schema/version checks, typed failures and cleanup. No dependency addition is authorised.
