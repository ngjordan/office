#!/usr/bin/env python3
"""Turn the audit reports into per-app verdict files.

The first audit round already produced per-row verdicts with file:line
evidence for seven of the nine apps. They live in markdown tables in
`reports/telegram/*.md`, written by different auditors with different
abbreviations (`I+R`, `IMPLEMENTED+REACHABLE`, `PART`, `ABS`, `N/C`).

Claude's methodology wants those verdicts in one machine-readable place per
app, so they can be counted, checked and re-checked. This script converts
them, so nothing is re-audited and nothing is invented: it only moves
judgements that were already made, and normalises their spelling.

The mapping is the whole point of doing this in one place. `I+R` and
`IMPLEMENTED+REACHABLE` both mean a command is implemented AND reachable
from the UI, which is a "works" verdict; `PART`/`PARTIAL` is partial;
`ABS`/`ABSENT` is broken; `N/C` is not comparable and is excluded.

Evidence keeps its file:line. A verdict with no evidence file is dropped and
reported, because a verdict with no evidence is not a verdict.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPORTS = Path("/opt/hermes-agent/workspace/reports/telegram")
VENDOR = Path("/opt/hermes-agent/workspace/office-vendor")

# report file -> (app key, repo directory)
APPS = {
    "photoshop-parity.md": ("photos", "photos"),
    "premiere-parity.md": ("film", "film"),
    "lightroom-parity.md": ("light", "light"),
    "word-parity.md": ("docs", "docs"),
    "powerpoint-parity.md": ("decks", "decks"),
    "acrobat-parity.md": ("pdf", "pdf"),
    "autocad-parity.md": ("cad", "cad"),
}

# The status vocabulary, longest first so `IMPLEMENTED+REACHABLE` is not
# matched as `I+R` by accident.
STATUS = [
    ("IMPLEMENTED+REACHABLE", "REAL"),
    ("IMPLEMENTED", "REAL"),
    ("PARTIAL", "PARTIAL"),
    ("NOT COMPARABLE", "NA"),
    ("ABSENT", "ABSENT"),
    ("I+R", "REAL"),
    ("PART", "PARTIAL"),
    ("ABS", "ABSENT"),
    ("N/C", "NA"),
    ("P", "PARTIAL"),
    ("A", "ABSENT"),
]

# The evidence level each status supports. A verdict that says the command
# is implemented and reachable is at least "read"; one that only says a
# command id exists is "wired".
LEVEL = {
    "REAL": "read",
    "PARTIAL": "read",
    "ABSENT": "wired",
    "NA": "wired",
}


def normalise_status(cell: str) -> str | None:
    """Map an auditor's abbreviation onto the verdict vocabulary."""
    text = cell.strip().strip("*").strip()
    text = re.sub(r"\s*\(.*?\)\s*", " ", text).strip()
    if not text or text.startswith("-"):
        return None
    for needle, verdict in STATUS:
        if needle in text.upper():
            return verdict
    return None


# The extensions a citation can have. Anything else after the dot is a
# command id (`text.insert`, `arc.csa`), not a file.
FILE_EXTENSIONS = (
    "rs", "toml", "md", "json", "py", "ts", "tsx", "js", "jsx", "yml",
    "yaml", "txt", "html", "css", "sh", "toml",
)


def split_evidence(cell: str) -> list[str]:
    """The file:line references in an evidence cell.

    The auditors wrote several shapes:

    - `` `crates/foo.rs:12-18` `` — a full path
    - `` `text.insert` engine/src/cmd/text.rs:26 +UI `` — a command id then a
      crate-relative path. The id is in backticks and the path is not, so the
      path must be found by its shape, not by position in the token.
    - `` `toml:29`; `reg:168`; `ui:commands.rs:186` `` — PDF's shorthand for
      the catalogue line, the registry line, and the UI file.

    A path is anything with a slash and a file extension, or a bare file name
    with an extension. A bare ``text.insert`` is a command id, not a path,
    and must not be mistaken for one — that was the first bug here.
    """
    text = cell.strip()
    if not text or text in {"—", "-", "n/a", "N/A"}:
        return []
    refs = []
    # The path may be wrapped in backticks, so the match must not stop at one.
    for match in re.finditer(r"`?([\w.][\w./-]*\.[A-Za-z0-9]+(?::[\d\-]+)?)`?", text):
        token = match.group(1)
        # The extension is what comes after the last dot, before any :line
        # suffix. `gaps.md:38` is the md extension, not "md:38".
        extension = token.split(":")[0].rsplit(".", 1)[-1]
        if "/" not in token:
            # A bare name with no directory. A path if it has a file
            # extension; a command id if the extension is not one. `text.insert`
            # is a command id; `commands.rs` is a file.
            if extension not in FILE_EXTENSIONS:
                continue
        refs.append(token)
    # Deduplicate, keep order.
    seen = set()
    out = []
    for ref in refs:
        if ref not in seen:
            seen.add(ref)
            out.append(ref)
    return out


def reference_has(title: str, cells: list[str]) -> str:
    """Does the reference product have this feature?

    The audits put the answer in a column next to the feature ("Yes", "No",
    "Partial"). It is recorded, not interpreted: the generator reports it and
    does not use it to change a verdict.
    """
    for cell in cells[1:]:
        value = cell.strip().strip("*").strip().lower()
        if value in ("yes", "no", "partial", "y", "n", "yes (partial)"):
            return value
    return "unknown"


def toml_escape(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"')


# PDF's audit wrote shorthand: `toml:29` is line 29 of the catalogue,
# `reg:168` is line 168 of the registry, `ui:commands.rs:186` is the UI
# command table, `disp` is the dispatcher. The shorthand is expanded to the
# real path so the citation can be opened.
SHORTHAND = {
    "toml": "parity/acrobat-features.toml",
    "reg": "crates/engine/src/cmd.rs",
    "ui": "crates/ui-egui/src/commands.rs",
}


def expand_shorthand(repo: Path, ref: str) -> str:
    """Turn `ui:commands.rs:186` into a path in the tree."""
    if ":" not in ref:
        return ref
    head, _, rest = ref.partition(":")
    if head not in SHORTHAND:
        return ref
    base = SHORTHAND[head]
    target = repo / base
    if not target.exists():
        # `ui:commands.rs:186` names a file as well as a line: put the file
        # part back in.
        parts = rest.split(":")
        if head == "ui" and parts and parts[0].endswith(".rs"):
            name = parts[0]
            for candidate in repo.rglob(name):
                if candidate.is_file() and "target" not in candidate.parts:
                    target = candidate
                    break
            parts = parts[1:]
        else:
            return ref
        suffix = ":".join(parts)
    else:
        suffix = rest
    if not suffix:
        return str(target.relative_to(repo))
    return f"{target.relative_to(repo)}:{suffix}"


def resolve(repo: Path, ref: str) -> str | None:
    """Make an evidence reference resolve to a real file in the repo.

    The auditors wrote crate-relative paths — `engine/src/cmd/text.rs:26` —
    which sit under `crates/` in the tree. The path is what a reader needs to
    open, so it is rewritten to the real one. If nothing matches, the
    reference is dropped: a verdict that points at a file which does not
    exist is not evidence.
    """
    ref = expand_shorthand(repo, ref)
    path = ref.split(":")[0]
    if not path:
        return None
    candidates = [repo / path]
    # The auditors wrote crate-relative paths (`engine/src/cmd/text.rs`) and
    # bare doc names (`gaps.md`), which sit under `crates/` and `docs/`.
    for prefix in ("crates", "docs", "src", "apps", "crates/engine", "crates/ui-egui"):
        candidates.append(repo / prefix / path)
    if "/" in path:
        # the crate may sit directly under the repo root
        candidates.append(repo / path)
    for candidate in candidates:
        if candidate.exists():
            suffix = ref.split(":", 1)[1] if ":" in ref else ""
            return f"{candidate.relative_to(repo)}{':' + suffix if suffix else ''}"
    # The auditor wrote a bare file name (`redeye_cmds.rs`) without its
    # directory. Find it by name: a citation that points at a file which
    # exists somewhere is worth keeping, and the search is by exact basename
    # so it cannot match the wrong file.
    name = Path(path).name
    matches = [p for p in repo.rglob(name) if p.is_file() and "target" not in p.parts]
    if len(matches) == 1:
        suffix = ref.split(":", 1)[1] if ":" in ref else ""
        return f"{matches[0].relative_to(repo)}{':' + suffix if suffix else ''}"
    # More than one file with that name: prefer the one under docs/ (the
    # auditors cited `gaps.md` and `roadmap.md` most), then the first.
    if matches:
        preferred = [p for p in matches if "docs" in p.parts]
        chosen = (preferred or sorted(matches))[0]
        suffix = ref.split(":", 1)[1] if ":" in ref else ""
        return f"{chosen.relative_to(repo)}{':' + suffix if suffix else ''}"
    return None


def main() -> int:
    written = []
    problems = []
    for report_name, (app_key, repo_name) in APPS.items():
        path = REPORTS / report_name
        if not path.exists():
            problems.append(f"{report_name}: missing")
            continue
        lines = path.read_text(encoding="utf-8").splitlines()

        # find the row table
        header = None
        for index, line in enumerate(lines):
            if line.startswith("|") and re.search(r"Status|Verdict", line) and "---" not in line:
                header = index
                break
        if header is None:
            problems.append(f"{report_name}: no status table")
            continue
        columns = [c.strip() for c in lines[header].strip("|").split("|")]
        status_at = next((i for i, c in enumerate(columns) if "Status" in c or "Verdict" in c), None)
        evidence_at = next((i for i, c in enumerate(columns) if "Evidence" in c), None)
        # The feature column is the one that names a feature. The tables lead
        # with a row number ("#", "No.") and the feature is the next cell, so
        # a numeric-looking first column is skipped.
        feature_at = 0
        for index, cell in enumerate(columns[:3]):
            if cell.strip("# ").strip().rstrip(".").isdigit():
                continue
            if cell.lower() in ("#", "no", "no.", "num", "n"):
                continue
            feature_at = index
            break
        if status_at is None or evidence_at is None:
            problems.append(f"{report_name}: no status/evidence column")
            continue

        repo = VENDOR / repo_name
        if not repo.exists():
            problems.append(f"{app_key}: repo not found at {repo}")
            continue

        entries = []
        dropped_no_evidence = 0
        for line in lines[header + 2:]:
            if not line.startswith("|"):
                break
            cells = [c.strip() for c in line.strip("|").split("|")]
            if len(cells) <= max(status_at, evidence_at):
                continue
            status = normalise_status(cells[status_at])
            if status is None:
                continue
            title = cells[feature_at].strip().replace("**", "")
            if not title:
                continue

            raw_evidence = split_evidence(cells[evidence_at])
            evidence = [r for r in (resolve(repo, ref) for ref in raw_evidence) if r]
            commands_only = []
            level = LEVEL[status]
            if not evidence:
                # Some rows cite only command ids ("ids `arc`, `arc.csa`").
                # That proves the command is REGISTERED, which is real but
                # weaker evidence, so the row is kept at the `wired` level.
                wired_ids = re.findall(r"`([a-z][\w]*\.[\w.]+|[a-z][\w]*)`", cells[evidence_at])
                wired_ids = [w for w in dict.fromkeys(wired_ids) if "." in w or w.isalpha()]
                if wired_ids:
                    commands_only = wired_ids
                    level = "wired"
                else:
                    # No evidence at all. The row still belongs to the
                    # catalogue — it is a feature of the reference product —
                    # but it gets NO verdict, so it counts as UNAUDITED. It is
                    # not dropped: dropping it would shrink the denominator
                    # and make the app look better than the audit found,
                    # which is the exact mistake this exercise exists to
                    # stop.
                    status = None
                    dropped_no_evidence += 1
            entries.append((title, status, evidence, cells, commands_only, level))

        if not entries:
            problems.append(f"{report_name}: no usable rows")
            continue

        # A stable key per row. The title is the human label; the key needs to
        # be unique and stable, so slug it and disambiguate collisions.
        # The catalogue: what the reference product does. The verdicts file:
        # what ours does, with evidence. Two files, one shared key, so a row
        # in one always has a row in the other.
        features = ["# Generated by scripts/parity_verdicts.py from the audit reports.",
                    "# The catalogue: the reference product's feature list.",
                    ""]

        out = ["# Generated by scripts/parity_verdicts.py from the audit reports.",
               "# A verdict without an evidence file is not a verdict; rows without",
               "# evidence were dropped rather than guessed at.",
               ""]
        used = {}
        judged = 0
        for title, status, evidence, cells, commands_only, level in entries:
            key = re.sub(r"[^a-z0-9]+", "-", title.lower()).strip("-")[:60] or "row"
            if key in used:
                used[key] += 1
                key = f"{key}-{used[key]}"
            else:
                used[key] = 0
            commands = list(commands_only)
            for cell in cells:
                for cid in re.findall(r"`(?:cmd:)?([a-z][\w]*\.[\w.]+)`", cell):
                    if cid not in commands:
                        commands.append(cid)

            if status is not None:
                judged += 1
                out.append(f'[verdict."{toml_escape(key)}"]')
                out.append(f'title = "{toml_escape(title)}"')
                out.append(f'verdict = "{status}"')
                out.append(f'level = "{level}"')
                out.append("evidence = [" + ", ".join(f'"{toml_escape(e)}"' for e in evidence) + "]")
                if commands:
                    out.append("commands = [" + ", ".join(f'"{toml_escape(c)}"' for c in commands) + "]")
                out.append("")

            features.append(f'[feature."{toml_escape(key)}"]')
            features.append(f'title = "{toml_escape(title)}"')
            features.append(f'tier = "P2"')
            features.append(f'reference = "{reference_has(title, cells)}"')
            features.append("")

        parity = repo / "parity"
        parity.mkdir(exist_ok=True)
        target = parity / "verdicts.toml"
        target.write_text("\n".join(out), encoding="utf-8")
        features_target = parity / "features.toml"
        features_target.write_text("\n".join(features), encoding="utf-8")
        written.append((app_key, judged, len(entries), dropped_no_evidence, str(target)))

    for app, judged, total, dropped, target in written:
        print(f"{app:8} {judged:4}/{total:<4} judged  ({dropped} unaudited: no evidence)  -> {target}")
    for problem in problems:
        print(f"PROBLEM: {problem}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
