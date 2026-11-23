//! UniFFI surface used by the SwiftUI app.
//!
//! The Rust core keeps [`std::path::PathBuf`]. This module converts to the
//! strings, records, and objects UniFFI can move across the FFI. Behaviour
//! stays in the core; nothing here decides what a duplicate is or which copy
//! to keep.

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::deletion::plan_deletion;
use crate::duplicates::{find_duplicates, DuplicateGroup, DuplicateReport};
use crate::format::format_bytes;
use crate::layout::{sunburst, treemap};
use crate::model::{FileQuery, FileSort, ScanNode, ScanTree, SizeMetric};
use crate::progress::{ProgressReporter, ProgressUpdate};
use crate::query::{query_files, summarize_categories};
use crate::scan::{scan, ScanOptions};
use crate::selection::{select_group, KeepStrategy, SelectionPolicy};
use crate::volume::filesystem_usage;
use crate::{FileCategory, NodeKind, ScanError};

/// Cooperative cancel flag safe to share with a Swift task.
#[derive(uniffi::Object)]
pub struct CancelFlag {
    inner: std::sync::atomic::AtomicBool,
}

#[uniffi::export]
impl CancelFlag {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self { inner: std::sync::atomic::AtomicBool::new(false) })
    }

    pub fn cancel(&self) {
        self.inner.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.load(Ordering::Relaxed)
    }
}

/// Implemented in Swift. Invoked on the thread that is running the scan,
/// between filesystem entries, so the method must return quickly and hop to
/// the main actor before touching UI state.
#[uniffi::export(foreign)]
pub trait ProgressSink: Send + Sync {
    fn on_progress(&self, update: ProgressUpdate);
}

struct ProgressBridge(Arc<dyn ProgressSink>);

impl ProgressReporter for ProgressBridge {
    fn report(&self, update: &ProgressUpdate) {
        self.0.on_progress(update.clone());
    }
}

/// An immutable scan the interface can query without copying the whole tree
/// on every click.
#[derive(uniffi::Object)]
pub struct ScanSession {
    tree: ScanTree,
}

#[derive(uniffi::Record)]
pub struct NodeSummary {
    pub name: String,
    pub path: String,
    pub kind: NodeKind,
    pub category: FileCategory,
    pub category_label: String,
    pub own_apparent: u64,
    pub own_allocated: u64,
    pub logical_apparent: u64,
    pub unique_apparent: u64,
    pub unique_allocated: u64,
    pub file_count: u64,
    pub child_count: u32,
    pub modified_unix_ms: Option<i64>,
    pub shared_elsewhere: bool,
    pub other_volume: bool,
    pub error: Option<String>,
}

#[derive(uniffi::Record)]
pub struct IssueRecord {
    pub path: String,
    pub message: String,
}

#[derive(uniffi::Record)]
pub struct FileQueryRecord {
    pub min_apparent_size: u64,
    pub modified_before_unix_ms: Option<i64>,
    pub modified_after_unix_ms: Option<i64>,
    pub category: Option<FileCategory>,
    pub limit: u32,
    pub sort: FileSort,
}

#[derive(uniffi::Record)]
pub struct FileRow {
    pub name: String,
    pub path: String,
    pub category: FileCategory,
    pub category_label: String,
    pub apparent_size: u64,
    pub allocated_size: u64,
    pub modified_unix_ms: Option<i64>,
    pub shared_elsewhere: bool,
    pub extension: String,
}

#[derive(uniffi::Record)]
pub struct CategoryRow {
    pub category: FileCategory,
    pub label: String,
    pub unique_apparent: u64,
    pub unique_allocated: u64,
    pub file_count: u64,
}

#[derive(uniffi::Record)]
pub struct ArcRecord {
    pub path: String,
    pub name: String,
    pub depth: u32,
    pub start_angle: f64,
    pub end_angle: f64,
    pub size: u64,
    pub fraction_of_parent: f64,
    pub category: FileCategory,
    pub kind: NodeKind,
    pub is_aggregate: bool,
}

#[derive(uniffi::Record)]
pub struct RectRecord {
    pub path: String,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub size: u64,
    pub depth: u32,
    pub category: FileCategory,
    pub kind: NodeKind,
}

#[uniffi::export]
impl ScanSession {
    pub fn root_path(&self) -> String {
        self.tree.root_path.display().to_string()
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.tree.elapsed_ms
    }

    pub fn entries_seen(&self) -> u64 {
        self.tree.entries_seen
    }

    pub fn issue_count(&self) -> u32 {
        self.tree.issues.len() as u32
    }

    pub fn root(&self) -> NodeSummary {
        summarize(&self.tree.root)
    }

    pub fn node(&self, path: String) -> Option<NodeSummary> {
        self.tree.node(PathBuf::from(path).as_path()).map(summarize)
    }

    pub fn children(&self, path: String) -> Vec<NodeSummary> {
        self.tree
            .node(PathBuf::from(path).as_path())
            .map_or_else(Vec::new, |node| node.children.iter().map(summarize).collect())
    }

    pub fn categories(&self) -> Vec<CategoryRow> {
        summarize_categories(&self.tree.root)
            .into_iter()
            .map(|row| CategoryRow {
                category: row.category,
                label: row.category.label().to_owned(),
                unique_apparent: row.unique_apparent,
                unique_allocated: row.unique_allocated,
                file_count: row.file_count,
            })
            .collect()
    }

    pub fn files(&self, query: FileQueryRecord) -> Vec<FileRow> {
        let mapped = FileQuery {
            min_apparent_size: query.min_apparent_size,
            modified_before_unix_ms: query.modified_before_unix_ms,
            modified_after_unix_ms: query.modified_after_unix_ms,
            category: query.category,
            limit: query.limit as usize,
            sort: query.sort,
        };
        query_files(&self.tree.root, &mapped)
            .into_iter()
            .map(|file| FileRow {
                name: file.name,
                path: file.path.display().to_string(),
                category: file.category,
                category_label: file.category.label().to_owned(),
                apparent_size: file.apparent_size,
                allocated_size: file.allocated_size,
                modified_unix_ms: file.modified_unix_ms,
                shared_elsewhere: file.shared_elsewhere,
                extension: file.extension,
            })
            .collect()
    }

    pub fn sunburst(
        &self,
        path: String,
        metric: SizeMetric,
        max_depth: u32,
        min_fraction: f64,
    ) -> Vec<ArcRecord> {
        let Some(node) = self.tree.node(PathBuf::from(path).as_path()) else { return Vec::new() };
        sunburst(node, metric, max_depth, min_fraction)
            .into_iter()
            .map(|arc| ArcRecord {
                path: arc.path,
                name: arc.name,
                depth: arc.depth,
                start_angle: arc.start_angle,
                end_angle: arc.end_angle,
                size: arc.size,
                fraction_of_parent: arc.fraction_of_parent,
                category: arc.category,
                kind: arc.kind,
                is_aggregate: arc.is_aggregate,
            })
            .collect()
    }

    pub fn treemap(
        &self,
        path: String,
        metric: SizeMetric,
        width: f64,
        height: f64,
        max_depth: u32,
        padding: f64,
    ) -> Vec<RectRecord> {
        let Some(node) = self.tree.node(PathBuf::from(path).as_path()) else { return Vec::new() };
        treemap(node, metric, width, height, max_depth, padding)
            .into_iter()
            .map(|rect| RectRecord {
                path: rect.path,
                name: rect.name,
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
                size: rect.size,
                depth: rect.depth,
                category: rect.category,
                kind: rect.kind,
            })
            .collect()
    }

    pub fn issues(&self) -> Vec<IssueRecord> {
        self.tree
            .issues
            .iter()
            .map(|issue| IssueRecord { path: issue.path.clone(), message: issue.message.clone() })
            .collect()
    }
}

#[derive(uniffi::Record)]
pub struct DuplicateFileRecord {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub modified_unix_ms: Option<i64>,
    pub inode: u64,
    pub device: u64,
    pub nlink: u64,
}

#[derive(uniffi::Record)]
pub struct DuplicateGroupRecord {
    pub size: u64,
    pub hash_hex: String,
    pub wasted_bytes: u64,
    pub files: Vec<DuplicateFileRecord>,
}

#[derive(uniffi::Object)]
pub struct DuplicateSession {
    report: DuplicateReport,
}

#[derive(uniffi::Record)]
pub struct SelectionPolicyRecord {
    pub strategy: KeepStrategy,
    pub preferred_directory: Option<String>,
    pub protect_system: bool,
    pub protect_library: bool,
    pub protect_packages: bool,
    pub extra_protected_prefixes: Vec<String>,
}

#[derive(uniffi::Record)]
pub struct FileDecisionRecord {
    pub path: String,
    pub keeper: bool,
    pub protected: bool,
    pub protection_reason: Option<String>,
    pub selected_for_removal: bool,
    pub explanation: String,
}

#[derive(uniffi::Record)]
pub struct GroupDecisionRecord {
    pub files: Vec<FileDecisionRecord>,
    pub reclaimable_bytes: u64,
}

#[derive(uniffi::Record)]
pub struct DeletionEntryRecord {
    pub path: String,
    pub kind: NodeKind,
    pub apparent_size: u64,
    pub allocated_size: u64,
    pub protected: bool,
    pub protection_reason: Option<String>,
    pub note: Option<String>,
}

#[derive(uniffi::Record)]
pub struct DeletionPlanRecord {
    pub entries: Vec<DeletionEntryRecord>,
    pub skipped: Vec<IssueRecord>,
    pub reclaimable_bytes: u64,
}

#[derive(uniffi::Record)]
pub struct VolumeUsageRecord {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub free_bytes: u64,
    pub used_bytes: u64,
    pub block_size: u64,
}

#[uniffi::export]
impl DuplicateSession {
    pub fn group_count(&self) -> u32 {
        self.report.groups.len() as u32
    }

    pub fn files_considered(&self) -> u64 {
        self.report.files_considered
    }

    pub fn files_hashed(&self) -> u64 {
        self.report.files_hashed
    }

    pub fn total_wasted_bytes(&self) -> u64 {
        self.report.groups.iter().map(DuplicateGroup::wasted_bytes).sum()
    }

    pub fn unreadable(&self) -> Vec<String> {
        self.report.unreadable.iter().map(|path| path.display().to_string()).collect()
    }

    pub fn groups(&self, offset: u32, limit: u32) -> Vec<DuplicateGroupRecord> {
        let start = offset as usize;
        self.report.groups.iter().skip(start).take(limit as usize).map(group_record).collect()
    }

    pub fn apply_selection(&self, policy: SelectionPolicyRecord) -> Vec<GroupDecisionRecord> {
        let policy = policy_from_record(policy);
        self.report
            .groups
            .iter()
            .map(|group| {
                let decision = select_group(group, &policy);
                GroupDecisionRecord {
                    reclaimable_bytes: decision.reclaimable_bytes,
                    files: decision
                        .files
                        .into_iter()
                        .map(|file| FileDecisionRecord {
                            path: file.path.display().to_string(),
                            keeper: file.keeper,
                            protected: file.protected,
                            protection_reason: file.protection_reason,
                            selected_for_removal: file.selected_for_removal,
                            explanation: file.explanation,
                        })
                        .collect(),
                }
            })
            .collect()
    }
}

/// Walk `path` on the calling thread. Swift should call this off the main actor.
///
/// # Errors
///
/// [`ScanError::Cancelled`] when `cancel` is set, or [`ScanError::NotFound`] /
/// [`ScanError::Io`] when the root cannot be read.
#[uniffi::export]
pub fn scan_folder(
    path: String,
    options: ScanOptions,
    cancel: Option<Arc<CancelFlag>>,
    progress: Option<Arc<dyn ProgressSink>>,
) -> Result<Arc<ScanSession>, ScanError> {
    let local = std::sync::atomic::AtomicBool::new(false);
    let flag = cancel.as_ref().map_or(&local, |flag| &flag.inner);
    let bridge;
    let reporter: Option<&dyn ProgressReporter> = match &progress {
        Some(sink) => {
            bridge = ProgressBridge(Arc::clone(sink));
            Some(&bridge)
        }
        None => None,
    };
    let tree = scan(path, &options, flag, reporter)?;
    Ok(Arc::new(ScanSession { tree }))
}

/// Scan `path` and hash duplicate candidates.
///
/// # Errors
///
/// Same as [`scan_folder`].
#[uniffi::export]
pub fn find_duplicate_files(
    path: String,
    options: crate::duplicates::DuplicateOptions,
    cancel: Option<Arc<CancelFlag>>,
    progress: Option<Arc<dyn ProgressSink>>,
) -> Result<Arc<DuplicateSession>, ScanError> {
    let local = std::sync::atomic::AtomicBool::new(false);
    let flag = cancel.as_ref().map_or(&local, |flag| &flag.inner);
    let bridge;
    let reporter: Option<&dyn ProgressReporter> = match &progress {
        Some(sink) => {
            bridge = ProgressBridge(Arc::clone(sink));
            Some(&bridge)
        }
        None => None,
    };
    let report = find_duplicates(path, &options, flag, reporter)?;
    Ok(Arc::new(DuplicateSession { report }))
}

/// `statvfs` for the volume that contains `path`.
///
/// # Errors
///
/// [`ScanError::Io`] when the path cannot be stated.
#[uniffi::export]
pub fn volume_usage(path: String) -> Result<VolumeUsageRecord, ScanError> {
    let usage = filesystem_usage(PathBuf::from(&path).as_path())?;
    Ok(VolumeUsageRecord {
        total_bytes: usage.total_bytes,
        available_bytes: usage.available_bytes,
        free_bytes: usage.free_bytes,
        used_bytes: usage.used_bytes,
        block_size: usage.block_size,
    })
}

/// Finder-style decimal byte string.
#[uniffi::export]
#[must_use]
pub fn format_byte_count(bytes: u64) -> String {
    format_bytes(bytes)
}

/// Why `path` is protected, if it is.
///
/// Smart selection uses the same check. The app calls this when someone
/// selects a protected file by hand, so the confirmation can quote the reason.
#[uniffi::export]
#[must_use]
pub fn explain_protection(path: String, policy: SelectionPolicyRecord) -> Option<String> {
    crate::selection::protection_reason(std::path::Path::new(&path), &policy_from_record(policy))
}

/// Review a deletion. Does not remove anything.
///
/// `include_protected` keeps explicitly chosen protected paths in the plan and
/// flags them. `false` records those paths as skipped instead.
#[uniffi::export]
#[must_use]
pub fn review_deletion(
    paths: Vec<String>,
    policy: SelectionPolicyRecord,
    include_protected: bool,
) -> DeletionPlanRecord {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let plan = plan_deletion(&paths, &policy_from_record(policy), include_protected);
    DeletionPlanRecord {
        reclaimable_bytes: plan.reclaimable_bytes,
        entries: plan
            .entries
            .into_iter()
            .map(|entry| DeletionEntryRecord {
                path: entry.path,
                kind: entry.kind,
                apparent_size: entry.apparent_size,
                allocated_size: entry.allocated_size,
                protected: entry.protected,
                protection_reason: entry.protection_reason,
                note: entry.note,
            })
            .collect(),
        skipped: plan
            .skipped
            .into_iter()
            .map(|issue| IssueRecord { path: issue.path, message: issue.message })
            .collect(),
    }
}

#[uniffi::export]
#[must_use]
pub fn default_scan_options() -> ScanOptions {
    ScanOptions::default()
}

#[uniffi::export]
#[must_use]
pub fn default_duplicate_options() -> crate::duplicates::DuplicateOptions {
    crate::duplicates::DuplicateOptions::default()
}

#[uniffi::export]
#[must_use]
pub fn default_selection_policy() -> SelectionPolicyRecord {
    policy_record(SelectionPolicy::default())
}

fn summarize(node: &ScanNode) -> NodeSummary {
    NodeSummary {
        name: node.name.clone(),
        path: node.path.display().to_string(),
        kind: node.kind,
        category: node.category,
        category_label: node.category.label().to_owned(),
        own_apparent: node.own_apparent,
        own_allocated: node.own_allocated,
        logical_apparent: node.logical_apparent,
        unique_apparent: node.unique_apparent,
        unique_allocated: node.unique_allocated,
        file_count: node.file_count,
        child_count: u32::try_from(node.children.len()).unwrap_or(u32::MAX),
        modified_unix_ms: node.modified_unix_ms,
        shared_elsewhere: node.shared_elsewhere,
        other_volume: node.other_volume,
        error: node.error.clone(),
    }
}

fn group_record(group: &DuplicateGroup) -> DuplicateGroupRecord {
    DuplicateGroupRecord {
        size: group.size,
        hash_hex: group.hash_hex.clone(),
        wasted_bytes: group.wasted_bytes(),
        files: group
            .files
            .iter()
            .map(|file| DuplicateFileRecord {
                path: file.path.display().to_string(),
                name: file.name.clone(),
                size: file.size,
                modified_unix_ms: file.modified_unix_ms,
                inode: file.inode,
                device: file.device,
                nlink: file.nlink,
            })
            .collect(),
    }
}

fn policy_from_record(record: SelectionPolicyRecord) -> SelectionPolicy {
    SelectionPolicy {
        strategy: record.strategy,
        preferred_directory: record.preferred_directory.map(PathBuf::from),
        protect_system: record.protect_system,
        protect_library: record.protect_library,
        protect_packages: record.protect_packages,
        extra_protected_prefixes: record
            .extra_protected_prefixes
            .into_iter()
            .map(PathBuf::from)
            .collect(),
    }
}

fn policy_record(policy: SelectionPolicy) -> SelectionPolicyRecord {
    SelectionPolicyRecord {
        strategy: policy.strategy,
        preferred_directory: policy.preferred_directory.map(|path| path.display().to_string()),
        protect_system: policy.protect_system,
        protect_library: policy.protect_library,
        protect_packages: policy.protect_packages,
        extra_protected_prefixes: policy
            .extra_protected_prefixes
            .into_iter()
            .map(|path| path.display().to_string())
            .collect(),
    }
}
