//! Owned snapshot of a scan.
//!
//! The tree is immutable after [`crate::scan`] returns. Children of each
//! directory are ordered by unique apparent size, largest first, then by
//! name. That order is what the outline and the sunburst both start from.

use std::path::{Path, PathBuf};

use crate::category::FileCategory;

/// How a directory entry relates to the walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Directory,
    File,
    /// A symbolic link that was not followed. Its size is not rolled into parents.
    Symlink,
    /// Socket, fifo, device node, or anything else with no meaningful length.
    Other,
}

/// Which byte measurement a view should display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SizeMetric {
    /// Apparent file length, counting each inode or clone group once.
    /// This is the default chart metric: it is stable across filesystems and
    /// matches what a user sees in a file inspector.
    #[default]
    UniqueApparent,
    /// Allocated blocks (`st_blocks * 512`), counting each shared identity once.
    /// Closer to `du` and to the space a delete would reclaim, except that
    /// APFS snapshots and purgeable space are still not included.
    UniqueAllocated,
    /// Apparent length summed once per path, so hard links and followed
    /// symlinks are counted again at every name.
    LogicalApparent,
}

/// One file or directory in the scan tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanNode {
    /// Final path component, or the full path for the filesystem root.
    pub name: String,
    pub path: PathBuf,
    pub kind: NodeKind,
    pub category: FileCategory,
    /// Byte length of this entry alone. Zero for directories.
    pub own_apparent: u64,
    /// Allocated size of this entry alone, in bytes.
    pub own_allocated: u64,
    /// Sum of apparent lengths of every file path under this node.
    pub logical_apparent: u64,
    /// Apparent bytes that this node uniquely contributes, plus the same for children.
    pub unique_apparent: u64,
    /// Allocated bytes counted once per hard link and once per APFS clone group.
    pub unique_allocated: u64,
    /// Number of regular files under this node, including hard-link aliases.
    pub file_count: u64,
    pub modified_unix_ms: Option<i64>,
    pub inode: Option<u64>,
    pub device: Option<u64>,
    /// Present only when clone detection ran and the volume returned an id.
    pub clone_id: Option<u64>,
    /// `true` when another path already owns this inode or clone id, so this
    /// node's unique sizes are zero.
    pub shared_elsewhere: bool,
    /// Directory whose device differs from the scan root. Contents were not walked
    /// when `ScanOptions::stay_on_filesystem` was set.
    pub other_volume: bool,
    /// Why this directory could not be read, if it could not.
    pub error: Option<String>,
    /// Child entries, largest unique apparent size first.
    pub children: Vec<ScanNode>,
}

impl ScanNode {
    #[must_use]
    pub fn size(&self, metric: SizeMetric) -> u64 {
        match metric {
            SizeMetric::UniqueApparent => self.unique_apparent,
            SizeMetric::UniqueAllocated => self.unique_allocated,
            SizeMetric::LogicalApparent => self.logical_apparent,
        }
    }

    /// Direct child with this file name.
    #[must_use]
    pub fn child(&self, name: &str) -> Option<&ScanNode> {
        self.children.iter().find(|child| child.name == name)
    }

    /// Walk every regular file under this node, in tree order.
    pub fn visit_files<'a>(&'a self, visit: &mut dyn FnMut(&'a ScanNode)) {
        if self.kind == NodeKind::File {
            visit(self);
        }
        for child in &self.children {
            child.visit_files(visit);
        }
    }
}

/// A problem that did not abort the scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanIssue {
    pub path: String,
    pub message: String,
}

/// Result of a successful scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanTree {
    pub root_path: PathBuf,
    pub root: ScanNode,
    pub issues: Vec<ScanIssue>,
    /// Entries visited during the walk, before tree folding.
    pub entries_seen: u64,
    pub elapsed_ms: u64,
    /// Device id of the root, used to recognise mount points.
    pub root_device: Option<u64>,
}

impl ScanTree {
    /// Node at an absolute path that was inside the scan.
    ///
    /// `path` must be equal to a node path. A missing trailing slash and a
    /// path that merely points at the same inode via another route do not match.
    #[must_use]
    pub fn node(&self, path: &Path) -> Option<&ScanNode> {
        if self.root.path == path {
            return Some(&self.root);
        }
        let relative = path.strip_prefix(&self.root.path).ok()?;
        let mut node = &self.root;
        for component in relative.components() {
            let std::path::Component::Normal(name) = component else {
                continue;
            };
            let name = name.to_string_lossy();
            node = node.children.iter().find(|child| child.name == name)?;
        }
        Some(node)
    }
}

/// How [`crate::query_files`] orders matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileSort {
    #[default]
    Largest,
    Smallest,
    Oldest,
    Newest,
    Name,
}

/// Filter for the large-file and old-file views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileQuery {
    pub min_apparent_size: u64,
    /// Keep files whose mtime is strictly earlier than this instant.
    pub modified_before_unix_ms: Option<i64>,
    /// Keep files whose mtime is strictly later than this instant.
    pub modified_after_unix_ms: Option<i64>,
    pub category: Option<FileCategory>,
    /// Maximum rows to return after sorting. `0` returns nothing.
    pub limit: usize,
    pub sort: FileSort,
}

impl Default for FileQuery {
    fn default() -> Self {
        Self {
            min_apparent_size: 0,
            modified_before_unix_ms: None,
            modified_after_unix_ms: None,
            category: None,
            limit: 100,
            sort: FileSort::Largest,
        }
    }
}

/// One row in the large-file list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub category: FileCategory,
    pub apparent_size: u64,
    pub allocated_size: u64,
    pub modified_unix_ms: Option<i64>,
    pub shared_elsewhere: bool,
    pub extension: String,
}

/// Rolled-up bytes for one [`FileCategory`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryTotal {
    pub category: FileCategory,
    pub unique_apparent: u64,
    pub unique_allocated: u64,
    pub file_count: u64,
}
