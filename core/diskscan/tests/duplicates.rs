//! Duplicate detection against temp directories.
//!
//! Selection rules that do not need a filesystem live next to `select_group`.
//! These tests check the staged hash, hard-link suppression, and the deletion plan.

use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, UNIX_EPOCH};

use diskscan::{
    find_duplicates, plan_deletion, DuplicateOptions, KeepStrategy, NodeKind, ScanError,
    SelectionPolicy,
};

fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    File::create(path).unwrap().write_all(bytes).unwrap();
}

fn options_with_window(partial_bytes: u64) -> DuplicateOptions {
    DuplicateOptions { partial_bytes, ..DuplicateOptions::default() }
}

#[test]
fn identical_files_form_one_group_and_different_bytes_do_not() {
    let dir = tempfile::tempdir().unwrap();
    let body = b"the same sixteen";
    write_file(&dir.path().join("a.txt"), body);
    write_file(&dir.path().join("nested/b.txt"), body);
    write_file(&dir.path().join("c.txt"), b"different!!!!!!! ");

    let report =
        find_duplicates(dir.path(), &options_with_window(4), &AtomicBool::new(false), None)
            .unwrap();
    assert_eq!(report.groups.len(), 1);
    assert_eq!(report.groups[0].files.len(), 2);
    assert_eq!(report.groups[0].size, body.len() as u64);
    assert_eq!(report.groups[0].wasted_bytes(), body.len() as u64);
    assert!(report.groups[0].hash_hex.len() == 64);
}

#[test]
fn same_edges_and_different_middle_are_not_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let mut left = vec![1u8; 64];
    let mut right = vec![1u8; 64];
    left[30] = 9;
    right[30] = 8;
    write_file(&dir.path().join("left.bin"), &left);
    write_file(&dir.path().join("right.bin"), &right);

    let report =
        find_duplicates(dir.path(), &options_with_window(8), &AtomicBool::new(false), None)
            .unwrap();
    assert!(report.groups.is_empty(), "full hash must split files that share a head and tail");
}

#[test]
fn partial_hash_splits_files_that_differ_at_the_start() {
    let dir = tempfile::tempdir().unwrap();
    let mut left = vec![2u8; 64];
    let mut right = vec![2u8; 64];
    left[0] = 1;
    right[0] = 3;
    write_file(&dir.path().join("left.bin"), &left);
    write_file(&dir.path().join("right.bin"), &right);
    let report =
        find_duplicates(dir.path(), &options_with_window(8), &AtomicBool::new(false), None)
            .unwrap();
    assert!(report.groups.is_empty());
}

#[test]
fn three_files_two_of_which_match() {
    let dir = tempfile::tempdir().unwrap();
    write_file(&dir.path().join("a"), b"alpha-content");
    write_file(&dir.path().join("b"), b"alpha-content");
    write_file(&dir.path().join("c"), b"alpha-content");
    write_file(&dir.path().join("d"), b"other-content!");
    let report =
        find_duplicates(dir.path(), &DuplicateOptions::default(), &AtomicBool::new(false), None)
            .unwrap();
    assert_eq!(report.groups.len(), 1);
    assert_eq!(report.groups[0].files.len(), 3);
}

#[test]
fn empty_files_are_excluded_unless_min_size_is_zero() {
    let dir = tempfile::tempdir().unwrap();
    write_file(&dir.path().join("empty-a"), b"");
    write_file(&dir.path().join("empty-b"), b"");
    write_file(&dir.path().join("empty-c"), b"");
    let excluded =
        find_duplicates(dir.path(), &DuplicateOptions::default(), &AtomicBool::new(false), None)
            .unwrap();
    assert!(excluded.groups.is_empty());

    let included = find_duplicates(
        dir.path(),
        &DuplicateOptions { min_size: 0, ..DuplicateOptions::default() },
        &AtomicBool::new(false),
        None,
    )
    .unwrap();
    assert_eq!(included.groups.len(), 1);
    assert_eq!(included.groups[0].files.len(), 3);
    assert_eq!(included.groups[0].size, 0);
}

#[test]
fn hard_links_alone_are_not_duplicates_but_a_real_copy_is() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("original.bin");
    write_file(&original, b"shared-bytes");
    fs::hard_link(&original, dir.path().join("link.bin")).unwrap();

    let only_links =
        find_duplicates(dir.path(), &DuplicateOptions::default(), &AtomicBool::new(false), None)
            .unwrap();
    assert!(only_links.groups.is_empty(), "one inode is not a duplicate set");

    write_file(&dir.path().join("copy.bin"), b"shared-bytes");
    let with_copy =
        find_duplicates(dir.path(), &DuplicateOptions::default(), &AtomicBool::new(false), None)
            .unwrap();
    assert_eq!(with_copy.groups.len(), 1);
    assert_eq!(with_copy.groups[0].files.len(), 3);
    assert_eq!(with_copy.groups[0].distinct_identities(), 2);
    assert_eq!(with_copy.groups[0].wasted_bytes(), b"shared-bytes".len() as u64);
}

#[test]
fn unreadable_file_is_skipped_and_cancellation_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    write_file(&dir.path().join("a.txt"), b"readable-dup");
    let locked = dir.path().join("b.txt");
    write_file(&locked, b"readable-dup");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let report =
        find_duplicates(dir.path(), &DuplicateOptions::default(), &AtomicBool::new(false), None)
            .unwrap();
    assert!(
        report.groups.is_empty() || report.unreadable.iter().any(|path| path.ends_with("b.txt"))
    );
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();

    let flag = AtomicBool::new(true);
    let cancelled = find_duplicates(dir.path(), &DuplicateOptions::default(), &flag, None);
    assert!(matches!(cancelled, Err(ScanError::Cancelled)));
}

#[test]
fn deletion_plan_sums_files_skips_protected_and_counts_hard_links_once() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.bin");
    let second = dir.path().join("second.bin");
    write_file(&first, &[7u8; 100]);
    write_file(&second, &[8u8; 50]);
    let link = dir.path().join("link.bin");
    fs::hard_link(&first, &link).unwrap();

    let policy = SelectionPolicy::default();
    let both_links = plan_deletion(&[first.clone(), link.clone()], &policy, false);
    assert_eq!(both_links.entries.len(), 2);
    assert!(both_links.reclaimable_bytes > 0);
    let one_link = plan_deletion(&[first.as_path()], &policy, false);
    assert_eq!(one_link.reclaimable_bytes, 0, "the other name still exists");

    let folder = dir.path().join("folder");
    write_file(&folder.join("a.txt"), b"hello");
    write_file(&folder.join("b.txt"), b"world!");
    let directory = plan_deletion(&[folder.as_path()], &policy, false);
    assert_eq!(directory.entries.len(), 1);
    assert_eq!(directory.entries[0].kind, NodeKind::Directory);
    assert_eq!(directory.entries[0].apparent_size, 11);
    assert!(directory.reclaimable_bytes > 0);

    let missing = plan_deletion(&[dir.path().join("nope")], &policy, false);
    assert!(missing.entries.is_empty());
    assert_eq!(missing.skipped.len(), 1);

    let link_only = dir.path().join("pointer");
    std::os::unix::fs::symlink(&second, &link_only).unwrap();
    let symlink_plan = plan_deletion(&[link_only], &policy, false);
    assert_eq!(symlink_plan.entries[0].kind, NodeKind::Symlink);
    assert_eq!(symlink_plan.reclaimable_bytes, 0);
    assert!(symlink_plan.entries[0].note.is_some());

    let protected = plan_deletion(&[PathBuf::from("/System/Library/no-such-file")], &policy, false);
    assert!(protected.entries.is_empty());
    assert!(!protected.skipped.is_empty());

    let allowed = SelectionPolicy { protect_system: false, ..SelectionPolicy::default() };
    let _ = allowed;
    let opened = plan_deletion(&[PathBuf::from("/System/Library/no-such-file")], &policy, true);
    // The path is protected and allowed, but it does not exist, so it is skipped
    // only when measurement fails. With allow_protected it is measured and then
    // reported missing.
    assert!(opened.entries.is_empty());
    assert!(opened.skipped.iter().any(|issue| issue.message.contains("not found")));
}

#[test]
fn deletion_of_two_real_copies_reclaims_one_allocation_each() {
    let dir = tempfile::tempdir().unwrap();
    let left = dir.path().join("left.txt");
    let right = dir.path().join("right.txt");
    write_file(&left, &[1u8; 32]);
    write_file(&right, &[1u8; 32]);
    let plan = plan_deletion(&[left, right], &SelectionPolicy::default(), false);
    assert_eq!(plan.entries.len(), 2);
    let single = plan_deletion(&[dir.path().join("left.txt")], &SelectionPolicy::default(), false);
    assert!(plan.reclaimable_bytes > single.reclaimable_bytes);
    assert!(plan.reclaimable_bytes >= 64 || plan.reclaimable_bytes % 512 == 0);
}

#[test]
fn smart_selection_on_real_mtimes() {
    let dir = tempfile::tempdir().unwrap();
    let older = dir.path().join("older.txt");
    let newer = dir.path().join("newer.txt");
    write_file(&older, b"same-contents");
    write_file(&newer, b"same-contents");
    File::options()
        .write(true)
        .open(&older)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(5))
        .unwrap();
    File::options()
        .write(true)
        .open(&newer)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(50))
        .unwrap();

    let report =
        find_duplicates(dir.path(), &DuplicateOptions::default(), &AtomicBool::new(false), None)
            .unwrap();
    let decision = diskscan::select_group(&report.groups[0], &SelectionPolicy::default());
    let kept = decision.files.iter().find(|file| file.keeper).unwrap();
    assert!(kept.path.ends_with("newer.txt"));
    assert!(decision
        .files
        .iter()
        .any(|file| file.selected_for_removal && file.path.ends_with("older.txt")));

    let oldest = SelectionPolicy { strategy: KeepStrategy::Oldest, ..SelectionPolicy::default() };
    let decision = diskscan::select_group(&report.groups[0], &oldest);
    assert!(decision.files.iter().any(|file| file.keeper && file.path.ends_with("older.txt")));
}
