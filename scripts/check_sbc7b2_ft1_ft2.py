#!/usr/bin/env python3
from __future__ import annotations

import argparse
import collections
import json
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
PHASE_A_ALLOWED = {
    "docs/SBC7B2_FT1_FT2_SUPERGATE_IMPLEMENTATION_CONTRACT_2026-09-15.md",
    "docs/SBC7B2_ACTION_SYSTEM_DESIGN_2026-09-15.md",
    "workspace/shark-foundation/src/action_system.rs",
    "workspace/shark-foundation/src/action_system/part01.rs",
    "workspace/shark-foundation/src/action_system/part02.rs",
    "workspace/shark-foundation/src/action_system/part03.rs",
    "workspace/shark-foundation/src/action_system/part04.rs",
    "workspace/shark-foundation/src/lib.rs",
    "workspace/shark-foundation/data/action_registry_v1_manifest.json",
    "workspace/shark-foundation/data/action_registry_v1_part01.jsonl",
    "workspace/shark-foundation/data/action_registry_v1_part02.jsonl",
    "workspace/shark-foundation/data/action_registry_v1_part03.jsonl",
    "workspace/shark-foundation/data/action_registry_v1_part04.jsonl",
    "workspace/shark-foundation/data/action_registry_v1_part05.jsonl",
    "workspace/shark-foundation/data/action_registry_v1_part06.jsonl",
    "workspace/shark-foundation/data/action_registry_v1_part07.jsonl",
    "workspace/shark-foundation/data/action_registry_v1_part08.jsonl",
    "workspace/shark-foundation/data/action_registry_v1_part09.jsonl",
    "workspace/shark-foundation/data/action_registry_v1_part10.jsonl",
    "workspace/shark-foundation/data/action_registry_v1_part11.jsonl",
    "workspace/shark-foundation/data/action_registry_v1_part12.jsonl",
    "scripts/check_sbc7b2_ft1_ft2.py",
    "workspace/shark-foundation/tests/action_system_contract.rs",
}


def fail(message: str) -> None:
    raise AssertionError(message)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)
    print(f"[PASS] {message}")


def load_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def git(repo: Path, *args: str) -> str:
    return subprocess.check_output(["git", "-C", str(repo), *args], text=True).strip()


def validate_registry(repo: Path) -> None:
    registry_manifest_path = repo / "workspace/shark-foundation/data/action_registry_v1_manifest.json"
    registry_part_paths = [repo / f"workspace/shark-foundation/data/action_registry_v1_part{i:02d}.jsonl" for i in range(1, 13)]
    source_path = repo / "workspace/shark-foundation/src/action_system.rs"
    source_part_paths = [repo / f"workspace/shark-foundation/src/action_system/part{i:02d}.rs" for i in range(1, 5)]
    design_path = repo / "docs/SBC7B2_ACTION_SYSTEM_DESIGN_2026-09-15.md"
    contract_path = repo / "docs/SBC7B2_FT1_FT2_SUPERGATE_IMPLEMENTATION_CONTRACT_2026-09-15.md"
    wrapper_path = repo / "workspace/shark-foundation/tests/action_system_contract.rs"

    for path in [registry_manifest_path, *registry_part_paths, source_path, *source_part_paths, design_path, contract_path, wrapper_path]:
        require(path.is_file(), f"required FT1/FT2 path exists: {path.relative_to(repo)}")

    registry = load_json(registry_manifest_path)
    actions = []
    for path in registry_part_paths:
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.strip():
                actions.append(json.loads(line))
    aliases = registry["aliases"]

    require(registry["schema"] == "sharkbooks-action-registry-v1", "registry schema id is v1")
    require(len(actions) == 206, "registry contains exactly 206 canonical actions")
    require(len(aliases) == 5, "registry contains exactly five convenience aliases")
    require(len({a["action_id"] for a in actions}) == 206, "canonical Action IDs are unique")
    require(not ({a["action_id"] for a in actions} & {a["alias_action_id"] for a in aliases}), "aliases do not duplicate canonical Action IDs")

    voice = [a for a in actions if a["voice_eligible"]]
    require(len(voice) == 175, "exactly 175 canonical actions are voice eligible")
    parity = collections.Counter(a["voice_parity"] for a in actions)
    require(parity["REQUIRED_WHEN_EXPOSED"] == 32, "exactly 32 actions require voice when exposed")
    require(parity["REQUIRED_WHEN_ACTIVATED"] == 143, "exactly 143 actions require voice when activated")
    require(parity["NOT_APPLICABLE_INTERNAL"] == 19, "exactly 19 canonical actions are internal/non-voice")
    require(parity["NO"] == 12, "exactly 12 canonical actions have voice parity NO")

    alias_map = {a["alias_action_id"]: a["target_action_id"] for a in aliases}
    require(alias_map == EXPECTED_ALIASES, "five convenience aliases canonicalise to the frozen targets")
    expected_presets = {
        "MONEY_IN.DAILY_TAKINGS": {"category": "Sales/Trading"},
        "MONEY_OUT.MIXED_USE": {"business_use": "mixed"},
        "MONEY_OUT.PRIVATE_FROM_BUSINESS_FUNDS": {"business_use": "private"},
        "CONTACT.CREATE_CUSTOMER": {"kind": "customer"},
        "CONTACT.CREATE_SUPPLIER": {"kind": "supplier"},
    }
    require({a["alias_action_id"]: a["preset"] for a in aliases} == expected_presets,
            "five convenience aliases retain exact deterministic preset facts")
    canonical_ids = {a["action_id"] for a in actions}
    require(all(target in canonical_ids for target in alias_map.values()), "every alias target is canonical")

    action_by_id = {a["action_id"]: a for a in actions}
    for action_id in sorted(BATCH_B_OVERLAY):
        action = action_by_id[action_id]
        require(action["implementation_state"] == "PRODUCTION_MAIN", f"{action_id} records protected-main implementation state")
        require(action["availability_state"] == "BACKEND_READY_UI_LOCKED", f"{action_id} remains UI-locked despite backend readiness")
        require(action["evidence_state"] == "SBC7B1_BATCH_B_DUAL_PLATFORM_PASS", f"{action_id} records dual-platform Batch-B evidence")
        require(action["backend_state"] == "READY", f"{action_id} is backend READY")

    for action_id in sorted(PROHIBITED):
        action = action_by_id[action_id]
        require(not action["voice_eligible"], f"prohibited {action_id} is not voice eligible")
        require(action["backend_state"] == "NOT_AUTHORISED", f"prohibited {action_id} is not executable")

    require(all(slot["slot_id"] != "none" for action in actions for slot in action["required_slots"]), "literal none input sentinel is represented as zero slots")

    for action in actions:
        if action["voice_eligible"]:
            require(len(action["utterances"]) >= 4, f"voice action {action['action_id']} has at least four utterance fixtures")
        else:
            require(len(action["utterances"]) == 0, f"non-voice action {action['action_id']} has no executable voice fixtures")

    embedded_fixture_count = sum(len(a["utterances"]) for a in voice)
    require(embedded_fixture_count == 700, "registry contains exactly 700 embedded voice fixture utterances")
    fixture_counts = collections.Counter({a["action_id"]: len(a["utterances"]) for a in voice})
    require(all(count == 4 for count in fixture_counts.values()), "every voice action has exactly four initial fixture rows")

    normalized = collections.defaultdict(set)
    for action in voice:
        for utterance in action["utterances"]:
            text = " ".join(str(utterance).lower().strip(" .!?\t\r\n").split())
            normalized[text].add(action["action_id"])
    require({"BOOKS.OPEN", "NAVIGATION.BOOKS_HOME"} <= normalized.get("open my books", set()), "open-my-books collision remains explicit for ambiguity testing")
    require(any({"QUOTE.PDF_RENDER", "INVOICE.PDF_RENDER"} <= ids for ids in normalized.values()), "quote/invoice render near-neighbour collision remains explicit")
    require(any({"QUOTE.SHARE_SEND", "INVOICE.SHARE_SEND"} <= ids for ids in normalized.values()), "quote/invoice share near-neighbour collision remains explicit")

    source_facade = source_path.read_text(encoding="utf-8")
    source = source_facade + "\n" + "\n".join(path.read_text(encoding="utf-8") for path in source_part_paths)
    require(all(f'include!("action_system/part{i:02d}.rs")' in source_facade for i in range(1, 5)), "Action System facade includes all four review-bounded source fragments")
    for anchor in [
        "Known", "Unknown", "Ambiguous", "Conflicting",
        "ExecutionAvailability", "ConfirmationReceipt", "ReplayGuard", "AttentionItem",
        "REPLAY_DETECTED", "STALE_CONFIRMATION", "load_action_registry", "AliasRecipe", "accept_call",
    ]:
        require(anchor in source, f"Action System source contains controller anchor: {anchor}")
    require("exposed_action_ids" in source, "controller requires an explicit reviewed exposure set")
    require("canonical_action_id" in source, "controller canonicalises reviewed aliases")
    require("facts_fingerprint" in source, "confirmation receipt binds supplied facts")
    require("missing_context_slots" in source and "MissingContext" in source, "controller distinguishes missing context from owner questions")
    require("alias.preset" in source and "AliasRecipe" in source, "controller applies deterministic alias recipe facts")
    require("state_revision" in source, "confirmation receipt binds authoritative state revision")
    accept_pos = source.index("pub fn accept_call")
    replay_pos = source.index("replay_guard.register", accept_pos)
    confirm_pos = source.index("validate_confirmation", accept_pos)
    require(replay_pos > confirm_pos, "replay id is consumed only after confirmation validation")
    wrapper = wrapper_path.read_text(encoding="utf-8")
    require("shark_foundation::{FoundationError, FoundationErrorCode, FoundationResult}" in wrapper, "compile wrapper reuses only existing Foundation error/result types")
    require("../src/action_system.rs" in wrapper, "compile wrapper targets the production Action System source")
    require("execute" not in source.lower() or "executable" in source.lower(), "Phase-A source does not introduce a generic execute API")

    design = design_path.read_text(encoding="utf-8")
    contract = contract_path.read_text(encoding="utf-8")
    require("206 canonical actions" in design and "175-action" in design and "700 fixtures" in design, "frozen design records registry/voice/fixture invariants")
    require("CANDIDATE NOT YET FROZEN" in contract, "contract keeps implementation candidate unfrozen")
    require("No assistant model" in contract and "accounting mutation dispatcher" in contract, "contract keeps AI/execution outside FT1+FT2 scope")


def phase_a_git_checks(repo: Path) -> None:
    head = git(repo, "rev-parse", "HEAD")
    git(repo, "merge-base", "--is-ancestor", BASE, head)
    changed = {line for line in git(repo, "diff", "--name-only", f"{BASE}..{head}").splitlines() if line}
    extra = changed - PHASE_A_ALLOWED
    require(not extra, f"Phase-A changed paths remain within dependency-free envelope (extra={sorted(extra)})")
    require("workspace/shark-foundation/Cargo.toml" not in changed, "Phase A does not change shark-foundation dependencies")
    require("workspace/Cargo.lock" not in changed, "Phase A does not change Cargo.lock")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", default=".")
    parser.add_argument("--skip-git", action="store_true")
    args = parser.parse_args()
    repo = Path(args.repo).resolve()
    try:
        validate_registry(repo)
        if not args.skip_git:
            phase_a_git_checks(repo)
    except (AssertionError, KeyError, ValueError, subprocess.CalledProcessError) as exc:
        print(f"[FAIL] {exc}", file=sys.stderr)
        return 1
    print("[PASS] SBC-7B2 FT1+FT2 Phase-A static gate")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
