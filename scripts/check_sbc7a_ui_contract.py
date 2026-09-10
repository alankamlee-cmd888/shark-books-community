#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = "3fcbbc69a8e30a6d6281ce410f1e5e80c8f43ace"
EXPECTED_PRODUCT_LOCK_SHA256 = "f4b5f2ec20054f8b5675480d2b47dcd708d1df96b4caeb8c35046d301435d3f6"
EXPECTED_NATIVE_LOCK_SHA256 = "3239385c688f64120a6701cdf3f603134950902f2591bae6b7be7248388ec4a9"

ALLOWED_PATHS = {
    "ci/run_sbc7a_windows.ps1",
    "docs/SBC7A_OWNER_FACING_UI_CONTRACT_2026-09-10.md",
    "docs/SBC7A_APPLICATION_OPERATION_MATRIX_2026-09-10.md",
    "scripts/check_sbc7a_ui_contract.py",
}

NAV = (
    "Home",
    "Money in",
    "Money out",
    "Bank",
    "Receipts",
    "Contacts",
    "Reports",
    "Settings",
)

JOURNEYS = (
    "J1 — Start or open books",
    "J2 — Import a bank statement",
    "J3 — Review and match bank activity",
    "J4 — Record money in",
    "J5 — Record money out",
    "J6 — Add a receipt or document",
    "J7 — Reconcile the bank",
    "J8 — Correct a mistake",
)

REQUIRED_CONTRACT_TERMS = (
    "loading",
    "empty/new user",
    "validation error",
    "permission/storage failure",
    "wrong-key/open failure",
    "OCR failure",
    "ambiguous match",
    "duplicate review required",
    "document integrity failure",
    "explicit owner confirmation",
    "keyboard",
    "visible focus",
    "screen-reader",
    "Windows desktop",
    "iPad",
    "iPhone",
    "SBC-17",
    "controlled application/native + UI integration stage",
    "Apple physical/simulator compile regression",
)

CAPABILITY_IDS = (
    "BOOKS.CREATE",
    "BOOKS.OPEN",
    "BOOKS.VERIFY",
    "HOME.STATUS",
    "MONEY_IN.PREVIEW",
    "MONEY_IN.SAVE",
    "MONEY_OUT.PREVIEW",
    "MONEY_OUT.SAVE",
    "BANK.IMPORT_PREVIEW_CSV",
    "BANK.IMPORT_PREVIEW_OFX_QFX",
    "BANK.IMPORT_CONFIRM",
    "BANK.ACTIVITY_LIST",
    "BANK.MATCH_REVIEW",
    "BANK.MATCH_CONFIRM",
    "BANK.RECONCILE_PREVIEW",
    "BANK.RECONCILE_FINALISE",
    "DOCUMENT.SELECT_REGISTER",
    "DOCUMENT.VERIFY",
    "DOCUMENT.ATTACH",
    "OCR.EXTRACT",
    "RECEIPT.SUGGEST_BANK",
    "RECEIPT.CONFIRM_BANK",
    "RECEIPT.REJECT_BANK",
    "CONTACTS.LIST",
    "CONTACTS.SAVE",
    "REPORT.TRIAL_BALANCE",
    "SETTINGS.BOOKS_INFO",
    "SETTINGS.STORAGE_ROOT",
)

ENTRY_TAURI_COMMANDS = (
    "foundation_health",
    "production_encryption_required",
    "books_create",
    "books_open",
    "books_verify",
    "books_trial_balance",
    "ocr_extract_receipt",
)

FRONTEND_ENTRY_COMMANDS = ENTRY_TAURI_COMMANDS[:-1]


def fail(message: str) -> None:
    print(f"[FAIL] {message}", flush=True)
    raise SystemExit(1)


def passed(message: str) -> None:
    print(f"[PASS] {message}", flush=True)


def git(*args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=False,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        fail(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def run_python(script: str, *args: str) -> None:
    result = subprocess.run(
        [sys.executable, str(ROOT / script), *args],
        cwd=ROOT,
        check=False,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n", flush=True)
    if result.stderr:
        print(result.stderr, end="" if result.stderr.endswith("\n") else "\n", file=sys.stderr, flush=True)
    if result.returncode != 0:
        fail(f"{script} exited {result.returncode}")


def main() -> int:
    if git("status", "--short"):
        fail("repository must be clean at SBC-7A validation entry")
    passed("Repository clean at entry")

    head = git("rev-parse", "HEAD")
    passed(f"Candidate HEAD resolved: {head}")

    ancestor = subprocess.run(
        ["git", "merge-base", "--is-ancestor", BASE, "HEAD"],
        cwd=ROOT,
        check=False,
    )
    if ancestor.returncode != 0:
        fail("protected SBC-6D merge is not candidate ancestor")
    passed("Protected SBC-6D merge is candidate ancestor")

    changed = {p for p in git("diff", "--name-only", f"{BASE}...HEAD").splitlines() if p}
    if changed != ALLOWED_PATHS:
        fail(
            "candidate diff is not exact SBC-7A allowlist\n"
            f"expected={sorted(ALLOWED_PATHS)}\nactual={sorted(changed)}"
        )
    passed("Candidate diff is exactly the authorised four-path SBC-7A set")

    if git("rev-parse", f"{BASE}:product") != git("rev-parse", "HEAD:product"):
        fail("product tree changed during SBC-7A design gate")
    if git("rev-parse", f"{BASE}:workspace") != git("rev-parse", "HEAD:workspace"):
        fail("workspace/native/frontend tree changed during SBC-7A design gate")
    passed("Product and workspace/native/frontend trees are byte-identical to SBC-6D main")

    product_lock = ROOT / "product" / "shark-books-core" / "Cargo.lock"
    native_lock = ROOT / "workspace" / "Cargo.lock"
    if sha256(product_lock) != EXPECTED_PRODUCT_LOCK_SHA256:
        fail("product Cargo.lock hash drift")
    if sha256(native_lock) != EXPECTED_NATIVE_LOCK_SHA256:
        fail("native Cargo.lock hash drift")
    passed("Product and native Cargo.lock hashes remain exact")

    contract = (ROOT / "docs" / "SBC7A_OWNER_FACING_UI_CONTRACT_2026-09-10.md").read_text(encoding="utf-8")
    matrix = (ROOT / "docs" / "SBC7A_APPLICATION_OPERATION_MATRIX_2026-09-10.md").read_text(encoding="utf-8")

    for item in NAV:
        if f"**{item}**" not in contract:
            fail(f"UI contract missing frozen navigation item: {item}")
    passed("Primary owner navigation is fully frozen")

    for journey in JOURNEYS:
        if journey not in contract:
            fail(f"UI contract missing owner journey: {journey}")
    passed("All eight required owner journeys are frozen")

    for marker in REQUIRED_CONTRACT_TERMS:
        if marker not in contract:
            fail(f"UI contract missing required state/boundary marker: {marker}")
    passed("Error/degrade, confirmation, accessibility and responsive contracts are present")

    for capability in CAPABILITY_IDS:
        if f"`{capability}`" not in matrix:
            fail(f"application operation matrix missing capability: {capability}")
    passed("Application operation matrix covers all authorised SBC-7 owner capability groups")

    for required in (
        "EXISTING_NATIVE",
        "CORE_AVAILABLE_BRIDGE_REQUIRED",
        "COMPOSITION_REQUIRED",
        "APPLICATION_GAP_7B",
        "native picker/approval owns path",
        "Windows regression is mandatory",
        "Apple physical-device and Simulator compile regression is mandatory",
        "generic shell execution",
        "raw SQL/database access",
        "Open Banking/live feeds",
        "AI/LLM accounting judgement",
    ):
        if required not in matrix:
            fail(f"operation matrix missing required boundary/classification: {required}")
    passed("7B bridge/change-control and forbidden-authority classifications are explicit")

    tauri_lib = (ROOT / "workspace" / "shark-tauri-spike" / "src" / "lib.rs").read_text(encoding="utf-8")
    ocr_native = (ROOT / "workspace" / "shark-tauri-spike" / "src" / "ocr_native.rs").read_text(encoding="utf-8")
    app_js = (ROOT / "workspace" / "dist" / "app.js").read_text(encoding="utf-8")

    for command in ENTRY_TAURI_COMMANDS[:-1]:
        if f"fn {command}(" not in tauri_lib or command not in tauri_lib:
            fail(f"current native shell command missing: {command}")
    if "fn ocr_extract_receipt(" not in ocr_native or "ocr_native::ocr_extract_receipt" not in tauri_lib:
        fail("current native OCR command is not present/wired in Rust handler")
    passed("Seven-command native entry inventory remains accurate")

    for command in FRONTEND_ENTRY_COMMANDS:
        if command not in app_js:
            fail(f"legacy frontend no longer contains expected engineering command: {command}")
    if "ocr_extract_receipt" in app_js:
        fail("SBC-7A must not prematurely wire OCR into the frontend")
    passed("Legacy engineering frontend remains unchanged and OCR remains frontend-unwired")

    run_python("scripts/check_frozen_baseline.py")
    passed("Frozen foundation/native-shell guard passes")
    run_python("scripts/check_sbc1g_change_control.py", "--base-ref", BASE)
    passed("Permanent SBC-1G change-control guard passes for docs/harness-only SBC-7A")

    if git("status", "--short"):
        fail("repository is dirty after SBC-7A validation")
    passed("Repository remains clean after SBC-7A validation")

    print("[PASS] SBC-7A owner-facing UI contract gate complete", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
