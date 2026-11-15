#!/usr/bin/env bash
# Rewrite packaging/web/apps.lock from the app checkouts beside this repository.
#
# The web image clones the nine apps from GitHub at the revisions this file
# names. Run this after an app's work is pushed, then commit the file: the
# image is built from a commit, so the new revision only reaches the site
# when it is written here.
#
# Usage: packaging/web/update-apps-lock.sh [workspace-parent-dir]
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
PARENT="${1:-$(dirname "$ROOT")}"

# app name : the directory it is checked out in
APPS=(sheets:gc-gridcraft docs:wc-wordcraft cad:cc-cadcraft decks:dc-deckcraft pdf:pdfcraft
      mail:mc-mailcraft photos:pc-photocraft film:fc-filmcraft light:lc-lightcraft)

out="$(mktemp)"
cat > "$out" <<'HEAD'
# The app revisions the web image is built from.
#
# The image clones each app from GitHub. Tracking the branch HEAD means a
# deployment serves whichever commit happened to be pushed, and the build
# layer is cached by its command: a change in an app would not reach the
# site. Pinning the revision makes the build reproducible and makes the
# change visible, because these lines live in this repository and so in
# the commit the image is built from.
#
# Regenerate with: packaging/web/update-apps-lock.sh
HEAD
for pair in "${APPS[@]}"; do
    name="${pair%%:*}"
    dir="${pair##*:}"
    src="$PARENT/$dir"
    if [[ ! -d "$src/.git" ]]; then
        echo "error: $src is not a git checkout" >&2
        rm -f "$out"
        exit 1
    fi
    sha="$(git -C "$src" rev-parse HEAD)"
    echo "$name=$sha" >> "$out"
done
cp "$out" "$HERE/apps.lock"
rm -f "$out"
echo "wrote $HERE/apps.lock"
