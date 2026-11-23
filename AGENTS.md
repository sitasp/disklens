# Working on DiskLens

DiskLens is a macOS disk map. Rust (`core/`) walks, hashes, and decides. SwiftUI (`apple/`) draws and talks to the Trash. Read `docs/ARCHITECTURE.md` before changing the boundary.

## Layout

- `core/diskscan` — library, UniFFI surface in `src/ffi.rs`, tests in `tests/` and next to the modules.
- `core/diskscan-cli` — `diskscan scan|dupes|usage`.
- `apple/DiskLens` — SwiftUI app. View models in `ViewModels/`, views in `Views/`.
- `apple/Generated` — UniFFI output. Do not edit by hand. Regenerate with `scripts/build-rust.sh`.
- `apple/project.yml` — XcodeGen spec. The `.xcodeproj` is generated and gitignored.
- `scripts/build-rust.sh` — host build plus Swift bindings. On macOS, also the universal XCFramework.
- `docs/` — architecture, the sandbox decision, and `docs/features/` for the Mac build and the fixes that landed with it.

## Commands

From the repository root:

```bash
make test          # cargo test for the workspace
make clippy        # cargo clippy --all-targets -- -D warnings
make fmt           # cargo fmt
make bindings      # scripts/build-rust.sh
```

On a Mac, after `make bindings`:

```bash
./scripts/generate-xcode.sh
open apple/DiskLens.xcodeproj
```

`generate-xcode.sh` needs XcodeGen on `PATH` (`brew install xcodegen`, or a binary such as `~/.local/bin/xcodegen` when Homebrew cannot install packages) and refuses to run until the XCFramework exists. The script resolves a symlinked `xcodegen` so its setting presets load.

## Conventions

- Rust: edition 2021, `cargo fmt`, clippy with `-D warnings`. No new dependency unless the scanner or the FFI needs it.
- The core API takes `PathBuf` and `&AtomicBool`. UniFFI types stay in `ffi.rs`. Do not put `PathBuf` on an exported record.
- Sizes: logical apparent counts every path; unique apparent and unique allocated count an inode or APFS clone once. Do not collapse those into one field.
- Duplicate selection must not auto-select protected paths. Add a unit test if you change `protection_reason` or `select_group`.
- Deletion planning does not delete. Trash and `removeItem` stay in Swift.
- Cancellation returns `ScanError::Cancelled` and no partial tree.
- Swift: macOS 14, Swift 5 language mode, `@Observable` view models, views that do not call `stat` themselves. Byte strings come from `formatByteCount`.
- `DiskLens` is the unsandboxed target. `DiskLensStore` defines `DISKLENS_SANDBOX` and must keep using `NSOpenPanel` plus security-scoped bookmarks (`FolderAccessStore`). Do not let the store target scan a volume root directly.
- Explicit deletion of a protected path sets `DeletionEntry.protected` and a reason. Do not drop those paths from the plan. The review must list them again and wait for `ProtectedRemovalGate`.
- New filesystem behaviour needs a temp-dir test: symlinks, hard links, unreadable entries, and cancellation are the cases that have gone wrong before.

## Tests that must keep passing

- `cargo test --workspace` from `core/`, including clippy `-D warnings`.
- On a Mac, `xcodebuild -project apple/DiskLens.xcodeproj -scheme DiskLens -destination 'platform=macOS' build test` after the Rust script and XcodeGen.

Linux cannot compile the Swift package. If you change `ffi.rs`, regenerate `apple/Generated` and keep the Swift call sites in step with the generated names (`scanFolder`, `findDuplicateFiles`, `reviewDeletion`, `CancelFlag`, `ProgressSink.onProgress`).

## Git

Small commits. Do not commit `core/target`, `apple/Vendor/DiskScanFFI.xcframework`, or `apple/DiskLens.xcodeproj`.
