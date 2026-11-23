#!/usr/bin/env bash
# Regenerate apple/DiskLens.xcodeproj from apple/project.yml.
# Requires XcodeGen: https://github.com/yonaskolb/XcodeGen (`brew install xcodegen`).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v xcodegen >/dev/null 2>&1; then
  echo "XcodeGen is not installed. Install it with: brew install xcodegen" >&2
  exit 1
fi

if [[ ! -d "$ROOT/apple/Vendor/DiskScanFFI.xcframework" ]]; then
  echo "XCFramework not found. Run scripts/build-rust.sh on this Mac first." >&2
  exit 1
fi

cd "$ROOT/apple"
xcodegen generate
echo "Opened by: open \"$ROOT/apple/DiskLens.xcodeproj\""
