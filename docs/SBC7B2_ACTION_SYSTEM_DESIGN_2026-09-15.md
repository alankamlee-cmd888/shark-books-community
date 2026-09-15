# SBC-7B2 Action System — FT1/FT2 Design

**Date:** 15 September 2026  
**Protected base:** `99c3b0d16fe4934f1b397a58956df745203f744f`  
**Status:** **DESIGN FROZEN FOR FIRST IMPLEMENTATION BATCH**

## Architecture

```text
manual UI / command text / voice transcript / camera / AppIntent
                    |
                    v
            candidate Action IDs
                    |
                    v
          Shark Action Registry v1
                    |
                    v
     deterministic Action Controller
       |       |        |        |
     KNOWN  UNKNOWN  AMBIGUOUS CONFLICTING
                    |
                    v
     availability + slots + confirmation
                    |
                    v
       typed Shark operation adapter
              (later layer)
```

The Action System is a control plane. It does not own ledger posting, bank matching, OCR facts, tax rules or document persistence.

## Registry document

The checked-in registry is data-driven and contains all 206 canonical actions plus five alias recipes.

Every canonical action records:

- stable Action ID + version;
- family, owner label and owner intent;
- implementation/availability/evidence state;
- backend readiness;
- authority class;
- raw and normalized confirmation policy;
- voice parity;
- required slot metadata;
- owner/context source classification;
- predefined-choice hints;
- hard boundary;
- backend/provenance basis;
- four representative utterance fixtures for every voice-eligible canonical action.

The 175-action voice manifest is therefore metadata on the same canonical actions, not a separate voice command set.

## Exposure rule

`BACKEND_READY_UI_LOCKED` means backend capability exists, not that the user is allowed to execute it from every surface.

`ActionController` receives an explicit reviewed exposure set. A backend-ready action absent from that set returns a locked result.

Future, SBC8 production-locked, internal and not-authorised actions always fail closed regardless of phrasing.

## Resolution state

- `KNOWN`: exactly one canonical Action ID is identified and supplied facts do not conflict. Execution may still be locked.
- `UNKNOWN`: no canonical Action ID can be identified, or required facts are missing.
- `AMBIGUOUS`: more than one distinct canonical Action ID remains plausible.
- `CONFLICTING`: the same slot has materially different supplied values.

The controller never guesses across those states.

## Slots

Planning-language `required_inputs` are converted to machine slot records. Each slot has:

- stable `slot_id`;
- owner-language label;
- required/optional flag;
- `OWNER` or `CONTEXT` source;
- choice hints where safely available.

For the current 32 `REQUIRED_WHEN_EXPOSED` actions, context classification is explicitly overlaid so internal values such as actor/current books/current statement/native selection/current suggestion do not become unnecessary conversational questions.

## Confirmation

Raw planning confirmation policies are preserved, while a normalized enforcement class is used by the controller:

- `NONE`;
- `EXPLICIT`;
- `STRONG_EXPLICIT`.

Special policies such as `EXPLICIT_AFTER_PREVIEW`, `EXPLICIT_OS_PICKER`, `EXPLICIT_PROVIDER_USE` remain in metadata but normalize to at least `EXPLICIT`.

An interpreter, voice adapter or UI cannot downgrade this class.

## Confirmation receipt

A confirmation receipt binds:

- canonical Action ID;
- Action version;
- current state revision;
- normalized confirmation class;
- deterministic fingerprint of the supplied facts.

Changing state, action version or facts invalidates the receipt.

## Replay

Operation IDs are registered through a deterministic replay guard. A repeated operation ID is rejected before a future executor is allowed to mutate state.

## Attention Queue

The controller can emit an `AttentionItem` for:

- unknown request/missing information;
- ambiguity;
- conflicting facts;
- locked action;
- internal-only request;
- prohibited/not-authorised request;
- stale confirmation;
- replay.

IDs are deterministic from reason + relevant action/slot material so repeated unresolved states do not create uncontrolled duplicates.

## Voice fixture corpus

Exactly four initial representative fixtures are required for each of 175 voice-eligible canonical actions = **700 fixtures**.

Known cross-action near-neighbour collisions are retained as ambiguity tests rather than rewritten to hide them, including:

- “Open my books” -> `BOOKS.OPEN` vs `NAVIGATION.BOOKS_HOME`;
- generic professional-document PDF wording -> quote vs invoice;
- generic rendered-document share/send wording -> quote vs invoice.

The deterministic controller must clarify those collisions before execution.

## Reuse/tooling seam

Runtime Action System Phase A uses no new dependency.

Schemars and Typeshare are tooling/schema-generation decisions layered onto these exact Rust/data contracts before candidate freeze. This lets implementation proceed immediately without weakening exact-version admission discipline.
