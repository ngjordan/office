#!/usr/bin/env bash
# Build, sign and (optionally) notarize the macOS release artifacts:
#
#   $DIST/iridium-office-<version>-macos-<arch>.dmg    Office.app on a drag-to-Applications DMG
#   $DIST/iridium-office-<version>-macos-<arch>.zip    the same app, zipped
#
# Usage: packaging/macos/package.sh [--arch universal|aarch64|x86_64] [--skip-build]
#
# Signing (env):
#   MACOS_SIGN_IDENTITY   codesign identity (name or SHA-1). Default "-" = ad-hoc (local testing;
#                         Gatekeeper will reject the result on other Macs). In CI, import-cert.sh sets it.
#   MACOS_KEYCHAIN        keychain holding the identity (optional)
# Notarization (env; all three needed, and a real identity):
#   APPLE_ID, APPLE_PASSWORD (app-specific password), APPLE_TEAM_ID
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
HERE="$ROOT/packaging/macos"
# The five apps come in through ../office-vendor; the script creates it if it is missing.
"$ROOT/packaging/setup-vendor.sh"

# The app's name and id live in one file, so renaming the suite does not touch this script.
# shellcheck source=../../branding/brand.env
. "$ROOT/branding/brand.env"
APP_NAME="${APP_NAME:-Office}"
APP_ID="${APP_ID:-ph.office}"

ARCH=universal
SKIP_BUILD=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    -h | --help) sed -n '2,16p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
case "$ARCH" in
  universal) TARGETS=(aarch64-apple-darwin x86_64-apple-darwin) ;;
  aarch64) TARGETS=(aarch64-apple-darwin) ;;
  x86_64) TARGETS=(x86_64-apple-darwin) ;;
  *) echo "unknown --arch $ARCH" >&2; exit 2 ;;
esac

# Keep in sync with LSMinimumSystemVersion in Info.plist.in.
export MACOSX_DEPLOYMENT_TARGET=11.0
IDENTITY="${MACOS_SIGN_IDENTITY:--}"
SHORT_VERSION="${VERSION%%-*}"
WORK="$CARGO_TARGET_DIR/macos-package"
# A scratch directory for the temporary notary key file.
TMP="$WORK/tmp"
APP="$WORK/$APP_NAME.app"
DMG="$DIST/iridium-office-$VERSION-macos-$ARCH.dmg"
ZIP="$DIST/iridium-office-$VERSION-macos-$ARCH.zip"

NOTARIZE=0
if [[ "$IDENTITY" = "-" ]]; then
  warn "macOS: ad-hoc signing (no MACOS_SIGN_IDENTITY); artifacts are not notarized"
elif [[ -n "${APPLE_API_KEY:-}" ]] && [[ -n "${APPLE_API_KEY_ID:-}" ]] && [[ -n "${APPLE_API_ISSUER_ID:-}" ]]; then
  NOTARIZE=1
elif [[ -n "${APPLE_ID:-}" ]] && [[ -n "${APPLE_PASSWORD:-}" ]] && [[ -n "${APPLE_TEAM_ID:-}" ]]; then
  NOTARIZE=1
else
  warn "macOS: no notary credentials (APPLE_API_KEY trio, or APPLE_ID/APPLE_PASSWORD/APPLE_TEAM_ID); signed but not notarized"
fi

echo "==> $APP_NAME $VERSION for macOS ($ARCH), identity: $IDENTITY, notarize: $NOTARIZE"

# ---- build -------------------------------------------------------------------------------------
if [[ "$SKIP_BUILD" = 0 ]]; then
  args=()
  for t in "${TARGETS[@]}"; do args+=(--target "$t"); done
  (cd "$ROOT" && cargo build --release --locked -p iridium-office "${args[@]}")
fi

rm -rf "$WORK"
mkdir -p "$WORK/bin" "$TMP"
inputs=()
for t in "${TARGETS[@]}"; do inputs+=("$CARGO_TARGET_DIR/$t/release/iridium-office"); done
lipo -create -output "$WORK/bin/iridium-office" "${inputs[@]}"
lipo -info "$WORK/bin/iridium-office"

# ---- signing helpers ---------------------------------------------------------------------------
sign() {
  # Hardened runtime + secure timestamp for a real identity; ad-hoc can't be timestamped.
  local ts=(--timestamp)
  [[ "$IDENTITY" = "-" ]] && ts=(--timestamp=none)
  local kc=()
  [[ -n "${MACOS_KEYCHAIN:-}" ]] && kc=(--keychain "$MACOS_KEYCHAIN")
  codesign --force --sign "$IDENTITY" ${kc[@]+"${kc[@]}"} "${ts[@]}" "$@"
}

# `stapler staple` and `stapler validate` both fetch the ticket from Apple's CloudKit, and a dropped
# connection there (NSURLErrorDomain -1005, "Error 68") fails them although notarization worked.
# Both are idempotent, so retry with a pause; a missing or bad ticket still fails every try.
stapler_retry() {
  local try
  for try in 1 2 3 4; do
    xcrun stapler "$1" "$2" && return 0
    [[ "$try" = 4 ]] && break
    echo "stapler $1 failed (try $try of 4); retrying in $((try * 20)) s" >&2
    sleep $((try * 20))
  done
  echo "error: stapler $1 $(basename "$2") failed 4 times" >&2
  exit 1
}

notarize() {
  local file="$1" out id status
  echo "==> notarizing $(basename "$file") (this can take a few minutes)"
  # Two ways to authenticate to the notary service:
  #   1. an App Store Connect API key (APPLE_API_KEY + APPLE_API_KEY_ID + APPLE_API_ISSUER_ID)
  #   2. an Apple ID and an app-specific password (APPLE_ID + APPLE_PASSWORD + APPLE_TEAM_ID)
  # The API key is preferred: it does not expire and needs no interactive 2FA.
  local auth=()
  if [[ -n "${APPLE_API_KEY:-}" && -n "${APPLE_API_KEY_ID:-}" && -n "${APPLE_API_ISSUER_ID:-}" ]]; then
    local key="$TMP/notary-$$.p8"
    # The key file goes even if notarization fails, so the trap covers the error path too.
    trap 'rm -f "$key"' RETURN
    ( umask 077; printf '%s' "$APPLE_API_KEY" | base64 --decode >"$key" )
    chmod 600 "$key"
    auth=(--key "$key" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER_ID")
  else
    auth=(--apple-id "$APPLE_ID" --password "$APPLE_PASSWORD" --team-id "$APPLE_TEAM_ID")
  fi
  # Submit without --wait, then poll the service. A team's first submission can sit In Progress
  # for hours; resubmitting would throw that work away and start the clock again.
  out="$(xcrun notarytool submit "$file" "${auth[@]}" --output-format json)" || true
  echo "$out"
  id="$(printf '%s' "$out" | plutil -extract id raw -o - - 2>/dev/null || true)"
  if [[ -z "$id" ]]; then
    echo "error: the notary service returned no submission id" >&2
    exit 1
  fi
  echo "==> submission $id; waiting for the notary service"
  status=""
  local attempt
  for attempt in $(seq 1 240); do
    status="$(xcrun notarytool info "$id" "${auth[@]}" --output-format json 2>/dev/null \
      | plutil -extract status raw -o - - 2>/dev/null || true)"
    if [[ "$status" = "Accepted" || "$status" = "Invalid" ]]; then
      break
    fi
    if [[ $((attempt % 20)) = 0 ]]; then
      echo "    still $status after $((attempt * 30))s"
    fi
    sleep 30
  done
  if [[ "$status" != "Accepted" ]]; then
    if [[ -n "$id" ]]; then
      xcrun notarytool log "$id" "${auth[@]}" || true
    fi
    echo "error: notarization of $(basename "$file") failed (status: ${status:-unknown})" >&2
    exit 1
  fi
  return 0
}

# ---- the app bundle ----------------------------------------------------------------------------
echo "==> assembling $APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
# The executable and icon carry the display name (CFBundleExecutable / CFBundleIconFile).
cp "$WORK/bin/iridium-office" "$APP/Contents/MacOS/$APP_NAME"
cp "$ROOT/assets/app-icon/iridium-office.icns" "$APP/Contents/Resources/$APP_NAME.icns"
# The licence and attribution files the app ships.
for f in NOTICE LICENSE LICENSE-MIT LICENSE-APACHE COPYRIGHT; do
  [[ -f "$ROOT/$f" ]] && cp "$ROOT/$f" "$APP/Contents/Resources/"
done
copy_font_licences "$APP/Contents/Resources"
sed -e "s/@APP_NAME@/$APP_NAME/g" -e "s/@APP_ID@/$APP_ID/g" \
  -e "s/@VERSION@/$VERSION/g" -e "s/@SHORT_VERSION@/$SHORT_VERSION/g" \
  -e "s/@BUILD_SHA@/${IRIDIUM_OFFICE_BUILD_SHA:-unknown}/g" \
  "$HERE/Info.plist.in" >"$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
printf 'APPL????' >"$APP/Contents/PkgInfo"

# Strip the extended attributes before signing. A copied bundle can carry a resource fork or
# Finder information, and codesign --strict rejects that as "detritus".
xattr -cr "$APP"

# Sign inside-out: nested code first, then the bundle itself (no --deep on the final signature).
sign --options runtime --entitlements "$HERE/entitlements.plist" "$APP/Contents/MacOS/$APP_NAME"
sign --options runtime --entitlements "$HERE/entitlements.plist" "$APP"
codesign --verify --strict --deep --verbose=2 "$APP"

if [[ "$NOTARIZE" = 1 ]]; then
  ditto -c -k --keepParent "$APP" "$WORK/notarize.zip"
  notarize "$WORK/notarize.zip"
  stapler_retry staple "$APP"
  stapler_retry validate "$APP"
  spctl --assess --type execute -vvv "$APP"
fi

# ---- DMG ---------------------------------------------------------------------------------------
echo "==> building $DMG"
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
ditto "$APP" "$STAGE/$APP_NAME.app"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
# `hdiutil create -srcfolder` writes the files as they are. `makehybrid -hfs` gave every file a
# com.apple.FinderInfo (a Finder location of -1,-1), which `codesign --verify --strict` rejects as
# "Finder information, or similar detritus" on the app in the image and on a copy installed from it,
# so MDM and compliance checks reported a correctly notarized app as damaged (#390).
# `create` attaches a device while it copies, which fails now and then on CI runners ("Resource
# busy"), so it gets a few tries.
hdiutil_retry() {
  local what="$1" attempt
  shift
  for attempt in 1 2 3 4 5; do
    if hdiutil "$@"; then
      return 0
    fi
    [[ "$attempt" = 5 ]] && { echo "hdiutil $what failed 5 times" >&2; return 1; }
    warn "hdiutil $what failed (attempt $attempt of 5); retrying in $((attempt * 10))s"
    sleep $((attempt * 10))
  done
}
hdiutil_retry create create -srcfolder "$STAGE" -volname "$APP_NAME" -fs HFS+ -format UDZO -imagekey zlib-level=9 -ov "$DMG"
sign "$DMG"
codesign --verify --strict --verbose=2 "$DMG"
# The app inside the image must still pass --strict, so a makehybrid-style regression can't ship.
MOUNT="$WORK/dmg-check"
rm -rf "$MOUNT"
mkdir -p "$MOUNT"
hdiutil_retry attach attach -nobrowse -readonly -mountpoint "$MOUNT" "$DMG" >/dev/null
if ! codesign --verify --strict --deep --verbose=2 "$MOUNT/$APP_NAME.app"; then
  hdiutil_retry detach detach "$MOUNT" >/dev/null || true
  exit 1
fi
hdiutil_retry detach detach "$MOUNT" >/dev/null
if [[ "$NOTARIZE" = 1 ]]; then
  notarize "$DMG"
  stapler_retry staple "$DMG"
  stapler_retry validate "$DMG"
  spctl --assess --type open --context context:primary-signature -vvv "$DMG"
fi

# ---- zip ---------------------------------------------------------------------------------------
# The DMG is the installer; the zip is what a CI job can attach to a release, and what a user who
# prefers not to mount an image can unzip and drag to Applications.
echo "==> building $ZIP"
rm -f "$ZIP"
ditto -c -k --keepParent "$APP" "$ZIP"

# The release notes have to say whether the artifacts are notarized, and the release job
# cannot ask the build job: it writes this instead.
if [[ "$NOTARIZE" = 1 ]]; then
  echo "notarized" >"$DIST/NOTARIZATION"
else
  echo "not notarized" >"$DIST/NOTARIZATION"
fi

echo "==> done"
ls -lh "$DMG" "$ZIP"
