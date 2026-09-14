#!/usr/bin/env python3
from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys

ENTRY_MAIN = "6f85734d0e9e11089a1a47b7850a306a4ed7ba87"
CRATE = pathlib.Path("incubator/sbc8-operations-forecast-core")

ALLOWED_PATHS = {
    "ci/run_sbc8_batch2_operations_forecast_windows.ps1",
    "docs/SBC8_BATCH2_PREACTIVATION_INCUBATION_AUTHORITY_2026-09-14.md",
    "docs/SBC8_BATCH2_OPERATIONS_FORECAST_CONTRACT_2026-09-14.md",
    "docs/SBC8_BATCH2_OPERATIONS_FORECAST_IMPLEMENTATION_DESIGN_2026-09-14.md",
    "incubator/sbc8-operations-forecast-core/Cargo.lock",
    "incubator/sbc8-operations-forecast-core/Cargo.toml",
    "incubator/sbc8-operations-forecast-core/src/date.rs",
    "incubator/sbc8-operations-forecast-core/src/forecast.rs",
    "incubator/sbc8-operations-forecast-core/src/lib.rs",
    "incubator/sbc8-operations-forecast-core/src/mileage.rs",
    "incubator/sbc8-operations-forecast-core/src/primitives.rs",
    "incubator/sbc8-operations-forecast-core/src/projects.rs",
    "incubator/sbc8-operations-forecast-core/src/recurrence.rs",
    "incubator/sbc8-operations-forecast-core/src/timesheets.rs",
    "scripts/check_sbc8_batch2_operations_forecast_domain.py",
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
    "mapkit",
    "openbanking",
    "open_banking",
    "account_code",
    "postingplan",
    "tax_rate",
    "card_number",
    "cvv",
    "passphrase",
)

REQUIRED_ANCHORS = {
    "src/date.rs": (
        "pub struct CivilDate",
        "pub fn add_days",
        "pub fn add_months_clamped",
        "pub fn add_years_clamped",
    ),
    "src/projects.rs": (
        "pub enum ProjectState",
        "pub struct Project",
        "pub struct ProjectFact",
        "pub fn summarize_project",
    ),
    "src/timesheets.rs": (
        "pub enum TimeEntryState",
        "pub struct TimeEntry",
        "pub fn correction",
        "pub fn to_draft_commercial_line_proposal",
        "pub fn detect_overlaps",
    ),
    "src/mileage.rs": (
        "pub enum MileageSourceMethod",
        "pub struct MileageEntry",
        "pub struct MileageRate",
        "pub fn select_rate",
        "pub fn to_draft_commercial_line_proposal",
    ),
    "src/recurrence.rs": (
        "pub struct RecurrenceSpec",
        "pub const MAX_GENERATED_OCCURRENCES",
        "pub fn generate",
    ),
    "src/forecast.rs": (
        "pub enum ForecastCertainty",
        "pub enum AmountEstimate",
        "pub struct ForecastEvent",
        "pub fn project_forecast",
        "pub struct ScheduledForecastTemplate",
        "pub fn propose",
    ),
}

REQUIRED_TESTS = (
    "bounded_ids_and_text_fail_closed",
    "checked_money_overflow_fails",
    "invalid_civil_dates_fail_closed",
    "leap_day_and_month_end_are_deterministic",
    "project_lifecycle_transitions_fail_closed",
    "project_profitability_is_deterministic_and_traceable",
    "invalid_time_interval_is_rejected",
    "overlap_detection_is_worker_scoped",
    "approved_time_correction_uses_new_identity",
    "approved_billable_time_creates_draft_line_proposal_only",
    "manual_mileage_requires_no_route_or_location_dependency",
    "odometer_inconsistency_is_rejected",
    "effective_dated_source_linked_rate_selection_is_explicit",
    "explicit_mileage_rate_can_create_draft_line_proposal",
    "recurrence_hard_cap_prevents_unbounded_generation",
    "recurrence_count_and_end_date_are_both_enforced",
    "monthly_anchor_does_not_drift_after_short_month",
    "explicit_month_end_policy_stays_at_month_end",
    "yearly_leap_day_is_clamped_without_float_or_timezone_guessing",
    "forecast_is_chronological_and_traceable",
    "forecast_range_propagates_low_expected_high_without_hiding_band",
    "scenario_events_require_explicit_enablement",
    "skipped_and_linked_actual_events_do_not_double_count",
    "recurrence_to_forecast_creates_proposals_only",
    "representative_project_time_mileage_and_forecast_flow_is_domain_only",
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
    check(ancestor.returncode == 0, "Batch 2 entry protected main is an ancestor of HEAD")

    changed = set(filter(None, git_text(repo, "diff", "--name-only", f"{ENTRY_MAIN}..HEAD").splitlines()))
    check(changed == ALLOWED_PATHS, f"exact Batch 2 incubation allow-list ({len(ALLOWED_PATHS)} paths)")

    for prefix in PROTECTED_PREFIXES:
        touched = any(path == prefix or path.startswith(prefix) for path in changed)
        check(not touched, f"active SBC-7/protected surface unchanged: {prefix.rstrip('/')}")

    authority = (repo / "docs/SBC8_BATCH2_PREACTIVATION_INCUBATION_AUTHORITY_2026-09-14.md").read_text(encoding="utf-8")
    contract = (repo / "docs/SBC8_BATCH2_OPERATIONS_FORECAST_CONTRACT_2026-09-14.md").read_text(encoding="utf-8")
    design = (repo / "docs/SBC8_BATCH2_OPERATIONS_FORECAST_IMPLEMENTATION_DESIGN_2026-09-14.md").read_text(encoding="utf-8")
    check("PRODUCTION INTEGRATION BLOCKED" in authority, "production integration remains blocked")
    check("RRule.rs remains a later SBC-8P dependency candidate" in authority, "recurrence dependency admission remains deferred")
    check("No persistence, accounting posting" in contract, "Batch 2 contract preserves domain-only boundary")
    check("Standalone Rust crate" in design, "standalone incubator design is frozen")

    cargo_toml = (repo / CRATE / "Cargo.toml").read_text(encoding="utf-8")
    dependencies_match = re.search(r"(?ms)^\[dependencies\]\s*(.*?)(?=^\[|\Z)", cargo_toml)
    check(dependencies_match is not None and not dependencies_match.group(1).strip(), "standalone crate declares zero dependencies")

    cargo_lock = (repo / CRATE / "Cargo.lock").read_text(encoding="utf-8")
    check(cargo_lock.count("[[package]]") == 1, "standalone lock contains only the incubator crate")
    check('name = "sbc8-operations-forecast-core"' in cargo_lock, "standalone lock identifies the Batch 2 crate")

    runtime_files = sorted((repo / CRATE / "src").glob("*.rs"))
    runtime = "\n".join(path.read_text(encoding="utf-8") for path in runtime_files).lower()
    check("#![forbid(unsafe_code)]" in (repo / CRATE / "src/lib.rs").read_text(encoding="utf-8"), "unsafe code is forbidden")
    for marker in FORBIDDEN_RUNTIME_MARKERS:
        check(marker not in runtime, f"runtime excludes forbidden/integration marker: {marker}")
    check("f32" not in runtime and "f64" not in runtime, "runtime excludes floating-point domain arithmetic")

    for relative, anchors in REQUIRED_ANCHORS.items():
        text = (repo / CRATE / relative).read_text(encoding="utf-8")
        for anchor in anchors:
            check(anchor in text, f"{relative} contains required anchor: {anchor}")

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
        check(result.returncode == 0, "standalone SBC-8 Batch 2 operations/forecast tests pass")
        status_after = git_text(repo, "status", "--porcelain")
        check(status_after == "", "repository clean after runtime gate")

    print(f"[PASS] SBC-8 Batch 2 operations/forecast domain gate complete at {head}")
    lock_hash = run(repo, "git", "hash-object", str(CRATE / "Cargo.lock"))
    if lock_hash.returncode == 0:
        print(f"[INFO] Cargo.lock git blob {lock_hash.stdout.strip()}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
