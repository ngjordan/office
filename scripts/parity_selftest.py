#!/usr/bin/env python3
"""The self-check for the parity measurement.

The measurement is only worth anything if it is right. These are the checks
that keep it right, and each one has caught a real defect:

- **The joins hold.** Every catalogue row has a verdict, or is counted as
  UNAUDITED. A verdict for a row that does not exist is an error.
- **The evidence resolves.** Every path a verdict cites is a file in the
  repo. A verdict citing a file that does not exist is not evidence.
- **The counts add up.** The four shares sum to the applicable count for
  every tier. If they do not, a row was counted twice or not at all.
- **The registry is not over-claiming.** The scan is checked against the
  ground truth it can be checked against: Docs' own checklist says 505 of
  520 catalogue features have commands, and the scan must find exactly 505.
  A scan that drifts from that has drifted from the app.
- **The gate catches drift.** Re-typing a parity number into a hand-written
  file must fail the check. A gate that cannot fail is decoration.

Run it:  python3 scripts/parity_selftest.py
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
VENDOR = ROOT.parent / "office-vendor"

sys.path.insert(0, str(HERE))
import importlib.util  # noqa: E402

spec = importlib.util.spec_from_file_location("parity", HERE / "parity.py")
parity = importlib.util.module_from_spec(spec)
spec.loader.exec_module(parity)

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def test_joins_hold() -> None:
    """Every catalogue row has a verdict, and every verdict has a row."""
    for app in parity.APPS:
        repo = parity.app_dir(app)
        if repo is None:
            continue
        catalogue = parity.load_catalogue(app, repo)
        verdicts = parity.load_verdicts(app, repo)
        rows = {row["id"] for row in catalogue if row.get("id")}
        for key in verdicts:
            check(key in rows,
                  f"{app}: verdict '{key}' has no catalogue row")
        # A row with no verdict is UNAUDITED, which is fine and honest. But a
        # catalogue file and a verdict file that share no rows at all is a
        # join that broke.
        if rows and verdicts:
            check(len(rows & set(verdicts)) > 0,
                  f"{app}: the catalogue and the verdicts share no rows")


def test_evidence_resolves() -> None:
    """Every cited evidence path is a file in the repo."""
    for app in parity.APPS:
        repo = parity.app_dir(app)
        if repo is None:
            continue
        for key, path in parity.bad_evidence(app, repo):
            failures.append(f"{app}: verdict {key} cites {path}, which does not exist")


def test_counts_add_up() -> None:
    """The four shares sum to the applicable count."""
    for app in parity.APPS:
        repo = parity.app_dir(app)
        if repo is None:
            continue
        report = parity.measure(app, repo)
        for tier, bucket in report["tiers"].items():
            if tier == "NA":
                continue
            total = (bucket["works"] + bucket["partial"] + bucket["broken"]
                     + bucket["unaudited"])
            check(total == bucket["applicable"],
                  f"{app} {tier}: the four shares sum to {total}, "
                  f"not {bucket['applicable']}")


def test_registry_matches_ground_truth() -> None:
    """The registry scan reproduces the one number it can be checked against.

    Docs' own checklist says 505 of 520 catalogue features have commands.
    The scan reads the same source the checklist is generated from, so it
    must find exactly 505. If it does not, the scan has drifted from the app
    and every "wired" count is suspect.
    """
    repo = parity.app_dir("docs")
    if repo is None:
        return
    catalogue = repo / "crates" / "engine" / "src" / "catalog.rs"
    if not catalogue.exists():
        return
    text = catalogue.read_text(encoding="utf-8")
    if 'pub const CATALOG: &str = "' not in text:
        return
    block = text.split('pub const CATALOG: &str = "')[1].split('";')[0]
    ids = [line.split("|")[-1].strip() for line in block.splitlines()
           if line.count("|") >= 3]
    registry = parity.load_registry("docs", repo)
    live = sum(1 for i in ids if i in registry)
    check(live == 505,
          f"docs: the registry scan finds {live} of {len(ids)} catalogue ids "
          f"live; the app's own checklist says 505 of 520")


def test_gate_catches_drift() -> None:
    """Re-typing a parity number must fail the check."""
    repo = parity.app_dir("docs")
    if repo is None:
        return
    roadmap = repo / "ROADMAP.md"
    if not roadmap.exists():
        return
    original = roadmap.read_text(encoding="utf-8")
    lines = original.splitlines(keepends=True)
    target = None
    for index, line in enumerate(lines):
        if "parity/parity.md" in line and line.startswith("|"):
            target = index
            break
    if target is None:
        return
    try:
        lines[target] = re.sub(r"\*\*\[parity/parity\.md\]\([^)]*\)\*\*",
                                "**505 / 520 (97.1%)**", lines[target])
        roadmap.write_text("".join(lines), encoding="utf-8")
        result = subprocess.run(
            [sys.executable, str(HERE / "parity.py"), "--check", "--app", "docs"],
            capture_output=True, text=True, cwd=ROOT, timeout=120,
        )
        check(result.returncode != 0,
              "the gate passed a re-typed parity number; it cannot fail, "
              "so it means nothing")
    finally:
        roadmap.write_text(original, encoding="utf-8")


def test_an_unpinned_absence_is_unaudited() -> None:
    """An ABSENT verdict with no pinned location is unaudited, not broken.

    A verdict that says a feature is absent is a claim about the code. When
    the evidence names a whole file and no line, the auditor could not
    conclude -- their own note says the sweep was inconclusive. Counting
    that as broken would claim a conclusion nobody reached.

    The check is a real one: take a row whose evidence pins a line, strip
    the line, and the broken share must fall while the unaudited share
    rises by exactly one.
    """
    repo = parity.app_dir("pdf")
    if repo is None:
        return
    verdicts = parity.load_verdicts("pdf", repo)
    catalogue = parity.load_catalogue("pdf", repo)
    by_title = parity.verdict_index(verdicts)

    # Find a row that the measurement joins to a catalogue entry and that
    # currently counts as broken. A verdict nobody joins changes nothing.
    target = None
    for row in catalogue:
        key = str(row.get("id") or "")
        verdict = verdicts.get(key) if key else None
        if verdict is None:
            verdict = by_title.get(
                parity.normalise_title(str(row.get("title") or key)))
        if verdict is None:
            continue
        label = str(verdict.get("verdict", "")).upper()
        if label not in ("ABSENT", "STUB"):
            continue
        if not parity.evidence_pins_location(verdict.get("evidence", []) or []):
            continue
        target = (key, verdict)
        break
    if target is None:
        return

    def shares() -> tuple[int, int]:
        report = parity.measure("pdf", repo)
        return (sum(b["broken"] for b in report["tiers"].values()),
                sum(b["unaudited"] for b in report["tiers"].values()))

    key, verdict = target
    before_broken, before_unaudited = shares()

    # How many rows share this citation? Unpinning one string in the file
    # unpins every row that cites it, so the shift is that many rows.
    evidence = str(verdict.get("evidence", [])[0])
    affected = 0
    for row in catalogue:
        row_key = str(row.get("id") or "")
        row_verdict = verdicts.get(row_key) if row_key else None
        if row_verdict is None:
            row_verdict = by_title.get(
                parity.normalise_title(str(row.get("title") or row_key)))
        if row_verdict is None:
            continue
        if str(row_verdict.get("verdict", "")).upper() not in ("ABSENT", "STUB"):
            continue
        if evidence in [str(e) for e in row_verdict.get("evidence", []) or []]:
            affected += 1

    # The measurement reads the file, so the edit has to be in the file.
    path = repo / "parity" / "verdicts.toml"
    original = path.read_text(encoding="utf-8")
    try:
        path.write_text(original.replace(evidence, re.sub(r":\d+.*$", "", evidence)),
                        encoding="utf-8")
        after_broken, after_unaudited = shares()
        check(after_broken == before_broken - affected,
              f"unpinning {key} left the broken share at {after_broken}, "
              f"not {before_broken - affected}: an absence with no pinned "
              f"location is still being counted as broken")
        check(after_unaudited == before_unaudited + affected,
              f"unpinning {key} left the unaudited share at "
              f"{after_unaudited}, not {before_unaudited + affected}")
    finally:
        path.write_text(original, encoding="utf-8")


def main() -> int:
    for test in (test_joins_hold, test_evidence_resolves, test_counts_add_up,
                 test_registry_matches_ground_truth, test_gate_catches_drift,
                 test_an_unpinned_absence_is_unaudited):
        test()
        print(f"  {test.__name__}: {'FAIL' if failures else 'ok'}")
    if failures:
        print(f"\n{len(failures)} failure(s):", file=sys.stderr)
        for failure in failures:
            print(f"  {failure}", file=sys.stderr)
        return 1
    print("\nparity self-check: all green")
    return 0


if __name__ == "__main__":
    sys.exit(main())
