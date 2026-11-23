# Clone identity for a followed symlink

Unique size counts a file's bytes once per identity. `core/diskscan/src/identity.rs` prefers an APFS clone id over the inode: `identity_key` returns `IdentityKey::Clone { device, clone_id }` when the clone id is present and non-zero, and `IdentityKey::Inode` otherwise. `account` in `scan.rs` walks paths in file-name order, inserts each identity into a set, and gives later paths `shared_elsewhere` so their `unique_apparent` stays 0.

`clone_id` in `core/diskscan/src/platform.rs` is the only macOS-specific call. It uses `getattrlist` with `ATTR_CMNEXT_CLONEID` in `forkattr` and `FSOPT_ATTR_CMN_EXTENDED`. The buffer is 24 bytes. A returned length of at least 16 is read as a naturally aligned `u64` at offset 8. A length of at least 12 is read as a packed `u64` at offset 4. Zero means "no clone id" and becomes `None`.

## The bug

`ScanOptions::follow_symlinks` asks `jwalk` to follow links. A symlink to a file is then classified as `NodeKind::File`, and `entry.metadata()` is `std::fs::metadata`, which stats the target. The inode is the target's inode.

Clone detection did not follow. The old call passed `FSOPT_NOFOLLOW`, so `getattrlist` described the symlink itself. A symlink has no clone id, so `clone_id` returned `None`. The real file and the followed link then produced different keys:

- the real path: `IdentityKey::Clone` of the file's clone id
- the symlink path: `IdentityKey::Inode` of the same inode

`account` treated them as two files. `symlinks_are_leaves_unless_follow_is_set` writes a 3-byte file, a 4096-byte file outside the tree, and a symlink to each. With follow on, unique size must be `3 + 4096 = 4099`. On APFS it was `4102`: the 3-byte file counted twice. Linux never calls `getattrlist`, so both paths stayed on the inode key and the test passed there.

## The change

`FSOPT_NOFOLLOW` was removed. `getattrlist` now follows a final symlink and returns the target's clone id. `clone_id` is only called for `NodeKind::File`. When follow is off, a symlink is `NodeKind::Symlink` and is not asked for a clone id, so dropping `NOFOLLOW` does not change that path. A regular file is not a symlink, so following the last component does not change its id either.

The scanner still does not count the link's own path text. `raw_from_entry` sets `own_apparent` from `meta.len()` only for files, and a followed symlink's metadata is the target's.

## Test

`core/diskscan/tests/scan_tree.rs`, `symlinks_are_leaves_unless_follow_is_set`:

- follow off: the outside link is a symlink leaf and `unique_apparent` is 3
- follow on: `unique_apparent` is `3 + 4096`, and logical size is at least that

On this Mac, after the change, that test and the rest of `cargo test --workspace` passed (49 tests).
