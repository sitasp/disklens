#!/usr/bin/env bash
# Regenerate apple/DiskLens.xcodeproj from apple/project.yml.
# Requires XcodeGen: https://github.com/yonaskolb/XcodeGen
# Homebrew (`brew install xcodegen`) works when the Command Line Tools are
# current. Otherwise put the xcodegen binary on PATH (this Mac uses
# ~/.local/bin/xcodegen).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v xcodegen >/dev/null 2>&1; then
  echo "XcodeGen is not installed." >&2
  echo "Install it with Homebrew (brew install xcodegen) when that works," >&2
  echo "or place the xcodegen binary on PATH, for example ~/.local/bin." >&2
  exit 1
fi

# Setting presets live next to the real binary (../share/xcodegen). A PATH
# symlink such as ~/.local/bin/xcodegen makes XcodeGen look beside the link,
# miss those presets, and emit an app with an empty product name.
XCODEGEN="$(command -v xcodegen)"
if command -v realpath >/dev/null 2>&1; then
  XCODEGEN="$(realpath "$XCODEGEN")"
fi

if [[ ! -d "$ROOT/apple/Vendor/DiskScanFFI.xcframework" ]]; then
  echo "XCFramework not found. Run scripts/build-rust.sh on this Mac first." >&2
  exit 1
fi

cd "$ROOT/apple"
"$XCODEGEN" generate
echo "Open with: open \"$ROOT/apple/DiskLens.xcodeproj\""
echo "Schemes: DiskLens (direct), DiskLensStore (sandboxed App Store build)"
