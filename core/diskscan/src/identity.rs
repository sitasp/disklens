//! Identity of a file's bytes on disk.
//!
//! Hard links share a device and inode. APFS clones have different inodes but
//! the same clone id (see `getattrlist` `ATTR_CMNEXT_CLONEID` in xnu
//! `bsd/sys/attr.h`). Two paths with the same [`IdentityKey`] occupy one
//! allocation; deleting one path does not free the space while another
//! remains.

/// Key used to count allocated bytes once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdentityKey {
    /// APFS clone group. `clone_id` is never zero; zero means "no clone id".
    Clone { device: u64, clone_id: u64 },
    /// Classic inode identity, including hard links.
    Inode { device: u64, inode: u64 },
}

/// Prefer a non-zero clone id over the inode so APFS clones are not counted twice.
///
/// A clone id of `Some(0)` is treated as missing. Apple documents a clone id
/// as the identity of a data stream; zero is not a real stream id in practice
/// and would otherwise collapse unrelated files.
#[must_use]
pub fn identity_key(device: u64, inode: u64, clone_id: Option<u64>) -> IdentityKey {
    match clone_id {
        Some(clone_id) if clone_id != 0 => IdentityKey::Clone { device, clone_id },
        _ => IdentityKey::Inode { device, inode },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_id_wins_over_inode() {
        let key = identity_key(1, 99, Some(7));
        assert_eq!(key, IdentityKey::Clone { device: 1, clone_id: 7 });
    }

    #[test]
    fn zero_clone_id_falls_back_to_inode() {
        assert_eq!(identity_key(4, 8, Some(0)), IdentityKey::Inode { device: 4, inode: 8 });
        assert_eq!(identity_key(4, 8, None), IdentityKey::Inode { device: 4, inode: 8 });
    }

    #[test]
    fn different_devices_do_not_collide() {
        assert_ne!(identity_key(1, 5, None), identity_key(2, 5, None));
    }
}
