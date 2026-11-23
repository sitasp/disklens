//! Queries over a finished [`ScanTree`](crate::ScanTree).
//!
//! These walk memory only. The large-file screen, the category breakdown,
//! and the CLI all go through here so the rules stay in one place.

use std::collections::BTreeMap;
use std::path::Path;

use crate::category::FileCategory;
use crate::model::{CategoryTotal, FileEntry, FileQuery, FileSort, NodeKind, ScanNode};

/// Files matching `query`, sorted and truncated to `query.limit`.
#[must_use]
pub fn query_files(root: &ScanNode, query: &FileQuery) -> Vec<FileEntry> {
    if query.limit == 0 {
        return Vec::new();
    }
    let mut matches = Vec::new();
    root.visit_files(&mut |node| {
        if !file_matches(node, query) {
            return;
        }
        matches.push(FileEntry {
            name: node.name.clone(),
            path: node.path.clone(),
            category: node.category,
            apparent_size: node.own_apparent,
            allocated_size: node.own_allocated,
            modified_unix_ms: node.modified_unix_ms,
            shared_elsewhere: node.shared_elsewhere,
            extension: extension_of(&node.path),
        });
    });
    matches.sort_by(|left, right| compare_files(left, right, query.sort));
    matches.truncate(query.limit);
    matches
}

fn file_matches(node: &ScanNode, query: &FileQuery) -> bool {
    if node.kind != NodeKind::File || node.own_apparent < query.min_apparent_size {
        return false;
    }
    if let Some(before) = query.modified_before_unix_ms {
        match node.modified_unix_ms {
            Some(modified) if modified < before => {}
            _ => return false,
        }
    }
    if let Some(after) = query.modified_after_unix_ms {
        match node.modified_unix_ms {
            Some(modified) if modified > after => {}
            _ => return false,
        }
    }
    if let Some(category) = query.category {
        if node.category != category {
            return false;
        }
    }
    true
}

fn compare_files(left: &FileEntry, right: &FileEntry, sort: FileSort) -> std::cmp::Ordering {
    let by_name = left.name.cmp(&right.name).then_with(|| left.path.cmp(&right.path));
    match sort {
        FileSort::Largest => right.apparent_size.cmp(&left.apparent_size).then(by_name),
        FileSort::Smallest => left.apparent_size.cmp(&right.apparent_size).then(by_name),
        FileSort::Oldest => left
            .modified_unix_ms
            .unwrap_or(i64::MAX)
            .cmp(&right.modified_unix_ms.unwrap_or(i64::MAX))
            .then(by_name),
        FileSort::Newest => right
            .modified_unix_ms
            .unwrap_or(i64::MIN)
            .cmp(&left.modified_unix_ms.unwrap_or(i64::MIN))
            .then(by_name),
        FileSort::Name => by_name,
    }
}

fn extension_of(path: &Path) -> String {
    path.extension().and_then(|ext| ext.to_str()).map(str::to_ascii_lowercase).unwrap_or_default()
}

/// Sum unique bytes and file counts per category.
///
/// Directories are not counted; a folder full of photos contributes to Images,
/// not to a separate folder bucket. Hard-link aliases add to `file_count` but
/// not to the unique byte totals.
#[must_use]
pub fn summarize_categories(root: &ScanNode) -> Vec<CategoryTotal> {
    let mut totals: BTreeMap<FileCategory, CategoryTotal> = BTreeMap::new();
    root.visit_files(&mut |node| {
        let entry = totals.entry(node.category).or_insert(CategoryTotal {
            category: node.category,
            unique_apparent: 0,
            unique_allocated: 0,
            file_count: 0,
        });
        entry.file_count += 1;
        if !node.shared_elsewhere {
            entry.unique_apparent = entry.unique_apparent.saturating_add(node.own_apparent);
            entry.unique_allocated = entry.unique_allocated.saturating_add(node.own_allocated);
        }
    });
    let mut rows: Vec<_> = totals.into_values().collect();
    rows.sort_by(|left, right| {
        right
            .unique_apparent
            .cmp(&left.unique_apparent)
            .then_with(|| left.category.cmp(&right.category))
    });
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NodeKind;
    use std::path::PathBuf;

    fn file(name: &str, size: u64, modified: Option<i64>, category: FileCategory) -> ScanNode {
        ScanNode {
            name: name.to_owned(),
            path: PathBuf::from(format!("/data/{name}")),
            kind: NodeKind::File,
            category,
            own_apparent: size,
            own_allocated: size,
            logical_apparent: size,
            unique_apparent: size,
            unique_allocated: size,
            file_count: 1,
            modified_unix_ms: modified,
            inode: None,
            device: None,
            clone_id: None,
            shared_elsewhere: false,
            other_volume: false,
            error: None,
            children: Vec::new(),
        }
    }

    fn root(children: Vec<ScanNode>) -> ScanNode {
        ScanNode {
            name: "data".to_owned(),
            path: PathBuf::from("/data"),
            kind: NodeKind::Directory,
            category: FileCategory::Other,
            own_apparent: 0,
            own_allocated: 0,
            logical_apparent: 0,
            unique_apparent: 0,
            unique_allocated: 0,
            file_count: children.len() as u64,
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

    #[test]
    fn large_file_query_sorts_and_limits() {
        let tree = root(vec![
            file("small.txt", 10, Some(30), FileCategory::Document),
            file("huge.bin", 500, Some(20), FileCategory::Other),
            file("mid.txt", 100, Some(10), FileCategory::Document),
        ]);
        let query = FileQuery {
            min_apparent_size: 50,
            limit: 1,
            sort: FileSort::Largest,
            ..FileQuery::default()
        };
        let rows = query_files(&tree, &query);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "huge.bin");
    }

    #[test]
    fn old_file_filter_excludes_missing_mtime() {
        let tree = root(vec![
            file("ancient.txt", 80, Some(5), FileCategory::Document),
            file("fresh.txt", 80, Some(500), FileCategory::Document),
            file("unknown.txt", 80, None, FileCategory::Document),
        ]);
        let query =
            FileQuery { modified_before_unix_ms: Some(100), limit: 10, ..FileQuery::default() };
        let rows = query_files(&tree, &query);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "ancient.txt");
    }

    #[test]
    fn category_filter_and_summary_skip_shared_bytes() {
        let mut alias = file("copy.bin", 40, None, FileCategory::Other);
        alias.shared_elsewhere = true;
        alias.unique_apparent = 0;
        let tree = root(vec![
            file("photo.jpg", 10, None, FileCategory::Image),
            file("note.txt", 4, None, FileCategory::Document),
            alias,
        ]);
        let only_images = query_files(
            &tree,
            &FileQuery { category: Some(FileCategory::Image), limit: 10, ..FileQuery::default() },
        );
        assert_eq!(only_images.len(), 1);
        let summary = summarize_categories(&tree);
        let other = summary.iter().find(|row| row.category == FileCategory::Other).unwrap();
        assert_eq!(other.file_count, 1);
        assert_eq!(other.unique_apparent, 0);
        let images = summary.iter().find(|row| row.category == FileCategory::Image).unwrap();
        assert_eq!(images.unique_apparent, 10);
    }
}
