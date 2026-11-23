# DiskLens

A macOS disk analysis and cleanup app. The scanning engine is a Rust library so it can walk large trees on a thread pool, keep the interface responsive, and be tested on Linux. The native SwiftUI app is added in later milestones and calls this library through UniFFI.

## What the core does today

- Parallel directory walk (`jwalk` on a private rayon pool) with cancellation and progress callbacks.
- A size tree with three measurements: logical apparent bytes, unique apparent bytes, and unique allocated bytes. Hard links share an inode and count once toward unique sizes. On macOS, APFS clone ids are read so clones share a unique size too.
- File categories (images, video, developer caches, app bundles, system paths, and others).
- Large-file and old-file queries, category totals.
- Sunburst angles and a squarified treemap, both pure functions of the tree.
- A small `diskscan` CLI for trying the engine without Xcode.

Symbolic links are leaves unless you opt in. Unreadable directories stay in the tree as errors instead of failing the scan.

## Build and test

Requires a stable Rust toolchain (1.83 or newer).

```bash
cd core
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -p diskscan-cli -- scan /path/to/folder --depth 2
cargo run -p diskscan-cli -- usage /path/to/folder
```

The Mac app, duplicate finder, and Xcode project are not in this commit yet. See `docs/ARCHITECTURE.md` once the later milestones land, and `AGENTS.md` for conventions.
