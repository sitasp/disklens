//! Review the space a deletion would reclaim.
//!
//! Nothing in this module deletes a file. The Swift app moves approved paths
//! to the Trash and writes a history record so the move can be undone.
//! Reclaim estimates count a hard-linked file only when every link `nlink`
//! reports is part of the selection. APFS clones seen by the scan are counted
//! once for the whole clone group.

use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use crate::duplicates::{reclaimable_bytes, DuplicateFile};
use crate::model::{NodeKind, ScanIssue, ScanNode};
use crate::platform::clone_id;
use crate::scan::{scan, ScanOptions};
use crate::selection::{protection_reason, SelectionPolicy};

/// One path the user asked to remove, after checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionEntry {
    pub path: String,
    pub kind: NodeKind,
    /// Unique apparent bytes. A symbolic link contributes 0: removing the
    /// link does not remove the target.
    pub apparent_size: u64,
    /// Unique allocated bytes under this path, before the plan-wide hard-link
    /// adjustment in [`DeletionPlan::reclaimable_bytes`].
    pub allocated_size: u64,
    pub protected: bool,
    pub protection_reason: Option<String>,
    pub note: Option<String>,
}

/// What the review screen shows before anything is touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionPlan {
    pub entries: Vec<DeletionEntry>,
    /// Paths that will not be touched, with the reason.
    ///
    /// A protected path lands here only when the caller did not explicitly
    /// include protected items. An explicit selection is flagged on the entry
    /// instead, so the review can show it again.
    pub skipped: Vec<ScanIssue>,
    /// Bytes deleting every entry would free.
    pub reclaimable_bytes: u64,
}

impl DeletionPlan {
    /// Entries the user chose that sit in a protected location.
    #[must_use]
    pub fn protected_entries(&self) -> Vec<&DeletionEntry> {
        self.entries.iter().filter(|entry| entry.protected).collect()
    }

    /// The review must ask again before these items are removed.
    ///
    /// Smart selection never marks protected paths. This is true only after
    /// someone explicitly included one.
    #[must_use]
    pub fn requires_protected_confirmation(&self) -> bool {
        self.entries.iter().any(|entry| entry.protected)
    }
}

/// Build a review plan for `paths`.
///
/// When `include_protected` is false, protected paths are recorded in
/// [`DeletionPlan::skipped`] and left out of the removable entries. That is
/// the automatic path. When it is true, the caller has explicitly selected
/// those paths: they stay in `entries` with [`DeletionEntry::protected`] set
/// and a [`DeletionEntry::protection_reason`], and
/// [`DeletionPlan::requires_protected_confirmation`] is true.
///
/// Missing paths are skipped. This function never deletes.
#[must_use]
pub fn plan_deletion(
    paths: &[impl AsRef<Path>],
    policy: &SelectionPolicy,
    include_protected: bool,
) -> DeletionPlan {
    let mut entries = Vec::new();
    let mut skipped = Vec::new();
    let mut pieces = Vec::new();
    for path in paths {
        let path = path.as_ref();
        if let Some(reason) = protection_reason(path, policy) {
            if !include_protected {
                skipped.push(ScanIssue { path: path.display().to_string(), message: reason });
                continue;
            }
        }
        match measure(path) {
            Ok((mut entry, found)) => {
                if let Some(reason) = protection_reason(path, policy) {
                    entry.protected = true;
                    entry.protection_reason = Some(reason);
                }
                pieces.extend(found);
                entries.push(entry);
            }
            Err(message) => skipped.push(ScanIssue { path: path.display().to_string(), message }),
        }
    }
    let selected = vec![true; pieces.len()];
    DeletionPlan { entries, skipped, reclaimable_bytes: reclaimable_bytes(&pieces, &selected) }
}

fn measure(path: &Path) -> Result<(DeletionEntry, Vec<DuplicateFile>), String> {
    let meta = std::fs::symlink_metadata(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            "not found".to_owned()
        } else {
            err.to_string()
        }
    })?;
    if meta.file_type().is_symlink() {
        return Ok((
            make_entry(
                path,
                NodeKind::Symlink,
                0,
                0,
                Some("symbolic link; only the link is removed, not its target".to_owned()),
            ),
            Vec::new(),
        ));
    }
    if meta.is_file() {
        let allocated = meta.blocks().saturating_mul(512);
        let piece = file_piece(path, &meta, allocated);
        return Ok((make_entry(path, NodeKind::File, meta.len(), allocated, None), vec![piece]));
    }
    if meta.is_dir() {
        let cancel = AtomicBool::new(false);
        let options = ScanOptions { stay_on_filesystem: true, ..ScanOptions::default() };
        let tree = scan(path, &options, &cancel, None).map_err(|err| err.to_string())?;
        let pieces = pieces_in(&tree.root);
        return Ok((
            make_entry(
                path,
                NodeKind::Directory,
                tree.root.unique_apparent,
                tree.root.unique_allocated,
                None,
            ),
            pieces,
        ));
    }
    Ok((
        make_entry(path, NodeKind::Other, 0, 0, Some("not a file or directory".to_owned())),
        Vec::new(),
    ))
}

fn make_entry(
    path: &Path,
    kind: NodeKind,
    apparent_size: u64,
    allocated_size: u64,
    note: Option<String>,
) -> DeletionEntry {
    DeletionEntry {
        path: path.display().to_string(),
        kind,
        apparent_size,
        allocated_size,
        protected: false,
        protection_reason: None,
        note,
    }
}

fn pieces_in(node: &ScanNode) -> Vec<DuplicateFile> {
    let mut pieces = Vec::new();
    node.visit_files(&mut |file| {
        let (Some(device), Some(inode)) = (file.device, file.inode) else { return };
        pieces.push(DuplicateFile {
            path: file.path.clone(),
            name: file.name.clone(),
            size: file.own_allocated,
            modified_unix_ms: file.modified_unix_ms,
            inode,
            device,
            clone_id: file.clone_id,
            nlink: file.nlink.unwrap_or(1).max(1),
        });
    });
    pieces
}

fn file_piece(path: &Path, meta: &std::fs::Metadata, allocated: u64) -> DuplicateFile {
    DuplicateFile {
        path: path.to_path_buf(),
        name: path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
        size: allocated,
        modified_unix_ms: None,
        inode: meta.ino(),
        device: meta.dev(),
        clone_id: clone_id(path),
        nlink: meta.nlink().max(1),
    }
}

#[cfg(test)]
mod plan_flags {
    use super::*;

    fn entry(path: &str, protected: bool) -> DeletionEntry {
        DeletionEntry {
            path: path.to_owned(),
            kind: NodeKind::File,
            apparent_size: 10,
            allocated_size: 10,
            protected,
            protection_reason: protected.then(|| "inside a Library folder".to_owned()),
            note: None,
        }
    }

    #[test]
    fn only_an_explicit_protected_entry_asks_again() {
        let mixed = DeletionPlan {
            entries: vec![entry("/tmp/a", false), entry("/tmp/Library/b", true)],
            skipped: vec![],
            reclaimable_bytes: 10,
        };
        assert!(mixed.requires_protected_confirmation());
        assert_eq!(mixed.protected_entries().len(), 1);
        assert_eq!(mixed.protected_entries()[0].path, "/tmp/Library/b");

        let ordinary = DeletionPlan {
            entries: vec![entry("/tmp/a", false)],
            skipped: vec![],
            reclaimable_bytes: 10,
        };
        assert!(!ordinary.requires_protected_confirmation());
        assert!(ordinary.protected_entries().is_empty());
    }
}
