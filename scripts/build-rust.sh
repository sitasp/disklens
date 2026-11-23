#!/usr/bin/env bash
# Build the DiskLens Rust core and the Swift bindings.
#
# On macOS this produces a universal static XCFramework and refreshes
# apple/Generated/diskscan.swift. On Linux it builds the host library and
# refreshes the same Swift file; it cannot create the XCFramework because
# that needs the Apple SDK and `xcodebuild`.
#
# Usage:
#   scripts/build-rust.sh            # build (and, on macOS, the XCFramework)
#   scripts/build-rust.sh --test     # also run the Rust test suite first

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORE="$ROOT/core"
GENERATED="$ROOT/apple/Generated"
VENDOR="$ROOT/apple/Vendor"
RUN_TESTS=0
if [[ "${1:-}" == "--test" ]]; then
  RUN_TESTS=1
fi

cd "$CORE"

if [[ "$RUN_TESTS" -eq 1 ]]; then
  cargo test --workspace
fi

mkdir -p "$GENERATED"

if [[ "$(uname -s)" != "Darwin" ]]; then
  cargo build -p diskscan
  cargo run -q -p diskscan --bin uniffi-bindgen -- generate \
    --library "$CORE/target/debug/libdiskscan.so" \
    --language swift \
    --out-dir "$GENERATED"
  echo
  echo "Swift bindings written to apple/Generated."
  echo "The XCFramework is built on macOS. Run this script there, then scripts/generate-xcode.sh."
  exit 0
fi

if ! command -v xcodebuild >/dev/null 2>&1; then
  echo "xcodebuild is required to create the XCFramework." >&2
  exit 1
fi

rustup target add aarch64-apple-darwin x86_64-apple-darwin

cargo build --release -p diskscan --target aarch64-apple-darwin
cargo build --release -p diskscan --target x86_64-apple-darwin

cargo run -q -p diskscan --bin uniffi-bindgen -- generate \
  --library "$CORE/target/aarch64-apple-darwin/release/libdiskscan.a" \
  --language swift \
  --out-dir "$GENERATED"

HEADERS="$VENDOR/include"
rm -rf "$HEADERS"
mkdir -p "$HEADERS"
cp "$GENERATED/diskscanFFI.h" "$HEADERS/diskscanFFI.h"
# xcodebuild looks for module.modulemap inside the headers directory.
cp "$GENERATED/diskscanFFI.modulemap" "$HEADERS/module.modulemap"

XCFRAMEWORK="$VENDOR/DiskScanFFI.xcframework"
rm -rf "$XCFRAMEWORK"
xcodebuild -create-xcframework \
  -library "$CORE/target/aarch64-apple-darwin/release/libdiskscan.a" -headers "$HEADERS" \
  -library "$CORE/target/x86_64-apple-darwin/release/libdiskscan.a" -headers "$HEADERS" \
  -output "$XCFRAMEWORK"

echo
echo "XCFramework: apple/Vendor/DiskScanFFI.xcframework"
echo "Swift bindings: apple/Generated/diskscan.swift"
echo "Next: scripts/generate-xcode.sh && open apple/DiskLens.xcodeproj"
