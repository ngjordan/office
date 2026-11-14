#!/usr/bin/env python3
"""Dump each app's command registry to `parity/registry.json`.

Why
---
The verdict layer answers "does this feature work". It cannot answer "is the
command even there". Those are different questions, and the first audit
round showed why both are needed: the catalogue claims a command exists, the
registry says it does, and the handler turns out to be a stub — Docs'
`file.share` only prints a hint.

So the machine half is the registry: the set of command ids the app
actually registers. The generator compares each catalogue row's command ids
against it, in both directions:

- a row citing an id the registry lacks claims something that is gone;
- an id no row cites is code the catalogue does not know about. The second
  kind is how CAD's object-snap tracking and Decks' handout masters were
  rated wrong in their own docs.

How the ids are found
---------------------
Each app registers its commands in its own Rust idiom — `CommandSpec::new`,
`spec(...)`, `id: "..."`, `("id", "label")` — and the ids are spread over
per-module `specs()` functions across the whole engine crate. So this reads
the engine crate and takes the id literals that appear in a registration
position.

It is a source scan, not a build. That is deliberate: the registry dump has
to be cheap enough to run in CI and to regenerate at every commit, and a
cargo build of nine apps is neither. The risk of a source scan is that it
sees a string that is not really an id, or misses an id built at runtime;
the generator's orphan check is what catches a miss, because a catalogue row
whose id is absent is reported rather than silently counted as unwired.

The ids found are committed to `parity/registry.json` so the measurement is
reproducible from the tree alone.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

VENDOR = Path("/opt/hermes-agent/workspace/office-vendor")

# The app repositories, and the crate that holds the registry.
APPS = {
    "sheets": ("gc-gridcraft", "crates/engine/src"),
    "docs": ("wc-wordcraft", "crates/engine/src"),
    "decks": ("dc-deckcraft", "crates/engine/src"),
    "pdf": ("pdfcraft", "crates/engine/src"),
    "cad": ("cc-cadcraft", "crates/engine/src"),
    "mail": ("mc-mailcraft", "crates/engine/src"),
    "photos": ("pc-photocraft", "crates/engine/src"),
    "film": ("fc-filmcraft", "crates/engine/src"),
    "light": ("lc-lightcraft", "crates/engine/src"),
}

# The registration idioms. Each pattern must capture the id.
PATTERNS = (
    # CommandSpec::new("id", ...), CommandSpec { id: "id", ...
    re.compile(r"CommandSpec::(?:new|with[a-z_]*)\s*\(\s*\"([a-z][A-Za-z0-9]*(?:\.[A-Za-z0-9]+)*)\""),
    re.compile(r"\bid\s*:\s*\"([a-z][A-Za-z0-9]*(?:\.[A-Za-z0-9]+)*)\""),
    # spec("id", "Label", ...)
    re.compile(r"\bspec\s*\(\s*\"([a-z][A-Za-z0-9]*(?:\.[A-Za-z0-9]+)*)\""),
    # ("id", "Label") — a tuple in a specs() table
    re.compile(r"^\s*\(\s*\"([a-z][A-Za-z0-9]*(?:\.[A-Za-z0-9]+)*)\"\s*,"),
    # Command { id: "id".into(), ... }
    re.compile(r"\"([a-z][A-Za-z0-9]*(?:\.[A-Za-z0-9]+)*)\"\.into\(\)"),
)

# An id is a dotted name that starts lower-case. The segments may contain
# capitals: `file.saveAs`, `edit.pasteText`, `view.readColumnWidth`. Anything
# else is a path, a URL or prose.
ID = re.compile(r"^[a-z][A-Za-z0-9]*(?:[.\-][A-Za-z0-9]+)*$")

# Files that hold no registry: tests, benches, examples.
SKIP = re.compile(r"(^|/)(tests?|benches|examples|fixtures?)/|_test\.rs$|/tests\.rs$|test_[^/]*\.rs$")


def collect(repo: Path) -> list[str]:
    """The command ids the app registers."""
    ids: set[str] = set()
    engine = repo / "crates" / "engine" / "src"
    if not engine.exists():
        return []
    for path in engine.rglob("*.rs"):
        rel = path.relative_to(engine).as_posix()
        if SKIP.search(rel):
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        for line in text.splitlines():
            for pattern in PATTERNS:
                for match in pattern.finditer(line):
                    value = match.group(1)
                    if ID.match(value):
                        ids.add(value)
    return sorted(ids)


def main() -> int:
    written = 0
    for app, (repo_name, _) in APPS.items():
        repo = Path("/opt/hermes-agent/workspace") / repo_name
        if not repo.exists():
            print(f"{app}: {repo} not found", file=sys.stderr)
            continue
        ids = collect(repo)
        if not ids:
            print(f"{app}: no ids found")
            continue
        parity = repo / "parity"
        parity.mkdir(exist_ok=True)
        target = parity / "registry.json"
        target.write_text(json.dumps({
            "app": app,
            "source": f"{repo_name}/crates/engine/src",
            "count": len(ids),
            "ids": ids,
        }, indent=1) + "\n", encoding="utf-8")
        print(f"{app:8} {len(ids):5} ids  -> {target}")
        written += 1
    print(f"{written} registries written")
    return 0


if __name__ == "__main__":
    sys.exit(main())
