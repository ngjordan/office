#!/usr/bin/env python3
"""Union two parity layers, matching rows by the command ids they cite.

Two audits of the same app describe the same features in different words, so
a title comparison cannot tell a new row from a re-worded one. The command
ids can: a row that cites `timeline.razor` and a row that cites
`timeline.razor` are the same feature, whatever they are called.

So: take the PRIMARY layer, and add a SECONDARY row only when it cites at
least one command id that no primary row cites. That is the test for "this is
a feature the primary did not cover", and it does not double-count a feature
that both audits found under different names.

    python3 union_parity_layers.py APP PRIMARY_DIR SECONDARY_DIR OUT_DIR
"""

import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity  # noqa: E402


def read_block(text: str, kind: str, slug: str) -> str:
    marker = f'[{kind}."{slug}"]'
    start = text.find(marker)
    if start < 0:
        return ""
    rest = text[start:]
    end = rest.find("\n[")
    return rest if end < 0 else rest[:end]


def cited_commands(verdicts: dict) -> set[str]:
    out = set()
    for row in verdicts.values():
        for cmd in row.get("commands", []) or []:
            out.add(str(cmd))
    return out


def main() -> int:
    if len(sys.argv) != 5:
        print(__doc__)
        return 2
    app = sys.argv[1]
    primary_dir = Path(sys.argv[2])
    secondary_dir = Path(sys.argv[3])
    out_dir = Path(sys.argv[4])
    out_dir.mkdir(parents=True, exist_ok=True)

    primary_v = parity.load_verdicts(app, _as_repo(primary_dir))
    secondary_v = parity.load_verdicts(app, _as_repo(secondary_dir))
    primary_f = parity.load_features_toml(_as_repo(primary_dir))
    secondary_f = parity.load_features_toml(_as_repo(secondary_dir))

    primary_commands = cited_commands(primary_v)
    primary_slugs = {row["id"] for row in primary_f}

    # A secondary row is new when it names a command the primary does not.
    new_verdicts = {}
    for slug, row in secondary_v.items():
        if slug in primary_slugs:
            continue
        cmds = {str(c) for c in (row.get("commands", []) or [])}
        if cmds and not (cmds & primary_commands):
            new_verdicts[slug] = row

    # A secondary feature row with no verdict is still a feature the primary
    # catalogue may not list. It counts when it names a command no primary
    # row cites.
    new_features = {}
    for row in secondary_f:
        slug = row["id"]
        if slug in primary_slugs or slug in new_verdicts:
            continue
        cmds = {str(c) for c in (row.get("commands", []) or [])}
        if cmds and not (cmds & primary_commands):
            new_features[slug] = row

    ptext = (primary_dir / "features.toml").read_text(encoding="utf-8")
    stext = (secondary_dir / "features.toml").read_text(encoding="utf-8")
    extra = "\n".join(
        read_block(stext, "feature", slug) for slug in sorted(new_features)
    )
    if extra.strip():
        ptext = ptext.rstrip() + "\n\n" + extra.strip() + "\n"
    (out_dir / "features.toml").write_text(ptext, encoding="utf-8")

    vtext = (primary_dir / "verdicts.toml").read_text(encoding="utf-8")
    svtext = (secondary_dir / "verdicts.toml").read_text(encoding="utf-8")
    extra = "\n".join(
        read_block(svtext, "verdict", slug) for slug in sorted(new_verdicts)
    )
    if extra.strip():
        vtext = vtext.rstrip() + "\n\n" + extra.strip() + "\n"
    (out_dir / "verdicts.toml").write_text(vtext, encoding="utf-8")

    print(f"features added: {len(new_features)}")
    print(f"verdicts added: {len(new_verdicts)}")
    for slug in sorted(new_features):
        print(f"  + feature {slug}")
    for slug in sorted(new_verdicts):
        print(f"  + verdict {slug}")
    return 0


def _as_repo(parity_dir: Path) -> Path:
    if (parity_dir / "parity").is_dir():
        return parity_dir
    holder = parity_dir.parent / f".parity-holder-{parity_dir.name}"
    (holder / "parity").mkdir(parents=True, exist_ok=True)
    for name in ("features.toml", "verdicts.toml"):
        source = parity_dir / name
        if source.exists():
            shutil.copy(source, holder / "parity" / name)
    return holder


if __name__ == "__main__":
    sys.exit(main())
