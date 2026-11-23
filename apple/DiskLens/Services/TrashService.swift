import Foundation

enum TrashService {
    /// Move each path to the Trash. A failure on one path is reported and the
    /// rest are still attempted. The returned items are the ones that moved.
    static func moveToTrash(paths: [String]) -> (moved: [TrashedItem], errors: [String]) {
        var moved: [TrashedItem] = []
        var errors: [String] = []
        for path in paths {
            let url = URL(fileURLWithPath: path)
            var resulting: NSURL?
            do {
                try FileManager.default.trashItem(at: url, resultingItemURL: &resulting)
                moved.append(
                    TrashedItem(
                        id: UUID(),
                        originalPath: path,
                        trashedPath: resulting?.path
                    )
                )
            } catch {
                errors.append("\(path): \(error.localizedDescription)")
            }
        }
        return (moved, errors)
    }

    static func deletePermanently(paths: [String]) -> (removed: [TrashedItem], errors: [String]) {
        var removed: [TrashedItem] = []
        var errors: [String] = []
        for path in paths {
            do {
                try FileManager.default.removeItem(atPath: path)
                removed.append(TrashedItem(id: UUID(), originalPath: path, trashedPath: nil))
            } catch {
                errors.append("\(path): \(error.localizedDescription)")
            }
        }
        return (removed, errors)
    }

    /// Move a trashed file back to its original path.
    static func putBack(_ item: TrashedItem) throws {
        guard let trashed = item.trashedPath else {
            throw CocoaError(.fileNoSuchFile)
        }
        let source = URL(fileURLWithPath: trashed)
        var destination = URL(fileURLWithPath: item.originalPath)
        if FileManager.default.fileExists(atPath: destination.path) {
            let folder = destination.deletingLastPathComponent()
            let stem = destination.deletingPathExtension().lastPathComponent
            let ext = destination.pathExtension
            let renamed = ext.isEmpty ? "\(stem) restored" : "\(stem) restored.\(ext)"
            destination = folder.appendingPathComponent(renamed)
        }
        try FileManager.default.moveItem(at: source, to: destination)
    }
}
