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


def main() -> int:
    for test in (test_joins_hold, test_evidence_resolves, test_counts_add_up,
                 test_registry_matches_ground_truth, test_gate_catches_drift):
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
