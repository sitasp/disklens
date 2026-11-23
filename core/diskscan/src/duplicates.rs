//! Staged duplicate detection.
//!
//! 1. Group regular files by size and drop groups that are a single inode
//!    or a single APFS clone id. Hard links of one file are not duplicates.
//! 2. Hash the first and last `partial_bytes` of each remaining file and
//!    drop groups whose partial hashes differ.
//! 3. Hash the whole file with BLAKE3 and keep groups that still match.
//!
//! Files no larger than the partial window skip step 2, because that read
//! would already be the whole file. Empty files are excluded unless
//! `min_size` is 0. Unreadable files are reported and skipped.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::error::ScanError;
use crate::identity::{identity_key, IdentityKey};
use crate::model::ScanNode;
use crate::progress::{ProgressReporter, ProgressUpdate, ScanPhase};
use crate::scan::{scan, ScanOptions};

const DOMAIN_PARTIAL: &[u8] = b"disklens-partial-v1";
const DOMAIN_FULL: &[u8] = b"disklens-full-v1";

/// Options for [`find_duplicates`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateOptions {
    /// Files smaller than this are ignored. `1` drops empty files, which
    /// otherwise form one giant useless group.
    pub min_size: u64,
    /// How many bytes to read from each end before the full hash.
    /// `0` skips the partial stage.
    pub partial_bytes: u64,
    pub follow_symlinks: bool,
    pub include_hidden: bool,
}

impl Default for DuplicateOptions {
    fn default() -> Self {
        Self { min_size: 1, partial_bytes: 4096, follow_symlinks: false, include_hidden: true }
    }
}

/// A file that survived size grouping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFile {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub modified_unix_ms: Option<i64>,
    pub inode: u64,
    pub device: u64,
    pub clone_id: Option<u64>,
    pub nlink: u64,
}

impl DuplicateFile {
    #[must_use]
    pub fn identity(&self) -> IdentityKey {
        identity_key(self.device, self.inode, self.clone_id)
    }
}

/// Files with the same size and the same BLAKE3 hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub size: u64,
    /// Lower-case hex of the full-content BLAKE3 hash.
    pub hash_hex: String,
    /// Sorted by path so the order does not depend on the walk.
    pub files: Vec<DuplicateFile>,
}

impl DuplicateGroup {
    /// Bytes that would be freed by deleting every identity except the first
    /// file's. This is the "keep one copy" estimate shown before a smart
    /// selection runs.
    #[must_use]
    pub fn wasted_bytes(&self) -> u64 {
        let Some(keeper) = self.files.first() else { return 0 };
        let keeper_id = keeper.identity();
        let selected: Vec<bool> =
            self.files.iter().map(|file| file.identity() != keeper_id).collect();
        reclaimable_bytes(&self.files, &selected)
    }

    #[must_use]
    pub fn distinct_identities(&self) -> usize {
        distinct_identities(&self.files)
    }
}

/// Confirmed groups plus files that could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateReport {
    pub groups: Vec<DuplicateGroup>,
    pub unreadable: Vec<PathBuf>,
    pub files_considered: u64,
    pub files_hashed: u64,
}

/// Scan `root` and return confirmed duplicate groups, largest waste first.
///
/// # Errors
///
/// Returns the scan error, or [`ScanError::Cancelled`] if `cancel` is set.
pub fn find_duplicates(
    root: impl AsRef<Path>,
    options: &DuplicateOptions,
    cancel: &AtomicBool,
    progress: Option<&dyn ProgressReporter>,
) -> Result<DuplicateReport, ScanError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(ScanError::Cancelled);
    }
    let scan_options = ScanOptions {
        follow_symlinks: options.follow_symlinks,
        include_hidden: options.include_hidden,
        detect_clones: true,
        stay_on_filesystem: false,
    };
    let tree = scan(root.as_ref(), &scan_options, cancel, progress)?;
    find_duplicates_in_node(&tree.root, options, cancel, progress)
}

/// Hash candidates already collected under `root`. Used by the app when a
/// scan is already on screen, and by tests that want to skip a second walk.
///
/// # Errors
///
/// Returns [`ScanError::Cancelled`] when the flag is set between groups.
pub fn find_duplicates_in_node(
    root: &ScanNode,
    options: &DuplicateOptions,
    cancel: &AtomicBool,
    progress: Option<&dyn ProgressReporter>,
) -> Result<DuplicateReport, ScanError> {
    let files = collect_candidates(root, options.min_size);
    let files_considered = files.len() as u64;
    report(progress, ScanPhase::Grouping, files_considered, 0, 0);
    let mut size_groups = group_by_size(files);
    size_groups.retain(|group| distinct_identities(group) >= 2);

    let mut unreadable = Vec::new();
    let mut files_hashed = 0u64;
    let mut full_candidates: Vec<Vec<DuplicateFile>> = Vec::new();

    for group in size_groups {
        if cancel.load(Ordering::Relaxed) {
            return Err(ScanError::Cancelled);
        }
        let size = group[0].size;
        if options.partial_bytes == 0 || size <= options.partial_bytes {
            full_candidates.push(group);
            continue;
        }
        report(progress, ScanPhase::HashingPartial, files_considered, files_hashed, 0);
        let (hashed, failed) = hash_group(&group, cancel, |file| {
            partial_hash(&file.path, file.size, options.partial_bytes)
        })?;
        files_hashed += hashed.len() as u64;
        unreadable.extend(failed);
        for (_, subgroup) in regroup(hashed) {
            if distinct_identities(&subgroup) >= 2 {
                full_candidates.push(subgroup);
            }
        }
    }

    let mut confirmed = Vec::new();
    for group in full_candidates {
        if cancel.load(Ordering::Relaxed) {
            return Err(ScanError::Cancelled);
        }
        report(
            progress,
            ScanPhase::HashingFull,
            files_considered,
            files_hashed,
            confirmed.len() as u64,
        );
        let (hashed, failed) = hash_group(&group, cancel, |file| full_hash(&file.path))?;
        files_hashed += hashed.len() as u64;
        unreadable.extend(failed);
        for (hash, mut subgroup) in regroup(hashed) {
            if distinct_identities(&subgroup) < 2 {
                continue;
            }
            subgroup.sort_by(|left, right| left.path.cmp(&right.path));
            confirmed.push(DuplicateGroup {
                size: subgroup[0].size,
                hash_hex: hash.to_hex().to_string(),
                files: subgroup,
            });
        }
    }

    confirmed.sort_by(|left, right| {
        right
            .wasted_bytes()
            .cmp(&left.wasted_bytes())
            .then_with(|| left.files[0].path.cmp(&right.files[0].path))
    });
    report(progress, ScanPhase::Done, files_considered, files_hashed, confirmed.len() as u64);
    Ok(DuplicateReport { groups: confirmed, unreadable, files_considered, files_hashed })
}

/// Bytes freed if every `selected[i]` file is deleted, counting each shared
/// identity once and only when no link would remain.
///
/// `nlink` is the filesystem's link count. Selecting fewer paths than `nlink`
/// leaves a name elsewhere, so the blocks stay. Clone groups use the same
/// rule across every inode that shares the clone id: the identity is freed
/// only when each of those inodes is fully unlinked.
#[must_use]
pub fn reclaimable_bytes(files: &[DuplicateFile], selected: &[bool]) -> u64 {
    if files.len() != selected.len() {
        return 0;
    }
    let mut by_identity: HashMap<IdentityKey, Vec<usize>> = HashMap::new();
    for (index, file) in files.iter().enumerate() {
        by_identity.entry(file.identity()).or_default().push(index);
    }
    let mut total = 0u64;
    for indexes in by_identity.values() {
        if !indexes.iter().all(|index| selected[*index]) {
            continue;
        }
        let mut by_inode: HashMap<(u64, u64), (u64, u64)> = HashMap::new();
        for index in indexes {
            let file = &files[*index];
            let entry = by_inode.entry((file.device, file.inode)).or_insert((file.nlink.max(1), 0));
            entry.1 += 1;
        }
        let fully_unlinked = by_inode.values().all(|(nlink, count)| *count >= *nlink);
        if fully_unlinked {
            total = total.saturating_add(files[indexes[0]].size);
        }
    }
    total
}

fn collect_candidates(root: &ScanNode, min_size: u64) -> Vec<DuplicateFile> {
    let mut files = Vec::new();
    root.visit_files(&mut |node| {
        if node.own_apparent < min_size {
            return;
        }
        let (Some(device), Some(inode)) = (node.device, node.inode) else { return };
        files.push(DuplicateFile {
            path: node.path.clone(),
            name: node.name.clone(),
            size: node.own_apparent,
            modified_unix_ms: node.modified_unix_ms,
            inode,
            device,
            clone_id: node.clone_id,
            nlink: node.nlink.unwrap_or(1).max(1),
        });
    });
    files
}

fn group_by_size(files: Vec<DuplicateFile>) -> Vec<Vec<DuplicateFile>> {
    let mut map: HashMap<u64, Vec<DuplicateFile>> = HashMap::new();
    for file in files {
        map.entry(file.size).or_default().push(file);
    }
    map.into_values().collect()
}

fn distinct_identities(files: &[DuplicateFile]) -> usize {
    let mut seen = HashMap::<IdentityKey, ()>::new();
    for file in files {
        seen.insert(file.identity(), ());
    }
    seen.len()
}

fn regroup(hashed: Vec<HashedFile>) -> Vec<(blake3::Hash, Vec<DuplicateFile>)> {
    let mut map: HashMap<blake3::Hash, Vec<DuplicateFile>> = HashMap::new();
    for (file, hash) in hashed {
        map.entry(hash).or_default().push(file);
    }
    map.into_iter().collect()
}

type HashedFile = (DuplicateFile, blake3::Hash);

fn hash_group(
    files: &[DuplicateFile],
    cancel: &AtomicBool,
    hash_one: impl Fn(&DuplicateFile) -> std::io::Result<blake3::Hash> + Send + Sync,
) -> Result<(Vec<HashedFile>, Vec<PathBuf>), ScanError> {
    let results: Vec<Result<Option<(DuplicateFile, blake3::Hash)>, ScanError>> = files
        .par_iter()
        .map(|file| {
            if cancel.load(Ordering::Relaxed) {
                return Err(ScanError::Cancelled);
            }
            match hash_one(file) {
                Ok(hash) => Ok(Some((file.clone(), hash))),
                Err(_) => Ok(None),
            }
        })
        .collect();
    let mut hashed = Vec::new();
    let mut failed = Vec::new();
    for (file, result) in files.iter().zip(results) {
        match result {
            Err(ScanError::Cancelled) => return Err(ScanError::Cancelled),
            Err(other) => return Err(other),
            Ok(Some(pair)) => hashed.push(pair),
            Ok(None) => failed.push(file.path.clone()),
        }
    }
    Ok((hashed, failed))
}

fn partial_hash(path: &Path, size: u64, window: u64) -> std::io::Result<blake3::Hash> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(DOMAIN_PARTIAL);
    hasher.update(&size.to_le_bytes());
    let window = usize::try_from(window)
        .unwrap_or(usize::MAX)
        .min(usize::try_from(size).unwrap_or(usize::MAX));
    let head = read_exact_at(&mut file, 0, window)?;
    hasher.update(&head);
    let tail_at = size.saturating_sub(window as u64);
    let tail = read_exact_at(&mut file, tail_at, window)?;
    hasher.update(&tail);
    Ok(hasher.finalize())
}

fn full_hash(path: &Path) -> std::io::Result<blake3::Hash> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(DOMAIN_FULL);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize())
}

fn read_exact_at(file: &mut File, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(offset))?;
    let mut buffer = vec![0u8; len];
    file.read_exact(&mut buffer)?;
    Ok(buffer)
}

fn report(
    progress: Option<&dyn ProgressReporter>,
    phase: ScanPhase,
    files_seen: u64,
    files_hashed: u64,
    groups_found: u64,
) {
    if let Some(progress) = progress {
        progress.report(&ProgressUpdate {
            phase,
            files_seen,
            bytes_seen: files_hashed,
            current_path: String::new(),
            groups_found,
        });
    }
}
