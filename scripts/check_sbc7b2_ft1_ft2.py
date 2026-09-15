#!/usr/bin/env python3
from __future__ import annotations

import argparse
import collections
import json
import re
import subprocess
import sys
from pathlib import Path

BASE = "99c3b0d16fe4934f1b397a58956df745203f744f"
EXPECTED_ALIASES = {
    "MONEY_IN.DAILY_TAKINGS": "MONEY_IN.SAVE",
    "MONEY_OUT.MIXED_USE": "MONEY_OUT.SAVE",
    "MONEY_OUT.PRIVATE_FROM_BUSINESS_FUNDS": "MONEY_OUT.SAVE",
    "CONTACT.CREATE_CUSTOMER": "CONTACTS.SAVE",
    "CONTACT.CREATE_SUPPLIER": "CONTACTS.SAVE",
}
EXPECTED_PRESETS = {
    "MONEY_IN.DAILY_TAKINGS": {"category": "Sales/Trading"},
    "MONEY_OUT.MIXED_USE": {"business_use": "mixed"},
    "MONEY_OUT.PRIVATE_FROM_BUSINESS_FUNDS": {"business_use": "private"},
    "CONTACT.CREATE_CUSTOMER": {"kind": "customer"},
    "CONTACT.CREATE_SUPPLIER": {"kind": "supplier"},
}
BATCH_B_OVERLAY = {
    "CONTACTS.LIST",
    "CONTACTS.SAVE",
    "SETTINGS.BOOKS_INFO",
    "SETTINGS.STORAGE_ROOT.SELECT",
    "REPORT.SUMMARY",
}
PROHIBITED = {
    "RECORD.DELETE",
    "INVENTORY.COGS",
    "INVENTORY.FULL_VALUATION",
    "AI.AUTONOMOUS_ACCOUNTING",
    "CIS.PROCESS",
    "COMPANY.PARTNERSHIP_ACCOUNTING",
    "HMRC.DIRECT_SUBMIT",
    "OPEN_BANKING.LIVE_FEEDS",
    "PAYROLL.PROCESS",
    "VAT.ACCOUNTING_FILING",
}

ALLOWED = {
    "docs/SBC7B2_FT1_FT2_SUPERGATE_IMPLEMENTATION_CONTRACT_2026-09-15.md",
    "docs/SBC7B2_ACTION_SYSTEM_DESIGN_2026-09-15.md",
    "docs/SBC7B2_FT1_FT2_REUSE_ADMISSION_2026-09-15.md",
    "workspace/shark-foundation/src/action_system.rs",
    "workspace/shark-foundation/src/lib.rs",
    "workspace/shark-foundation/data/action_registry_v1_manifest.json",
    "workspace/shark-foundation/Cargo.toml",
    "workspace/Cargo.lock",
    "tools/action-contract/action_contract.rs",
    "contracts/action-system/v1/action_contract.schema.json",
    "contracts/action-system/v1/action_contract.ts",
    "contracts/action-system/v1/action_contract.swift",
    "scripts/check_sbc7b2_ft1_ft2.py",
    "workspace/shark-foundation/tests/action_system_contract.rs",
    "ci/run_sbc7b2_supergate_windows.ps1",
    "ci/run_sbc7b2_supergate_codemagic.sh",
    "codemagic.yaml",
} | {
    f"workspace/shark-foundation/data/action_registry_v1_chunk{i:02d}.jsonl"
    for i in range(1, 25)
} | {
    f"workspace/shark-foundation/src/action_system/part{i:02d}.rs"
    for i in range(1, 5)
}

REQUIRED_INTEGRATION_PATHS = {
    "workspace/shark-foundation/src/lib.rs",
    "workspace/shark-foundation/Cargo.toml",
    "workspace/Cargo.lock",
    "tools/action-contract/action_contract.rs",
    "contracts/action-system/v1/action_contract.schema.json",
    "contracts/action-system/v1/action_contract.ts",
    "ci/run_sbc7b2_supergate_windows.ps1",
    "ci/run_sbc7b2_supergate_codemagic.sh",
    "codemagic.yaml",
}


def fail(message: str) -> None:
    raise AssertionError(message)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)
    print(f"[PASS] {message}")


def git(repo: Path, *args: str) -> str:
    return subprocess.check_output(["git", "-C", str(repo), *args], text=True).strip()


def load_registry(repo: Path):
    manifest_path = repo / "workspace/shark-foundation/data/action_registry_v1_manifest.json"
    require(manifest_path.is_file(), "Action Registry manifest exists")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    actions = []
    for i in range(1, 25):
        path = repo / f"workspace/shark-foundation/data/action_registry_v1_chunk{i:02d}.jsonl"
        require(path.is_file(), f"Action Registry chunk {i:02d} exists")
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.strip():
                actions.append(json.loads(line))
    return manifest, actions


def validate_registry(repo: Path) -> None:
    manifest, actions = load_registry(repo)
    aliases = manifest["aliases"]
    require(manifest["schema"] == "sharkbooks-action-registry-v1", "registry schema id is v1")
    require(len(actions) == 206, "registry contains exactly 206 canonical actions")
    require(len(aliases) == 5, "registry contains exactly five convenience aliases")

    ids = [a["action_id"] for a in actions]
    require(len(set(ids)) == 206, "canonical Action IDs are unique")
    action_by_id = {a["action_id"]: a for a in actions}
    alias_map = {a["alias_action_id"]: a["target_action_id"] for a in aliases}
    alias_presets = {a["alias_action_id"]: a["preset"] for a in aliases}
    require(alias_map == EXPECTED_ALIASES, "convenience aliases canonicalise to frozen targets")
    require(alias_presets == EXPECTED_PRESETS, "convenience aliases retain deterministic preset facts")
    require(not (set(ids) & set(alias_map)), "aliases do not duplicate canonical Action IDs")
    require(all(target in action_by_id for target in alias_map.values()), "every alias target is canonical")

    voice = [a for a in actions if a["voice_eligible"]]
    parity = collections.Counter(a["voice_parity"] for a in actions)
    require(len(voice) == 175, "exactly 175 canonical actions are voice eligible")
    require(parity["REQUIRED_WHEN_EXPOSED"] == 32, "32 actions require voice when exposed")
    require(parity["REQUIRED_WHEN_ACTIVATED"] == 143, "143 actions require voice when activated")
    require(parity["NOT_APPLICABLE_INTERNAL"] == 19, "19 actions are internal/non-voice")
    require(parity["NO"] == 12, "12 actions have voice parity NO")
    require(sum(len(a["utterances"]) for a in voice) == 700, "registry contains exactly 700 voice fixtures")
    require(all(len(a["utterances"]) == 4 for a in voice), "every voice action has exactly four initial fixtures")
    require(all(not a["utterances"] for a in actions if not a["voice_eligible"]), "non-voice actions carry no executable utterance fixtures")
    require(all(slot["slot_id"] != "none" for a in actions for slot in a["required_slots"]), "literal none sentinel is represented as zero slots")

    for action_id in sorted(BATCH_B_OVERLAY):
        action = action_by_id[action_id]
        require(action["implementation_state"] == "PRODUCTION_MAIN", f"{action_id} is protected-main implemented")
        require(action["availability_state"] == "BACKEND_READY_UI_LOCKED", f"{action_id} remains UI locked")
        require(action["evidence_state"] == "SBC7B1_BATCH_B_DUAL_PLATFORM_PASS", f"{action_id} retains Batch-B evidence")
        require(action["backend_state"] == "READY", f"{action_id} backend is READY")

    for action_id in sorted(PROHIBITED):
        action = action_by_id[action_id]
        require(not action["voice_eligible"], f"prohibited {action_id} is not voice eligible")
        require(action["backend_state"] == "NOT_AUTHORISED", f"prohibited {action_id} is not executable")

    normalized = collections.defaultdict(set)
    for action in voice:
        for utterance in action["utterances"]:
            text = " ".join(str(utterance).lower().strip(" .!?\t\r\n").split())
            normalized[text].add(action["action_id"])
    require({"BOOKS.OPEN", "NAVIGATION.BOOKS_HOME"} <= normalized.get("open my books", set()), "open-my-books ambiguity fixture remains explicit")
    require(any({"QUOTE.PDF_RENDER", "INVOICE.PDF_RENDER"} <= x for x in normalized.values()), "quote/invoice render collision remains explicit")
    require(any({"QUOTE.SHARE_SEND", "INVOICE.SHARE_SEND"} <= x for x in normalized.values()), "quote/invoice share collision remains explicit")


def validate_controller(repo: Path) -> None:
    facade = repo / "workspace/shark-foundation/src/action_system.rs"
    parts = [repo / f"workspace/shark-foundation/src/action_system/part{i:02d}.rs" for i in range(1, 5)]
    wrapper = repo / "workspace/shark-foundation/tests/action_system_contract.rs"
    lib = repo / "workspace/shark-foundation/src/lib.rs"
    for path in [facade, *parts, wrapper, lib]:
        require(path.is_file(), f"required Action System source exists: {path.relative_to(repo)}")
    facade_text = facade.read_text(encoding="utf-8")
    source = facade_text + "\n" + "\n".join(p.read_text(encoding="utf-8") for p in parts)
    for i in range(1, 5):
        require(f'include!("action_system/part{i:02d}.rs")' in facade_text, f"Action System facade includes part {i:02d}")
    for anchor in [
        "ResolutionState", "Known", "Unknown", "Ambiguous", "Conflicting",
        "ExecutionAvailability", "ConfirmationReceipt", "ReplayGuard", "AttentionItem",
        "REPLAY_DETECTED", "STALE_CONFIRMATION", "load_action_registry", "AliasRecipe",
        "accept_call", "missing_context_slots", "state_revision", "facts_fingerprint",
    ]:
        require(anchor in source, f"controller anchor exists: {anchor}")
    accept_pos = source.index("pub fn accept_call")
    require(source.index("replay_guard.register", accept_pos) > source.index("validate_confirmation", accept_pos), "replay id is consumed only after confirmation validation")
    require("pub mod action_system;" in lib.read_text(encoding="utf-8"), "Action System is exported through the production Foundation facade")


def validate_reuse_and_generated_contracts(repo: Path) -> None:
    cargo = (repo / "workspace/shark-foundation/Cargo.toml").read_text(encoding="utf-8")
    lock = (repo / "workspace/Cargo.lock").read_text(encoding="utf-8")
    generator = (repo / "tools/action-contract/action_contract.rs").read_text(encoding="utf-8")
    reuse = (repo / "docs/SBC7B2_FT1_FT2_REUSE_ADMISSION_2026-09-15.md").read_text(encoding="utf-8")
    schema_path = repo / "contracts/action-system/v1/action_contract.schema.json"
    ts_path = repo / "contracts/action-system/v1/action_contract.ts"

    require('action-contract-gen = ["dep:schemars", "dep:ts-rs"]' in cargo, "contract generator feature is explicit and off by default")
    require('schemars = { version = "=1.2.2"' in cargo, "Schemars is exact pinned to 1.2.2")
    require('ts-rs = { version = "=12.0.1"' in cargo, "ts-rs is exact pinned to 12.0.1")
    require('name = "schemars"\nversion = "1.2.2"' in lock, "Cargo.lock resolves Schemars 1.2.2")
    require('name = "ts-rs"\nversion = "12.0.1"' in lock, "Cargo.lock resolves ts-rs 12.0.1")
    require("Config::default()" in generator, "ts-rs generation uses deterministic Config::default")
    require("Config::from_env" not in generator, "generator does not admit TS_RS environment drift")
    require(generator.count("struct Smoke") + generator.count("enum Smoke") >= 10, "ten-type generator smoke corpus is present")
    require("Typeshare 1.0.5" in reuse and "ts-rs 12.0.1" in reuse, "reuse record captures Typeshare rejection and ts-rs fallback")

    require(schema_path.is_file(), "generated JSON Schema contract exists")
    require(ts_path.is_file(), "generated TypeScript contract exists")
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    ts = ts_path.read_text(encoding="utf-8")
    require(schema.get("$id") == "urn:sharkbooks:action-system:v1", "generated schema has frozen Action System id")
    generators = schema.get("x-shark-generators", {})
    require(generators.get("json_schema") == "schemars 1.2.2", "generated schema records Schemars 1.2.2")
    require(generators.get("typescript") == "ts-rs 12.0.1", "generated schema records ts-rs 12.0.1")
    for name in ["ActionRegistryCounts", "ActionRegistryPolicy", "ActionAlias", "ConfirmationClass", "SlotSource", "SlotSpec", "ActionSpec", "ActionRegistryDocument"]:
        require(name in schema.get("$defs", {}), f"generated schema contains {name}")
        require(name in ts, f"generated TypeScript contains {name}")
    require("schemars 1.2.2" in ts and "ts-rs 12.0.1" in ts, "generated TypeScript records exact generator versions")


def validate_supergate_scaffolding(repo: Path) -> None:
    win = repo / "ci/run_sbc7b2_supergate_windows.ps1"
    mac = repo / "ci/run_sbc7b2_supergate_codemagic.sh"
    cm = repo / "codemagic.yaml"
    for path in [win, mac, cm]:
        require(path.is_file(), f"Super-Gate path exists: {path.relative_to(repo)}")
    win_text = win.read_text(encoding="utf-8")
    mac_text = mac.read_text(encoding="utf-8")
    cm_text = cm.read_text(encoding="utf-8")
    for gate in [f"SG{i}" for i in range(9)]:
        require(gate in win_text and gate in mac_text, f"Windows and Apple runners represent {gate}")
    require("NOT_APPLICABLE_TO_THIS_CANDIDATE" in win_text and "NOT_APPLICABLE_TO_THIS_CANDIDATE" in mac_text, "FT1/FT2 Super-Gate records UI SG3/SG4 as not applicable")
    require("sbc7b2-ft1-ft2-supergate-apple" in cm_text, "Codemagic registers the FT1/FT2 Apple Super-Gate workflow")


def validate_git_scope(repo: Path) -> None:
    head = git(repo, "rev-parse", "HEAD")
    git(repo, "merge-base", "--is-ancestor", BASE, head)
    changed = {x for x in git(repo, "diff", "--name-only", f"{BASE}..{head}").splitlines() if x}
    extra = changed - ALLOWED
    require(not extra, f"candidate changed paths remain inside frozen/amended envelope (extra={sorted(extra)})")
    missing = REQUIRED_INTEGRATION_PATHS - changed
    require(not missing, f"candidate includes required integration paths (missing={sorted(missing)})")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", default=".")
    parser.add_argument("--skip-git", action="store_true")
    args = parser.parse_args()
    repo = Path(args.repo).resolve()
    try:
        validate_registry(repo)
        validate_controller(repo)
        validate_reuse_and_generated_contracts(repo)
        validate_supergate_scaffolding(repo)
        if not args.skip_git:
            validate_git_scope(repo)
    except (AssertionError, KeyError, ValueError, json.JSONDecodeError, subprocess.CalledProcessError) as exc:
        print(f"[FAIL] {exc}", file=sys.stderr)
        return 1
    print("[PASS] SBC-7B2 FT1+FT2 integrated candidate static gate")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
