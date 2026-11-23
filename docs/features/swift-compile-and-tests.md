# Swift compile fixes and both test hosts

The Swift sources were written against the committed UniFFI 0.32 bindings and had never been compiled. Xcode 27, Swift 5 language mode, `SWIFT_STRICT_CONCURRENCY = minimal`, is strict enough that three call sites and the history test failed. The sandboxed scheme had no test action at all.

## `UncheckedSendable`

`AppModel.beginScan` and `beginDuplicates` build a `ProgressRelay` on the main actor and hand it to `Task.detached`. The relay's closure captures `self`. `UncheckedSendable` exists so that capture is explicit:

```swift
struct UncheckedSendable<T>: @unchecked Sendable {
    let value: T
}
```

The memberwise initializer is `init(value:)`. The call sites were `UncheckedSendable(self)`, which Swift rejects. All four sites (`scan` and `duplicates`, the progress box and the result box) now pass `UncheckedSendable(value: self)`. The type is still unchecked on purpose: the closure hops to the main actor with `Task { @MainActor in ... }` before it writes `scanProgress` or `duplicateProgress`.

## Large-file context menu

`LargeFilesView` shows `model.largeFiles` in a `Table` whose selection binding is `Set<String>`. `FileRow` is `Identifiable` with `id` equal to `path`, so the table's selection identifier is `String`.

The context menu was declared `forSelectionType: FileRow.self`. The closure then received `Set<FileRow>` and passed a row to `reveal`, `quickLook`, and `reviewPaths`, which take strings. It is now `forSelectionType: String.self`, matching the selection. The menu actions receive the selected paths directly.

## History test on the main actor

`HistoryStore` is `@MainActor` because the window observes it. `testHistoryRoundTrip` constructed it from a plain `XCTestCase` method. Xcode 27 reports that as an error (`ActorIsolatedCall`), including through the `XCTAssertEqual` autoclosure. The test method is marked `@MainActor`. XCTest runs it on the main actor, so `init(directory:)`, `record`, `entries`, and `markUndone` are legal. The other tests in the file do not touch a main-actor type and stay nonisolated.

The test still uses a fresh directory under `FileManager.default.temporaryDirectory` and does not write the real Application Support history.

## One test file, two hosts

`apple/DiskLensTests/PathAndHistoryTests.swift` is compiled into both unit-test bundles.

```swift
#if DISKLENS_SANDBOX
@testable import DiskLensStore
#else
@testable import DiskLens
#endif
```

`DiskLensTests` hosts in `DiskLens.app` and imports the `DiskLens` module. `DiskLensStoreTests` is a second target in `apple/project.yml`:

- same source path, `apple/DiskLensTests`
- `SWIFT_ACTIVE_COMPILATION_CONDITIONS: DISKLENS_SANDBOX`
- `TEST_HOST` and `BUNDLE_LOADER` point at `DiskLensStore.app`
- `PRODUCT_MODULE_NAME` is `DiskLensStoreTests` so it does not collide with `DiskLensTests`

The store scheme's test action runs `DiskLensStoreTests`. CI (`.github/workflows/ci.yml`) runs `build test` for that scheme, not only `build`.

## The App Intents build warning

Xcode 27 runs `appintentsmetadataprocessor` for every app target. With no link to `AppIntents.framework` it prints:

```text
warning: Metadata extraction skipped, no AppIntents.framework dependency found
```

`--quiet-warnings` does not filter that line. Passing `--disable` replaces it with a different warning, `Metadata extraction disabled by --disable`.

`apple/DiskLens/Support/NoAppIntents.swift` imports `AppIntents` and declares an empty enum. There is no intent, shortcut, or query. The import is what links the framework. The processor then logs `Extracted no relevant App Intents symbols, skipping writing output` and does not emit a warning.

On this Mac both schemes executed 8 tests with 0 failures. The test host can still log a failure to contact `com.apple.linkd.autoShortcut` while the process starts. That comes from the system, not from an intent we defined, and the tests still pass.
