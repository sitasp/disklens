//! Cooperative progress reporting.
//!
//! The scanner invokes the reporter on the thread that is consuming walk
//! results, at the start of the scan, about every 64 entries, and again when
//! the phase changes. Implementations must be fast and must not re-enter the
//! scanner. A UI should hop to its main thread before touching widgets.

/// What a long-running job is doing.
///
/// Walking and finalizing belong to the directory scan. Grouping and the two
/// hashing phases belong to duplicate detection. One enum lets the UI bind a
/// single progress view to either job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanPhase {
    /// Reading directory entries and file metadata.
    Walking,
    /// Folding the flat walk into a size tree.
    Finalizing,
    /// Grouping candidate files by size.
    Grouping,
    /// Hashing the first and last blocks of each candidate.
    HashingPartial,
    /// Hashing entire candidate files.
    HashingFull,
    /// The job finished and the result is about to be returned.
    Done,
}

/// A point-in-time snapshot safe to copy across threads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressUpdate {
    pub phase: ScanPhase,
    /// Directory entries visited so far, including directories and symlinks.
    pub files_seen: u64,
    /// Sum of apparent byte lengths seen so far. Hard links are included once
    /// per path; this is not the unique total.
    pub bytes_seen: u64,
    /// Path most recently visited. Empty before the first entry.
    pub current_path: String,
    /// Duplicate groups confirmed so far. Zero during a plain scan.
    pub groups_found: u64,
}

/// Sink for [`ProgressUpdate`] values.
///
/// Implemented automatically for `Fn(&ProgressUpdate) + Send + Sync`, so tests
/// and the CLI can pass a closure.
pub trait ProgressReporter: Send + Sync {
    fn report(&self, update: &ProgressUpdate);
}

impl<F> ProgressReporter for F
where
    F: Fn(&ProgressUpdate) + Send + Sync,
{
    fn report(&self, update: &ProgressUpdate) {
        self(update);
    }
}
