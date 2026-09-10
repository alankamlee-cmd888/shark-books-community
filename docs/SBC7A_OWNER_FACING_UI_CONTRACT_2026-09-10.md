# Shark Books Community — SBC-7A Owner-Facing UI Contract

**Date:** 10 September 2026  
**Protected entry main:** `3fcbbc69a8e30a6d6281ce410f1e5e80c8f43ace`  
**Stage:** SBC-7A — contract/design only  
**Implementation gate:** SBC-7B remains blocked until this contract passes and is merged

## 1. Purpose

Freeze the information architecture, owner journeys, language, states, confirmation points, platform intent and application-operation boundary for the first owner-facing Shark Books Community interface.

SBC-7A does **not** implement the Vue/Tauri owner UI and does not change product, native, accounting, OCR or dependency code.

The UI is for a UK sole trader who should not need to understand double-entry bookkeeping. The interface gathers facts and explicit owner choices, displays deterministic results in plain language, and invokes bounded Shark-owned application operations. Accounting/domain code remains authoritative outside the UI.

## 2. Primary navigation — frozen

1. **Home** — what needs attention, recent activity and safe next actions.
2. **Money in** — invoices, customer payments and other business income.
3. **Money out** — expenses, supplier payments, owner-paid expenses and owner drawings where applicable.
4. **Bank** — statement import, bank activity review, matching and reconciliation.
5. **Receipts** — user-controlled documents, local factual OCR and receipt-to-bank suggestions.
6. **Contacts** — customers and suppliers.
7. **Reports** — factual bookkeeping views supported by the existing application/facade; no tax-liability claim.
8. **Settings** — books/business metadata, privacy, storage configuration when supported, and later backup settings.

Do not add Chart of Accounts, Journals, Debits/Credits, Beankeeper, SQLite or SQLCipher as normal top-level owner destinations.

## 3. Home contract

Home answers:
- **What needs my attention?**
- **What changed recently?**
- **What should I do next?**

Only proven states may appear. Examples include bank activity needing review, unresolved matches, unlinked receipts/documents, reconciliation state, recent money in/out and explicit operation errors.

Home must not invent tax liability, tax reserve, compliance advice or Books Check conclusions. Those belong to later deterministic/business-utility gates.

## 4. Frozen owner journeys

### J1 — Start or open books
`open app -> create/open books -> encrypted books verified -> Home`

Use: **Create my books**, **Open my books**. Translate infrastructure failures into owner-actionable messages without exposing secrets or raw database internals.

### J2 — Import a bank statement
`Bank -> Import statement -> choose CSV or OFX/QFX -> mapping/preview -> duplicate review -> explicit import confirmation -> Bank activity`

Requirements:
- preview before any persistence;
- file-exact/strong duplicate evidence and heuristic similarity remain distinguishable;
- heuristic similarity never silently removes a row;
- malformed rows/currency failures explain what must be corrected;
- V1 does not advertise Open Banking.

### J3 — Review and match bank activity
`Bank activity -> suggested bookkeeping candidate(s) -> reasons -> owner confirm/reject -> cleared state -> optional reconciliation`

Requirements:
- Exact/Likely/Possible/Unmatched semantics remain distinguishable using friendly wording;
- ties/ambiguity are shown as **Needs review**, not silently selected;
- every material match requires explicit owner confirmation;
- **Cleared** and **Reconciled** remain distinct.

### J4 — Record money in
`Money in -> New income or invoice -> customer/details/amount/date -> preview -> explicit save -> optional payment allocation`

The owner is never asked for debit/credit entries. Any accounting posting is produced and validated by the Shark application/accounting boundary.

### J5 — Record money out
`Money out -> New expense -> supplier/details/amount/date -> business/private/mixed owner choice where relevant -> preview -> explicit save`

Business/private/mixed is an owner fact/choice, not AI inference. A bookkeeping category must not be presented as a tax-deductibility conclusion. Owner contributions/drawings must remain distinct from business income/expense.

### J6 — Add a receipt or document
Manual path:
`Receipts -> Add document -> native/user-controlled selection -> integrity/reference -> attach or leave unlinked`

OCR path:
`Receipts -> Add document -> local factual OCR -> review merchant/date/amount/currency/reference -> deterministic receipt-to-bank suggestion -> owner confirm/reject`

OCR failure/unavailability must fall back to manual document handling and must not block ordinary bookkeeping.

### J7 — Reconcile the bank
`Bank -> Reconcile -> statement/end balance/date -> cleared entries + difference -> resolve difference -> explicit finalise only at exact zero-pence difference`

A non-zero difference cannot be finalised. The screen should explain what remains outstanding rather than teach accounting theory.

### J8 — Correct a mistake
`record -> Correct -> show original -> preview reversal/correction/replacement -> explicit confirm`

Do not offer destructive silent rewriting when the existing audit/correction boundary requires history to be preserved.

## 5. Screen inventory

Minimum owner-facing views for SBC-7B/7C:

| View | Purpose | Principal owner actions |
|---|---|---|
| Books start/open | enter or resume local books | create, open, retry |
| Home | status and attention queue | navigate to unresolved work |
| Money in list/detail/editor | factual income/invoice workflow | create, preview, save, correct |
| Money out list/detail/editor | factual expense workflow | create, choose use state, preview, save, correct |
| Bank activity | imported bank rows and review state | inspect, match, import, reconcile |
| Bank import | file format/mapping/preview/duplicates | preview, resolve, confirm import |
| Match review | deterministic candidate reasons | confirm or reject |
| Reconciliation | statement difference and eligible entries | preview, resolve, finalise at zero |
| Receipts | document/integrity/OCR/link state | add, inspect, OCR, link/unlink through bounded actions |
| Receipt review | factual OCR fields and bank suggestions | confirm/reject/manual fallback |
| Contacts | customers/suppliers used by owner workflows | add/edit/select where supported |
| Reports | factual supported bookkeeping views | view/export later only when authorised |
| Settings | business/books/privacy/storage information | bounded settings actions only |

## 6. Required state model

Every material view must define and visibly handle as applicable:
- loading;
- empty/new user;
- populated;
- validation error;
- operation failure;
- permission/storage failure;
- wrong-key/open failure;
- optional local OCR unavailable;
- OCR failure;
- ambiguous match / owner decision required;
- duplicate review required;
- document integrity failure/read-only state;
- unreconciled/non-zero-difference state;
- offline state where relevant.

Empty states must provide a safe next action. Example: an empty Bank page offers **Import a statement**, not an unavailable live bank feed.

## 7. Owner language — frozen

Prefer:
- Money in / Money out
- Bank activity
- Match / possible match
- Needs review
- Receipt / document
- Customer / supplier
- Paid / unpaid / overdue
- Cleared / Reconciled, with short explanation where needed
- Money I put into the business
- Money I took out of the business

Avoid as primary owner UI terminology:
- Journal / posting / ledger mutation
- Debit / credit
- Suspense account
- internal score/RRF/confidence mechanics
- Beankeeper
- SQLite / SQLCipher
- implementation DTO/schema names

## 8. Confirmation rules

Explicit owner confirmation is mandatory before an action that materially changes bookkeeping state unless an already-proven deterministic contract makes the operation non-discretionary and the application contract itself authorises direct execution.

At minimum, explicit confirmation is required for:
- accepting a statement import after preview/duplicate review;
- accepting a bank/bookkeeping match;
- accepting a receipt-to-bank suggestion;
- finalising reconciliation;
- saving/correcting a material bookkeeping record after preview where the journey specifies preview;
- correction/reversal/replacement actions.

Confirmation must show the factual input, proposed action and affected record(s). Cancel/reject must not mutate the underlying facts.

## 9. Hard UI prohibitions

The SBC-7 UI must not:
- read/write raw SQLite or Beankeeper internals;
- contain accounting balancing/posting rules;
- infer tax deductibility, VAT/CIS/payroll/company treatment or tax liability;
- infer business/private/mixed use as if it were a user fact;
- auto-confirm bank or receipt matches;
- auto-reconcile;
- expose arbitrary filesystem/database/model/executable/shell paths;
- introduce a generic shell or broad filesystem surface;
- require cloud OCR, Open Banking or Shark-central document custody;
- add AI/LLM judgement;
- perform direct HMRC filing;
- claim browser/PWA persistence, encryption or OCR parity.

## 10. Accessibility and interaction baseline

SBC-7B must provide:
- full keyboard operation on desktop;
- visible focus states;
- semantic labels and error associations;
- no colour-only status meaning;
- sufficient text/control contrast;
- scalable text without clipping critical actions;
- touch-sized controls for tablet/mobile layouts;
- meaningful screen-reader names for icon-only controls;
- predictable focus after confirm/cancel/error;
- clearly distinguished correction/destructive actions.

## 11. Responsive/platform intent

- **Windows desktop:** primary SBC-7B implementation and functional proof surface.
- **iPad/tablet:** same information architecture, responsive two-pane/list-detail patterns where appropriate.
- **iPhone:** same semantics with stacked navigation and one primary task per screen; no platform-specific accounting rules.
- **Browser/PWA:** responsive design may be reusable, but persistence/encryption/OCR browser parity remains SBC-17.

## 12. Component/dependency position

SBC-7A introduces no UI dependency.

For SBC-7B, default to Vue/CSS already compatible with the existing shell/build path unless a third-party component library provides a material accessibility/maintenance advantage. `frappe-ui` remains a candidate only, not an approved dependency. Adoption requires an exact version pin plus licence/NOTICE/dependency review and the normal change-control evidence.

## 13. 7B architecture classification

The current Tauri shell exposes only a small engineering command set. Many already-proven product-core/foundation capabilities required by these journeys are not yet presented as bounded owner-facing native operations.

Therefore SBC-7B is classified in advance as a **controlled application/native + UI integration stage**, not merely visual styling.

7B must:
- introduce an exact typed application-operation bridge for only the capabilities authorised in the companion operation matrix;
- keep JavaScript/Vue away from raw core/database/facade internals;
- update Tauri command manifests/permissions only by exact allowlist;
- keep native file/document selection Rust-owned so the webview receives opaque identifiers rather than arbitrary paths;
- treat any Tauri/native dependency/configuration change as a formal change-control event;
- run Windows regression plus Apple physical/simulator compile regression when native code/config/dependency graph changes.

No exact third-party/native dependency is approved by this 7A contract merely because 7B may need a bounded picker/bridge implementation.

## 14. SBC-7C acceptance journeys

Before SBC-7 closes, executable/visible acceptance must demonstrate at minimum:
1. fresh sole trader creates/opens books and reaches a useful Home state;
2. statement import shows preview and duplicate outcomes before acceptance;
3. one money-in/invoice workflow reaches validated persistence through the Shark boundary;
4. one business expense plus a private/mixed-use case preserves explicit owner choice;
5. a document can be handled without OCR;
6. a receipt can use local factual OCR, show extracted facts and require confirm/reject for a suggested bank match;
7. a bank match and exact-zero reconciliation complete only through explicit owner actions;
8. a correction preserves history;
9. wrong-key/storage/integrity/OCR-unavailable failure can be recovered from without losing unrelated work;
10. core journeys never ask the owner to enter debits/credits or expose database/accounting-engine internals.

## 15. Exit condition

SBC-7A may PASS only when this contract and the companion application-operation matrix are internally consistent with protected `main`, frozen-boundary validation passes and no product/native/UI implementation has been smuggled into the design stage.

Only after SBC-7A PASS and protected merge may SBC-7B implementation begin.