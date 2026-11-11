# shellcheck shell=bash
# Shared setup for the packaging scripts. Source it: `. "$(dirname "$0")/../env.sh"`.
#
# Exports:
#   ROOT             workspace root
#   VERSION          [workspace.package] version from Cargo.toml (override: IRIDIUM_OFFICE_VERSION)
#   DIST             output directory for release artifacts (default: $ROOT/dist/release)
#   IRIDIUM_OFFICE_BUILD_SHA   git commit, recorded in the macOS Info.plist
#   CARGO_TARGET_DIR cargo's target dir (default: $ROOT/target)

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export ROOT

# The version lives in exactly one place: `[workspace.package] version` in the root Cargo.toml.
workspace_version() {
  awk '
    /^\[/ { in_pkg = ($0 == "[workspace.package]") ; next }
    in_pkg && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }
  ' "$ROOT/Cargo.toml"
  return
}

VERSION="${IRIDIUM_OFFICE_VERSION:-$(workspace_version)}"
if [[ -z "$VERSION" ]]; then
  echo "error: could not read [workspace.package] version from $ROOT/Cargo.toml" >&2
  exit 1
fi
export VERSION

DIST="${DIST:-$ROOT/dist/release}"
mkdir -p "$DIST"
export DIST

if [[ -z "${IRIDIUM_OFFICE_BUILD_SHA:-}" ]]; then
  IRIDIUM_OFFICE_BUILD_SHA="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || true)"
fi
export IRIDIUM_OFFICE_BUILD_SHA
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

# Emit a GitHub Actions warning (plain stderr outside Actions).
warn() {
  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then echo "::warning::$*"; else echo "warning: $*" >&2; fi
  return
}

# Builds made with the optional craft-fonts input (CRAFT_FONTS_DIR) embed its fonts, so the
# package carries their licences: fonts/<family>/OFL.txt -> OFL-<family>.txt.
copy_font_licences() {
  local dest="$1" f family
  [[ -n "${CRAFT_FONTS_DIR:-}" ]] || return 0
  for f in "$CRAFT_FONTS_DIR"/fonts/*/OFL.txt; do
    [[ -f "$f" ]] || continue
    family="$(basename "$(dirname "$f")")"
    cp "$f" "$dest/OFL-$family.txt"
  done
}

# Copy the licence and notice files a package must ship into <dest>.
copy_docs() {
  local dest="$1" f
  for f in README.md LICENSE-MIT LICENSE-APACHE NOTICE; do
    if [[ -f "$ROOT/$f" ]]; then cp "$ROOT/$f" "$dest/"; fi
  done
  return
}
