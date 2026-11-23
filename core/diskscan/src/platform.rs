//! Operating-system details that `std` does not expose.
//!
//! Hard-link identity comes from `std::fs::Metadata` (`st_dev` / `st_ino`) and
//! needs no special code. APFS clone identity is macOS-only.

use std::path::Path;

/// Return the APFS clone id for `path`, if the platform and volume provide one.
///
/// Always returns `None` off macOS. A return of `None` on macOS means the call
/// failed or the file has no clone id; the scanner then deduplicates by inode
/// only. Failures are intentionally quiet: clone detection is an optimisation
/// for unique-size accounting, not a reason to abort a scan.
#[must_use]
pub fn clone_id(path: &Path) -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        macos::clone_id(path)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        None
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    // Values from xnu `bsd/sys/attr.h` (apple-oss-distributions, 2023-era
    // header that still matches the public getattrlist(2) contract).
    const ATTR_BIT_MAP_COUNT: u16 = 5;
    /// `ATTR_CMNEXT_CLONEID` — stored in `forkattr` when
    /// `FSOPT_ATTR_CMN_EXTENDED` is set.
    const ATTR_CMNEXT_CLONEID: u32 = 0x0000_0100;
    const FSOPT_NOFOLLOW: u64 = 0x0000_0001;
    const FSOPT_ATTR_CMN_EXTENDED: u64 = 0x0000_0020;

    #[repr(C)]
    struct AttrList {
        bitmapcount: u16,
        reserved: u16,
        commonattr: u32,
        volattr: u32,
        dirattr: u32,
        fileattr: u32,
        forkattr: u32,
    }

    extern "C" {
        /// `options` is `unsigned long` in the man page. DiskLens targets
        /// 64-bit Darwin, where that is `u64`.
        fn getattrlist(
            path: *const libc::c_char,
            attr_list: *mut AttrList,
            attr_buf: *mut libc::c_void,
            attr_buf_size: libc::size_t,
            options: u64,
        ) -> libc::c_int;
    }

    pub(super) fn clone_id(path: &Path) -> Option<u64> {
        let bytes = path.as_os_str().as_bytes();
        let c_path = CString::new(bytes).ok()?;
        let mut list = AttrList {
            bitmapcount: ATTR_BIT_MAP_COUNT,
            reserved: 0,
            commonattr: 0,
            volattr: 0,
            dirattr: 0,
            fileattr: 0,
            forkattr: ATTR_CMNEXT_CLONEID,
        };
        // Room for a length prefix plus a naturally aligned u64, with a few
        // spare bytes so a packed layout still fits.
        let mut buf = [0u8; 24];
        // SAFETY: `list` and `buf` are valid for the duration of the call,
        // `c_path` is a NUL-terminated path, and `buf` is larger than either
        // documented layout (packed 12 bytes or aligned 16 bytes).
        let rc = unsafe {
            getattrlist(
                c_path.as_ptr(),
                &mut list,
                buf.as_mut_ptr().cast(),
                buf.len(),
                FSOPT_NOFOLLOW | FSOPT_ATTR_CMN_EXTENDED,
            )
        };
        if rc != 0 {
            return None;
        }
        let length = u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]);
        // getattrlist aligns each attribute to its natural alignment, so a
        // u64 clone id sits at offset 8 and the returned length is 16. Some
        // callers observe a packed length of 12 (u64 at offset 4). Accept
        // both; anything shorter is not a clone id.
        let id = if length >= 16 {
            u64::from_ne_bytes(buf[8..16].try_into().ok()?)
        } else if length >= 12 {
            u64::from_ne_bytes(buf[4..12].try_into().ok()?)
        } else {
            return None;
        };
        if id == 0 {
            None
        } else {
            Some(id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_id_is_none_without_apfs() {
        // On Linux this is a hard guarantee. On macOS the temp directory is
        // often APFS and may return a real id, so only assert the contract
        // that the function does not error.
        let id = clone_id(Path::new("."));
        #[cfg(not(target_os = "macos"))]
        assert!(id.is_none());
        #[cfg(target_os = "macos")]
        let _ = id;
    }
}
