#!/usr/bin/env python3
from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys

ENTRY_MAIN = "6f85734d0e9e11089a1a47b7850a306a4ed7ba87"
CRATE = pathlib.Path("incubator/sbc8-inventory-lite-core")

ALLOWED_PATHS = {
    "ci/run_sbc8_batch3_inventory_lite_windows.ps1",
    "docs/SBC8_BATCH3_PREACTIVATION_INCUBATION_AUTHORITY_2026-09-14.md",
    "docs/SBC8_BATCH3_INVENTORY_LITE_CONTRACT_2026-09-14.md",
    "docs/SBC8_BATCH3_INVENTORY_LITE_IMPLEMENTATION_DESIGN_2026-09-14.md",
    "incubator/sbc8-inventory-lite-core/Cargo.lock",
    "incubator/sbc8-inventory-lite-core/Cargo.toml",
    "incubator/sbc8-inventory-lite-core/src/inventory.rs",
    "incubator/sbc8-inventory-lite-core/src/lib.rs",
    "incubator/sbc8-inventory-lite-core/src/primitives.rs",
    "incubator/sbc8-inventory-lite-core/src/stocktake.rs",
    "incubator/sbc8-inventory-lite-core/src/transfer.rs",
    "scripts/check_sbc8_batch3_inventory_lite_domain.py",
}

PROTECTED_PREFIXES = (
    "product/",
    "workspace/",
    "browser/",
    "upstream/",
    ".github/",
    "codemagic.yaml",
)

FORBIDDEN_RUNTIME_MARKERS = (
    "rusqlite",
    "beankeeper",
    "tauri::",
    "tauri_plugin",
    "reqwest",
    "ureq",
    "std::net",
    "tokio::",
    "std::process",
    "command::new",
    "sqlite",
    "http://",
    "https://",
    "rrule::",
    "postingplan",
    "account_code",
    "money",
    "cogs",
    "fifo",
    "moving_average",
    "moving average",
    "valuation",
    "unit conversion",
)

REQUIRED_ANCHORS = {
    "src/primitives.rs": (
        "pub struct PositiveQuantity",
        "pub struct StockBalance",
        "pub fn checked_add",
    ),
    "src/inventory.rs": (
        "pub struct InventoryItem",
        "pub struct InventoryLocation",
        "pub enum StockMovementKind",
        "pub struct StockMovement",
        "pub fn correction_adjustment",
        "pub fn signed_delta",
        "pub fn project_location_balance",
        "pub fn summarize_item",
        "stock movement identity reused",
    ),
    "src/transfer.rs": (
        "pub struct StockTransferProposal",
        "pub fn validate_transfer_pair",
        "transfer pair must conserve quantity",
    ),
    "src/stocktake.rs": (
        "pub struct StocktakeObservation",
        "pub struct StocktakeAdjustmentProposal",
        "pub fn propose_adjustment",
    ),
}

SEALED_STRUCTS = {
    "src/primitives.rs": ("EntityId", "BoundedText", "PositiveQuantity", "StockBalance"),
    "src/inventory.rs": ("InventoryItem", "InventoryLocation", "StockMovement"),
    "src/transfer.rs": ("StockTransferProposal",),
    "src/stocktake.rs": ("StocktakeObservation", "StocktakeAdjustmentProposal"),
}

REQUIRED_TESTS = (
    "bounded_ids_and_text_fail_closed",
    "positive_quantity_rejects_zero",
    "signed_balance_preserves_negative_values",
    "item_and_location_fields_are_validated_and_private",
    "movement_rejects_wrong_tracked_unit",
    "all_movement_kinds_have_deterministic_signed_effects",
    "duplicate_stock_movement_identity_is_rejected",
    "location_balance_is_deterministic_and_traceable",
    "negative_stock_is_explicit_not_clamped",
    "item_summary_derives_cross_location_total",
    "correction_is_append_only_and_links_original",
    "transfer_rejects_same_location",
    "valid_transfer_conserves_total_and_shifts_locations",
    "malformed_transfer_pair_is_rejected",
    "stocktake_equal_count_creates_no_adjustment",
    "stocktake_excess_creates_append_only_increase",
    "stocktake_shortage_creates_append_only_decrease_and_reaches_observed",
    "representative_inventory_lite_flow_is_append_only_and_domain_only",
)


def run(repo: pathlib.Path, *args: str, capture: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        list(args),
        cwd=repo,
        text=True,
        capture_output=capture,
        check=False,
    )


def check(ok: bool, message: str) -> None:
    if not ok:
        print(f"[FAIL] {message}")
        raise SystemExit(1)
    print(f"[PASS] {message}")


def git_text(repo: pathlib.Path, *args: str) -> str:
    result = run(repo, "git", *args)
    check(result.returncode == 0, f"git {' '.join(args)}")
    return result.stdout.strip()


def struct_body(text: str, struct_name: str) -> str | None:
    match = re.search(
        rf"(?ms)^pub struct {re.escape(struct_name)}(?:\([^;]*\);|\s*\{{(?P<body>.*?)^\}})",
        text,
    )
    if match is None:
        return None
    return match.groupdict().get("body") or ""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--static-only", action="store_true")
    parser.add_argument("--expected-head")
    args = parser.parse_args()

    repo = pathlib.Path(__file__).resolve().parents[1]
    head = git_text(repo, "rev-parse", "HEAD")
    check(bool(re.fullmatch(r"[0-9a-f]{40}", head)), "resolved exact 40-character HEAD")
    if args.expected_head:
        check(head.lower() == args.expected_head.lower(), "exact expected candidate HEAD")

    ancestor = run(repo, "git", "merge-base", "--is-ancestor", ENTRY_MAIN, "HEAD")
    check(ancestor.returncode == 0, "Batch 3 entry protected main is an ancestor of HEAD")

    changed = set(filter(None, git_text(repo, "diff", "--name-only", f"{ENTRY_MAIN}..HEAD").splitlines()))
    check(changed == ALLOWED_PATHS, f"exact Batch 3 incubation allow-list ({len(ALLOWED_PATHS)} paths)")

    for prefix in PROTECTED_PREFIXES:
        touched = any(path == prefix or path.startswith(prefix) for path in changed)
        check(not touched, f"active SBC-7/protected surface unchanged: {prefix.rstrip('/')}")

    authority = (repo / "docs/SBC8_BATCH3_PREACTIVATION_INCUBATION_AUTHORITY_2026-09-14.md").read_text(encoding="utf-8")
    contract = (repo / "docs/SBC8_BATCH3_INVENTORY_LITE_CONTRACT_2026-09-14.md").read_text(encoding="utf-8")
    design = (repo / "docs/SBC8_BATCH3_INVENTORY_LITE_IMPLEMENTATION_DESIGN_2026-09-14.md").read_text(encoding="utf-8")
    check("PRODUCTION INTEGRATION BLOCKED" in authority, "production integration remains blocked")
    check("SBC-8F2 valuation/COGS" in authority, "full valuation remains explicitly excluded")
    check("No valuation/COGS" in contract, "Inventory Lite contract excludes valuation/accounting expansion")
    check("Standalone zero-dependency Rust crate" in design, "standalone incubator design is frozen")

    cargo_toml = (repo / CRATE / "Cargo.toml").read_text(encoding="utf-8")
    dependencies_match = re.search(r"(?ms)^\[dependencies\]\s*(.*?)(?=^\[|\Z)", cargo_toml)
    check(dependencies_match is not None and not dependencies_match.group(1).strip(), "standalone crate declares zero dependencies")

    cargo_lock = (repo / CRATE / "Cargo.lock").read_text(encoding="utf-8")
    check(cargo_lock.count("[[package]]") == 1, "standalone lock contains only the incubator crate")
    check('name = "sbc8-inventory-lite-core"' in cargo_lock, "standalone lock identifies the Batch 3 crate")

    runtime_files = sorted((repo / CRATE / "src").glob("*.rs"))
    runtime = "\n".join(path.read_text(encoding="utf-8") for path in runtime_files).lower()
    check("#![forbid(unsafe_code)]" in (repo / CRATE / "src/lib.rs").read_text(encoding="utf-8"), "unsafe code is forbidden")
    for marker in FORBIDDEN_RUNTIME_MARKERS:
        check(marker not in runtime, f"runtime excludes forbidden/integration marker: {marker}")
    check("f32" not in runtime and "f64" not in runtime, "runtime excludes floating-point quantity arithmetic")

    for relative, anchors in REQUIRED_ANCHORS.items():
        text = (repo / CRATE / relative).read_text(encoding="utf-8")
        for anchor in anchors:
            check(anchor in text, f"{relative} contains required anchor: {anchor}")

    for relative, names in SEALED_STRUCTS.items():
        text = (repo / CRATE / relative).read_text(encoding="utf-8")
        for name in names:
            body = struct_body(text, name)
            check(body is not None, f"{relative} exposes sealed struct declaration: {name}")
            if body:
                public_field = re.search(r"(?m)^\s*pub\s+[A-Za-z_][A-Za-z0-9_]*\s*:", body)
                check(public_field is None, f"{relative} keeps {name} invariant fields private")

    for test_name in REQUIRED_TESTS:
        check(test_name in runtime, f"required focused regression declared: {test_name}")

    status_before = git_text(repo, "status", "--porcelain")
    check(status_before == "", "repository clean before runtime gate")

    if not args.static_only:
        command = [
            "cargo",
            "+1.98.1",
            "test",
            "--manifest-path",
            str(CRATE / "Cargo.toml"),
            "--locked",
        ]
        result = run(repo, *command, capture=False)
        check(result.returncode == 0, "standalone SBC-8 Batch 3 Inventory Lite tests pass")
        status_after = git_text(repo, "status", "--porcelain")
        check(status_after == "", "repository clean after runtime gate")

    print(f"[PASS] SBC-8 Batch 3 Inventory Lite domain gate complete at {head}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
