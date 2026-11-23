//! Smart duplicate selection.
//!
//! The rules decide which copy to keep. Protected locations are never
//! selected for removal, even when they lose the "keep" comparison. If every
//! copy is protected, nothing is selected.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use crate::duplicates::{reclaimable_bytes, DuplicateFile, DuplicateGroup};

/// Which copy should survive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, uniffi::Enum)]
pub enum KeepStrategy {
    /// Highest modification time. Missing timestamps lose to real ones.
    #[default]
    Newest,
    /// Lowest modification time. Missing timestamps lose to real ones.
    Oldest,
    /// A file inside [`SelectionPolicy::preferred_directory`]. Falls back to
    /// the shortest path, then the newest, when nothing matches.
    PreferredDirectory,
    /// Fewest path components, then the shortest path string.
    ShortestPath,
    /// Prefer a name that does not look like "copy", "Copy of", or "(1)".
    MostOriginalName,
}

/// Safety policy for [`select_group`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionPolicy {
    pub strategy: KeepStrategy,
    pub preferred_directory: Option<PathBuf>,
    /// `/System`, `/usr`, `/bin`, `/sbin`, `/Library`, `/private/var`, `/dev`.
    pub protect_system: bool,
    /// Any path component named `Library`, which covers `~/Library` as well
    /// as `/Library`. User caches live there; they are not auto-selected.
    pub protect_library: bool,
    /// Files inside `.app`, `.photoslibrary`, `.framework`, `.bundle`,
    /// `.kext`, `.plugin`, or `.prefPane` packages.
    pub protect_packages: bool,
    /// Extra path prefixes the user marked as hands-off.
    pub extra_protected_prefixes: Vec<PathBuf>,
}

impl Default for SelectionPolicy {
    fn default() -> Self {
        Self {
            strategy: KeepStrategy::Newest,
            preferred_directory: None,
            protect_system: true,
            protect_library: true,
            protect_packages: true,
            extra_protected_prefixes: Vec::new(),
        }
    }
}

/// Why a path must not be auto-selected, if it must not.
#[must_use]
pub fn protection_reason(path: &Path, policy: &SelectionPolicy) -> Option<String> {
    if let Some(prefix) =
        policy.extra_protected_prefixes.iter().find(|prefix| path_within(path, prefix))
    {
        return Some(format!("inside {}", prefix.display()));
    }
    if policy.protect_packages {
        if let Some(package) = package_component(path) {
            return Some(format!("inside a {package} package"));
        }
    }
    if policy.protect_system {
        if let Some(prefix) = system_prefix(path) {
            return Some(format!("inside the system location {prefix}"));
        }
    }
    if policy.protect_library && has_library_component(path) {
        return Some("inside a Library folder".to_owned());
    }
    None
}

/// One file after a keep-rule runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDecision {
    pub path: PathBuf,
    /// The copy the rule chose to preserve. A protected file can be the
    /// keeper; it is still not selected.
    pub keeper: bool,
    pub protected: bool,
    pub protection_reason: Option<String>,
    /// `true` only when the file should move to the trash.
    pub selected_for_removal: bool,
    pub explanation: String,
}

/// Decisions for one duplicate group, plus the bytes those decisions would free.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupDecision {
    pub files: Vec<FileDecision>,
    pub reclaimable_bytes: u64,
}

/// Apply `policy` to one group.
///
/// The keeper is chosen from every file, including protected ones. Every
/// other file is selected unless it is protected. Reclaimable bytes count an
/// inode or clone only when every one of its links in the group is selected
/// and `nlink` says no link would remain outside the group.
#[must_use]
pub fn select_group(group: &DuplicateGroup, policy: &SelectionPolicy) -> GroupDecision {
    if group.files.is_empty() {
        return GroupDecision { files: Vec::new(), reclaimable_bytes: 0 };
    }
    let keeper = pick_keeper(&group.files, policy);
    let mut files = Vec::with_capacity(group.files.len());
    let mut selected = vec![false; group.files.len()];
    for (index, file) in group.files.iter().enumerate() {
        let reason = protection_reason(&file.path, policy);
        let protected = reason.is_some();
        let is_keeper = index == keeper;
        let selected_for_removal = !is_keeper && !protected;
        selected[index] = selected_for_removal;
        files.push(FileDecision {
            path: file.path.clone(),
            keeper: is_keeper,
            protected,
            protection_reason: reason.clone(),
            selected_for_removal,
            explanation: explain(is_keeper, protected, reason.as_deref(), policy.strategy),
        });
    }
    // When the natural keeper is the only unprotected file, the others may
    // all be protected and `selected` stays empty. That is intentional.
    let reclaimable_bytes = reclaimable_bytes(&group.files, &selected);
    GroupDecision { files, reclaimable_bytes }
}

fn explain(keeper: bool, protected: bool, reason: Option<&str>, strategy: KeepStrategy) -> String {
    if protected {
        let why = reason.unwrap_or("a protected location");
        return format!("left in place because it is {why}");
    }
    if keeper {
        return match strategy {
            KeepStrategy::Newest => "kept as the newest copy".to_owned(),
            KeepStrategy::Oldest => "kept as the oldest copy".to_owned(),
            KeepStrategy::PreferredDirectory => {
                "kept because it is in the preferred folder".to_owned()
            }
            KeepStrategy::ShortestPath => "kept because it has the shortest path".to_owned(),
            KeepStrategy::MostOriginalName => {
                "kept because the name looks like the original".to_owned()
            }
        };
    }
    "selected because another copy is kept".to_owned()
}

fn pick_keeper(files: &[DuplicateFile], policy: &SelectionPolicy) -> usize {
    let mut best = 0;
    for index in 1..files.len() {
        if better_keeper(&files[index], &files[best], policy) {
            best = index;
        }
    }
    best
}

fn better_keeper(
    candidate: &DuplicateFile,
    current: &DuplicateFile,
    policy: &SelectionPolicy,
) -> bool {
    match policy.strategy {
        KeepStrategy::Newest => newer(candidate, current),
        KeepStrategy::Oldest => older(candidate, current),
        KeepStrategy::ShortestPath => shorter(candidate, current),
        KeepStrategy::MostOriginalName => {
            let score = originality(&candidate.name).cmp(&originality(&current.name));
            match score {
                Ordering::Greater => true,
                Ordering::Less => false,
                Ordering::Equal => shorter(candidate, current),
            }
        }
        KeepStrategy::PreferredDirectory => {
            let preferred = policy.preferred_directory.as_deref();
            let candidate_in = preferred.is_some_and(|dir| path_within(&candidate.path, dir));
            let current_in = preferred.is_some_and(|dir| path_within(&current.path, dir));
            match (candidate_in, current_in) {
                (true, false) => true,
                (false, true) => false,
                _ => shorter(candidate, current),
            }
        }
    }
}

fn time_key(file: &DuplicateFile, missing: i64) -> i64 {
    file.modified_unix_ms.unwrap_or(missing)
}

/// Missing timestamps lose: they sort as the oldest instant, so a real
/// timestamp is preferred when keeping the newest.
fn newer(candidate: &DuplicateFile, current: &DuplicateFile) -> bool {
    match time_key(candidate, i64::MIN).cmp(&time_key(current, i64::MIN)) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => shorter(candidate, current),
    }
}

/// Missing timestamps lose here too: they sort as the newest instant, so a
/// file with a real older timestamp is the one we keep.
fn older(candidate: &DuplicateFile, current: &DuplicateFile) -> bool {
    match time_key(candidate, i64::MAX).cmp(&time_key(current, i64::MAX)) {
        Ordering::Less => true,
        Ordering::Greater => false,
        Ordering::Equal => shorter(candidate, current),
    }
}

fn shorter(candidate: &DuplicateFile, current: &DuplicateFile) -> bool {
    let candidate_rank =
        (candidate.path.components().count(), candidate.path.as_os_str().len(), &candidate.path);
    let current_rank =
        (current.path.components().count(), current.path.as_os_str().len(), &current.path);
    candidate_rank < current_rank
}

fn originality(name: &str) -> i32 {
    if looks_like_copy(name) {
        0
    } else {
        1
    }
}

fn looks_like_copy(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("copy of ") || lower.contains(" copy") || lower.contains("_copy") {
        return true;
    }
    if let Some(open) = lower.rfind('(') {
        if let Some(close) = lower[open..].find(')') {
            let inside = &lower[open + 1..open + close];
            if !inside.is_empty() && inside.chars().all(|ch| ch.is_ascii_digit()) {
                return true;
            }
        }
    }
    false
}

fn path_within(path: &Path, prefix: &Path) -> bool {
    let path = path.to_string_lossy().to_ascii_lowercase();
    let prefix = prefix.to_string_lossy().trim_end_matches('/').to_ascii_lowercase();
    if prefix.is_empty() {
        return false;
    }
    path == prefix || path.starts_with(&(prefix + "/"))
}

fn system_prefix(path: &Path) -> Option<&'static str> {
    const PREFIXES: &[&str] =
        &["/System", "/usr", "/bin", "/sbin", "/Library", "/private/var", "/dev"];
    let text = path.to_string_lossy().to_ascii_lowercase();
    PREFIXES.iter().copied().find(|prefix| {
        let lower = prefix.to_ascii_lowercase();
        text == lower || text.starts_with(&(lower + "/"))
    })
}

fn has_library_component(path: &Path) -> bool {
    path.components().any(|component| {
        component.as_os_str().to_str().is_some_and(|text| text.eq_ignore_ascii_case("Library"))
    })
}

fn package_component(path: &Path) -> Option<&'static str> {
    const SUFFIXES: &[(&str, &str)] = &[
        (".photoslibrary", ".photoslibrary"),
        (".photolibrary", ".photolibrary"),
        (".framework", ".framework"),
        (".prefpane", ".prefPane"),
        (".plugin", ".plugin"),
        (".bundle", ".bundle"),
        (".kext", ".kext"),
        (".app", ".app"),
    ];
    for component in path.components() {
        let Some(text) = component.as_os_str().to_str() else { continue };
        let lower = text.to_ascii_lowercase();
        for (suffix, label) in SUFFIXES {
            if lower.len() > suffix.len() && lower.ends_with(suffix) {
                return Some(label);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duplicates::DuplicateGroup;

    fn file(path: &str, modified: Option<i64>, inode: u64) -> DuplicateFile {
        DuplicateFile {
            path: PathBuf::from(path),
            name: Path::new(path).file_name().unwrap().to_string_lossy().into_owned(),
            size: 10,
            modified_unix_ms: modified,
            inode,
            device: 1,
            clone_id: None,
            nlink: 1,
        }
    }

    fn group(files: Vec<DuplicateFile>) -> DuplicateGroup {
        DuplicateGroup { size: 10, hash_hex: "abc".to_owned(), files }
    }

    #[test]
    fn keeps_the_newest_and_selects_the_rest() {
        let decision = select_group(
            &group(vec![file("/data/old.txt", Some(10), 1), file("/data/new.txt", Some(50), 2)]),
            &SelectionPolicy::default(),
        );
        let new = decision.files.iter().find(|item| item.path.ends_with("new.txt")).unwrap();
        let old = decision.files.iter().find(|item| item.path.ends_with("old.txt")).unwrap();
        assert!(new.keeper && !new.selected_for_removal);
        assert!(old.selected_for_removal);
        assert_eq!(decision.reclaimable_bytes, 10);
        assert!(new.explanation.contains("newest"));
    }

    #[test]
    fn keeps_the_oldest() {
        let policy =
            SelectionPolicy { strategy: KeepStrategy::Oldest, ..SelectionPolicy::default() };
        let decision = select_group(
            &group(vec![file("/data/old.txt", Some(10), 1), file("/data/new.txt", Some(50), 2)]),
            &policy,
        );
        assert!(decision.files.iter().any(|item| item.keeper && item.path.ends_with("old.txt")));
    }

    #[test]
    fn prefers_a_directory_then_falls_back() {
        let mut policy = SelectionPolicy {
            strategy: KeepStrategy::PreferredDirectory,
            preferred_directory: Some(PathBuf::from("/vault")),
            ..SelectionPolicy::default()
        };
        let decision = select_group(
            &group(vec![
                file("/vault/nested/keep.txt", Some(1), 1),
                file("/downloads/copy.txt", Some(9), 2),
            ]),
            &policy,
        );
        assert!(decision.files.iter().any(|item| item.keeper && item.path.starts_with("/vault")));

        policy.preferred_directory = Some(PathBuf::from("/nowhere"));
        let fallback = select_group(
            &group(vec![
                file("/data/long/path/file.txt", Some(5), 1),
                file("/data/a.txt", Some(1), 2),
            ]),
            &policy,
        );
        assert!(fallback.files.iter().any(|item| item.keeper && item.path.ends_with("a.txt")));
    }

    #[test]
    fn shortest_path_and_original_name() {
        let mut policy =
            SelectionPolicy { strategy: KeepStrategy::ShortestPath, ..SelectionPolicy::default() };
        let decision = select_group(
            &group(vec![
                file("/data/deep/down/file.txt", Some(9), 1),
                file("/data/file.txt", Some(1), 2),
            ]),
            &policy,
        );
        assert!(decision
            .files
            .iter()
            .any(|item| item.keeper && item.path == Path::new("/data/file.txt")));

        policy.strategy = KeepStrategy::MostOriginalName;
        let named = select_group(
            &group(vec![
                file("/data/Report copy.txt", Some(9), 1),
                file("/data/Copy of Report.txt", Some(8), 2),
                file("/data/Report.txt", Some(1), 3),
                file("/data/Report (1).txt", Some(7), 4),
            ]),
            &policy,
        );
        assert!(named.files.iter().any(|item| item.keeper && item.path.ends_with("Report.txt")));
    }

    #[test]
    fn never_selects_protected_locations() {
        let policy = SelectionPolicy::default();
        assert!(protection_reason(Path::new("/System/Library/Fonts/A"), &policy).is_some());
        assert!(protection_reason(Path::new("/usr/bin/swift"), &policy).is_some());
        assert!(protection_reason(Path::new("/Users/me/Library/Caches/x"), &policy).is_some());
        assert!(protection_reason(
            Path::new("/Applications/Mail.app/Contents/MacOS/Mail"),
            &policy
        )
        .is_some());
        assert!(protection_reason(
            Path::new("/Users/me/Pictures/Library.photoslibrary/originals/a"),
            &policy
        )
        .is_some());
        assert!(protection_reason(Path::new("/Users/me/Documents/photo.jpg"), &policy).is_none());
        // `/usr` must not protect a different prefix.
        assert!(protection_reason(Path::new("/usr-local/bin/tool"), &policy).is_none());

        let decision = select_group(
            &group(vec![
                file("/System/Library/a.txt", Some(1), 1),
                file("/Users/me/Documents/a.txt", Some(2), 2),
            ]),
            &policy,
        );
        let system = decision.files.iter().find(|item| item.path.starts_with("/System")).unwrap();
        let home = decision.files.iter().find(|item| item.path.starts_with("/Users")).unwrap();
        assert!(system.protected && !system.selected_for_removal);
        assert!(!home.protected);
        // The newer home copy is the keeper, so nothing is selected: the
        // system copy is protected and the home copy is kept.
        assert!(!home.selected_for_removal);
        assert_eq!(decision.reclaimable_bytes, 0);

        let older_home = select_group(
            &group(vec![
                file("/Users/me/Documents/original.txt", Some(5), 1),
                file("/Users/me/Documents/copy.txt", Some(1), 2),
                file("/Applications/Demo.app/Contents/original.txt", Some(9), 3),
            ]),
            &policy,
        );
        assert!(older_home
            .files
            .iter()
            .any(|item| item.selected_for_removal && item.path.ends_with("copy.txt")));
        assert!(older_home.files.iter().all(|item| {
            !item.path.to_string_lossy().contains(".app") || !item.selected_for_removal
        }));
    }

    #[test]
    fn all_protected_selects_nothing() {
        let decision = select_group(
            &group(vec![file("/System/a.txt", Some(1), 1), file("/Library/b.txt", Some(2), 2)]),
            &SelectionPolicy::default(),
        );
        assert!(decision.files.iter().all(|item| !item.selected_for_removal));
        assert_eq!(decision.reclaimable_bytes, 0);
        assert_eq!(decision.files.iter().filter(|item| item.keeper).count(), 1);
    }

    #[test]
    fn extra_prefix_is_protected() {
        let policy = SelectionPolicy {
            protect_system: false,
            protect_library: false,
            protect_packages: false,
            extra_protected_prefixes: vec![PathBuf::from("/work/keep")],
            ..SelectionPolicy::default()
        };
        let decision = select_group(
            &group(vec![
                file("/work/keep/a.txt", Some(5), 1),
                file("/work/drop/a.txt", Some(1), 2),
            ]),
            &policy,
        );
        let kept = decision.files.iter().find(|item| item.path.starts_with("/work/keep")).unwrap();
        let dropped =
            decision.files.iter().find(|item| item.path.starts_with("/work/drop")).unwrap();
        assert!(kept.protected && !kept.selected_for_removal);
        assert!(dropped.selected_for_removal);
    }

    #[test]
    fn hard_link_identity_is_not_double_counted() {
        let mut first = file("/data/a.txt", Some(1), 7);
        let mut second = file("/data/b.txt", Some(2), 7);
        let third = file("/data/c.txt", Some(3), 8);
        first.nlink = 2;
        second.nlink = 2;
        let decision =
            select_group(&group(vec![first, second, third]), &SelectionPolicy::default());
        // Newest is c.txt (inode 8). a.txt and b.txt share inode 7, nlink 2,
        // and both are selected, so that inode is fully removed.
        assert_eq!(decision.reclaimable_bytes, 10);
    }
}
