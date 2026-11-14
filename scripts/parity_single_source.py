#!/usr/bin/env python3
"""Replace hand-typed coverage numbers with a link to the generated page.

Why
---
Every app's own files state the catalogue coverage, and they do not agree.
Docs says 493/508 in ROADMAP.md, 504/520 in the same file's history and
505/520 in target-app-parity.md. Light's ROADMAP says 81.6 % of 532 rows
while its own parity doc says 83.2 % of 546. A reader cannot tell which is
current, which is why the first audit round had to re-measure every app
before it could say anything.

The fix is not to pick a winner. It is to state the number once, in the
generated page, and have every other place point at it. Then there is no
second number to disagree.

What this rewrites
------------------
A table cell whose label names the coverage metric and whose value is a
share. The value becomes a link to `parity/parity.md`, and the row's
"measured" note keeps pointing at the tool that produced it.

What it does not touch
----------------------
- A line that states a GOAL rather than a measurement.
- A per-feature or per-area depth percentage, which is a different
  measurement and lives in its own table.
- A share that is not the coverage metric.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

VENDOR = Path("/opt/hermes-agent/workspace/office-vendor")

# The metric labels whose value must be single-sourced.
METRIC = re.compile(
    r"catalog(?:ue)? coverage|ribbon(?: and menu| and|/)? catalog|menu catalog|"
    r"feature breadth",
    re.IGNORECASE,
)

# A goal, not a measurement.
TARGET = re.compile(
    r"definition of done|\bgoal\b|\btarget\b|\baim\b|\bwant\b|\bplan to\b|"
    r"\bwill\b|\bshould reach\b|\bby v\d|\buntil\b",
    re.IGNORECASE,
)

# The share itself. The forms that appear:
#   "493 / 508 (97.0%)"      a count with its percentage
#   "81.6% of 532 rows"      a percentage over a row count
#   "~76%"                   a bare percentage
#   "245 / 491 = 50%"        a count with a percentage
#   "465 shipped + 119 partial of 834"   two counts over a total
#
# The whole thing is matched, because the number is removed entirely: a
# partial match would leave "493 / 508" behind next to the link, and that is
# exactly the stale number this script exists to remove.
SHARE = re.compile(
    # A number that is NOT a tier label: "P0 50%" is a weight, not a share.
    r"(?<!P)(?<!p)(?<!R)(?<!r)(?<!F)(?<!f)(?<!C)(?<!c)(?<!T)(?<!t)"
    r"~?\d+(?:\.\d+)?\s*%?"
    r"(?:\s*\(\s*\d+(?:\.\d+)?\s*%\s*\))?"
    r"(?:\s*=\s*\d+(?:\.\d+)?\s*%)?"
    r"(?:\s*/\s*\d+(?:\.\d+)?\s*(?:\(\s*\d+(?:\.\d+)?\s*%\s*\))?)?"
    r"(?:\s*\+\s*\d+(?:\.\d+)?\s*\w+\s*of\s*\d+(?:\.\d+)?)?"
    r"(?:\s*of\s*\d+(?:\.\d+)?\s*(?:in-scope\s+)?rows?)?",
    re.IGNORECASE,
)

# The per-area depth tables: a row under a heading about per-area depth.
DEPTH_ROW = re.compile(r"^\|.*\|\s*\d+(\.\d+)?\s*%\s*\|")


def rewrite(repo: Path, rel: str, app: str) -> list[str]:
    """Rewrite one file. Returns the lines that changed."""
    path = repo / rel
    if not path.exists():
        return []
    lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
    changed = []
    for index, line in enumerate(lines):
        if not line.startswith("|"):
            # A heading can state the headline on its own:
            #   "## Feature breadth: ~76% (weighted)"
            #
            # Only a heading, and only when the share is the metric's own
            # value. A bullet or a sentence in prose names the metric and
            # then goes on to talk about menus and effects; rewriting those
            # turns a sentence into a broken one, and the sub-metrics they
            # quote are a different measurement from the headline.
            if not line.lstrip().startswith("#"):
                continue
            if not METRIC.search(line) or TARGET.search(line):
                continue
            share = SHARE.search(line)
            if share is None:
                continue
            before = line[:share.start()]
            if not METRIC.search(before):
                continue
            lines[index] = rewrite_inline(line)
            changed.append(f"{rel}:{index + 1}")
            continue
        if TARGET.search(line) or DEPTH_ROW.match(line.strip()):
            continue
        cells = line.split("|")
        # Find the cell that names the metric and the cell that carries the
        # share. Usually the label is in cell 1 and the value in cell 2.
        metric_at = None
        for i, cell in enumerate(cells):
            if METRIC.search(cell) and not SHARE.search(cell):
                metric_at = i
                break
        if metric_at is None:
            continue
        value_at = None
        for i in range(metric_at + 1, min(metric_at + 3, len(cells))):
            if SHARE.search(cells[i]):
                value_at = i
                break
        if value_at is None:
            continue

        original = cells[value_at]
        # The WHOLE value cell is replaced by the link, not just the share.
        #
        # The number lives in one place now, so any prose that qualifies it
        # ("of 532 rows", "tier-weighted", "≈ 67%") is either stale or
        # belongs with the number. Leaving it behind gives a reader a
        # qualifier for a number they cannot see, which is worse than
        # pointing at the page.
        #
        # The exception is the row's own note cell, which says HOW the
        # number was measured and still belongs here.
        if SHARE.search(original) is None:
            continue
        emphasis = original.count("**")

        link = "[parity/parity.md](parity/parity.md)"
        # The relative link depends on where the file lives.
        if rel.startswith("docs/"):
            link = "[parity/parity.md](../parity/parity.md)"
        if emphasis >= 2:
            link = f"**{link}**"
        replacement = link
        if replacement == original:
            continue
        cells[value_at] = replacement
        lines[index] = "|".join(cells)
        changed.append(f"{rel}:{index + 1}")
    if changed:
        path.write_text("".join(lines), encoding="utf-8")
    return changed


def rewrite_inline(line: str) -> str:
    """A heading or bullet: put the link where the share was.

    Only the share is replaced. The rest of the line — the sentence that
    explains what the number means — stays, because it is the prose, not
    the measurement.
    """
    share = SHARE.search(line)
    if share is None:
        return line
    # Replace exactly the matched span and nothing else. The heading's own
    # emphasis and wording are untouched, so the line still reads as a
    # heading.
    link = "[parity/parity.md](parity/parity.md)"
    return line[:share.start()] + link + line[share.end():]


def main() -> int:
    total = 0
    for repo in sorted(VENDOR.iterdir()):
        # The vendor entries are symlinks to the app checkouts.
        if not repo.is_dir():
            continue
        repo = repo.resolve()
        for rel in ["ROADMAP.md", "README.md", "docs/target-app-parity.md", "docs/gaps.md"]:
            changed = rewrite(repo, rel, repo.name)
            if changed:
                print(f"{repo.name}/{rel}: {len(changed)} lines")
                for item in changed:
                    print(f"    {item}")
                total += len(changed)
    print(f"{total} coverage numbers now point at the generated page")
    return 0


if __name__ == "__main__":
    sys.exit(main())
