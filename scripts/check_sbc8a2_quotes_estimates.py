#!/usr/bin/env python3
"""Fail-closed SBC8A2 implementation contract and boundary check."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CONTRACT = Path(r"C:\SharkAutopilot\lanes\SBC\evidence\SBC8A2_CONTRACT_FREEZE.json")


def git(*args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or f"git {' '.join(args)} failed")
    return result.stdout.strip()


def changed_paths(base: str) -> set[str]:
    changed = set(filter(None, git("diff", "--name-only", base).splitlines()))
    for line in git("status", "--porcelain", "--untracked-files=all").splitlines():
        if not line.startswith("?? "):
            continue
        changed.add(line[3:].replace("\\", "/"))
    return changed


def require(condition: bool, code: str, problems: list[dict[str, object]], detail: object) -> None:
    if not condition:
        problems.append({code: detail})


def source(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8", errors="strict")


def main() -> int:
    problems: list[dict[str, object]] = []
    contract = json.loads(CONTRACT.read_text(encoding="utf-8-sig"))
    base = contract["entry_sha"]
    head = git("rev-parse", "HEAD")
    allowed = set(contract["allowed_paths"])
    changed = changed_paths(base)

    require(contract.get("result") == "PASS_CONTRACT_FROZEN", "CONTRACT_NOT_FROZEN", problems, contract.get("result"))
    require(head == base, "ENTRY_SHA_MOVED", problems, {"expected": base, "actual": head})
    require(changed <= allowed, "CHANGED_PATH_OUTSIDE_CONTRACT", problems, sorted(changed - allowed))
    require(not any(Path(path).name in {"Cargo.lock", "package-lock.json", "pnpm-lock.yaml", "yarn.lock"} for path in changed), "DEPENDENCY_LOCK_CHANGED", problems, sorted(changed))

    required_files = {
        "product/shark-books-core/src/lib.rs": [
            "pub enum QuoteState { Draft, Issued, Accepted, Rejected, Expired, Cancelled }",
            "pub struct IssuedQuoteSnapshot",
            "pub const fn conversion_eligible",
        ],
        "workspace/shark-foundation/src/quote_application.rs": [
            "pub fn create_quote_draft",
            "pub fn issue_quote",
            "pub fn transition_quote",
            "pub fn quote_history",
            "conversion_eligible",
        ],
        "workspace/shark-foundation/src/bank_application/mod.rs": [
            "shark_quote_issue_snapshot",
            "trg_shark_quote_snapshot_no_update",
            "trg_shark_quote_mutation_no_delete",
        ],
        "workspace/shark-tauri-spike/src/owner_supporting_data.rs": [
            "OwnerQuoteMutationRequest",
            "invoice_created: false",
            "non_posting: true",
        ],
        "workspace/ui/src/screens/QuotesScreen.vue": [
            "eligible for later 8A3 conversion",
            "Mutation history",
            "No invoice",
        ],
        "workspace/ui/src/components/QuotesTable.vue": ["Quotes and estimates"],
    }
    for path, markers in required_files.items():
        file_path = ROOT / path
        require(file_path.is_file(), "MISSING_REQUIRED_FILE", problems, path)
        if file_path.is_file():
            text = source(path)
            missing = [marker for marker in markers if marker not in text]
            require(not missing, "MISSING_REQUIRED_MARKER", problems, {path: missing})

    quote_application = source("workspace/shark-foundation/src/quote_application.rs")
    runtime = quote_application.split("#[cfg(test)]", 1)[0].lower()
    require(".post(" not in runtime, "POSTING_AUTHORITY_PRESENT", problems, ".post(")
    invoice_creation_markers = ["create_invoice", "insert into shark_invoice", "shark_invoice"]
    require(
        not any(marker in runtime for marker in invoice_creation_markers),
        "INVOICE_CREATION_SURFACE_PRESENT",
        problems,
        invoice_creation_markers,
    )

    combined = "\n".join(
        source(path).lower()
        for path in required_files
        if (ROOT / path).is_file()
    )
    forbidden_markers = [
        "vat_rate",
        "cis_rate",
        "open_banking",
        "payment_provider",
        "pdf_renderer",
        "purchase_order",
        "inventory_item",
        "work_order",
        "reqwest",
    ]
    found_forbidden = [marker for marker in forbidden_markers if marker in combined]
    require(not found_forbidden, "PROHIBITED_CAPABILITY_MARKER", problems, found_forbidden)

    bridge = source("workspace/shark-tauri-spike/src/owner_supporting_data.rs")
    tauri_client = source("workspace/ui/src/lib/tauri.ts")
    require("owner_quotes_" not in bridge + tauri_client, "UNREGISTERED_TAURI_COMMAND_INTRODUCED", problems, "owner_quotes_*")
    require(
        all(command in tauri_client for command in ['nativeInvoke("owner_contacts_list"', 'nativeInvoke("owner_contacts_save"']),
        "REGISTERED_BRIDGE_COMPATIBILITY_SEAM_MISSING",
        problems,
        ["owner_contacts_list", "owner_contacts_save"],
    )

    result = {
        "result": "PASS" if not problems else "FAIL",
        "roadmap_node": "SBC8A2_IMPLEMENT",
        "base_sha": base,
        "head": head,
        "changed_paths": sorted(changed),
        "posting": contract.get("posting"),
        "problems": problems,
    }
    print(json.dumps(result, indent=2))
    return 0 if not problems else 2


if __name__ == "__main__":
    raise SystemExit(main())
