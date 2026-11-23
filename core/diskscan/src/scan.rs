//! Parallel directory walk and size tree.
//!
//! `jwalk` reads directories on a private rayon pool. The consuming thread
//! checks the cancellation flag between entries, so a click on Stop is
//! observed without waiting for the whole disk. Individual unreadable
//! directories are recorded on the node and in [`ScanTree::issues`]; they do
//! not fail the scan.

use std::collections::{BTreeMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, UNIX_EPOCH};

use jwalk::{Parallelism, WalkDir};

use crate::category::{categorize, FileCategory};
use crate::error::ScanError;
use crate::identity::{identity_key, IdentityKey};
use crate::model::{NodeKind, ScanIssue, ScanNode, ScanTree};
use crate::platform::clone_id;
use crate::progress::{ProgressReporter, ProgressUpdate, ScanPhase};

const PROGRESS_EVERY: u64 = 64;
const PROGRESS_INTERVAL_MS: u128 = 200;

/// Knobs for [`scan`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOptions {
    /// Descend through symbolic links. Off by default: a link is reported as
    /// a symlink leaf, cycles are impossible, and a linked tree is not counted
    /// a second time. When on, cycles become errors on the offending entry
    /// and hard-link / inode accounting still prevents double-counted unique bytes.
    pub follow_symlinks: bool,
    /// Include names that start with `.`. On by default; disk usage that skips
    /// dotfiles silently under-reports a home directory.
    pub include_hidden: bool,
    /// Ask APFS for clone ids so cloned files share one unique size.
    /// This is a no-op on Linux and on volumes that do not report clone ids.
    /// The extra `getattrlist` per file is the reason to turn it off for a
    /// faster, slightly less accurate scan.
    pub detect_clones: bool,
    /// Do not descend into directories whose device id differs from the root.
    /// Use this when the user scans a volume rather than a folder that is
    /// allowed to span mounts.
    pub stay_on_filesystem: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            follow_symlinks: false,
            include_hidden: true,
            detect_clones: true,
            stay_on_filesystem: false,
        }
    }
}

/// Walk `path` and return an owned size tree.
///
/// `cancel` may be shared with another thread; store `true` to stop the walk.
/// The function returns [`ScanError::Cancelled`] and no partial tree. Progress
/// callbacks run on the calling thread.
///
/// # Errors
///
/// * [`ScanError::NotFound`] if `path` does not exist or is a broken symlink.
/// * [`ScanError::Cancelled`] if `cancel` is set before or during the walk.
/// * [`ScanError::Io`] if the root itself cannot be stated.
pub fn scan(
    path: impl AsRef<Path>,
    options: &ScanOptions,
    cancel: &AtomicBool,
    progress: Option<&dyn ProgressReporter>,
) -> Result<ScanTree, ScanError> {
    let started = Instant::now();
    if cancel.load(Ordering::Relaxed) {
        return Err(ScanError::Cancelled);
    }
    let root = normalize_root(path.as_ref())?;
    let root_meta = std::fs::symlink_metadata(&root).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            ScanError::not_found(&root)
        } else {
            ScanError::io(&root, &err)
        }
    })?;
    if root_meta.file_type().is_symlink() && std::fs::metadata(&root).is_err() {
        return Err(ScanError::not_found(&root));
    }
    let root_device = root_meta.dev();

    report(progress, ScanPhase::Walking, 0, 0, &root, 0);

    let entries = walk(&root, options, root_device, cancel, progress)?;
    if cancel.load(Ordering::Relaxed) {
        return Err(ScanError::Cancelled);
    }
    report(progress, ScanPhase::Finalizing, entries.seen, entries.bytes, &root, 0);
    let tree = build_tree(&root, root_device, entries, started.elapsed().as_millis());
    report(progress, ScanPhase::Done, tree.entries_seen, tree.root.logical_apparent, &root, 0);
    Ok(tree)
}

struct WalkOutput {
    raw: Vec<RawEntry>,
    issues: Vec<ScanIssue>,
    seen: u64,
    bytes: u64,
}

struct RawEntry {
    path: PathBuf,
    kind: NodeKind,
    own_apparent: u64,
    own_allocated: u64,
    modified_unix_ms: Option<i64>,
    inode: Option<u64>,
    device: Option<u64>,
    clone_id: Option<u64>,
    error: Option<String>,
    other_volume: bool,
}

fn walk(
    root: &Path,
    options: &ScanOptions,
    root_device: u64,
    cancel: &AtomicBool,
    progress: Option<&dyn ProgressReporter>,
) -> Result<WalkOutput, ScanError> {
    let threads = std::thread::available_parallelism().map_or(2, |count| count.get().max(1));
    let include_hidden = options.include_hidden;
    let stay = options.stay_on_filesystem;
    // Cancellation is observed on the consuming thread (below) rather than
    // inside this callback. `process_read_dir` has to be `'static`, and the
    // flag is borrowed from the caller for the duration of `scan` only.
    // Dropping the iterator on cancel stops jwalk's pool promptly.
    let walker = WalkDir::new(root)
        .skip_hidden(false)
        .follow_links(options.follow_symlinks)
        .parallelism(Parallelism::RayonNewPool(threads))
        .process_read_dir(move |_depth, _path, _state, children| {
            if !include_hidden {
                children.retain(|child| match child {
                    Ok(entry) => entry.depth == 0 || !is_hidden_name(entry.file_name()),
                    Err(_) => true,
                });
            }
            if stay {
                for child in children.iter_mut() {
                    let Ok(entry) = child else { continue };
                    if entry.depth == 0 || !entry.file_type().is_dir() {
                        continue;
                    }
                    if let Ok(meta) = entry.metadata() {
                        if meta.dev() != root_device {
                            entry.read_children_path = None;
                        }
                    }
                }
            }
        });

    let mut raw = Vec::new();
    let mut issues = Vec::new();
    let mut seen = 0u64;
    let mut bytes = 0u64;
    let mut last_report = Instant::now();

    for item in walker {
        if cancel.load(Ordering::Relaxed) {
            return Err(ScanError::Cancelled);
        }
        match item {
            Ok(entry) => {
                seen += 1;
                let path = entry.path();
                match raw_from_entry(&entry, options, root_device) {
                    Ok(record) => {
                        bytes = bytes.saturating_add(record.own_apparent);
                        if let Some(message) = record.error.clone() {
                            issues.push(ScanIssue {
                                path: record.path.display().to_string(),
                                message,
                            });
                        }
                        let report_path = record.path.clone();
                        raw.push(record);
                        let due = seen == 1
                            || seen % PROGRESS_EVERY == 0
                            || last_report.elapsed().as_millis() >= PROGRESS_INTERVAL_MS;
                        if due {
                            report(progress, ScanPhase::Walking, seen, bytes, &report_path, 0);
                            last_report = Instant::now();
                        }
                    }
                    Err(message) => {
                        issues.push(ScanIssue { path: path.display().to_string(), message })
                    }
                }
            }
            Err(err) => {
                let path =
                    err.path().map_or_else(String::new, |failed| failed.display().to_string());
                issues.push(ScanIssue { path, message: err.to_string() });
            }
        }
    }

    Ok(WalkOutput { raw, issues, seen, bytes })
}

fn raw_from_entry(
    entry: &jwalk::DirEntry<((), ())>,
    options: &ScanOptions,
    root_device: u64,
) -> Result<RawEntry, String> {
    let path = entry.path();
    let kind = if entry.path_is_symlink() && !options.follow_symlinks {
        NodeKind::Symlink
    } else if entry.file_type().is_dir() {
        NodeKind::Directory
    } else if entry.file_type().is_file() {
        NodeKind::File
    } else if entry.file_type().is_symlink() {
        NodeKind::Symlink
    } else {
        NodeKind::Other
    };
    let meta = entry.metadata().map_err(|err| err.to_string())?;
    let apparent = if kind == NodeKind::File { meta.len() } else { 0 };
    let allocated = if kind == NodeKind::File || kind == NodeKind::Directory {
        meta.blocks().saturating_mul(512)
    } else {
        0
    };
    let inode = Some(meta.ino());
    let device = Some(meta.dev());
    let clone =
        if options.detect_clones && kind == NodeKind::File { clone_id(&path) } else { None };
    let error = entry.read_children_error.as_ref().map(ToString::to_string);
    let other_volume = device.is_some_and(|dev| dev != root_device);
    Ok(RawEntry {
        path,
        kind,
        own_apparent: apparent,
        own_allocated: allocated,
        modified_unix_ms: modified_ms(&meta),
        inode,
        device,
        clone_id: clone,
        error,
        other_volume,
    })
}

fn modified_ms(meta: &std::fs::Metadata) -> Option<i64> {
    let modified = meta.modified().ok()?;
    let duration = modified.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_millis()).ok()
}

fn build_tree(
    root_path: &Path,
    root_device: u64,
    entries: WalkOutput,
    elapsed_ms: u128,
) -> ScanTree {
    let mut partial = Partial::default();
    for raw in entries.raw {
        insert(&mut partial, root_path, raw);
    }
    let mut root = partial.into_node(root_path, fallback_name(root_path));
    // A scan of an empty but readable directory still needs a root node when
    // jwalk yielded nothing (should not happen) or only errors.
    if root.path.as_os_str().is_empty() {
        root.path = root_path.to_path_buf();
        root.name = fallback_name(root_path);
        root.kind = NodeKind::Directory;
    }
    let mut seen = HashSet::new();
    account(&mut root, &mut seen);
    ScanTree {
        root_path: root_path.to_path_buf(),
        entries_seen: entries.seen,
        elapsed_ms: u64::try_from(elapsed_ms).unwrap_or(u64::MAX),
        root_device: Some(root_device),
        issues: entries.issues,
        root,
    }
}

#[derive(Default)]
struct Partial {
    raw: Option<RawEntry>,
    children: BTreeMap<OsString, Partial>,
}

fn insert(node: &mut Partial, root: &Path, raw: RawEntry) {
    let relative = match raw.path.strip_prefix(root) {
        Ok(relative) => relative.to_path_buf(),
        Err(_) => return,
    };
    if relative.as_os_str().is_empty() {
        node.raw = Some(raw);
        return;
    }
    let mut cursor = node;
    let components: Vec<Component> = relative.components().collect();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            continue;
        };
        let is_last = index + 1 == components.len();
        cursor = cursor.children.entry((*name).to_os_string()).or_default();
        if is_last {
            cursor.raw = Some(raw);
            return;
        }
    }
}

impl Partial {
    fn into_node(self, path: &Path, name: String) -> ScanNode {
        let children: Vec<ScanNode> = self
            .children
            .into_iter()
            .map(|(child_name, child)| {
                let child_path = path.join(&child_name);
                let display = child_name.to_string_lossy().into_owned();
                child.into_node(&child_path, display)
            })
            .collect();
        let Some(raw) = self.raw else {
            return placeholder(name, path.to_path_buf(), children);
        };
        ScanNode {
            name,
            path: raw.path,
            kind: raw.kind,
            category: FileCategory::Other,
            own_apparent: raw.own_apparent,
            own_allocated: raw.own_allocated,
            logical_apparent: 0,
            unique_apparent: 0,
            unique_allocated: 0,
            file_count: 0,
            modified_unix_ms: raw.modified_unix_ms,
            inode: raw.inode,
            device: raw.device,
            clone_id: raw.clone_id,
            shared_elsewhere: false,
            other_volume: raw.other_volume,
            error: raw.error,
            children,
        }
    }
}

fn placeholder(name: String, path: PathBuf, children: Vec<ScanNode>) -> ScanNode {
    ScanNode {
        name,
        path,
        kind: NodeKind::Directory,
        category: FileCategory::Other,
        own_apparent: 0,
        own_allocated: 0,
        logical_apparent: 0,
        unique_apparent: 0,
        unique_allocated: 0,
        file_count: 0,
        modified_unix_ms: None,
        inode: None,
        device: None,
        clone_id: None,
        shared_elsewhere: false,
        other_volume: false,
        error: None,
        children,
    }
}

/// Assign shared-byte ownership in lexicographic path order, then roll sizes up.
///
/// Children are still in `BTreeMap` order (byte-wise file name) when this runs,
/// which makes the owning path deterministic. They are re-sorted by size at the
/// end of each directory so the UI has a stable default.
fn account(node: &mut ScanNode, seen: &mut HashSet<IdentityKey>) {
    if let Some(key) = node_identity(node) {
        if !seen.insert(key) {
            node.shared_elsewhere = true;
        }
    }
    for child in &mut node.children {
        account(child, seen);
    }

    let mut logical = 0u64;
    let mut unique_apparent = 0u64;
    let mut unique_allocated = 0u64;
    let mut file_count = 0u64;
    let mut category_bytes: Vec<(FileCategory, u64)> = Vec::new();
    for child in &node.children {
        logical = logical.saturating_add(child.logical_apparent);
        unique_apparent = unique_apparent.saturating_add(child.unique_apparent);
        unique_allocated = unique_allocated.saturating_add(child.unique_allocated);
        file_count = file_count.saturating_add(child.file_count);
        if child.unique_apparent > 0 {
            category_bytes.push((child.category, child.unique_apparent));
        }
    }

    match node.kind {
        NodeKind::File => {
            node.logical_apparent = node.own_apparent;
            if node.shared_elsewhere {
                node.unique_apparent = 0;
                node.unique_allocated = 0;
            } else {
                node.unique_apparent = node.own_apparent;
                node.unique_allocated = node.own_allocated;
            }
            node.file_count = 1;
            node.category = categorize(&node.path);
        }
        NodeKind::Directory => {
            node.logical_apparent = logical;
            node.unique_apparent = unique_apparent;
            let own = if node.shared_elsewhere { 0 } else { node.own_allocated };
            node.unique_allocated = unique_allocated.saturating_add(own);
            node.file_count = file_count;
            node.category = dominant_category(&category_bytes);
        }
        NodeKind::Symlink | NodeKind::Other => {
            node.logical_apparent = 0;
            node.unique_apparent = 0;
            node.unique_allocated = 0;
            node.file_count = 0;
            node.category = categorize(&node.path);
        }
    }

    node.children.sort_by(|left, right| {
        right.unique_apparent.cmp(&left.unique_apparent).then_with(|| left.name.cmp(&right.name))
    });
}

fn node_identity(node: &ScanNode) -> Option<IdentityKey> {
    if !matches!(node.kind, NodeKind::File | NodeKind::Directory) {
        return None;
    }
    Some(identity_key(node.device?, node.inode?, node.clone_id))
}

fn dominant_category(parts: &[(FileCategory, u64)]) -> FileCategory {
    let mut best: Option<(FileCategory, u64)> = None;
    for (category, bytes) in parts {
        match best {
            None => best = Some((*category, *bytes)),
            Some((best_category, best_bytes)) => {
                if *bytes > best_bytes || (*bytes == best_bytes && *category < best_category) {
                    best = Some((*category, *bytes));
                }
            }
        }
    }
    best.map_or(FileCategory::Other, |(category, _)| category)
}

fn normalize_root(path: &Path) -> Result<PathBuf, ScanError> {
    if path.as_os_str().is_empty() {
        return Err(ScanError::not_found(path));
    }
    let absolute = if path.is_absolute() {
        lexical_normalize(path)
    } else {
        let cwd = std::env::current_dir().map_err(|err| ScanError::io(path, &err))?;
        lexical_normalize(&cwd.join(path))
    };
    match std::fs::symlink_metadata(&absolute) {
        Ok(_) => Ok(absolute),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Err(ScanError::not_found(&absolute))
        }
        Err(err) => Err(ScanError::io(&absolute, &err)),
    }
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::Normal(part) => out.push(part),
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(Component::RootDir.as_os_str())
    } else {
        out
    }
}

fn fallback_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
}

fn is_hidden_name(name: &OsStr) -> bool {
    name.to_str().is_some_and(|text| text.starts_with('.') && text != "." && text != "..")
}

fn report(
    progress: Option<&dyn ProgressReporter>,
    phase: ScanPhase,
    files_seen: u64,
    bytes_seen: u64,
    path: &Path,
    groups_found: u64,
) {
    if let Some(progress) = progress {
        progress.report(&ProgressUpdate {
            phase,
            files_seen,
            bytes_seen,
            current_path: path.display().to_string(),
            groups_found,
        });
    }
}
