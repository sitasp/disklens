//! File-type buckets used by the sunburst, the category screen, and filters.
//!
//! Classification looks at path components first (a `.js` file inside
//! `node_modules` is a developer cache, not source) and then at the extension.
//! It does not read file contents.

use std::path::Path;

/// Coarse kind of file, stable and exhaustive so the UI can color every node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FileCategory {
    Application,
    Audio,
    Video,
    Image,
    Document,
    Archive,
    Code,
    DeveloperCache,
    System,
    Other,
}

impl FileCategory {
    /// Short label for menus and legends.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Application => "Applications",
            Self::Audio => "Audio",
            Self::Video => "Video",
            Self::Image => "Images",
            Self::Document => "Documents",
            Self::Archive => "Archives",
            Self::Code => "Code",
            Self::DeveloperCache => "Developer caches",
            Self::System => "System",
            Self::Other => "Other",
        }
    }

    /// Every category, in legend order.
    #[must_use]
    pub fn all() -> &'static [FileCategory] {
        &ALL
    }
}

const ALL: [FileCategory; 10] = [
    FileCategory::Application,
    FileCategory::Audio,
    FileCategory::Video,
    FileCategory::Image,
    FileCategory::Document,
    FileCategory::Archive,
    FileCategory::Code,
    FileCategory::DeveloperCache,
    FileCategory::System,
    FileCategory::Other,
];

/// Classify `path`.
///
/// Priority, highest first:
/// 1. Developer-cache directory components (`node_modules`, `DerivedData`, …).
/// 2. Anything inside an `.app` bundle.
/// 3. System prefixes (`/System`, `/usr`, `/bin`, `/sbin`, `/Library`).
/// 4. File extension.
/// 5. [`FileCategory::Other`].
#[must_use]
pub fn categorize(path: &Path) -> FileCategory {
    if is_developer_cache(path) {
        return FileCategory::DeveloperCache;
    }
    if is_app_bundle(path) {
        return FileCategory::Application;
    }
    if is_system_path(path) {
        return FileCategory::System;
    }
    extension_category(path).unwrap_or(FileCategory::Other)
}

fn is_developer_cache(path: &Path) -> bool {
    path.components().any(|component| {
        let Some(text) = component.as_os_str().to_str() else {
            return false;
        };
        CACHE_COMPONENTS.iter().any(|needle| text.eq_ignore_ascii_case(needle))
    })
}

/// Directory names that are regenerable build or package caches.
/// `target` is included because Cargo (and a few other tools) put large,
/// disposable build output there. A real folder the user named `target` will
/// be labelled as a cache; the category view is a hint, not a delete list.
const CACHE_COMPONENTS: &[&str] = &[
    "node_modules",
    "DerivedData",
    ".gradle",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".tox",
    ".npm",
    ".pnpm-store",
    "Carthage",
    "Pods",
    ".venv",
    "venv",
    "site-packages",
    ".cache",
    "target",
];

fn is_app_bundle(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|text| text.len() > 4 && text.to_ascii_lowercase().ends_with(".app"))
    })
}

fn is_system_path(path: &Path) -> bool {
    const PREFIXES: &[&str] = &["/System", "/usr", "/bin", "/sbin", "/Library"];
    let text = path.to_string_lossy();
    let lower = text.to_ascii_lowercase();
    PREFIXES.iter().any(|prefix| {
        let prefix = prefix.to_ascii_lowercase();
        lower == prefix || lower.starts_with(&(prefix + "/"))
    })
}

fn extension_category(path: &Path) -> Option<FileCategory> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "heic" | "heif" | "tif" | "tiff" | "webp" | "bmp"
        | "raw" | "cr2" | "cr3" | "nef" | "arw" | "dng" | "svg" | "ico" | "psd" => {
            FileCategory::Image
        }
        "mp4" | "mov" | "m4v" | "mkv" | "avi" | "wmv" | "flv" | "webm" | "mpeg" | "mpg" | "mts" => {
            FileCategory::Video
        }
        "mp3" | "aac" | "m4a" | "flac" | "wav" | "aiff" | "aif" | "ogg" | "opus" | "wma" => {
            FileCategory::Audio
        }
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "pages" | "numbers" | "key"
        | "txt" | "rtf" | "md" | "csv" | "odt" | "ods" | "odp" | "eml" | "msg" => {
            FileCategory::Document
        }
        "zip" | "rar" | "7z" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "dmg" | "iso" | "pkg"
        | "sparseimage" => FileCategory::Archive,
        "rs" | "swift" | "py" | "js" | "ts" | "tsx" | "jsx" | "go" | "java" | "kt" | "c" | "h"
        | "cpp" | "hpp" | "m" | "mm" | "rb" | "php" | "cs" | "json" | "yaml" | "yml" | "toml"
        | "xml" | "html" | "css" | "scss" | "sh" | "sql" | "lock" => FileCategory::Code,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_maps() {
        assert_eq!(categorize(Path::new("/Users/a/photo.HEIC")), FileCategory::Image);
        assert_eq!(categorize(Path::new("/Users/a/clip.mov")), FileCategory::Video);
        assert_eq!(categorize(Path::new("/Users/a/song.flac")), FileCategory::Audio);
        assert_eq!(categorize(Path::new("/Users/a/notes.md")), FileCategory::Document);
        assert_eq!(categorize(Path::new("/Users/a/disk.dmg")), FileCategory::Archive);
        assert_eq!(categorize(Path::new("/Users/a/main.rs")), FileCategory::Code);
        assert_eq!(categorize(Path::new("/Users/a/blob.bin")), FileCategory::Other);
    }

    #[test]
    fn caches_outrank_extensions() {
        let path = Path::new("/Users/a/proj/node_modules/left-pad/index.js");
        assert_eq!(categorize(path), FileCategory::DeveloperCache);
        assert_eq!(
            categorize(Path::new("/Users/a/Library/Developer/Xcode/DerivedData/Logs/a.txt")),
            FileCategory::DeveloperCache
        );
        assert_eq!(
            categorize(Path::new("/Users/a/proj/target/release/app")),
            FileCategory::DeveloperCache
        );
    }

    #[test]
    fn app_bundles_and_system_paths() {
        assert_eq!(
            categorize(Path::new("/Applications/Mail.app/Contents/MacOS/Mail")),
            FileCategory::Application
        );
        assert_eq!(categorize(Path::new("/System/Library/Fonts/Arial.ttf")), FileCategory::System);
        assert_eq!(categorize(Path::new("/usr/bin/swift")), FileCategory::System);
        // `/usr` must not swallow an unrelated prefix.
        assert_eq!(categorize(Path::new("/Users/a/photo.png")), FileCategory::Image);
        assert_eq!(categorize(Path::new("/usr-local/photo.png")), FileCategory::Image);
    }
}
