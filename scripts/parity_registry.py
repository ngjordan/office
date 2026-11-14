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
Each app registers its commands in its own Rust idiom, and the ids are spread
over per-module `specs()` functions across the whole engine crate:

    CommandSpec::new("file.saveAs", ...)          Docs, CAD, PDF
    cmd!("develop.adjust", ...)                    Sheets, Decks, Photos, Film, Light
    spec("calendar.newAppointment", ...)           Mail
    module("nav.mail", ...)                       Mail's module rail

So this reads the engine crate and takes the id literals that appear in a
registration position. A closure that builds a spec (`let spec = |id, ...|`)
is matched by the name the file gives it, so the scan does not pick up every
call whose first argument happens to be a dotted string.

It is a source scan, not a build. That is deliberate: the registry has to be
cheap enough to run in CI and to regenerate at every commit, and a cargo
build of nine apps is neither. The scan is not perfect — it records
`complete: false` for that reason — so the generator uses it for the "wired"
count, which is safe in one direction, and does NOT use it to fail a build
on a missing id, which is not.

The correctness check: run against Docs, whose own checklist says 505 of 520
catalogue features have commands, the scan finds exactly 505.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

# The app repositories.
APPS = {
    "sheets": "gc-gridcraft",
    "docs": "wc-wordcraft",
    "decks": "dc-deckcraft",
    "pdf": "pdfcraft",
    "cad": "cc-cadcraft",
    "mail": "mc-mailcraft",
    "photos": "pc-photocraft",
    "film": "fc-filmcraft",
    "light": "lc-lightcraft",
}

# A command id. It may be namespaced (`file.saveAs`), or a bare word, and a
# bare word may be anything CAD's command names are: `-attedit` (a leading
# dash means the command-line form), `3dorbit` (a leading digit), `ai_deselect`
# (an underscore). So a bare word is accepted on the strength of where it was
# found — inside a registration call — not on the strength of its shape.
ID = re.compile(r"^-?[A-Za-z0-9][A-Za-z0-9_]*(?:\.[A-Za-z0-9_]+)*$")

# The extensions a fixture or a document name ends in. A command id never
# ends in one of these: the namespaced form is `area.action`.
FILE_EXTENSIONS = {
    "pdf", "xml", "json", "csv", "tsv", "txt", "html", "htm", "png", "jpg",
    "jpeg", "gif", "svg", "webp", "bmp", "tif", "tiff", "zip", "gz", "tar",
    "pdfa", "docx", "doc", "xlsx", "xls", "pptx", "ppt", "odt", "ods", "odp",
    "xfdf", "jpg2", "jp2", "jfif", "heic", "heif", "avif", "dcm", "ini",
    "cfg", "conf", "yaml", "yml", "toml", "log", "md", "rtf", "eml", "mbox",
    "icc", "icm", "cur", "dsf", "psd", "psb", "indd", "cr2", "cr3", "nef",
    "arw", "dng", "raf", "orf", "rw2", "pef", "srw", "gifv", "m4a", "m4v",
    "mp3", "mp4", "mov", "wav", "aif", "aiff", "mkv", "webm", "avi", "flv",
}

# An id with a dot is namespaced and is certainly a command id.

# A bare word that also has to look like a command rather than a file, for
# the patterns that can match a plain string anywhere in the crate: the scan
# sees `b.jpg` and `pageWidth` otherwise. An internal capital, or an
# underscore, or a leading dash, is what a command name has.
SINGLE_WORD = re.compile(r"^(?:-[a-z0-9]+|[a-z0-9]*[A-Z][A-Za-z0-9_]*|[a-z0-9]+_[A-Za-z0-9_]+)$")

# The registration idioms, in the order they are tried.
#
# Each pattern is (regex, strict). `strict` means a bare word has to look
# like a command rather than a file, because the pattern can match a plain
# string anywhere in the crate. The two patterns that are unmistakably a spec
# builder take any bare word, because CAD's command names are things like
# `-attedit` and `3dorbit`.
PATTERNS = (
    # CommandSpec::new("id", ...), CommandSpec::with_label("id", ...)
    (re.compile(r"CommandSpec::(?:new|with[a-z_]*)\s*\(\s*\"([^\"]+)\""), False),
    # cmd!("id", "Label", ...) and cmd!(query "id", ...) — the macro five of
    # the nine apps register through. The id is the first string literal.
    (re.compile(r"cmd!\(\s*(?:(?:noundo|noundo|query|undo|raw|silent)\s+)*\"([^\"]+)\""), False),
    # spec("id", "Label", ...)
    (re.compile(r"\bspec\s*\(\s*\"([a-z][A-Za-z0-9_]*(?:\.[A-Za-z0-9_]+)*)\""), True),
    # CommandSpec { id: "id", ... }
    (re.compile(r"\bid\s*:\s*\"([a-z][A-Za-z0-9_]*(?:\.[A-Za-z0-9_]+)*)\""), True),
    # A spec built by a closure, with the id on its own line:
    #     spec(
    #         "calendar.newAppointment",
    #         "New Appointment",
    #     )
    (re.compile(r"^\s*\"([a-z][A-Za-z0-9_]*(?:\.[A-Za-z0-9_]+)*)\"\s*,\s*$"), True),
    # Command { id: "id".into(), ... }
    (re.compile(r"\"([a-z][A-Za-z0-9_]*(?:\.[A-Za-z0-9_]+)*)\"\.into\(\)"), True),
)

# A closure that builds a command spec:
#     let spec = |id, label, location, shortcut, run| CommandSpec { id, .. };
#     let module = |id, label, location, shortcut, run| CommandSpec { id, .. };
# The name is whatever the file calls it, so the names are read from the file
# itself and only those calls are matched.
CLOSURE = re.compile(r"\blet\s+([a-z_][a-z0-9_]*)\s*=\s*\|[^|]*\bid\b[^|]*\|")

# A builder written as a function rather than a closure:
#     const fn c(id: &'static str, label: &'static str, ...) -> CommandSpec
CONST_FN = re.compile(
    r"\bconst\s+fn\s+([a-z_][a-z0-9_]*)\s*\(\s*id\s*:\s*&"
)

# Files that hold no registry: tests, benches, examples.
SKIP = re.compile(
    r"(^|/)(tests?|benches|examples|fixtures?)/|_test\.rs$|/tests\.rs$|test_[^/]*\.rs$"
)


def looks_like_an_id(value: str, strict: bool = True) -> bool:
    """Is this string a command id?

    `strict` is for the patterns that can match a plain string anywhere in
    the crate: a bare word there has to look like a command, not a file.
    The registration patterns match an id in a known position, so a bare
    word is accepted there whatever its shape.
    """
    if not ID.match(value):
        return False
    # A one- or two-letter bare word is a variable, not a command. CAD's
    # command names are longer than that.
    if len(value) < 3 and "." not in value:
        return False
    # A namespaced id is certainly a command id -- but only if the last
    # segment is not a file extension. "a.pdf", "camera.jpg" and "l1.xml" are
    # fixtures, not commands, and a registry full of them is worse than no
    # registry: every lookup misses and the row looks broken.
    if "." in value:
        tail = value.rsplit(".", 1)[1].lower()
        return tail not in FILE_EXTENSIONS
    if "-" in value:
        return True
    if not strict:
        return True
    return bool(SINGLE_WORD.match(value))


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

        patterns = list(PATTERNS)
        for name in set(CLOSURE.findall(text)):
            patterns.append((
                re.compile(r"\b" + re.escape(name) +
                           r"\s*\(\s*\"([^\"]+)\"\s*,"),
                False,
            ))

        # A `const fn` builder takes its id as the first string argument, the
        # same way a closure does. Find the builders by their own signature
        # and match the calls.
        for name in set(CONST_FN.findall(text)):
            patterns.append((
                re.compile(r"\b" + re.escape(name) +
                           r"\s*\(\s*\"([^\"]+)\"\s*,"),
                False,
            ))

        for line in text.splitlines():
            for pattern, strict in patterns:
                for match in pattern.finditer(line):
                    value = match.group(1)
                    if looks_like_an_id(value, strict=strict):
                        ids.add(value)
    return sorted(ids)


def main() -> int:
    written = 0
    for app, repo_name in APPS.items():
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
            "scan": "source",
            "complete": False,
            "count": len(ids),
            "ids": ids,
        }, indent=1) + "\n", encoding="utf-8")
        print(f"{app:8} {len(ids):5} ids  -> {target}")
        written += 1
    print(f"{written} registries written")
    return 0


if __name__ == "__main__":
    sys.exit(main())
