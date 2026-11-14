#!/usr/bin/env python3
"""Move a verdict's trailing `# comment` into its `note = "..."` field.

Some audits wrote the "what is missing" as a TOML comment after the last key:

    evidence = ["crates/engine/src/cmd/draw.rs:13"]
    # only clears the active draw tool (draw.rs:13); selection by drag is UI-side

The generator reads `note`, not comments, so that text is invisible to
anything that acts on the measurement — including a fixer that has to work
out what is missing. This moves it into the field the generator reads.

    python3 fix_parity_notes.py APP REPO_DIR

It only touches rows that have a comment and no note. A row that already has
a note keeps it.
"""

import re
import sys
from pathlib import Path


def escape(text: str) -> str:
    return text.replace("\\", "\\\\").replace('"', '\\"').replace("\n", " ").strip()


def fix(path: Path) -> int:
    text = path.read_text(encoding="utf-8")
    blocks = re.split(r"(?=^\[verdict\.\")", text, flags=re.M)
    out = []
    moved = 0
    for block in blocks:
        if not block.startswith('[verdict."'):
            out.append(block)
            continue
        lines = block.rstrip("\n").splitlines()
        # The comment is the last thing in the block, if there is one.
        comment = None
        while lines and not lines[-1].strip():
            lines.pop()
        if lines and lines[-1].lstrip().startswith("#"):
            comment = lines[-1].lstrip().lstrip("#").strip()
            lines.pop()
        has_note = any(line.lstrip().startswith("note") for line in lines)
        if comment and not has_note:
            lines.append(f'note = "{escape(comment)}"')
            moved += 1
        out.append("\n".join(lines) + "\n")
    path.write_text("\n".join(out), encoding="utf-8")
    return moved


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print(__doc__)
        sys.exit(2)
    app, repo = sys.argv[1], Path(sys.argv[2])
    path = repo / "parity" / "verdicts.toml"
    count = fix(path)
    print(f"{app}: moved {count} comment(s) into note")
