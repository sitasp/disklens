import Foundation
import Observation

struct StoredBookmark: Codable, Equatable, Identifiable {
    var id: UUID
    var path: String
    var name: String
    var bookmark: Data
}

/// Path matching for saved folders. Security-scoped bookmarks use the same rule
/// to decide which saved folder covers a file about to be scanned or deleted.
enum FolderAccess {
    static func bestCovering(path: String, folders: [StoredBookmark]) -> StoredBookmark? {
        let normalized = standardize(path)
        return folders
            .filter { covers(root: standardize($0.path), path: normalized) }
            .max { standardize($0.path).count < standardize($1.path).count }
    }

    static func covers(root: String, path: String) -> Bool {
        if path == root {
            return true
        }
        let prefix = root.hasSuffix("/") ? root : root + "/"
        return path.hasPrefix(prefix)
    }

    static func standardize(_ path: String) -> String {
        URL(fileURLWithPath: path).standardizedFileURL.path
    }
}

/// Persisted security-scoped bookmarks.
///
/// The App Store build refuses to scan or delete until one of these resolves
/// and `startAccessingSecurityScopedResource()` succeeds. The direct build
/// records bookmarks when the user picks a folder, and does not require them.
@MainActor
@Observable
final class FolderAccessStore {
    private(set) var folders: [StoredBookmark] = []
    private let fileURL: URL
    /// Each successful `startAccessingSecurityScopedResource()` is counted so
    /// a later stop can balance it. Access is held for the session.
    private var holds: [AccessHold] = []

    init(directory: URL) {
        fileURL = directory.appendingPathComponent("bookmarks.json")
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        load()
    }

    convenience init() {
        self.init(directory: HistoryStore.defaultDirectory())
    }

    /// Remember `url` after an open panel and start using it.
    ///
    /// A returned string is a problem the UI should show. The direct build
    /// ignores bookmark failures because it can still read the path.
    func grantPickedFolder(_ url: URL) -> String? {
        let standardized = url.standardizedFileURL
        let started = retain(standardized)
        if Distribution.isAppStore && !started {
            return "macOS did not grant access to \(standardized.path)."
        }
        do {
            let data = try standardized.bookmarkData(
                options: [.withSecurityScope],
                includingResourceValuesForKeys: nil,
                relativeTo: nil
            )
            let path = standardized.path
            if let index = folders.firstIndex(where: { FolderAccess.standardize($0.path) == path }) {
                folders[index].bookmark = data
                folders[index].path = path
                folders[index].name = DiskPaths.displayName(path)
            } else {
                folders.append(
                    StoredBookmark(
                        id: UUID(),
                        path: path,
                        name: DiskPaths.displayName(path),
                        bookmark: data
                    )
                )
            }
            save()
            return nil
        } catch {
            if Distribution.isAppStore {
                return "Could not remember \(standardized.path): \(error.localizedDescription)"
            }
            return nil
        }
    }

    /// Begin access for a scan. Direct builds always succeed.
    func prepareToRead(_ path: String) -> String? {
        guard Distribution.isAppStore else { return nil }
        if renew(path) {
            return nil
        }
        return "DiskLens does not have saved access to \(path). Open the folder again to grant it."
    }

    /// Begin access again for every path about to be removed.
    func prepareToDelete(_ paths: [String]) -> String? {
        guard Distribution.isAppStore else { return nil }
        let missing = paths.filter { !renew($0) }
        guard missing.isEmpty else {
            let noun = missing.count == 1 ? "item" : "items"
            return "Access expired for \(missing.count) \(noun). Grant access to the folder again, then retry the deletion."
        }
        return nil
    }

    private func renew(_ path: String) -> Bool {
        guard let stored = FolderAccess.bestCovering(path: path, folders: folders) else { return false }
        var stale = false
        guard let resolved = try? URL(
            resolvingBookmarkData: stored.bookmark,
            options: [.withSecurityScope],
            relativeTo: nil,
            bookmarkDataIsStale: &stale
        ) else {
            return false
        }
        if stale,
           let refreshed = try? resolved.bookmarkData(
               options: [.withSecurityScope],
               includingResourceValuesForKeys: nil,
               relativeTo: nil
           ),
           let index = folders.firstIndex(where: { $0.id == stored.id }) {
            folders[index].bookmark = refreshed
            folders[index].path = resolved.standardizedFileURL.path
            save()
        }
        return retain(resolved)
    }

    private func retain(_ url: URL) -> Bool {
        if let index = holds.firstIndex(where: { $0.url.path == url.path }) {
            guard url.startAccessingSecurityScopedResource() else { return false }
            holds[index].count += 1
            return true
        }
        guard url.startAccessingSecurityScopedResource() else { return false }
        holds.append(AccessHold(url: url, count: 1))
        return true
    }

    private struct AccessHold {
        var url: URL
        var count: Int
    }

    private func load() {
        guard let data = try? Data(contentsOf: fileURL) else { return }
        folders = (try? JSONDecoder().decode([StoredBookmark].self, from: data)) ?? []
    }

    private func save() {
        guard let data = try? JSONEncoder().encode(folders) else { return }
        try? data.write(to: fileURL, options: .atomic)
    }
}
