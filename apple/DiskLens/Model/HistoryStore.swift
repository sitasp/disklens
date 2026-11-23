import Foundation
import Observation

struct TrashedItem: Codable, Equatable, Identifiable {
    var id: UUID
    var originalPath: String
    var trashedPath: String?
}

struct HistoryEntry: Codable, Equatable, Identifiable {
    var id: UUID
    var createdAt: Date
    var byteEstimate: UInt64
    var permanent: Bool
    var undone: Bool
    var note: String
    var items: [TrashedItem]
}

/// A JSON log of deletions so Trash moves can be put back.
@MainActor
@Observable
final class HistoryStore {
    private(set) var entries: [HistoryEntry] = []
    private let fileURL: URL
    var lastError: String?

    init(directory: URL) {
        fileURL = directory.appendingPathComponent("history.json")
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        load()
    }

    convenience init() {
        self.init(directory: HistoryStore.defaultDirectory())
    }

    func record(items: [TrashedItem], byteEstimate: UInt64, permanent: Bool, note: String) {
        let entry = HistoryEntry(
            id: UUID(),
            createdAt: Date(),
            byteEstimate: byteEstimate,
            permanent: permanent,
            undone: false,
            note: note,
            items: items
        )
        entries.insert(entry, at: 0)
        save()
    }

    func markUndone(id: UUID) {
        guard let index = entries.firstIndex(where: { $0.id == id }) else { return }
        entries[index].undone = true
        save()
    }

    private func load() {
        guard let data = try? Data(contentsOf: fileURL) else { return }
        do {
            entries = try JSONDecoder().decode([HistoryEntry].self, from: data)
        } catch {
            lastError = error.localizedDescription
        }
    }

    private func save() {
        do {
            let data = try JSONEncoder().encode(entries)
            try data.write(to: fileURL, options: .atomic)
        } catch {
            lastError = error.localizedDescription
        }
    }

    static func defaultDirectory() -> URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSTemporaryDirectory())
        return base.appendingPathComponent("DiskLens", isDirectory: true)
    }
}
