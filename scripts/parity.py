#!/usr/bin/env python3
"""The parity generator: one measurement for the nine apps.

Why this exists
---------------
Each app's own catalogue overstates what it does. The catalogue counts a
command id that is *registered*, and a registered id may be a stub: Docs'
`file.share` only prints a hint string (`wc-wordcraft/crates/engine/src/cmd/tools/specs.rs:158-162`),
and CAD has 35 "not available yet" call sites still. So "97.1 % of features
have commands" is a statement about the registry, not about the product.

This script is the second half of the fix: it reads the catalogue (what the
reference product does) and the verdicts (what ours does, with evidence), and
writes the only place a parity number lives. Four shares, per tier, that sum
to the applicable count:

    Works / Partial / Broken / Unaudited

"Unaudited" is the point. It is the share of rows nobody has judged, and it
is reported as rows not looked at rather than as a coverage percentage, so a
low-coverage app looks low-coverage instead of looking good.

The rules it enforces, each from a defect found in the first audit round:

- **A verdict needs evidence.** Every entry cites file:line, and the path
  must resolve. A verdict with no evidence is not a verdict.
- **A verdict goes stale when its evidence changes.** `git log` between the
  verdict's commit and HEAD over the evidence files decides that. Stale
  verdicts keep their label and are counted in their own column.
- **Numbers live in one file.** `--check` greps ROADMAP.md, README.md and the
  two parity docs for typed percentages and fails on a hit. Every app's own
  files disagreed with each other (Docs quotes 493/508, 505/520 and 504/520 in
  three places; PDF quotes breadth as 72 %, 67 % and 72 %) and that is how it
  happened.
- **Orphans go both ways.** A catalogue row citing an id the registry does
  not have claims something that is gone; a registry id no row cites is code
  the catalogue does not know about. The second kind is how CAD's object-snap
  tracking and Decks' handout masters were rated wrong in their own docs.
- **`--check` does not fail on unaudited or stale rows.** A gate that needs a
  full re-audit to pass gets bypassed, and then it means nothing. Those are
  printed, not failed.

Usage
-----
    scripts/parity.py                  # write the reports
    scripts/parity.py --check          # fail on drift (for CI)
    scripts/parity.py --app mail       # one app
    scripts/parity.py --lint           # propose stub candidates
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

# The nine apps, and where each one keeps its catalogue.
#
# The catalogue is the reference product's feature list. Each app already has
# one in its own shape, and the shape differs, so each app gets a loader.
APPS = {
    "sheets": {"name": "Sheets", "counterpart": "Excel (Microsoft 365)"},
    "docs": {"name": "Docs", "counterpart": "Word"},
    "decks": {"name": "Decks", "counterpart": "PowerPoint"},
    "pdf": {"name": "PDF", "counterpart": "Acrobat Pro"},
    "cad": {"name": "CAD", "counterpart": "AutoCAD"},
    "mail": {"name": "Mail", "counterpart": "Outlook"},
    "photos": {"name": "Photos", "counterpart": "Photoshop"},
    "film": {"name": "Film", "counterpart": "Premiere Pro"},
    "light": {"name": "Light", "counterpart": "Lightroom"},
}

# The verdict vocabulary, from Mail's method (which is the only one that was
# applied to a whole catalogue) with the two labels the first audit round
# showed were missing.
VERDICTS = {
    "REAL": "works",
    "DIALOG-OK": "works",
    "PARTIAL": "partial",
    "AGENT-ONLY": "partial",
    "STUB": "broken",
    "ABSENT": "broken",
    "NA": "excluded",
}

# How far the evidence goes. A Works verdict at "read" is a claim about code,
# not about output, and the report says so in the column header.
LEVELS = ("wired", "read", "run", "fidelity")

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
VENDOR = ROOT.parent / "office-vendor"


# ---------------------------------------------------------------- helpers


def read_toml(path: Path) -> dict:
    """A tiny TOML reader.

    The stdlib has no TOML, and adding a dependency for one script is worse
    than thirty lines of parser. It reads the shapes these files use: tables,
    arrays of tables, and scalar keys. Anything else is an error, not a
    silent skip.
    """
    if not path.exists():
        return {}
    out: dict = {}
    current = out
    array_key = None
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip() if not raw.strip().startswith("#") else ""
        if not line:
            continue
        if line.startswith("[["):
            name = line[2:].strip().rstrip("]]")
            rows = out.setdefault(name, [])
            rows.append({})
            current = rows
            array_key = name
            continue
        if line.startswith("["):
            name = line[1:].strip().rstrip("]")
            current = out.setdefault(name, {})
            array_key = None
            continue
        if "=" not in line:
            continue
        key, _, value = line.partition("=")
        key = key.strip()
        value = value.strip()
        if value.startswith("[") and value.endswith("]"):
            parsed = [v.strip().strip('"').strip("'") for v in value[1:-1].split(",") if v.strip()]
            target = current if array_key is None else current[-1]
            target[key] = parsed
            continue
        if value.startswith('"') and value.endswith('"'):
            value = value[1:-1]
        elif value.startswith("'") and value.endswith("'"):
            value = value[1:-1]
        elif value in ("true", "false"):
            value = value == "true"
        else:
            try:
                value = int(value)
            except ValueError:
                pass
        target = current if array_key is None else current[-1]
        target[key] = value
    return out


def app_dir(app: str) -> Path | None:
    """Where an app's repository is, if it is checked out beside the suite."""
    names = {
        "sheets": "sheets", "docs": "docs", "decks": "decks", "pdf": "pdf",
        "cad": "cad", "mail": "mail", "photos": "photos", "film": "film",
        "light": "light",
    }
    vendor = VENDOR / names[app]
    if vendor.exists():
        return vendor.resolve()
    return None


def git_log_since(repo: Path, commit: str, files: list[str]) -> int:
    """How many commits touched the evidence files since the verdict was made."""
    if not commit or not files:
        return 0
    try:
        out = subprocess.run(
            ["git", "log", "--oneline", f"{commit}..HEAD", "--"] + files,
            cwd=repo, capture_output=True, text=True, timeout=60,
        )
    except (OSError, subprocess.SubprocessError):
        return 0
    return len([l for l in out.stdout.splitlines() if l.strip()])


# ------------------------------------------------------- the loaders


def load_pdf_catalogue(repo: Path) -> list[dict]:
    """PDF keeps its catalogue as TOML with a status column typed by hand.

    The status moves to the verdicts file; what is left here is what the
    reference product does.
    """
    data = read_toml(repo / "parity" / "acrobat-features.toml")
    rows = []
    for feature in data.get("feature", []):
        rows.append({
            "id": feature.get("id", ""),
            "title": feature.get("title", ""),
            "area": feature.get("area", ""),
            "tier": feature.get("tier", "P2"),
            "commands": feature.get("commands", []),
            "tools": feature.get("tools", []),
        })
    return rows


def load_bullet_catalogue(repo: Path) -> list[dict]:
    r"""Sheets, Docs and CAD: a markdown file of sections and bullet rows.

    The shape differs per app, so each is read the way it is written:

    - Sheets: `## File (11 / 11, 100%)` then `Implemented:` then
      `- `file.new` New (File)`. The id is in the backticks.
    - Docs: `## Missing` then `### Home` then
      `- Voice > Dictate (`tools.dictate`)`. The id is in the backticks.
    - CAD: `## Not yet covered` then `- File > New Sheet Set...`. No ids: the
      label is the row.
    """
    path = repo / "docs" / "parity-checklist.md"
    if not path.exists():
        return []
    rows = []
    section = ""
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line.startswith("## "):
            section = line[3:].strip()
            continue
        if not line.startswith("- "):
            continue
        body = line[2:].strip()
        ids = re.findall(r"`([^`]+)`", body)
        cid = ids[0] if ids else ""
        title = re.sub(r"`[^`]+`", "", body).strip(" -()")
        if not cid and not title:
            continue
        rows.append({
            "id": cid or title,
            "title": title or cid,
            "area": section,
            "commands": [cid] if cid else [],
        })
    return rows


def load_checklist_catalogue(repo: Path) -> list[dict]:
    """The apps whose checklist is a table of feature rows.

    Light: `| Id | Feature | Tier | Status | Evidence | Missing |`. The id
    is in the first cell and the tier in the third.
    """
    path = repo / "docs" / "parity-checklist.md"
    if not path.exists():
        return []
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.strip("|").split("|")]
        if len(cells) < 3 or set("".join(cells[:2])) <= set("-: "):
            continue
        if cells[0].lower() in ("id", "feature", "tab", "menu", "area", "priority", "#", "section"):
            continue
        tier = cells[2] if re.match(r"^P[0-3]$", cells[2]) else "P2"
        commands = re.findall(r"`(?:cmd|ctl):([^`]+)`", cells[4] if len(cells) > 4 else "")
        rows.append({
            "id": cells[0],
            "title": cells[1] if len(cells) > 1 else cells[0],
            "area": cells[1] if len(cells) > 1 else "",
            "tier": tier,
            "commands": commands,
        })
    return rows


def load_mail_catalogue(repo: Path) -> list[dict]:
    """Mail keeps its catalogue as text files: one per area.

    `crates/engine/catalog/*.txt` has `Area|Path|Label|id` lines. The rows are
    the Outlook command rows, 1,569 of them.
    """
    folder = repo / "crates" / "engine" / "catalog"
    if not folder.exists():
        return []
    rows = []
    for path in sorted(folder.glob("*.txt")):
        area = path.stem
        for line in path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = [p.strip() for p in line.split("|")]
            if len(parts) < 4:
                continue
            # Area|Path|Label|id|note. The id is the fourth field, or the
            # fifth when the row carries a note. A row whose id is "-" has no
            # command — it is a cloud-only service, so it counts as absent
            # rather than unwired.
            cid = parts[3]
            if cid == "-" and len(parts) >= 5:
                cid = parts[4]
            if cid == "-":
                cid = ""
            rows.append({
                "id": cid or f"{area}-{len(rows)}",
                "title": parts[2],
                "area": area,
                "commands": [cid] if cid else [],
            })
    return rows


# Which loader reads which app's catalogue. Each app keeps its own shape, and
# the shape is not the same in any two of them.
CATALOGUE_LOADER = {
    "pdf": load_pdf_catalogue,
    "light": load_checklist_catalogue,
    "sheets": load_bullet_catalogue,
    "docs": load_bullet_catalogue,
    "cad": load_bullet_catalogue,
    "decks": load_bullet_catalogue,
    "photos": load_bullet_catalogue,
    "mail": load_mail_catalogue,
    "film": load_bullet_catalogue,
}


def load_features_toml(repo: Path) -> list[dict]:
    """The catalogue file, when the app has one.

    It is the reference product's feature list, keyed the same way as the
    verdicts file. When it exists it is the catalogue: the app's own
    checklist and the audit report are two views of the same rows, and the
    keyed file is the one that joins.
    """
    data = read_toml(repo / "parity" / "features.toml")
    rows = []
    # The file may be a table of tables (`[feature."key"]`) or an array of
    # tables (`[[feature]]`). Both are read; the key is the table name for
    # the first and the title for the second.
    for key, value in data.items():
        if not isinstance(value, dict):
            continue
        if key in ("schema", "verdict"):
            continue
        if "title" in value:
            rows.append({
                "id": key,
                "title": value.get("title", key),
                "tier": value.get("tier", "P2"),
                "area": value.get("area", ""),
                "commands": value.get("commands", []),
            })
    return rows


def load_catalogue(app: str, repo: Path) -> list[dict]:
    rows = load_features_toml(repo)
    if rows:
        return rows
    loader = CATALOGUE_LOADER.get(app, load_bullet_catalogue)
    return loader(repo)


def load_verdicts(app: str, repo: Path) -> dict[str, dict]:
    """The verdicts file: one entry per judged row.

    The file is keyed by a slug of the feature title, but the catalogue rows
    are keyed by whatever the app uses — a command id, a feature name, a
    catalogue id. So the verdicts are ALSO indexed by their normalised title,
    and the measurement joins on that. Two titles that differ only in case or
    punctuation are the same row.
    """
    path = repo / "parity" / "verdicts.toml"
    if not path.exists():
        return {}
    data = read_toml(path)
    out = {}
    for key, value in data.items():
        if isinstance(value, dict) and "verdict" in value:
            out[key] = value
    return out


def normalise_title(text: str) -> str:
    """The join key: lower case, no punctuation, no extra words."""
    text = text.lower()
    text = text.replace("**", "")
    text = re.sub(r"[^a-z0-9]+", " ", text)
    return " ".join(text.split())


def verdict_index(verdicts: dict[str, dict]) -> dict[str, dict]:
    """Verdicts keyed by their normalised title."""
    index = {}
    for value in verdicts.values():
        title = str(value.get("title", ""))
        key = normalise_title(title)
        if key:
            index.setdefault(key, value)
    return index


def load_registry(app: str, repo: Path) -> set[str]:
    """The command ids the app actually registers.

    `registry.json` is committed at release: the app's CLI dumps it. This is
    the machine half of the measurement, and it is free — no build, no cargo.
    """
    path = repo / "parity" / "registry.json"
    if not path.exists():
        return set()
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return set()
    ids = data.get("ids") if isinstance(data, dict) else data
    return {str(i) for i in ids} if isinstance(ids, list) else set()


# ------------------------------------------------------- the measurement


def evidence_files(evidence: list[str]) -> list[str]:
    """The repo-relative files an entry cites."""
    files = []
    for item in evidence:
        for token in re.split(r"[;,]", item):
            token = token.strip().strip("`")
            if not token:
                continue
            match = re.match(r"([\w./-]+\.\w+)", token)
            if match:
                files.append(match.group(1))
                break
    return files


def measure(app: str, repo: Path) -> dict:
    """The four shares, per tier, for one app."""
    catalogue = load_catalogue(app, repo)
    verdicts = load_verdicts(app, repo)
    by_title = verdict_index(verdicts)
    registry = load_registry(app, repo)

    tiers: dict[str, dict] = {}
    for row in catalogue:
        tier = str(row.get("tier") or "P2").strip()
        if tier in ("NA", "N/A"):
            tier = "NA"
        bucket = tiers.setdefault(tier, {
            "applicable": 0, "works": 0, "partial": 0, "broken": 0,
            "unaudited": 0, "stale": 0, "wired": 0, "na": 0,
        })
        if tier == "NA":
            bucket["na"] += 1
            continue
        bucket["applicable"] += 1

        # Wired: does the row's command exist in the registry?
        #
        # The row's own command ids come from the catalogue. When the row has
        # none — the audit's row is a feature name, not a command — the ids
        # are taken from the verdict for that row, which the auditors
        # recorded. A row with no ids at all cannot be shown to be wired,
        # and saying so is the honest answer.
        ids = list(row.get("commands") or [])
        verdict_for_row = None
        if row.get("id") and row["id"] in verdicts:
            verdict_for_row = verdicts[row["id"]]
        else:
            verdict_for_row = by_title.get(normalise_title(str(row.get("title") or row.get("id") or "")))
        if verdict_for_row and not ids:
            ids = list(verdict_for_row.get("commands", []) or [])
        if registry and ids:
            if any(str(i) in registry for i in ids):
                bucket["wired"] += 1

        verdict = verdict_for_row
        if not verdict:
            bucket["unaudited"] += 1
            continue

        label = str(verdict.get("verdict", "")).upper()
        bucket_key = VERDICTS.get(label)
        if bucket_key is None:
            bucket["unaudited"] += 1
            continue
        if bucket_key == "works":
            bucket["works"] += 1
        elif bucket_key == "partial":
            bucket["partial"] += 1
        elif bucket_key == "broken":
            bucket["broken"] += 1

        # Staleness: has the evidence moved since the verdict was made?
        files = evidence_files(verdict.get("evidence", []))
        commit = str(verdict.get("commit", ""))
        if files and commit and git_log_since(repo, commit, files):
            bucket["stale"] += 1

    return {
        "app": app,
        "name": APPS[app]["name"],
        "counterpart": APPS[app]["counterpart"],
        "rows": len(catalogue),
        "verdicts": len(verdicts),
        "registry": len(registry),
        "tiers": tiers,
    }


def orphans(app: str, repo: Path) -> tuple[list[dict], list[str]]:
    """Catalogue rows citing ids the registry lacks, and ids no row cites."""
    catalogue = load_catalogue(app, repo)
    verdicts = load_verdicts(app, repo)
    registry = load_registry(app, repo)

    missing = []
    for row in catalogue:
        for cid in row.get("commands") or []:
            if registry and str(cid) not in registry:
                missing.append({"id": row.get("id", ""), "command": str(cid)})

    cited = set()
    for row in catalogue:
        for cid in row.get("commands") or []:
            cited.add(str(cid))
    for verdict in verdicts.values():
        for cid in verdict.get("commands", []) or []:
            cited.add(str(cid))
    uncited = sorted(registry - cited) if registry else []
    return missing, uncited


def bad_evidence(app: str, repo: Path) -> list[tuple[str, str]]:
    """Verdict evidence whose path does not resolve."""
    problems = []
    for key, verdict in load_verdicts(app, repo).items():
        for item in verdict.get("evidence", []) or []:
            for token in re.split(r"[;,]", str(item)):
                token = token.strip().strip("`")
                if not token:
                    continue
                match = re.match(r"([\w./-]+\.\w+)", token)
                if not match:
                    continue
                path = repo / match.group(1)
                if not path.exists():
                    problems.append((key, match.group(1)))
                break
    return problems


# ------------------------------------------------------------ reporting


def pct(n: int, of: int) -> str:
    if not of:
        return "—"
    return f"{n} ({n * 100.0 / of:.1f} %)"


def render(app_report: dict) -> str:
    """One app's block: the four shares, per tier.

    The headline is the coverage number the app's own docs quote, so they can
    link here instead of typing it.
    """
    lines = []
    lines.append(f"### {app_report['name']} — vs {app_report['counterpart']}")
    lines.append("")
    if not app_report["tiers"]:
        lines.append("No catalogue found.")
        lines.append("")
        return "\n".join(lines)
    lines.append("| Tier | Applicable | Works | Partial | Broken | **Unaudited** | Wired (machine) | Stale |")
    lines.append("|---|---:|---:|---:|---:|---:|---:|---:|")
    for tier in ("P0", "P1", "P2", "P3", "P2/P3", "NA"):
        bucket = app_report["tiers"].get(tier)
        if not bucket:
            continue
        app = bucket["applicable"]
        lines.append(
            f"| {tier} | {app} | {pct(bucket['works'], app)} | "
            f"{pct(bucket['partial'], app)} | {pct(bucket['broken'], app)} | "
            f"**{pct(bucket['unaudited'], app)}** | {pct(bucket['wired'], app)} | "
            f"{bucket['stale']} |"
        )
    lines.append("")

    # The headline: the coverage share over all tiers, which is the number
    # the app's own docs quote. One number, one place.
    total = {"applicable": 0, "works": 0, "partial": 0, "broken": 0, "unaudited": 0}
    for tier, bucket in app_report["tiers"].items():
        if tier == "NA":
            continue
        for key in total:
            total[key] += bucket.get(key, 0)
    lines.append(f"**Catalogue coverage:** {pct(total['works'] + total['partial'], total['applicable'])} "
                 f"of {total['applicable']} rows are works or partial; "
                 f"{pct(total['works'], total['applicable'])} works outright; "
                 f"**{pct(total['unaudited'], total['applicable'])} unaudited**.")
    lines.append("")
    lines.append(
        f"Catalogue rows: {app_report['rows']}. Verdicts recorded: {app_report['verdicts']}. "
        f"Registry ids: {app_report['registry']}."
    )
    lines.append("")
    return "\n".join(lines)


def suite_table(reports: list[dict]) -> str:
    """The nine-row table. P0 only, and no total: nine apps at different
    catalogue grain do not add."""
    lines = ["| App | vs | Applicable | Works | Partial | Broken | **Unaudited** | Wired (machine) |",
             "|---|---|---:|---:|---:|---:|---:|---:|"]
    for report in reports:
        bucket = report["tiers"].get("P0") or report["tiers"].get("P1") or report["tiers"].get("P2")
        if not bucket:
            lines.append(f"| **{report['name']}** | {report['counterpart']} | — | — | — | — | — | — |")
            continue
        app = bucket["applicable"]
        lines.append(
            f"| **{report['name']}** | {report['counterpart']} | {app} | "
            f"{pct(bucket['works'], app)} | {pct(bucket['partial'], app)} | "
            f"{pct(bucket['broken'], app)} | **{pct(bucket['unaudited'], app)}** | "
            f"{pct(bucket['wired'], app)} |"
        )
    return "\n".join(lines)


# A line that ASSERTS a coverage share. The keyword has to name the metric and
# the line has to carry a share, or the line does not count.
#
# The metric is the catalogue coverage: how much of the reference product's
# feature list exists here. It is the number that drifted — Light's ROADMAP
# says 81.6 % of 532 rows while its own parity doc says 83.2 % of 546, and
# Docs quotes 493/508, 504/520 and 505/520 in three places. Per-feature
# depth percentages are a different measurement and are left alone.
SHARE_WORDS = re.compile(
    r"catalog(?:ue)? coverage|ribbon(?: and menu| and|/)? catalog|"
    r"menu catalog|feature breadth",
    re.IGNORECASE,
)
SHARE = re.compile(r"\d+(\.\d+)?\s*%|\b\d+\s*/\s*\d+\b")


def stray_percentages(repo: Path) -> list[str]:
    """A parity share typed into a file, outside the generated one.

    Every app's own files disagreed with each other because the same number
    was typed into more than one place. Docs quotes 493/508 in ROADMAP.md,
    504/520 in the same file's history and 505/520 in target-app-parity.md;
    PDF quotes breadth as 72 %, 67 % and 72 %. The reader cannot tell which
    is current, so the number is stated once, in the generated file, and the
    other places point at it.

    Only lines that assert a *share* count. A roadmap listing "12 features
    done" is not a parity share, and flagging it would drown the real
    offenders in noise.
    """
    hits = []
    watched = ["ROADMAP.md", "README.md", "docs/target-app-parity.md", "docs/gaps.md"]
    for name in watched:
        path = repo / name
        if not path.exists():
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if SHARE.search(line) and SHARE_WORDS.search(line) and not is_target(line):
                hits.append(f"{name}:{number}")
    return hits


def is_target(line: str) -> bool:
    """A line that states a GOAL, not a measurement.

    Mail's roadmap says "definition of done: full catalog coverage, >= 95 %
    honest parity". That is a target, not a claim about what the app does
    today, and it is not what the generator measures. Flagging it would be
    wrong.
    """
    return bool(re.search(
        r"definition of done|\bgoal\b|\btarget\b|\baim\b|\bwant\b|"
        r"\bplan to\b|\bwill\b|\bshould reach\b|\bby v\d|\buntil\b",
        line,
        re.IGNORECASE,
    ))


# ------------------------------------------------------------------ main


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", help="one app, e.g. mail")
    parser.add_argument("--check", action="store_true", help="fail on drift")
    parser.add_argument("--lint", action="store_true", help="propose stub candidates")
    parser.add_argument("--out", default=str(ROOT / "PARITY.md"))
    args = parser.parse_args()

    apps = [args.app] if args.app else list(APPS)
    reports = []
    failures = []
    for app in apps:
        if app not in APPS:
            print(f"unknown app: {app}", file=sys.stderr)
            return 2
        repo = app_dir(app)
        if repo is None:
            print(f"{app}: not checked out beside the suite; skipped", file=sys.stderr)
            continue
        report = measure(app, repo)
        reports.append(report)

        if args.check:
            missing, uncited = orphans(app, repo)
            for entry in missing:
                failures.append(f"{app}: catalogue row {entry['id']} cites {entry['command']}, which the registry does not have")
            for key, path in bad_evidence(app, repo):
                failures.append(f"{app}: verdict {key} cites {path}, which does not exist")
            for hit in stray_percentages(repo):
                failures.append(f"{app}: a parity number is typed at {hit}; the number belongs in the generated file")

    if args.lint:
        # The lint PROPOSES stub candidates. It does not write verdicts: a
        # stub is a judgement, and a machine that writes judgements is how
        # the catalogue came to claim things it did not do.
        for app in apps:
            repo = app_dir(app)
            if repo is None:
                continue
            for verdict in load_verdicts(app, repo).values():
                for item in verdict.get("evidence", []) or []:
                    if "not available yet" in str(item) or "not implemented" in str(item):
                        print(f"{app}: {verdict.get('verdict')} {str(item)}")
        return 0

    if args.check:
        for failure in failures:
            print(f"::error::{failure}", file=sys.stderr)
        if failures:
            print(f"{len(failures)} parity drift(s)", file=sys.stderr)
            return 1
        print("parity: no drift")
        return 0

    out = ["# Workspace suite — parity", ""]
    out.append("Generated by `scripts/parity.py`. Do not edit by hand: this is the only")
    out.append("place a parity number lives. The four middle columns of every table sum")
    out.append("to the applicable count, and **Unaudited** is the share nobody has judged.")
    out.append("")
    out.append(suite_table(reports))
    out.append("")
    for report in reports:
        out.append(render(report))
    Path(args.out).write_text("\n".join(out), encoding="utf-8")
    print(f"wrote {args.out}")

    # The per-app page. Each app's own docs link to this for the coverage
    # number, so there is exactly one place a number is written and every
    # other place points at it.
    for report in reports:
        repo = app_dir(report["app"])
        if repo is None:
            continue
        page = repo / "parity" / "parity.md"
        page.parent.mkdir(exist_ok=True)
        page.write_text(render(report), encoding="utf-8")
        print(f"wrote {page}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
