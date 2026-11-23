#!/usr/bin/env bash
# Build a Release DiskLens.app and zip it for a GitHub release.
#
# Usage, from the repository root:
#   scripts/package-mac.sh v0.1.0-alpha.1
#
# The marketing version inside the app comes from apple/project.yml.
# The argument is the git tag and is used only in the zip name.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAG="${1:-}"
if [[ -z "$TAG" ]]; then
  echo "usage: scripts/package-mac.sh v0.1.0-alpha.1" >&2
  exit 1
fi

DERIVED="$ROOT/build/DerivedData"
APP="$DERIVED/Build/Products/Release/DiskLens.app"
DIST="$ROOT/build/dist"
ZIP="$DIST/DiskLens-${TAG}-mac.zip"

export PATH="$HOME/.local/bin:$PATH"

cd "$ROOT"
./scripts/generate-xcode.sh

xcodebuild \
  -project apple/DiskLens.xcodeproj \
  -scheme DiskLens \
  -configuration Release \
  -destination 'platform=macOS' \
  -derivedDataPath "$DERIVED" \
  build

if [[ ! -d "$APP" ]]; then
  echo "Release app was not produced at $APP" >&2
  exit 1
fi

mkdir -p "$DIST"
rm -f "$ZIP"
ditto -c -k --keepParent "$APP" "$ZIP"

echo
echo "App: $APP"
echo "Zip: $ZIP"
defaults read "$APP/Contents/Info" CFBundleShortVersionString
defaults read "$APP/Contents/Info" CFBundleVersion
defaults read "$APP/Contents/Info" DiskLensReleaseChannel
codesign -dv "$APP" 2>&1 | awk -F= '/^Signature=|^Identifier=|^TeamIdentifier=/'
ls -lh "$ZIP"
