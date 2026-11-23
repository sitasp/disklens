//! Free and used space for a mounted filesystem.
//!
//! These numbers come from `statvfs` and include space the scanner cannot see
//! (snapshots, purgeable files, other users). The UI should show them beside
//! the scanned total rather than expecting the two to match.

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::error::ScanError;

/// Capacity of the filesystem that contains `path`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemUsage {
    /// Total size of the filesystem, in bytes.
    pub total_bytes: u64,
    /// Bytes available to the current user, honouring quotas and reserved blocks.
    pub available_bytes: u64,
    /// Bytes not allocated to files, including space reserved for root.
    pub free_bytes: u64,
    /// `total_bytes - free_bytes`. This is the figure a volume dashboard should graph.
    pub used_bytes: u64,
    /// Fragment size used to convert block counts (`f_frsize`).
    pub block_size: u64,
}

/// Read filesystem capacity for the volume that holds `path`.
///
/// # Errors
///
/// Returns [`ScanError::Io`] when the path cannot be encoded or `statvfs` fails.
pub fn filesystem_usage(path: &Path) -> Result<FilesystemUsage, ScanError> {
    let bytes = path.as_os_str().as_bytes();
    let c_path = CString::new(bytes).map_err(|_| ScanError::Io {
        path: path.display().to_string(),
        message: "path contains an interior NUL byte".to_owned(),
    })?;
    let mut stat = unsafe { std::mem::zeroed::<libc::statvfs>() };
    // SAFETY: `c_path` is NUL-terminated and `stat` is a valid out-buffer that
    // we only read after a successful return.
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if rc != 0 {
        return Err(ScanError::io(path, &io::Error::last_os_error()));
    }
    let block = nonzero_u64(stat.f_frsize).or_else(|| nonzero_u64(stat.f_bsize)).unwrap_or(512);
    let total = block.saturating_mul(count_u64(stat.f_blocks));
    let free = block.saturating_mul(count_u64(stat.f_bfree));
    let available = block.saturating_mul(count_u64(stat.f_bavail));
    Ok(FilesystemUsage {
        total_bytes: total,
        available_bytes: available,
        free_bytes: free,
        used_bytes: total.saturating_sub(free),
        block_size: block,
    })
}

fn nonzero_u64(value: impl TryInto<u64>) -> Option<u64> {
    match value.try_into() {
        Ok(converted) if converted > 0 => Some(converted),
        _ => None,
    }
}

fn count_u64(value: impl TryInto<u64>) -> u64 {
    value.try_into().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_directory_reports_a_volume() {
        let usage = filesystem_usage(Path::new(".")).expect("statvfs");
        assert!(usage.total_bytes > 0, "total should be non-zero");
        assert!(usage.block_size > 0);
        assert!(usage.used_bytes <= usage.total_bytes);
        assert!(usage.available_bytes <= usage.total_bytes);
    }
}
