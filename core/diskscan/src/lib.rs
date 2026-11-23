//! DiskLens core: parallel disk scanning, categorisation, and chart layout.
//!
//! The crate is Unix-only. Linux CI exercises the filesystem behaviour; the
//! macOS app links the same library and additionally reads APFS clone ids.
//!
//! # Size model
//!
//! * **Logical apparent** counts every file path's byte length, so two hard
//!   links add up to twice the length.
//! * **Unique apparent** counts that length once per inode, and once per APFS
//!   clone id when the volume reports one.
//! * **Unique allocated** is `st_blocks * 512`, with the same sharing rules,
//!   plus each directory's own allocated size.
//!
//! Symbolic links are leaves unless [`ScanOptions::follow_symlinks`] is set.
//! Their targets are not added to parent totals. Unreadable directories are
//! kept in the tree with [`ScanNode::error`] set and do not abort the scan.
//!
//! # Cancellation
//!
//! Pass a shared [`std::sync::atomic::AtomicBool`]. The walk checks it between
//! entries and returns [`ScanError::Cancelled`] without a partial tree.

#![cfg_attr(not(unix), allow(unused))]

#[cfg(not(unix))]
compile_error!("diskscan supports Unix only (macOS and Linux)");

mod category;
mod error;
mod format;
mod identity;
mod layout;
mod model;
mod platform;
mod progress;
mod query;
mod scan;
mod volume;

pub use category::{categorize, FileCategory};
pub use error::ScanError;
pub use format::{format_bytes, format_tree};
pub use identity::{identity_key, IdentityKey};
pub use layout::{sunburst, treemap, SunburstArc, TreemapRect};
pub use model::{
    CategoryTotal, FileEntry, FileQuery, FileSort, NodeKind, ScanIssue, ScanNode, ScanTree,
    SizeMetric,
};
pub use progress::{ProgressReporter, ProgressUpdate, ScanPhase};
pub use query::{query_files, summarize_categories};
pub use scan::{scan, ScanOptions};
pub use volume::{filesystem_usage, FilesystemUsage};
