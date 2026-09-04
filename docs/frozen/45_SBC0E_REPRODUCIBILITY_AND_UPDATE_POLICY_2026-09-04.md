# SBC-0E Reproducibility and Dependency Update Policy

## Frozen identifiers
- Beankeeper commit: `d573db5e61089b0922f95c991732394d08e3cf92`
- Shark facade SHA-256: `2c679b0fd5e146e2f82050a32e047c48fa9aa02d386964f8b307c2cae68fb87b`
- Cargo.lock SHA-256: `3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9`
- Rust baseline: 1.98.1
- Tauri core/CLI/build: 2.11.5 / 2.11.4 / 2.6.3

## Proven Windows commands
- `cargo test --workspace --locked` in pinned Beankeeper workspace -> 473 passed / 0 failed.
- Shark foundation/Tauri C4: `cargo build -p shark-tauri-spike --locked -j 1`.

## Proven Apple commands/classes
- `cargo build --locked -p shark-foundation --target aarch64-apple-ios`
- `cargo build --locked -p shark-foundation --target aarch64-apple-ios-sim`
- pinned Tauri CLI `ios init --ci` then unsigned Simulator build.

## Lockfile strategy
The application workspace commits `Cargo.lock`. CI must use `--locked`. A lockfile change is a reviewed dependency change, never incidental build output.

## Update rule
Changes to Beankeeper, rusqlite/SQLCipher/OpenSSL, Tauri, Rust MSRV/toolchain or licence classes require:
1. explicit dependency-change commit/PR;
2. regenerated SBOM/licence register;
3. Shark facade behavioural regression (GBP, reopen, duplicate OFX, reversal/audit, documents, encryption);
4. Windows Tauri compile;
5. iOS physical-target + Simulator compile if the native graph changed;
6. evidence/adjudication before release.

Do not run `cargo update` casually against the frozen production workspace.
