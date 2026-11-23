import Foundation

/// Path helpers that do not need the Rust core, so unit tests can cover them
/// without a scan.
enum DiskPaths {
    static func displayName(_ path: String) -> String {
        if path == "/" {
            return "/"
        }
        let name = URL(fileURLWithPath: path).lastPathComponent
        return name.isEmpty ? path : name
    }

    /// Root-to-current chain. The first crumb is the scan root.
    static func crumbs(root: String, current: String) -> [(title: String, path: String)] {
        let rootURL = URL(fileURLWithPath: root).standardizedFileURL
        let currentURL = URL(fileURLWithPath: current).standardizedFileURL
        if rootURL.path == currentURL.path {
            return [(displayName(rootURL.path), rootURL.path)]
        }
        var chain: [URL] = []
        var cursor = currentURL
        chain.append(cursor)
        while cursor.path != rootURL.path {
            let parent = cursor.deletingLastPathComponent()
            if parent.path == cursor.path {
                break
            }
            cursor = parent
            chain.append(cursor)
            if cursor.path == rootURL.path {
                break
            }
            if cursor.path.count < rootURL.path.count {
                chain.append(rootURL)
                break
            }
        }
        if chain.last?.path != rootURL.path {
            chain.append(rootURL)
        }
        return chain.reversed().map { url in
            (displayName(url.path), url.path)
        }
    }

    static func parent(root: String, current: String) -> String? {
        let crumbs = crumbs(root: root, current: current)
        guard crumbs.count >= 2 else { return nil }
        return crumbs[crumbs.count - 2].path
    }

    /// Whole system locations. A folder inside one of these is a normal choice.
    /// The location itself is not: the open panel can hand it back with one
    /// keypress, and walking it reads every file under it.
    static func isBroadSystemLocation(_ path: String) -> Bool {
        let standardized = URL(fileURLWithPath: path).standardizedFileURL.path
        return broadSystemLocations.contains(standardized)
    }

    private static let broadSystemLocations: Set<String> = [
        "/",
        "/Applications",
        "/System",
        "/Library",
        "/Users",
        "/Volumes",
        "/usr",
        "/bin",
        "/sbin",
        "/opt",
        "/private",
    ]
}

enum ProtectedRemovalGate {
    /// Protected items stay in the plan, and deletion waits until this is true.
    static func allowsDeletion(hasProtectedItems: Bool, acknowledged: Bool) -> Bool {
        !hasProtectedItems || acknowledged
    }
}

enum PermanentDeleteGate {
    /// The review screen requires this exact word before a permanent delete.
    static let phrase = "DELETE"

    static func accepts(_ typed: String) -> Bool {
        typed.trimmingCharacters(in: .whitespacesAndNewlines) == phrase
    }
}

enum ScanErrorText {
    static func message(for error: Error) -> String {
        guard let scan = error as? ScanError else {
            return error.localizedDescription
        }
        switch scan {
        case .Cancelled:
            return "The scan was cancelled."
        case .NotFound(let path):
            return "Nothing is at \(path)."
        case .Io(let path, let message):
            return "\(path): \(message)"
        }
    }
}
