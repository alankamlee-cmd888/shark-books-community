#!/usr/bin/env python3
from __future__ import annotations

import argparse
import collections
import json
import re
from pathlib import Path

EXPECTED_EXPOSED = {
    "BOOKS.CREATE",
    "BOOKS.OPEN",
    "BOOKS.VERIFY",
    "HOME.STATUS",
    "MONEY_IN.PREVIEW",
    "MONEY_IN.SAVE",
    "MONEY_OUT.PREVIEW",
    "MONEY_OUT.SAVE",
    "MONEY.RECORDS.LIST",
    "MONEY.RECORD.DETAIL",
    "CORRECTION.PREVIEW",
    "CORRECTION.CONFIRM",
    "CORRECTION.HISTORY",
    "BANK.IMPORT_PREVIEW_CSV",
    "BANK.IMPORT_PREVIEW_OFX_QFX",
    "BANK.IMPORT_CONFIRM_CSV",
    "BANK.IMPORT_CONFIRM_OFX_QFX",
    "BANK.ACTIVITY_LIST",
    "BANK.ACTIVITY_DETAIL",
    "BANK.MATCH_REVIEW",
    "BANK.MATCH_CONFIRM",
    "BANK.RECONCILE_PREVIEW",
    "BANK.RECONCILE_FINALISE",
    "DOCUMENT.SELECT_REGISTER",
    "DOCUMENT.VERIFY",
    "DOCUMENT.LIST",
    "DOCUMENT.OPEN_VIEW",
    "DOCUMENT.ATTACH",
    "OCR.EXTRACT_RECEIPT",
    "RECEIPT.SUGGEST_BANK",
    "RECEIPT.CONFIRM_BANK",
    "RECEIPT.REJECT_BANK",
    "CONTACTS.LIST",
    "CONTACTS.SAVE",
    "REPORT.SUMMARY",
    "SETTINGS.BOOKS_INFO",
    "SETTINGS.STORAGE_ROOT.SELECT",
}

R4_RECONCILED_READY = {
    "MONEY.RECORDS.LIST",
    "MONEY.RECORD.DETAIL",
    "BANK.ACTIVITY_DETAIL",
    "DOCUMENT.LIST",
    "DOCUMENT.OPEN_VIEW",
}


def require(ok: bool, message: str) -> None:
    if not ok:
        raise AssertionError(message)
    print(f"PASS: {message}")


def read(repo: Path, rel: str) -> str:
    path = repo / rel
    require(path.is_file(), f"required path exists: {rel}")
    return path.read_text(encoding="utf-8")


def load_registry(repo: Path):
    root = repo / "workspace/shark-foundation/data"
    manifest = json.loads((root / "action_registry_v1_manifest.json").read_text(encoding="utf-8"))
    rows = []
    for index in range(1, 25):
        path = root / f"action_registry_v1_chunk{index:02d}.jsonl"
        require(path.is_file(), f"Action Registry chunk {index:02d} exists")
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.strip():
                rows.append(json.loads(line))
    return manifest, rows


def normalise(value: str) -> str:
    return " ".join(value.lower().strip(" .!?\t\r\n").split())


def parse_exposed(native: str) -> set[str]:
    block = re.search(
        r'COMMAND_TEXT_EXPOSED_ACTION_IDS:\s*&\[&str\]\s*=\s*&\[(.*?)\];',
        native,
        flags=re.S,
    )
    require(block is not None, "native command-text exposure constant is present")
    return set(re.findall(r'"([A-Z0-9_.]+)"', block.group(1)))


def generated_binding_ids(repo: Path) -> set[str]:
    ids = set()
    for rel in [
        "workspace/ui/src/generated/uia-actions.ts",
        "workspace/ui/src/generated/uib-actions.ts",
    ]:
        ids.update(re.findall(r'"actionId":\s*"([A-Z0-9_.]+)"', read(repo, rel)))
    return ids


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", default=".")
    args = parser.parse_args()
    repo = Path(args.repo).resolve()

    try:
        manifest, rows = load_registry(repo)
        by_id = {row["action_id"]: row for row in rows}
        require(manifest["schema"] == "sharkbooks-action-registry-v1", "Action Registry schema remains v1")
        require(len(rows) == 206 and len(by_id) == 206, "Action Registry remains exactly 206 unique canonical actions")
        voice = [row for row in rows if row["voice_eligible"]]
        require(len(voice) == 175, "exactly 175 canonical actions remain voice eligible")
        require(sum(len(row["utterances"]) for row in voice) == 700, "exactly 700 finite utterance fixtures remain frozen")
        require(all(len(row["utterances"]) == 4 for row in voice), "every voice-eligible action retains four fixtures")

        collisions: dict[str, set[str]] = collections.defaultdict(set)
        for row in voice:
            for utterance in row["utterances"]:
                collisions[normalise(utterance)].add(row["action_id"])
        require(
            {"BOOKS.OPEN", "NAVIGATION.BOOKS_HOME"} <= collisions.get("open my books", set()),
            "open-my-books near-neighbour ambiguity remains explicit",
        )
        require(
            any({"QUOTE.PDF_RENDER", "INVOICE.PDF_RENDER"} <= ids for ids in collisions.values()),
            "quote/invoice PDF collision remains explicit",
        )
        require(
            any({"QUOTE.SHARE_SEND", "INVOICE.SHARE_SEND"} <= ids for ids in collisions.values()),
            "quote/invoice share collision remains explicit",
        )

        facade = read(repo, "workspace/shark-foundation/src/action_system.rs")
        part05 = read(repo, "workspace/shark-foundation/src/action_system/part05.rs")
        require('include!("action_system/part05.rs")' in facade, "Action System facade includes bounded R3 part05")
        for anchor in [
            "normalize_command_text",
            "finite_command_candidates",
            "resolve_command_text",
            "action.utterances",
            "action.manual_label",
            "action.owner_intent",
            "alias.label",
            "InvocationSource::CommandText",
        ]:
            require(anchor in part05, f"finite matcher anchor exists: {anchor}")
        for forbidden in [
            "levenshtein",
            "jaro",
            "embedding",
            "cosine",
            "reqwest",
            "http://",
            "https://",
            "llama",
            "openai",
            "model.predict",
        ]:
            require(forbidden not in part05.lower(), f"finite matcher has no fuzzy/model/network authority: {forbidden}")

        native = read(repo, "workspace/shark-tauri-spike/src/owner_command_text.rs")
        exposed = parse_exposed(native)
        require(exposed == EXPECTED_EXPOSED, "native command-text exposure set is exactly the 37 frozen UI-A/UI-B Action IDs")
        binding_ids = generated_binding_ids(repo)
        require(binding_ids == EXPECTED_EXPOSED, "generated UI-A/UI-B Action IDs reconcile exactly to the command-text exposure set")
        for action_id in sorted(R4_RECONCILED_READY):
            require(by_id[action_id]["backend_state"] == "READY", f"{action_id} is READY after explicit R4 metadata reconciliation")
        require("owner_command_text_resolve" in native, "dedicated owner_command_text_resolve native command exists")
        require("candidate Action ID is not a candidate" in native or "selected Action ID is not a candidate" in native, "ambiguity selection is rebound to the original finite candidate set")
        for forbidden in [
            "std::process",
            "Command::new",
            "database_path",
            "db_path",
            "file_name",
            "shell_command",
            "reqwest",
            "http://",
            "https://",
        ]:
            require(forbidden not in native, f"native command adapter excludes raw path/shell/network authority: {forbidden}")

        lib = read(repo, "workspace/shark-tauri-spike/src/lib.rs")
        build = read(repo, "workspace/shark-tauri-spike/build.rs")
        permission = read(repo, "workspace/shark-tauri-spike/permissions/shark-shell.toml")
        tauri = read(repo, "workspace/ui/src/lib/tauri.ts")
        home = read(repo, "workspace/ui/src/screens/HomeScreen.vue")
        panel = read(repo, "workspace/ui/src/components/CommandPanel.vue")
        attention = read(repo, "workspace/ui/src/components/AttentionPanel.vue")
        routes = read(repo, "workspace/ui/src/lib/command-routes.ts")

        for source, label in [
            (lib, "Tauri handler"),
            (build, "AppManifest"),
            (permission, "permission"),
            (tauri, "bounded frontend union"),
        ]:
            require("owner_command_text_resolve" in source, f"owner_command_text_resolve is registered in {label}")

        require("CommandPanel" in home, "Assistant Home activates the finite CommandPanel")
        require("disabled" not in re.search(r'<CommandPanel.*?</article>', home, flags=re.S).group(0), "Assistant Home command panel is not a disabled placeholder")
        require("commandAttention" in attention, "command controller Attention is rendered in the factual Attention surface")
        require("FIXED_ROUTES" in routes and "Record<string, CommandSection>" in routes, "frontend command continuation uses an explicit finite navigation map")
        require("nativeInvoke(\"owner_command_text_resolve\"" in tauri, "frontend calls only the dedicated fixed command-text native command")
        require("nativeInvoke(command" not in panel, "CommandPanel cannot choose an arbitrary Tauri command")
        for source, label in [(panel, "CommandPanel"), (routes, "command routes")]:
            for forbidden in ["fetch(", "XMLHttpRequest", "WebSocket", "http://", "https://", "shellCommand", "databasePath", "dbPath"]:
                require(forbidden not in source, f"{label} excludes forbidden authority: {forbidden}")

        require(all(by_id[x]["backend_state"] == "READY" for x in R4_RECONCILED_READY), "R4 metadata reconciliation makes the five proven read/view actions controller-executable")

    except (AssertionError, KeyError, ValueError, json.JSONDecodeError) as exc:
        print(f"FAIL: {exc}")
        return 1

    print("PASS: SBC-7B2 R3/FT4 finite command-text static contract")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
