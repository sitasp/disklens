//! Failures that abort a scan.
//!
//! Unreadable individual files and directories are recorded on the tree as
//! [`crate::ScanIssue`] values and do not fail the whole scan. These errors
//! are reserved for conditions that mean there is no usable result.

use std::path::Path;

/// Error returned when a scan cannot produce a tree.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum ScanError {
    /// The caller cancelled the scan before it finished.
    #[error("scan cancelled")]
    Cancelled,

    /// `path` does not exist or is a broken symlink.
    #[error("path not found: {path}")]
    NotFound { path: String },

    /// The root path could not be read at all.
    #[error("cannot read {path}: {message}")]
    Io { path: String, message: String },
}

impl ScanError {
    pub(crate) fn not_found(path: &Path) -> Self {
        Self::NotFound { path: path.display().to_string() }
    }

    pub(crate) fn io(path: &Path, err: &std::io::Error) -> Self {
        Self::Io { path: path.display().to_string(), message: err.to_string() }
    }
}
