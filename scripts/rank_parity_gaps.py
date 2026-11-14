#!/usr/bin/env python3
"""Rank the parity gaps by how bounded the fix is.

A gap's note says what is missing. Some of those are a missing flag on a
handler that already exists -- a few lines. Some need a trained model or a
licensed codec, and no amount of engineering fixes them here.

This scores each gap from its note, so the fixer works on the ones that can
actually be closed. It is a heuristic and it prints its reasoning, so the
ranking can be argued with.

    python3 rank_parity_gaps.py APP            # the ranked list for one app
    python3 rank_parity_gaps.py --all          # every app, summary only
"""

import re
import sys
from pathlib import Path

VENDOR = Path(__file__).resolve().parent.parent.parent / "office-vendor"

# A gap that needs one of these is not a code gap. It needs data, a licence
# or a model, and it stays open.
BLOCKED = re.compile(
    r"learned model|no learned|ML |machine learning|per-lens profile database|"
    r"thesaurus lexicon|1000 bodies|1000\+ bodies|"
    r"licensed|licence required|subscription|cloud-only|"
    r"not supported by the format|spec does not",
    re.IGNORECASE,
)

# A gap whose note names a small, specific thing is a code gap.
BOUNDED = re.compile(
    r"no flip state|no stacking order|toggles the flag only|"
    r"only clears|only selects|only sets|only toggles|"
    r"one frame per|no transitions or builds|"
    r"no separate windows|not re-checked|"
    r"no authoring commands|no BACTION|no XYZE|"
    r"no lossy|no raw settings|"
    r"stays preview-only|manual \(not automatic\)|"
    r"read-only from XMP|classical measures only|"
    r"does not do|is not applied|is never applied|does not save|"
    r"only a subset|subset of",
    re.IGNORECASE,
)


def parse(repo: Path) -> list[dict]:
    path = repo / "parity" / "verdicts.toml"
    if not path.exists():
        return []
    text = path.read_text(encoding="utf-8")
    out = []
    for m in re.finditer(r'\[verdict\."(.*?)"\]\n(.*?)(?=\n\[|\Z)', text, re.S):
        body = m.group(2)

        def field(key: str) -> str:
            found = re.search(rf'{key} = "(.*?)"', body)
            return found.group(1) if found else ""

        out.append({
            "slug": m.group(1),
            "title": field("title"),
            "verdict": field("verdict"),
            "note": field("note"),
        })
    return out


def score(row: dict) -> tuple[str, str]:
    """(tier, reason) for one gap."""
    note = row["note"]
    if row["verdict"] == "ABSENT":
        if BLOCKED.search(note):
            return "blocked", "needs a model, a licence or data"
        return "absent", "the feature is not there"
    if BLOCKED.search(note):
        return "blocked", "needs a model, a licence or data"
    if BOUNDED.search(note):
        return "bounded", "the note names one specific thing to change"
    if len(note) < 150:
        return "bounded", "a short, specific note"
    return "large", "the note names several things"


def main() -> int:
    apps = ["sheets", "docs", "decks", "pdf", "cad", "mail", "photos", "film", "light"]
    if len(sys.argv) > 1 and sys.argv[1] != "--all":
        apps = [sys.argv[1]]

    for app in apps:
        repo = VENDOR / app
        rows = [r for r in parse(repo) if r["verdict"] in ("PARTIAL", "ABSENT", "STUB")]
        buckets: dict[str, list] = {}
        for row in rows:
            tier, reason = score(row)
            buckets.setdefault(tier, []).append((row, reason))
        if "--all" in sys.argv:
            counts = {k: len(v) for k, v in sorted(buckets.items())}
            print(f"{app:8} gaps={len(rows):4}  {counts}")
            continue
        print(f"=== {app}: {len(rows)} gaps")
        for tier in ("bounded", "large", "absent", "blocked"):
            items = buckets.get(tier, [])
            if not items:
                continue
            print(f"--- {tier} ({len(items)})")
            for row, reason in items[:40]:
                print(f"    {row['slug'][:38]:40} {row['note'][:104]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
