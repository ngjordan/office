#!/usr/bin/env python3
"""Merge two parity layers for one app: the stronger rows win, the rest stay.

Two audits of the same app can produce different row sets -- one
finer-grained, one covering areas the other missed. The rows are not in
conflict; they are in different places. The honest result is the union.

    python3 merge_parity_layers.py APP PRIMARY_PARITY_DIR SECONDARY_PARITY_DIR OUT_PARITY_DIR

Each directory holds features.toml and verdicts.toml. The PRIMARY layer is
the one with the stronger citations; a row that exists in both is taken from
it. A row only in the SECONDARY is carried over unchanged, so nothing the
secondary found is lost -- including the rows it left unjudged, which stay
unjudged.
"""

import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity  # noqa: E402


def read_block(text: str, kind: str, slug: str) -> str:
    """The text of one `[kind."slug"]` table, header included."""
    marker = f'[{kind}."{slug}"]'
    start = text.find(marker)
    if start < 0:
        return ""
    rest = text[start:]
    end = rest.find("\n[")
    return rest if end < 0 else rest[:end]


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

    primary_slugs = {row["id"] for row in primary_f}
    secondary_slugs = {row["id"] for row in secondary_f}

    added_features = sorted(secondary_slugs - primary_slugs)
    added_verdicts = sorted(set(secondary_v) - set(primary_v))

    # Features: the primary's rows, plus the secondary's rows it lacks.
    ptext = (primary_dir / "features.toml").read_text(encoding="utf-8")
    stext = (secondary_dir / "features.toml").read_text(encoding="utf-8")
    extra = "\n".join(read_block(stext, "feature", slug) for slug in added_features)
    if extra.strip():
        ptext = ptext.rstrip() + "\n\n" + extra.strip() + "\n"
    (out_dir / "features.toml").write_text(ptext, encoding="utf-8")

    # Verdicts: the primary's rows, plus the secondary's judged rows it lacks.
    vtext = (primary_dir / "verdicts.toml").read_text(encoding="utf-8")
    svtext = (secondary_dir / "verdicts.toml").read_text(encoding="utf-8")
    extra = "\n".join(read_block(svtext, "verdict", slug) for slug in added_verdicts)
    if extra.strip():
        vtext = vtext.rstrip() + "\n\n" + extra.strip() + "\n"
    (out_dir / "verdicts.toml").write_text(vtext, encoding="utf-8")

    print(f"features added: {len(added_features)}")
    print(f"verdicts added: {len(added_verdicts)}")
    for slug in added_features:
        judged = "judged" if slug in secondary_v else "unjudged"
        print(f"  + {slug} ({judged})")
    return 0


def _as_repo(parity_dir: Path) -> Path:
    """A directory shaped like an app repo, so the loader finds parity/."""
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
