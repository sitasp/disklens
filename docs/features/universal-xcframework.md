# Universal XCFramework

`scripts/build-rust.sh` is what the Swift app links. On a Mac it builds `libdiskscan.a` twice, packs the headers, and writes `apple/Vendor/DiskScanFFI.xcframework`. The framework is gitignored. The generated Swift in `apple/Generated/diskscan.swift` is committed.

## What failed

The script used to pass both static libraries to one `xcodebuild -create-xcframework` invocation:

```text
-library aarch64/.../libdiskscan.a -headers include
-library x86_64/.../libdiskscan.a -headers include
```

Xcode 27 treats macOS arm64 and macOS x86_64 as the same platform. The command exited 70 with:

```text
Both 'macos-x86_64' and 'macos-arm64' represent two equivalent library definitions.
```

A second link warning showed up after the framework did build. `blake3` compiles C (`blake3_neon.o`) through the `cc` crate. With no deployment target set, that object recorded minos 27.2, while the app's `MACOSX_DEPLOYMENT_TARGET` is 14.0. `ld` warned that the object was built for a newer macOS than the binary being linked. Rust's prebuilt standard library stayed at minos 11.0, which is older than 14.0 and is fine.

## What the script does now

1. Export `MACOSX_DEPLOYMENT_TARGET` before either `cargo build`, defaulting to `14.0`. An existing value in the environment is kept. `cc` and `rustc` both read that variable, so the C objects and the Rust objects record minos 14.0.
2. `rustup target add aarch64-apple-darwin x86_64-apple-darwin`, then a release build of the `diskscan` package for each target. The host `cargo run` of `uniffi-bindgen` uses the arm64 archive to regenerate `apple/Generated/diskscan.swift`, `diskscanFFI.h`, and `diskscanFFI.modulemap`.
3. Copy the header and the module map into `apple/Vendor/include`. The module map is renamed to `module.modulemap` because that is the name `xcodebuild` looks for inside a headers directory. The map imports `diskscanFFI.h` and re-exports the Darwin builtins UniFFI 0.32 emits (`Darwin`, `_Builtin_stdbool`, `_Builtin_stdint`).
4. `lipo -create` the two thin archives into `core/target/universal-apple-darwin/release/libdiskscan.a`. That directory is under `target/` and is gitignored. `lipo -info` on the result lists `x86_64` and `arm64`.
5. Create the XCFramework from that one library:

```bash
xcodebuild -create-xcframework \
  -library "$UNIVERSAL" -headers "$HEADERS" \
  -output apple/Vendor/DiskScanFFI.xcframework
```

The slice directory is `macos-arm64_x86_64`, not two `macos-*` directories. `apple/project.yml` links it as a non-embedded framework (`embed: false`) because it is a static archive. The Swift file calls the C functions directly, so the linker pulls the referenced objects. `-force_load` was not required. If a later toolchain drops the archive, the flag in the README points at `macos-arm64_x86_64/libdiskscan.a`.

Linux is unchanged. The script builds the host `.so`, regenerates the Swift file, and exits before `lipo` or `xcodebuild`.

## How to confirm

```bash
lipo -info apple/Vendor/DiskScanFFI.xcframework/macos-arm64_x86_64/libdiskscan.a
otool -l apple/Vendor/DiskScanFFI.xcframework/macos-arm64_x86_64/libdiskscan.a | grep minos
```

The fat archive contains both architectures. Object minos values are 14.0 for this crate and 11.0 for the prebuilt standard library. A Debug link of DiskLens no longer warns about a 27.2 object.
