# SBC-7B2 R4 — Integrated Pre-Candidate Reconciliation

Date: 21 September 2026
Entry SHA: `65f1fb6b4456d8b6a60fddc81fdbfad1a0a63f8f`
Branch: `sbc7b2-ft3-ft4-integrated-20260916`
PR: #29 (draft / unmerged)

R4 reconciles the integrated FT3A + FT3B + FT4 development tree before any immutable candidate freeze.

The bounded change does five things only:

1. Reconciles the five later-proven owner read/view Action Registry rows from stale 15 September LOCKED metadata to truthful **development-branch / owner-UI-bound / Windows-development-pass / READY** metadata.
2. Regenerates UI-A/UI-B Action metadata from the same registry, reconciles the inherited Rust registry parity validator/count regression from the pre-R4 32/143 snapshot to the structural 175-action parity invariant, and updates finite command-text regressions so the five proven reads are executable through the existing controller.
3. Upgrades both Super-Gate runners so SG3 UI-A, SG4 UI-B and SG5 actual finite command-text tests are mandatory for the integrated candidate.
4. Moves the Super-Gate comparison base to protected post-FT1/FT2 main `ebf1ae9d...` and updates the Codemagic workflow/evidence identity to FT3/FT4.
5. Banks the mandatory R4 privacy delta/cumulative/data-flow/wording-impact evidence.

No accounting semantics, persistence schema, dependency, lockfile, network endpoint, remote AI, speech runtime, Open Banking, VAT/CIS/payroll/HMRC capability, public hosting, merge or candidate freeze is introduced.

R4 must complete its full local static/build/affected-regression preflight and publication guard before this development reconciliation may be published. A successful R4 development commit is **eligible for R5 review/freeze**, not itself the immutable candidate.
