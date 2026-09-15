# Shark Books Community — SBC-8 Preactivation Incubation Amendment

**Date:** 14 September 2026  
**Status:** AUTHORISED AMENDMENT — ISOLATED DOMAIN INCUBATION ONLY  
**Protected entry main:** `8f55eaf67b311a7c2d69ebea86ae702e4934a56c`  
**Branch:** `sbc8-incubation-batch1-commercial-engine-20260914`

## 1. Authority and relationship to the frozen SBC-8 plan

The frozen SBC-8 V1/V2 authority remains the controlling production plan. This amendment records the owner's explicit 14 September 2026 decision to allow useful SBC-8 work to proceed in parallel with SBC-7 **without activating SBC-8 production integration**.

This amendment therefore changes only the preactivation rule:

- pure Shark-owned domain logic, fixtures, tests, contracts and non-integrated reference work may be incubated before SBC-7 closes;
- no incubated SBC-8 code may enter the active SBC-7 workspace, database schema, Tauri command/permission graph, frontend, native platform graph, production Cargo graph or protected `main` while SBC-7 remains open;
- production SBC-8 integration still requires SBC-7 PASS / MERGED / CLOSED and an explicit SBC-8P integration/admission entry from the then-current protected `main`;
- third-party dependencies remain unadmitted unless and until the frozen SBC-8P component admission process passes for the exact version and feature set.

The old freeze records are preserved as historical evidence and are not overwritten.

## 2. Preactivation architecture

The permitted pattern is:

`isolated incubator crate -> Shark-owned pure domain semantics -> zero runtime/network/platform/persistence dependency -> later typed adapter into proven SBC foundation`

The prohibited pattern before SBC-7 closure is:

`incubator -> workspace/shark-foundation / Tauri / Vue / SQLite / Beankeeper / network provider / native plugin`

## 3. Batch 1 authority

The first authorised incubation batch is **SBC-8 Batch 1 — Commercial Engine**.

It may implement, as pure domain code:

- commercial identities and immutable document snapshots;
- quotes/estimates and quote-to-draft-invoice conversion;
- invoices and credit-note relationships;
- provider-neutral payment intent/event/settlement state semantics;
- explicit invoice payment allocation semantics;
- supplier bills and supplier credit/payment semantics;
- purchase orders, partial receiving and deterministic PO-to-bill variance facts;
- deterministic integer-minor-unit arithmetic and bounded identifiers;
- domain events/results and exhaustive deterministic tests.

It may **not** implement yet:

- PDF renderer/runtime admission;
- native share/email integration;
- Stripe or any other provider network connector;
- provider secrets, card data or webhook hosting;
- ledger posting/persistence integration;
- Shark database migrations;
- Tauri commands/permissions;
- Vue/UI binding;
- document filesystem integration;
- VAT/e-invoicing/multi-currency;
- Open Banking;
- autonomous AI/accounting/tax judgement.

## 4. Reference/reuse rule

The V2 source hierarchy remains binding. SolidInvoice and InvoicePlane are behavioural/test references for commercial documents; official payment-provider documentation is primary for provider semantics; Kill Bill/Hyperswitch/FOSSBilling are reference or threat-model sources only; GnuCash/OFBiz are AP/accounting/procurement oracles; InvenTree is the principal purchase-order reference.

No reference application runtime is embedded or closely ported by default. Batch 1 code is Shark-owned.

## 5. Evidence and merge rule

Because this is preactivation incubation, a successful standalone domain proof does **not** authorise production merge or integration. The branch may remain a draft incubation branch/PR until SBC-7 closes.

Before later integration, the then-current SBC-8P gate must re-adjudicate:

- interfaces against the final SBC-7 foundation;
- exact dependencies (if any);
- persistence and posting boundaries;
- Windows and Apple proof appropriate to any integrated/native changes;
- SBOM/licence/NOTICE/security obligations;
- protected-main merge authority.

## 6. Freeze decision

**PREACTIVATION INCUBATION: AUTHORISED.**  
**SBC-8 PRODUCTION ACTIVATION / INTEGRATION: STILL BLOCKED UNTIL SBC-7 CLOSES.**
