# XcodeGen presets

`scripts/generate-xcode.sh` turns `apple/project.yml` into `apple/DiskLens.xcodeproj`. The project is gitignored. XcodeGen ships setting presets (`PRODUCT_NAME`, `SDKROOT`, the clang warning set, `ALWAYS_SEARCH_USER_PATHS`) as YAML next to the real binary, at `../share/xcodegen/SettingPresets` relative to that binary.

## What failed

On this Mac Homebrew cannot install packages because the Command Line Tools are older than the OS. XcodeGen 2.48 was installed by its own script into `~/.local/share/xcodegen/bin/xcodegen`, with a symlink at `~/.local/bin/xcodegen`.

Invoking the symlink made XcodeGen look for presets beside `~/.local/bin`, not beside the real binary. Generation printed:

```text
No "base" settings found
No "debug config" settings found
No "release config" settings found
No "macOS" settings found
```

and the DiskLens target was emitted with an empty `PRODUCT_NAME`. `xcodebuild` then failed with `Module name "" is not a valid identifier` and wrote the product as `Debug/.app`. Invoking the real binary loaded the presets and the warning disappeared.

A Homebrew install usually works without this, because Homebrew also links `share/xcodegen` next to the `bin` symlink. The script does not assume that layout.

## What the script does

1. Require `xcodegen` on `PATH`. The error text names both `brew install xcodegen` and a `~/.local/bin` binary, since either is valid.
2. `command -v xcodegen`, then `realpath` when it exists. macOS 13 and later provide `realpath`. The resolved path is what gets executed, so preset lookup uses the directory of the real binary.
3. Refuse to generate until `apple/Vendor/DiskScanFFI.xcframework` exists. The project links that framework; generating earlier produces an Xcode project that cannot build.

`apple/project.yml` does not use an `info:` block. XcodeGen treats `info.path` as a plist to regenerate, and a run with an empty property set replaced `apple/DiskLens/Info.plist` with a stub that dropped `CFBundleDisplayName`, `LSMinimumSystemVersion`, the category, and the high-resolution flag. Display name and minimum system version stay in the hand-written plist. Both targets point at it with `GENERATE_INFOPLIST_FILE: NO` and `INFOPLIST_FILE: DiskLens/Info.plist`. The store target's `PRODUCT_NAME` is `DiskLensStore`, so the on-disk product does not overwrite `DiskLens.app`, while `CFBundleDisplayName` in the shared plist stays DiskLens.

## Schemes

The generated project has two schemes.

- **DiskLens** builds the unsandboxed app and `DiskLensTests`. The test bundle's `TEST_HOST` is `DiskLens.app`.
- **DiskLensStore** builds the sandboxed app (`DISKLENS_SANDBOX`) and `DiskLensStoreTests`. That bundle compiles the same `apple/DiskLensTests` sources with the sandbox flag and hosts them in `DiskLensStore.app`. Details of the test import are in [swift-compile-and-tests.md](swift-compile-and-tests.md).
