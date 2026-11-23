# Features implemented on the Mac

These notes cover the work that made DiskLens build, test, and run on macOS. The product behaviour they sit on is described in [ARCHITECTURE.md](../ARCHITECTURE.md) and [SANDBOX.md](../SANDBOX.md).

| Feature | What changed |
| --- | --- |
| [Universal XCFramework](universal-xcframework.md) | One fat static library for arm64 and x86_64, deployment target 14.0 |
| [Ad-hoc signing](adhoc-signing.md) | Sign to Run Locally, no Apple Developer team |
| [XcodeGen presets](xcodegen-presets.md) | Resolve a symlinked `xcodegen` so the app gets a product name |
| [Clone identity](clone-identity.md) | A followed symlink shares the target's APFS clone id |
| [Swift compile and tests](swift-compile-and-tests.md) | Call-site fixes, main-actor history test, store test host |
| [Open a folder from the command line](open-folder-from-cli.md) | `--args /path` and "search the scanned folder" |
| [Open panel and system locations](open-panel-system-locations.md) | The panel starts at home, and `/Applications` asks again |
| [Chart and duplicate readability](chart-and-duplicate-readability.md) | Sunburst hole, treemap contrast, duplicate toolbar |
| [Release versioning](release-versioning.md) | `0.1.0` alpha tags, build numbers, and the Mac zip |
