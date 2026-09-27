# SBC-7B2 R3 / FT4 — Finite Command-Text Development Implementation

Date: 19 September 2026
Entry SHA: `ca5fa293869d4e3bf26d1324c23becae6ecfb6ea`
Scope authority: Library Authority 221 — R3/FT4 manual implementation contract.

This commit implements only the frozen finite command-text and Attention integration:

- exact deterministic matching over the existing Shark Action Registry metadata;
- reuse of the existing ActionController for KNOWN / UNKNOWN / AMBIGUOUS / CONFLICTING;
- exact current UI-A/UI-B exposure set;
- locked/future actions remain fail-closed;
- deterministic owner-slot clarification metadata;
- one dedicated bounded Tauri command `owner_command_text_resolve`;
- Assistant Home command input and Attention integration;
- explicit screen-only continuation mapping into existing manual flows.

The command-text route performs no generic dispatch and directly mutates no bookkeeping records.
Existing manual preview, confirmation, state reread and bounded Tauri operations remain authoritative.

No Action Registry row, accounting rule, persistence schema, npm/Cargo dependency, lockfile,
network authority, LLM/model runtime, speech runtime, tax/HMRC/Open Banking capability, merge or
final-candidate authority is introduced.

Five later R1/R2 read/view bridges whose 15 September registry backend labels are still stale remain
fail-closed from command text; R4 owns that explicit metadata reconciliation.
