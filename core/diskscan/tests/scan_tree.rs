//! Filesystem fixtures for the scanner.
//!
//! Each test builds a throwaway directory. Nothing here touches the user's
//! home folder. Permission tests restore modes on drop so the temp directory
//! can be deleted.

use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

use diskscan::{
    query_files, scan, summarize_categories, FileCategory, FileQuery, NodeKind, ProgressUpdate,
    ScanError, ScanOptions, ScanPhase,
};

fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut file = File::create(path).unwrap();
    file.write_all(bytes).unwrap();
}

struct RestorePerms(PathBuf, u32);
impl Drop for RestorePerms {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(self.1));
    }
}

fn scan_ok(path: &Path, options: &ScanOptions) -> diskscan::ScanTree {
    let cancel = AtomicBool::new(false);
    scan(path, options, &cancel, None).unwrap_or_else(|err| panic!("scan failed: {err}"))
}

#[test]
fn nested_sizes_and_categories_add_up() {
    let dir = tempfile::tempdir().unwrap();
    write_file(&dir.path().join("photo.jpg"), &[1u8; 300]);
    write_file(&dir.path().join("docs/notes.md"), b"hello");
    write_file(&dir.path().join("proj/src/main.rs"), b"fn main() {}");
    write_file(&dir.path().join("empty.dat"), b"");

    let tree = scan_ok(dir.path(), &ScanOptions::default());
    assert_eq!(tree.root.file_count, 4);
    assert_eq!(tree.root.unique_apparent, 300 + 5 + 12);
    assert_eq!(tree.root.logical_apparent, tree.root.unique_apparent);
    assert!(tree.root.unique_apparent <= tree.root.logical_apparent);

    let photo = tree.root.child("photo.jpg").unwrap();
    assert_eq!(photo.kind, NodeKind::File);
    assert_eq!(photo.category, FileCategory::Image);
    assert_eq!(photo.own_apparent, 300);
    assert!(photo.own_allocated > 0);
    assert_eq!(photo.own_allocated % 512, 0);

    let notes = tree.node(&dir.path().join("docs/notes.md")).unwrap();
    assert_eq!(notes.category, FileCategory::Document);
    assert_eq!(notes.own_apparent, 5);

    let code = tree.node(&dir.path().join("proj/src/main.rs")).unwrap();
    assert_eq!(code.category, FileCategory::Code);

    let empty = tree.root.child("empty.dat").unwrap();
    assert_eq!(empty.own_apparent, 0);
    assert_eq!(empty.file_count, 1);

    // Largest child is the photo, so it leads the default ordering.
    assert_eq!(tree.root.children[0].name, "photo.jpg");
}

#[test]
fn hard_links_count_once_toward_unique_size() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("a.txt");
    let alias = dir.path().join("nested/b.txt");
    write_file(&original, b"hello world");
    fs::create_dir_all(alias.parent().unwrap()).unwrap();
    fs::hard_link(&original, &alias).unwrap();

    let tree = scan_ok(dir.path(), &ScanOptions::default());
    assert_eq!(tree.root.file_count, 2);
    assert_eq!(tree.root.logical_apparent, 22);
    assert_eq!(tree.root.unique_apparent, 11);
    assert!(tree.root.logical_apparent >= tree.root.unique_apparent);

    let owner = tree.node(&original).unwrap();
    let shared = tree.node(&alias).unwrap();
    assert!(!owner.shared_elsewhere, "lexicographically first path owns the bytes");
    assert!(shared.shared_elsewhere);
    assert_eq!(owner.unique_apparent, 11);
    assert_eq!(shared.unique_apparent, 0);
    assert_eq!(owner.inode, shared.inode);
}

#[test]
fn symlinks_are_leaves_unless_follow_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let big = outside.path().join("big.bin");
    write_file(&big, &[7u8; 4096]);
    let inside = dir.path().join("inside.bin");
    write_file(&inside, b"abc");
    symlink(&big, dir.path().join("outside-link")).unwrap();
    symlink(&inside, dir.path().join("inside-link")).unwrap();
    symlink(dir.path().join("missing"), dir.path().join("broken")).unwrap();

    let plain = scan_ok(dir.path(), &ScanOptions::default());
    assert_eq!(plain.root.child("outside-link").unwrap().kind, NodeKind::Symlink);
    assert_eq!(plain.root.unique_apparent, 3, "targets are not counted");
    assert_eq!(plain.root.file_count, 1);

    let followed = ScanOptions { follow_symlinks: true, ..ScanOptions::default() };
    let walked = scan_ok(dir.path(), &followed);
    assert_eq!(walked.root.unique_apparent, 3 + 4096);
    assert!(walked.root.logical_apparent >= walked.root.unique_apparent);
}

#[test]
fn symlink_cycles_terminate() {
    let dir = tempfile::tempdir().unwrap();
    write_file(&dir.path().join("file.txt"), b"data");
    symlink(dir.path(), dir.path().join("loop")).unwrap();
    let options = ScanOptions { follow_symlinks: true, ..ScanOptions::default() };
    let tree = scan_ok(dir.path(), &options);
    assert_eq!(tree.root.child("file.txt").unwrap().own_apparent, 4);
    assert!(tree.root.file_count >= 1);
}

#[test]
fn hidden_files_honor_the_option_and_unicode_names_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    write_file(&dir.path().join(".secret"), b"hidden");
    write_file(&dir.path().join("résumé.pdf"), b"cv");

    let all = scan_ok(dir.path(), &ScanOptions::default());
    assert!(all.root.child(".secret").is_some());
    let resume = all.root.child("résumé.pdf").unwrap();
    assert_eq!(resume.category, FileCategory::Document);
    assert_eq!(resume.own_apparent, 2);

    let visible = ScanOptions { include_hidden: false, ..ScanOptions::default() };
    let filtered = scan_ok(dir.path(), &visible);
    assert!(filtered.root.child(".secret").is_none());
    assert!(filtered.root.child("résumé.pdf").is_some());
}

#[test]
fn unreadable_directory_is_an_issue_not_a_failed_scan() {
    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("locked");
    write_file(&locked.join("secret.txt"), b"nope");
    write_file(&dir.path().join("visible.txt"), b"ok");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let _restore = RestorePerms(locked.clone(), 0o755);

    let tree = scan_ok(dir.path(), &ScanOptions::default());
    assert_eq!(tree.root.child("visible.txt").unwrap().own_apparent, 2);
    let locked_node = tree.root.child("locked").unwrap();
    assert!(locked_node.error.is_some(), "expected a read error, got {locked_node:?}");
    assert!(
        tree.issues.iter().any(|issue| issue.path.contains("locked")),
        "issues: {:?}",
        tree.issues
    );
    assert_eq!(tree.root.unique_apparent, 2);
}

#[test]
fn unreadable_file_still_contributes_its_length() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("private.bin");
    write_file(&path, &[9u8; 128]);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    let _restore = RestorePerms(path.clone(), 0o644);

    let tree = scan_ok(dir.path(), &ScanOptions::default());
    assert_eq!(tree.root.child("private.bin").unwrap().own_apparent, 128);
}

#[test]
fn missing_path_is_not_found_and_a_file_root_is_a_single_node() {
    let missing = scan(
        "/this/path/does/not/exist-disklens",
        &ScanOptions::default(),
        &AtomicBool::new(false),
        None,
    );
    assert!(matches!(missing, Err(ScanError::NotFound { .. })));

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("solo.txt");
    write_file(&file, b"abcdef");
    let tree = scan_ok(&file, &ScanOptions::default());
    assert_eq!(tree.root.kind, NodeKind::File);
    assert_eq!(tree.root.own_apparent, 6);
    assert!(tree.root.children.is_empty());
}

#[test]
fn cancellation_before_and_during_the_walk() {
    let dir = tempfile::tempdir().unwrap();
    write_file(&dir.path().join("a.txt"), b"a");
    let flag = AtomicBool::new(true);
    let cancelled = scan(dir.path(), &ScanOptions::default(), &flag, None);
    assert!(matches!(cancelled, Err(ScanError::Cancelled)));

    for index in 0..400 {
        write_file(&dir.path().join(format!("f{index}.bin")), b"0123456789");
    }
    let flag = AtomicBool::new(false);
    let seen = Mutex::new(Vec::new());
    let result = scan(
        dir.path(),
        &ScanOptions::default(),
        &flag,
        Some(&|update: &ProgressUpdate| {
            if update.files_seen > 0 {
                flag.store(true, Ordering::Relaxed);
            }
            seen.lock().unwrap().push(update.phase);
        }),
    );
    assert!(matches!(result, Err(ScanError::Cancelled)));
    assert!(!seen.lock().unwrap().is_empty());
}

#[test]
fn progress_reports_done_for_a_finished_scan() {
    let dir = tempfile::tempdir().unwrap();
    write_file(&dir.path().join("a.txt"), b"abc");
    let updates = Mutex::new(Vec::new());
    let cancel = AtomicBool::new(false);
    scan(
        dir.path(),
        &ScanOptions::default(),
        &cancel,
        Some(&|update: &ProgressUpdate| updates.lock().unwrap().push(update.clone())),
    )
    .unwrap();
    let updates = updates.lock().unwrap();
    assert!(updates.iter().any(|update| update.phase == ScanPhase::Walking));
    assert!(updates.iter().any(|update| update.phase == ScanPhase::Done));
    assert!(updates
        .iter()
        .any(|update| update.current_path.contains("a.txt") || update.files_seen > 0));
}

#[test]
fn queries_find_large_old_files_and_categories() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old.txt");
    let young = dir.path().join("young.txt");
    write_file(&old, &[1u8; 200]);
    write_file(&young, &[1u8; 50]);
    write_file(&dir.path().join("song.mp3"), &[2u8; 80]);
    let file = File::options().write(true).open(&old).unwrap();
    file.set_modified(UNIX_EPOCH + Duration::from_secs(10)).unwrap();

    let tree = scan_ok(dir.path(), &ScanOptions::default());
    let large = query_files(
        &tree.root,
        &FileQuery { min_apparent_size: 100, limit: 10, ..FileQuery::default() },
    );
    assert_eq!(large.len(), 1);
    assert_eq!(large[0].name, "old.txt");

    let old_rows = query_files(
        &tree.root,
        &FileQuery { modified_before_unix_ms: Some(20_000), limit: 10, ..FileQuery::default() },
    );
    assert_eq!(old_rows.len(), 1);
    assert_eq!(old_rows[0].extension, "txt");

    let summary = summarize_categories(&tree.root);
    let audio = summary.iter().find(|row| row.category == FileCategory::Audio).unwrap();
    assert_eq!(audio.unique_apparent, 80);
    assert_eq!(audio.file_count, 1);
}
