#!/usr/bin/env python3
"""Convert a parity pair from the array-of-tables form to the generator's form.

The generator reads `[feature."slug"]` and `[verdict."slug"]` tables whose
values are plain key/value pairs. Some audits produce `[[features]]` and
`[[verdicts]]` arrays with a `slug` key and, for verdicts, a markdown table
in a `table` cell. That form parses as TOML but the generator's reader finds
no rows in it, so copying it over would silently zero the app's measurement.

This converts one form to the other. It does not judge anything: it moves the
audit's own conclusions across and preserves them.

    python3 convert_parity_format.py FEATURES_IN VERDICTS_IN OUT_DIR

The evidence cells hold `path:line` and `path:line-line` references. The
generator's rule is that an ABSENT or STUB verdict must pin a line, so a
range is reduced to its first line -- `path:3305-3313` becomes `path:3305`.
A reference with no line at all is kept as a file path, which the generator
counts as unaudited rather than broken.
"""

import re
import sys
import tomllib
from pathlib import Path


def cell(table: str, name: str) -> str:
    """The value of a `| Name | value |` row in a markdown table cell."""
    for line in table.splitlines():
        if not line.startswith("|"):
            continue
        parts = [p.strip() for p in line.strip().strip("|").split("|")]
        if len(parts) >= 2 and parts[0].lower() == name.lower():
            return parts[1]
    return ""


def pin(path: str) -> str:
    """Reduce `path:3305-3313` to `path:3305`; keep a bare path as it is."""
    match = re.match(r"^(.*?):(\d+)(?:-(\d+))?$", path.strip())
    if not match:
        return path.strip()
    return f"{match.group(1)}:{match.group(2)}"


def esc(text: str) -> str:
    """A TOML basic string."""
    return text.replace("\\", "\\\\").replace('"', '\\"').replace("\n", " ").strip()


def parse_evidence(raw: str) -> list[str]:
    """Every `path:line` in the cell, deduped, in order."""
    refs = re.findall(r"[A-Za-z0-9_./-]+\.[A-Za-z0-9_]+(?::\d+(?:-\d+)?)?", raw)
    out = []
    for ref in refs:
        pinned = pin(ref)
        if pinned not in out:
            out.append(pinned)
    return out


def convert(features_in: Path, verdicts_in: Path, out_dir: Path, repo: Path | None = None) -> None:
    features = tomllib.loads(features_in.read_text(encoding="utf-8")).get("features", [])
    verdicts = tomllib.loads(verdicts_in.read_text(encoding="utf-8")).get("verdicts", [])

    out_dir.mkdir(parents=True, exist_ok=True)

    # An evidence path that does not exist is worse than no citation: the
    # generator's rule is that an absence must pin a place, and a path that
    # resolves to nothing pins nothing. Drop the paths that do not resolve
    # when a repo is given, and say how many were dropped.
    dropped = 0

    def keep(evidence: list[str]) -> list[str]:
        nonlocal dropped
        if repo is None:
            return evidence
        out = []
        for ref in evidence:
            path = ref.split(":")[0]
            if (repo / path).exists():
                out.append(ref)
            else:
                dropped += 1
        return out

    lines = [
        "# Film verdicts: what the handlers actually do, with file:line evidence.",
        "# Converted from the array-of-tables form by scripts/convert_parity_format.py.",
        "",
    ]
    for row in verdicts:
        slug = row["slug"]
        table = row.get("table", "")
        verdict = cell(table, "Verdict").upper()
        level = cell(table, "Evidence Level").lower()
        evidence = keep(parse_evidence(cell(table, "Implementation Evidence")))
        gap = cell(table, "Gap")
        behaviour = cell(table, "Premiere Behaviour")

        note = behaviour
        if gap:
            note = f"{behaviour} Missing: {gap}" if behaviour else f"Missing: {gap}"

        lines.append(f'[verdict."{slug}"]')
        title = ""
        for feature in features:
            if feature.get("slug") == slug:
                title = feature.get("title", "")
                break
        if title:
            lines.append(f'title = "{esc(title)}"')
        lines.append(f'verdict = "{esc(verdict or "ABSENT")}"')
        lines.append(f'level = "{esc(level or "wired")}"')
        lines.append("evidence = [" + ", ".join(f'"{e}"' for e in evidence) + "]")
        feature_commands = []
        for feature in features:
            if feature.get("slug") == slug:
                feature_commands = feature.get("commands", []) or []
                break
        lines.append("commands = [" + ", ".join(f'"{esc(c)}"' for c in feature_commands) + "]")
        if note:
            lines.append(f'note = "{esc(note)}"')
        lines.append("")

    (out_dir / "verdicts.toml").write_text("\n".join(lines), encoding="utf-8")

    lines = [
        "# Film feature catalogue: one row per Adobe Premiere Pro feature.",
        "# Converted from the array-of-tables form by scripts/convert_parity_format.py.",
        "",
    ]
    for feature in features:
        lines.append(f'[feature."{feature["slug"]}"]')
        lines.append(f'title = "{esc(feature.get("title", ""))}"')
        lines.append(f'tier = "{esc(feature.get("tier", "all"))}"')
        area = feature.get("area", "")
        if area:
            lines.append(f'area = "{esc(area)}"')
        commands = feature.get("commands", []) or []
        lines.append("commands = [" + ", ".join(f'"{esc(c)}"' for c in commands) + "]")
        lines.append('reference = "yes"')
        lines.append("")

    (out_dir / "features.toml").write_text("\n".join(lines), encoding="utf-8")

    if dropped:
        print(f"dropped {dropped} evidence path(s) that do not exist in the repo")


if __name__ == "__main__":
    if len(sys.argv) not in (4, 5):
        print(__doc__)
        sys.exit(2)
    repo = Path(sys.argv[4]) if len(sys.argv) == 5 else None
    convert(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), repo)
    print("converted")
