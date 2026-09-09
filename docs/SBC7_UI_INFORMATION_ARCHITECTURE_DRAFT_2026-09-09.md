# Shark Books Community - SBC-7 owner-facing UI information architecture draft

Date: 2026-09-09
Status: RESEARCH / CONTRACT PREPARATION ONLY - NO VUE PRODUCT IMPLEMENTATION YET
Entry production main: `d20a356e374e6b7b56d4f025503896506a6027ce`

## 1. Product principle

SBC-7 must turn the already-proven accounting, bank, reconciliation and document contracts into an interface a sole trader can use without understanding double-entry bookkeeping.

The UI uses **plain owner language**. It does not expose Beankeeper, SQLite, SQLCipher, DTO, posting-plan, debit/credit or suspense terminology as the normal user journey. Those concepts remain internal implementation details.

There are **no accounting rules in the UI**. The UI gathers facts and user choices, previews consequences in owner language and calls Shark-owned application/facade operations. Deterministic domain/accounting code remains authoritative.

## 2. Primary navigation

The initial desktop/tablet navigation contract is deliberately small:

1. **Home** - what needs attention, recent activity and clear next actions.
2. **Money in** - invoices, customer payments and other business income.
3. **Money out** - expenses, supplier payments, owner-paid expenses and drawings where relevant.
4. **Bank** - import statements, review bank lines, match and reconcile.
5. **Receipts** - documents/receipts, attachment state and optional SBC-6 extraction/match suggestions.
6. **Contacts** - customers and suppliers.
7. **Reports** - simple factual bookkeeping views already supported by the domain/facade; advanced tax claims are excluded here.
8. **Settings** - business/book metadata, storage locations, backup-related settings when later gates provide them, and privacy/about information.

Do not add a permanent top-level Chart of Accounts, Journals or Reconciliation jargon page merely because the accounting engine has those concepts. Advanced diagnostics may exist later behind a clearly labelled advanced/support surface.

## 3. Home contract

Home answers three owner questions:

- **What needs my attention?**
- **What changed recently?**
- **What should I do next?**

Initial cards/sections may include only states supported by existing contracts:

- bank transactions waiting to be reviewed/matched;
- invoices waiting for payment;
- receipts/documents not yet linked to a record;
- reconciliation status;
- recent money in/out;
- explicit errors or unresolved user confirmations.

Do not invent tax-liability, tax-reserve or compliance warnings before the relevant deterministic rule engine exists. `Books Check`, tax reserve and broader utilities belong to **SBC-8** or later and may be represented only as future design hooks.

## 4. Core user journeys to freeze before visual polish

### J1 - Start/open books

`open app -> choose/create books -> encrypted books open -> Home`

Owner-facing language:
- "Create my books"
- "Open my books"
- "This file could not be opened with that key" rather than raw database/SQLCipher errors.

### J2 - Import a bank statement

`Bank -> Import statement -> choose CSV or OFX/QFX -> mapping/preview -> duplicate review -> confirm import -> Bank review list`

Requirements inherited from SBC-3:
- preview before persistence;
- strong/file-exact/heuristic duplicate distinctions must not be collapsed into an unexplained "duplicate" label;
- heuristic similarity never silently removes a line;
- malformed/unsupported currency state explains what the user needs to fix.

### J3 - Review/match bank activity

`Bank line -> suggested bookkeeping candidate(s) -> show reasons -> user confirms or rejects -> cleared state -> reconciliation`

Requirements inherited from SBC-4:
- EXACT / LIKELY / POSSIBLE / UNMATCHED may be translated to friendly wording, but the distinction must remain visible;
- multiple exact-looking candidates are not silently treated as exact;
- every match requires the user's confirmation;
- cleared and reconciled remain distinct states.

### J4 - Record money in

`Money in -> New income / New invoice -> customer/details/amount/date -> preview -> save -> optional payment allocation`

The UI never asks the user for debit/credit accounts. User-facing categories map through the deterministic domain contract.

### J5 - Record money out

`Money out -> New expense -> supplier/details/amount/date -> business/private/mixed choice where relevant -> preview -> save`

Important wording:
- private/business/mixed is a user fact/choice, not an AI conclusion;
- owner contribution/drawing must be described distinctly from income/expense;
- tax deductibility is not implied merely because a bookkeeping category is selected.

### J6 - Add a receipt/document

Without OCR:
`Receipts -> Add document -> user-selected storage/root -> attach to a record or leave unlinked -> integrity/reference saved`

With SBC-6 after it earns PASS:
`Receipts -> Add document -> local factual extraction -> show extracted merchant/date/amount/reference -> deterministic candidate suggestion -> user confirms/edit/reject -> normal Shark action path`

OCR failure must degrade to the no-OCR path rather than blocking ordinary bookkeeping.

### J7 - Reconcile the bank

`Bank -> Reconcile -> choose statement/end balance/date -> show cleared entries and difference -> resolve difference -> confirm when exact zero-pence difference`

The normal screen should say what is outstanding rather than teach reconciliation theory. A non-zero difference cannot be finalised.

### J8 - Correct a mistake

`record -> Correct -> show original -> create correction/reversal/replacement preview -> confirm`

Do not offer destructive silent history rewriting where the existing audit/reversal contract requires preserved history.

## 5. Screen-state contract

Every important view needs explicit states, not just a happy-path mock-up:

- loading;
- empty/new user;
- populated;
- validation error;
- operation failure;
- permission/storage failure;
- wrong-key/open failure where relevant;
- offline/local runtime unavailable for optional OCR;
- ambiguous match/user decision required;
- read-only/integrity problem where relevant.

Empty states must contain a useful next action. Example: an empty Bank page should explain that SharkBooks uses statement-file import in V1 and offer "Import a statement", not advertise unavailable Open Banking.

## 6. Owner-language vocabulary

Prefer:
- Money in / Money out
- Bank activity
- Match / possible match
- Receipt / document
- Customer / supplier
- Paid / unpaid / overdue
- Needs review
- Reconciled
- Money I put into the business
- Money I took out of the business

Avoid as primary UI labels:
- Journal
- Posting
- Debit / credit
- Ledger mutation
- Suspense account
- RRF/score/confidence internals
- Beankeeper
- SQL/SQLite/SQLCipher

Where accounting terminology is unavoidable in reports/support material, add short plain-English explanations.

## 7. Safety and confirmation design

Confirmation is required when the choice can materially change accounting state and the deterministic contracts do not already make the action unambiguous.

The UI must never turn:
- OCR confidence into permission to post;
- a likely bank match into automatic reconciliation;
- a bookkeeping category into a tax-deductibility claim;
- inferred private/business use into an explicit user fact.

Confirmation screens show the factual input, the proposed action and the record(s) that will change.

## 8. Accessibility and interaction baseline

SBC-7 implementation must establish before visual polish:

- full keyboard access on desktop;
- visible focus states;
- semantic form labels and error associations;
- no colour-only status communication;
- sufficient text/control contrast;
- scalable text without clipped critical controls;
- touch targets suitable for tablet/mobile shells;
- screen-reader meaningful names for icon-only actions;
- predictable confirmation/cancel focus behaviour;
- destructive/correction actions clearly differentiated from ordinary save.

A later formal accessibility audit may tighten the target, but the component/interface contract must not make accessibility an afterthought.

## 9. Responsive/platform boundary

SBC-7 owns shared information architecture and application semantics, not browser persistence parity.

- Windows desktop: primary implementation/proof surface.
- iPhone/iPad: reuse journeys/components where platform shell permits; physical-device/release proof remains later.
- Browser/PWA: the UI may be designed responsively, but SQLite/OPFS durability, encryption parity and OCR/browser-runtime proof remain SBC-17.

Do not create platform-specific accounting rules.

## 10. Component-reuse position

Frappe Books remains a behavioural/reference product, not a code source for SharkBooks runtime because its application licence is not the selected SharkBooks licence boundary.

The separate MIT-licensed `frappe-ui` component project remains a candidate for generic Vue controls/components. SBC-7 implementation must still:
- pin an exact reviewed version if adopted;
- record licence/NOTICE obligations;
- use it for generic UI mechanics, not copy Frappe Books product/business logic;
- keep SharkBooks information architecture and journeys Shark-owned;
- prefer a smaller dependency surface if ordinary Vue/CSS components satisfy the same requirements.

No Vue/Frappe UI dependency is introduced by this research draft.

## 11. UI acceptance fixtures

Before SBC-7 closes, exercise at minimum:

1. fresh sole trader opens books and reaches useful Home empty state;
2. imports a statement and sees preview/duplicate outcomes;
3. records one invoice and allocates a payment;
4. records a business expense and a private/mixed-use case with explicit choice;
5. attaches a document without OCR;
6. if SBC-6 is selected, adds a receipt, reviews factual extraction and confirms/rejects a suggested match;
7. confirms a bank match and completes exact-zero reconciliation;
8. corrects a record through the preserved-history path;
9. handles an invalid file/wrong-key/storage/OCR-unavailable error without losing work;
10. completes all core journeys without being asked to enter a debit or credit.

## 12. Stage E/F boundary

This document is a Stage A/E design artefact only.

SBC-7 product implementation is blocked until:
- SBC-6 is formally PASS/merged, or OCR is explicitly deferred from V1 by its preregistered gate;
- the UI component dependency decision is frozen;
- the exact application/facade operations required by each journey have been reconciled with the repository;
- a bounded implementation path and regression plan are approved.

Visual branding/polish follows functional journey proof. The first objective is not to make an accounting engine look attractive; it is to make ordinary sole-trader bookkeeping understandable and difficult to misuse.
