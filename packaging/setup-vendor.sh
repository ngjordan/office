#!/usr/bin/env bash
# Create the vendor tree the workspace's path dependencies point at.
#
# The six apps live in their own repositories and are checked out beside this one. The
# workspace refers to them through ../office-vendor/<app>, a directory of symlinks, for two
# reasons:
#
#   - the path is stable: a clone knows where to put the apps and what to call them.
#   - the apps' own names do not end up in the binary. Rust embeds the source path of every
#     crate in file!(), and a path dependency outside the workspace is embedded by its
#     absolute path, so the checkout directory's name would ship in the binary.
#
# Usage: packaging/setup-vendor.sh [workspace-parent-dir]
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
PARENT="${1:-$(dirname "$ROOT")}"

# app name : the repository it comes from
APPS=(sheets:gc-gridcraft docs:wc-wordcraft cad:cc-cadcraft decks:dc-deckcraft pdf:pdfcraft mail:mc-mailcraft)
VENDOR="$PARENT/office-vendor"
mkdir -p "$VENDOR"
for pair in "${APPS[@]}"; do
  name="${pair%%:*}"; dir="${pair##*:}"
  src="$PARENT/$dir"
  if [[ ! -d "$src" ]]; then
    echo "error: $src is missing; clone ngjordan/$name there first" >&2
    exit 1
  fi
  link="$VENDOR/$name"
  if [[ -L "$link" ]]; then rm "$link"; fi
  ln -s "../$dir" "$link"
  echo "  $link -> ../$dir"
done
