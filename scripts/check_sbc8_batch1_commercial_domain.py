#!/usr/bin/env python3
"""Fail-closed SBC-8 Batch 1 preactivation commercial-domain validator."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import subprocess
import sys

BASE = "8f55eaf67b311a7c2d69ebea86ae702e4934a56c"
RUST_TOOLCHAIN = "1.98.1"

ALLOWED_PATHS = {
    "ci/run_sbc8_batch1_commercial_windows.ps1",
    "docs/SBC8_BATCH1_COMMERCIAL_ENGINE_CONTRACT_2026-09-14.md",
    "docs/SBC8_BATCH1_COMMERCIAL_ENGINE_IMPLEMENTATION_DESIGN_2026-09-14.md",
    "docs/SBC8_PREACTIVATION_INCUBATION_AMENDMENT_2026-09-14.md",
    "incubator/sbc8-commercial-core/Cargo.lock",
    "incubator/sbc8-commercial-core/Cargo.toml",
    "incubator/sbc8-commercial-core/src/documents.rs",
    "incubator/sbc8-commercial-core/src/identity.rs",
    "incubator/sbc8-commercial-core/src/lib.rs",
    "incubator/sbc8-commercial-core/src/payments.rs",
    "incubator/sbc8-commercial-core/src/primitives.rs",
    "incubator/sbc8-commercial-core/src/purchasing.rs",
    "scripts/check_sbc8_batch1_commercial_domain.py",
}

RUNTIME_FILES = [
    "incubator/sbc8-commercial-core/src/documents.rs",
    "incubator/sbc8-commercial-core/src/identity.rs",
    "incubator/sbc8-commercial-core/src/lib.rs",
    "incubator/sbc8-commercial-core/src/payments.rs",
    "incubator/sbc8-commercial-core/src/primitives.rs",
    "incubator/sbc8-commercial-core/src/purchasing.rs",
]


def run(repo: Path, args: list[str]) -> None:
    proc = subprocess.run(args, cwd=repo, text=True)
    if proc.returncode != 0:
        raise RuntimeError(f"command failed ({proc.returncode}): {' '.join(args)}")


def capture(repo: Path, args: list[str]) -> str:
    proc = subprocess.run(
        args,
        cwd=repo,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or f"command failed: {' '.join(args)}")
    return proc.stdout


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)
    print(f"[PASS] {message}")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def text(repo: Path, path: str) -> str:
    return (repo / path).read_text(encoding="utf-8")


def static_gate(repo: Path) -> dict[str, object]:
    head = capture(repo, ["git", "rev-parse", "HEAD"]).strip()
    run(repo, ["git", "merge-base", "--is-ancestor", BASE, head])
    require(True, "SBC-8 incubation entry main is an ancestor of HEAD")

    changed = {
        line.strip()
        for line in capture(repo, ["git", "diff", "--name-only", f"{BASE}..HEAD"]).splitlines()
        if line.strip()
    }
    require(changed == ALLOWED_PATHS, f"exact Batch 1 incubation allow-list ({len(ALLOWED_PATHS)} paths)")

    for protected in [
        "product",
        "workspace",
        "browser",
        "upstream",
        "codemagic.yaml",
        ".github",
    ]:
        drift = capture(repo, ["git", "diff", "--name-only", f"{BASE}..HEAD", "--", protected]).strip()
        require(not drift, f"active SBC-7/protected surface unchanged: {protected}")

    amendment = text(repo, "docs/SBC8_PREACTIVATION_INCUBATION_AMENDMENT_2026-09-14.md")
    contract = text(repo, "docs/SBC8_BATCH1_COMMERCIAL_ENGINE_CONTRACT_2026-09-14.md")
    design = text(repo, "docs/SBC8_BATCH1_COMMERCIAL_ENGINE_IMPLEMENTATION_DESIGN_2026-09-14.md")
    runner = text(repo, "ci/run_sbc8_batch1_commercial_windows.ps1")
    require("PREACTIVATION INCUBATION: AUTHORISED" in amendment, "explicit preactivation incubation authority present")
    require("PRODUCTION ACTIVATION / INTEGRATION: STILL BLOCKED" in amendment, "production integration remains blocked")
    require("zero runtime dependencies" in contract, "Batch 1 contract freezes zero dependency runtime")
    require("incubator/sbc8-commercial-core" in design, "standalone incubator package is frozen")
    require("[string]$ExpectedHead" in runner, "Windows proof requires an explicit expected candidate SHA")
    require("Wrong candidate HEAD before proof" in runner, "Windows proof rejects the wrong entry candidate SHA")
    require("Candidate HEAD changed during proof" in runner, "Windows proof rejects candidate movement during execution")
    require("expected_candidate_head" in runner, "Windows evidence records the expected candidate SHA")

    manifest = text(repo, "incubator/sbc8-commercial-core/Cargo.toml")
    lock = text(repo, "incubator/sbc8-commercial-core/Cargo.lock")
    require("[dependencies]\n" in manifest and manifest.rstrip().endswith("[dependencies]"), "standalone crate declares zero dependencies")
    require(lock.count("[[package]]") == 1 and 'name = "sbc8-commercial-core"' in lock, "standalone lock contains only the incubator crate")

    runtime = "\n".join(text(repo, path).split("#[cfg(test)]", 1)[0] for path in RUNTIME_FILES).lower()
    forbidden = [
        "rusqlite",
        "beankeeper",
        "tauri::",
        "tauri_plugin",
        "reqwest",
        "ureq",
        "std::net",
        "tokio::",
        "command::new",
        "std::process",
        "sqlite",
        "http://",
        "https://",
        "account_code",
        "postingplan",
        "vat",
        "tax_rate",
        "card_number",
        "cvv",
        "passphrase",
    ]
    for marker in forbidden:
        require(marker not in runtime, f"runtime excludes forbidden/integration marker: {marker}")

    anchors = {
        "documents.rs": [
            "pub enum QuoteState",
            "pub struct IssuedQuoteSnapshot",
            "pub fn to_draft_invoice",
            "pub enum InvoiceState",
            "pub struct IssuedInvoiceSnapshot",
            "pub fn apply_payment",
            "pub fn create_credit_note",
        ],
        "payments.rs": [
            "pub struct PaymentIntent",
            "pub struct PaymentEvent",
            "pub struct PaymentEventRegistry",
            "pub fn apply_verified_event",
            "pub fn allocate_to_invoice",
            "provider event id was reused with conflicting immutable content",
            "cumulative invoice allocations exceed payment intent amount",
            "pub struct Settlement",
        ],
        "purchasing.rs": [
            "pub struct SupplierBill",
            "pub fn approve",
            "pub fn duplicate_key",
            "pub struct PurchaseOrder",
            "pub fn receive",
            "pub fn compare_bill",
            "duplicate bill comparison mapping for purchase order line",
        ],
    }
    for filename, values in anchors.items():
        body = text(repo, f"incubator/sbc8-commercial-core/src/{filename}")
        for anchor in values:
            require(anchor in body, f"{filename} contains required anchor: {anchor}")

    required_tests = [
        "bounded_ids_fail_closed",
        "checked_money_overflow_fails",
        "snapshots_do_not_follow_later_party_edits",
        "quote_draft_mutates_then_issue_freezes_snapshot",
        "invalid_quote_transition_fails_closed",
        "accepted_quote_creates_new_draft_invoice_identity",
        "invoice_draft_customer_mutates_before_issue_then_freezes",
        "invoice_partial_then_final_payment_is_exact",
        "invoice_overpayment_fails",
        "credit_note_is_distinct_and_bounded_by_outstanding",
        "issued_invoice_snapshot_is_immutable",
        "provider_event_registry_is_idempotent_and_conflict_closed",
        "invalid_payment_event_does_not_mutate_intent",
        "invalid_first_event_does_not_bind_provider_identity",
        "verified_success_does_not_allocate_invoice_without_explicit_action",
        "cumulative_payment_allocation_cannot_exceed_intent_amount",
        "settlement_arithmetic_is_exact",
        "supplier_bill_requires_explicit_approval_then_partial_and_final_payment",
        "supplier_payment_overallocation_fails",
        "supplier_credit_identity_is_distinct_from_bill",
        "supplier_duplicate_key_is_canonical",
        "purchase_order_receives_partially_then_fully_and_rejects_overreceipt",
        "po_to_bill_comparison_reports_variance_only",
        "po_to_bill_comparison_rejects_duplicate_line_mappings",
        "po_to_bill_comparison_rejects_negative_unit_cost",
        "representative_quote_invoice_payment_bill_and_po_flow_is_domain_only",
    ]
    all_source = "\n".join(text(repo, path) for path in RUNTIME_FILES)
    for test_name in required_tests:
        require(test_name in all_source, f"required focused regression declared: {test_name}")

    status = capture(repo, ["git", "status", "--porcelain"]).strip()
    require(not status, "repository clean before runtime gate")

    return {
        "head": head,
        "changed_paths": sorted(changed),
        "cargo_lock_sha256": sha256(repo / "incubator/sbc8-commercial-core/Cargo.lock"),
    }


def runtime_gate(repo: Path) -> None:
    manifest = repo / "incubator" / "sbc8-commercial-core" / "Cargo.toml"
    run(repo, ["rustup", "run", RUST_TOOLCHAIN, "rustc", "--version"])
    run(
        repo,
        [
            "rustup",
            "run",
            RUST_TOOLCHAIN,
            "cargo",
            "test",
            "--manifest-path",
            str(manifest),
            "--locked",
            "--jobs",
            "1",
        ],
    )
    require(True, "standalone SBC-8 Batch 1 commercial-domain tests pass")
    status = capture(repo, ["git", "status", "--porcelain"]).strip()
    require(not status, "repository clean after runtime gate")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--static-only", action="store_true")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    try:
        result = static_gate(repo)
        if not args.static_only:
            runtime_gate(repo)
        print(f"[PASS] SBC-8 Batch 1 commercial-domain gate complete at {result['head']}")
        print(f"[INFO] Cargo.lock SHA-256 {result['cargo_lock_sha256']}")
        return 0
    except Exception as exc:
        print(f"[FAIL] {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
