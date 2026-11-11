#!/usr/bin/env bash
# Build and package the Office suite for Linux (<arch> is x86_64 or aarch64):
#
#   $DIST/iridium-office-<version>-linux-<arch>.deb     Debian, Ubuntu, Mint, Pop!_OS, ...
#   $DIST/iridium-office-<version>-linux-<arch>.rpm     Fedora, openSUSE, RHEL, ...
#   $DIST/iridium-office-<version>-linux-<arch>.tar.gz  plain FHS-style tree (bin/, share/)
#
# The suite is one binary that hosts all five apps, so this is the PDF app's script with its
# extra binaries and its own app id dropped.
#
# Usage: packaging/linux/package.sh [--skip-build] [--formats "deb rpm tar"]
#
# Needs: cargo. Optional: nfpm for deb/rpm (https://nfpm.goreleaser.com), desktop-file-validate
# and appstreamcli for the checks. Build on an old distro (CI: Ubuntu 22.04, glibc 2.35) so the
# binaries run on newer ones.
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
HERE="$ROOT/packaging/linux"
# The five apps come in through ../office-vendor; the script creates it if it is missing.
"$ROOT/packaging/setup-vendor.sh"
# The app's name and id live in one file, so renaming the suite does not touch this script.
# shellcheck source=../../branding/brand.env
. "$ROOT/branding/brand.env"
APP_ID="${APP_ID:-ph.office}"
APP_NAME="${APP_NAME:-Office}"

SKIP_BUILD=0
FORMATS="deb rpm tar"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build) SKIP_BUILD=1; shift ;;
    --formats) FORMATS="$2"; shift 2 ;;
    -h | --help) sed -n '2,13p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

ARCH="$(uname -m)"
case "$ARCH" in
  x86_64) DEB_ARCH=amd64 ;;
  aarch64 | arm64) ARCH=aarch64; DEB_ARCH=arm64 ;;
  *) echo "unsupported architecture $ARCH" >&2; exit 2 ;;
esac
export IRIDIUM_OFFICE_MAINTAINER="${IRIDIUM_OFFICE_MAINTAINER:-Office maintainers <office@iridium.ph>}"
BASENAME="iridium-office-$VERSION-linux-$ARCH"

echo "==> $APP_NAME $VERSION for Linux $ARCH ($FORMATS)"

if [[ "$SKIP_BUILD" = 0 ]]; then
  (cd "$ROOT" && cargo build --release --locked -p iridium-office)
fi
BIN="$CARGO_TARGET_DIR/release"
WORK="$CARGO_TARGET_DIR/linux-package"
STAGE="$WORK/root"
rm -rf "$WORK"

# ---- stage an FHS tree (shared by every format) -------------------------------------------------
install -Dm755 "$BIN/iridium-office" "$STAGE/usr/bin/iridium-office"
strip "$STAGE/usr/bin/iridium-office" 2>/dev/null || true
install -Dm644 "$HERE/$APP_ID.desktop" "$STAGE/usr/share/applications/$APP_ID.desktop"
install -Dm644 "$ROOT/assets/app-icon/hicolor/128x128/apps/$APP_ID.png" "$STAGE/usr/share/icons/hicolor/128x128/apps/$APP_ID.png"
install -Dm644 "$ROOT/assets/app-icon/hicolor/256x256/apps/$APP_ID.png" "$STAGE/usr/share/icons/hicolor/256x256/apps/$APP_ID.png"
install -Dm644 "$ROOT/assets/app-icon/hicolor/scalable/apps/$APP_ID.svg" "$STAGE/usr/share/icons/hicolor/scalable/apps/$APP_ID.svg"
mkdir -p "$STAGE/usr/share/metainfo"
sed -e "s/@VERSION@/$VERSION/g" -e "s/@DATE@/${IRIDIUM_OFFICE_BUILD_DATE:-$(date -u +%Y-%m-%d)}/g" \
  -e "s/@APP_ID@/$APP_ID/g" -e "s/@APP_NAME@/$APP_NAME/g" \
  "$HERE/$APP_ID.metainfo.xml.in" >"$STAGE/usr/share/metainfo/$APP_ID.metainfo.xml"
mkdir -p "$STAGE/usr/share/doc/iridium-office"
copy_docs "$STAGE/usr/share/doc/iridium-office"

# ---- deb and rpm --------------------------------------------------------------------------------
if [[ "$FORMATS" == *deb* || "$FORMATS" == *rpm* ]]; then
  command -v nfpm >/dev/null || {
    echo "error: nfpm is not installed (https://nfpm.goreleaser.com)" >&2
    exit 1
  }
  echo "==> building the deb and rpm"
  export NFPM_ARCH="$DEB_ARCH"
  export VERSION
  export RELEASE="1"
  nfpm package -f "$HERE/nfpm.yaml" -p deb -t "$DIST/"
  nfpm package -f "$HERE/nfpm.yaml" -p rpm -t "$DIST/"
fi

# ---- tar.gz -------------------------------------------------------------------------------------
if [[ "$FORMATS" == *tar* ]]; then
  echo "==> building the tarball"
  tar -C "$STAGE" -czf "$DIST/$BASENAME.tar.gz" .
fi

echo "==> done"
ls -lh "$DIST"/*"$VERSION"*linux* 2>/dev/null || true
