# Shark Books Community

Shark Books Community is the open-source, local-first bookkeeping project from MTD Shark.

This repository begins from the frozen SBC-0 foundation accepted on 4 September 2026. The accounting core is Govcraft Beankeeper pinned to commit `d573db5e61089b0922f95c991732394d08e3cf92`, integrated behind a Shark-owned Rust facade.

## Accounting boundary

All application callers must go through the Shark facade. UI, Tauri commands, importers, browser adapters and future AI features must never call raw SQLite/SQLCipher or Beankeeper persistence/posting internals directly.

For posting, the facade must perform typed Beankeeper validation before persistence. The frozen facade hash is:

`2c679b0fd5e146e2f82050a32e047c48fa9aa02d386964f8b307c2cae68fb87b`

The reviewed Cargo lockfile hash is:

`3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9`

## Frozen baseline

Run:

```text
python scripts/check_frozen_baseline.py
python scripts/bootstrap_beankeeper.py
```

The first command checks frozen hashes, exact Tauri pins, the Beankeeper commit pin, and the validate-before-persist boundary. The second materialises the exact upstream Beankeeper commit at `upstream/beankeeper` without updating dependencies.

Do not run a casual `cargo update`. Dependency changes require explicit review, regenerated SBOM/licence evidence, and focused regression evidence.

## SBC-1 scope

SBC-1 builds the platform shell and production Shark accounting facade. VAT, CIS, payroll, companies, partnerships, direct HMRC filing, Open Banking, OCR, Qwen, speech, UK tax calculations and final UI polish are outside this gate.

## Licence

Shark-original code is Apache-2.0. See `LICENSE`, `NOTICE`, and `docs/frozen/42_SBC0E_LICENSE_AND_NOTICE_REGISTER_2026-09-04.csv` for third-party notices.
