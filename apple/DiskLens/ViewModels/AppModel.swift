import AppKit
import Foundation
import Observation

enum Activity: Equatable {
    case idle
    case running
    case ready
    case failed(String)
    case cancelled
}

struct VolumeInfo: Identifiable, Equatable {
    var id: String { path }
    var name: String
    var path: String
    var removable: Bool
    var usage: VolumeUsageRecord?
    var error: String?
}

enum OutlineSort: String, CaseIterable, Identifiable {
    case size
    case name
    var id: String { rawValue }
    var title: String { self == .size ? "Size" : "Name" }
}

enum AgeFilter: String, CaseIterable, Identifiable {
    case any
    case month
    case halfYear
    case year
    var id: String { rawValue }
    var title: String {
        switch self {
        case .any: return "Any age"
        case .month: return "Older than a month"
        case .halfYear: return "Older than 6 months"
        case .year: return "Older than a year"
        }
    }

    var modifiedBefore: Int64? {
        let days: Int
        switch self {
        case .any: return nil
        case .month: days = 30
        case .halfYear: days = 182
        case .year: days = 365
        }
        let cutoff = Date().addingTimeInterval(TimeInterval(-days * 24 * 60 * 60))
        return Int64(cutoff.timeIntervalSince1970 * 1000)
    }
}

enum SizeFloor: UInt64, CaseIterable, Identifiable {
    case tenMB = 10_000_000
    case hundredMB = 100_000_000
    case oneGB = 1_000_000_000
    var id: UInt64 { rawValue }
    var title: String {
        switch self {
        case .tenMB: return "10 MB and up"
        case .hundredMB: return "100 MB and up"
        case .oneGB: return "1 GB and up"
        }
    }
}

@MainActor
@Observable
final class AppModel {
    var section: SidebarSection = .overview
    var settings: SettingsStore
    var history: HistoryStore
    let quickLook = QuickLookPresenter()

    var volumes: [VolumeInfo] = []
    var volumesLoading = false
    var volumesError: String?

    var scanActivity: Activity = .idle
    var scanProgress: ProgressUpdate?
    var session: ScanSession?
    var scanRoot: String?
    var currentPath: String?
    var metric: SizeMetric = .uniqueApparent {
        didSet { reloadMap() }
    }
    var visualization: MapVisualization = .sunburst
    var hovered: NodeSummary?
    var outlineSort: OutlineSort = .size
    var children: [NodeSummary] = []
    var arcs: [ArcRecord] = []
    var rects: [RectRecord] = []
    var categories: [CategoryRow] = []
    var largeFiles: [FileRow] = []
    var sizeFloor: SizeFloor = .hundredMB {
        didSet { reloadLargeFiles() }
    }
    var ageFilter: AgeFilter = .any {
        didSet { reloadLargeFiles() }
    }
    var selectedLargePaths: Set<String> = []

    var duplicateActivity: Activity = .idle
    var duplicateProgress: ProgressUpdate?
    var duplicateRoot: String?
    var groups: [DuplicateGroupRecord] = []
    var decisions: [GroupDecisionRecord] = []
    var unreadableCount = 0
    var duplicateLoaded = 0
    var duplicateTotal = 0

    var review: DeletionPlanRecord?
    var reviewPermanent = false
    var reviewPhrase = ""
    var reviewError: String?
    var showOnboarding: Bool

    private var scanCancel: CancelFlag?
    private var scanRelay: ProgressRelay?
    private var duplicateCancel: CancelFlag?
    private var duplicateRelay: ProgressRelay?
    private var scanGeneration = 0
    private var duplicateGeneration = 0

    init(settings: SettingsStore, history: HistoryStore) {
        self.settings = settings
        self.history = history
        showOnboarding = !settings.didFinishOnboarding
    }

    convenience init() {
        self.init(settings: SettingsStore(), history: HistoryStore())
    }

    var displayedChildren: [NodeSummary] {
        switch outlineSort {
        case .size:
            return children
        case .name:
            return children.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
        }
    }

    var crumbs: [(title: String, path: String)] {
        guard let scanRoot, let currentPath else { return [] }
        return DiskPaths.crumbs(root: scanRoot, current: currentPath)
    }

    func dismissOnboarding() {
        settings.didFinishOnboarding = true
        showOnboarding = false
    }

    func openFullDiskAccessSettings() {
        let candidates = [
            "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_AllFiles",
            "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles",
        ]
        for raw in candidates {
            if let url = URL(string: raw), NSWorkspace.shared.open(url) {
                return
            }
        }
    }

    func refreshVolumes() {
        volumesLoading = true
        volumesError = nil
        let urls = FileManager.default.mountedVolumeURLs(
            includingResourceValuesForKeys: [.volumeNameKey, .volumeIsRemovableKey, .volumeIsLocalKey],
            options: [.skipHiddenVolumes]
        ) ?? []
        var next: [VolumeInfo] = []
        for url in urls {
            let values = try? url.resourceValues(forKeys: [.volumeNameKey, .volumeIsRemovableKey])
            let path = url.path
            var info = VolumeInfo(
                name: values?.volumeName ?? DiskPaths.displayName(path),
                path: path,
                removable: values?.volumeIsRemovable ?? false,
                usage: nil,
                error: nil
            )
            do {
                info.usage = try volumeUsage(path: path)
            } catch {
                info.error = ScanErrorText.message(for: error)
            }
            next.append(info)
        }
        volumes = next.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
        volumesLoading = false
        if volumes.isEmpty {
            volumesError = "No mounted volumes were visible. Grant Full Disk Access if disks you expect are missing."
        }
    }

    func chooseFolder(stayOnFilesystem: Bool) {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.prompt = "Scan"
        panel.message = "Choose a folder or volume to map."
        guard panel.runModal() == .OK, let url = panel.url else { return }
        beginScan(path: url.path, stayOnFilesystem: stayOnFilesystem)
        section = .map
    }

    func beginScan(path: String, stayOnFilesystem: Bool) {
        scanCancel?.cancel()
        scanGeneration += 1
        let generation = scanGeneration
        let flag = CancelFlag()
        scanCancel = flag
        let box = UncheckedSendable(self)
        let relay = ProgressRelay { update in
            Task { @MainActor in
                box.value.scanProgress = update
            }
        }
        scanRelay = relay
        scanActivity = .running
        scanProgress = nil
        hovered = nil
        let options = settings.scanOptions(stayOnFilesystem: stayOnFilesystem)
        Task.detached(priority: .userInitiated) { [weak self] in
            do {
                let session = try scanFolder(path: path, options: options, cancel: flag, progress: relay)
                await MainActor.run {
                    guard let self, self.scanGeneration == generation else { return }
                    self.adoptScan(session, root: path)
                }
            } catch {
                await MainActor.run {
                    guard let self, self.scanGeneration == generation else { return }
                    self.finishScanFailure(error)
                }
            }
        }
    }

    func cancelScan() {
        scanCancel?.cancel()
    }

    func drill(to path: String) {
        guard session?.node(path: path) != nil else { return }
        currentPath = path
        hovered = nil
        reloadMap()
    }

    func goUp() {
        guard let scanRoot, let currentPath, let parent = DiskPaths.parent(root: scanRoot, current: currentPath) else {
            return
        }
        drill(to: parent)
    }

    func updateTreemap(width: Double, height: Double) {
        guard let session, let currentPath, width > 1, height > 1 else {
            rects = []
            return
        }
        rects = session.treemap(
            path: currentPath,
            metric: metric,
            width: width,
            height: height,
            maxDepth: 2,
            padding: 3
        )
    }

    func reloadLargeFiles() {
        guard let session else {
            largeFiles = []
            return
        }
        largeFiles = session.files(
            query: FileQueryRecord(
                minApparentSize: sizeFloor.rawValue,
                modifiedBeforeUnixMs: ageFilter.modifiedBefore,
                modifiedAfterUnixMs: nil,
                category: nil,
                limit: 300,
                sort: .largest
            )
        )
        selectedLargePaths = selectedLargePaths.filter { path in
            largeFiles.contains { $0.path == path }
        }
    }

    func reveal(_ path: String) {
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
    }

    func beginDuplicates(path: String) {
        duplicateCancel?.cancel()
        duplicateGeneration += 1
        let generation = duplicateGeneration
        let flag = CancelFlag()
        duplicateCancel = flag
        let box = UncheckedSendable(self)
        let relay = ProgressRelay { update in
            Task { @MainActor in
                box.value.duplicateProgress = update
            }
        }
        duplicateRelay = relay
        duplicateActivity = .running
        duplicateProgress = nil
        duplicateRoot = path
        let options = settings.duplicateOptions()
        Task.detached(priority: .userInitiated) { [weak self] in
            do {
                let found = try findDuplicateFiles(path: path, options: options, cancel: flag, progress: relay)
                await MainActor.run {
                    guard let self, self.duplicateGeneration == generation else { return }
                    self.adoptDuplicates(found, root: path)
                }
            } catch {
                await MainActor.run {
                    guard let self, self.duplicateGeneration == generation else { return }
                    self.finishDuplicateFailure(error)
                }
            }
        }
    }

    func cancelDuplicates() {
        duplicateCancel?.cancel()
    }

    func loadMoreDuplicates() {
        guard let duplicateSession = currentDuplicateSession else { return }
        let start = groups.count
        let next = duplicateSession.groups(offset: UInt32(start), limit: 100)
        guard !next.isEmpty else { return }
        let fresh = duplicateSession.applySelection(policy: settings.policy())
        groups.append(contentsOf: next)
        duplicateLoaded = groups.count
        if fresh.count >= groups.count {
            decisions.append(contentsOf: fresh[start..<groups.count])
        }
    }

    func applySmartSelection() {
        guard let duplicateSession = currentDuplicateSession else { return }
        let fresh = duplicateSession.applySelection(policy: settings.policy())
        decisions = Array(fresh.prefix(groups.count))
    }

    func toggleDuplicate(group: Int, file: Int) {
        guard decisions.indices.contains(group), decisions[group].files.indices.contains(file) else { return }
        if decisions[group].files[file].protected { return }
        decisions[group].files[file].selectedForRemoval.toggle()
        if decisions[group].files[file].selectedForRemoval {
            decisions[group].files[file].keeper = false
        }
    }

    func selectedDuplicatePaths() -> [String] {
        decisions.flatMap { decision in
            decision.files.filter(\.selectedForRemoval).map(\.path)
        }
    }

    func reviewPaths(_ paths: [String]) {
        let unique = Array(Set(paths)).sorted()
        guard !unique.isEmpty else { return }
        review = reviewDeletion(paths: unique, policy: settings.policy(), allowProtected: false)
        reviewPermanent = false
        reviewPhrase = ""
        reviewError = nil
    }

    func confirmMoveToTrash() {
        guard let review else { return }
        let paths = review.entries.map(\.path)
        let outcome = TrashService.moveToTrash(paths: paths)
        history.record(
            items: outcome.moved,
            byteEstimate: review.reclaimableBytes,
            permanent: false,
            note: outcome.errors.isEmpty ? "Moved to Trash" : "Moved to Trash with errors"
        )
        reviewError = outcome.errors.isEmpty ? nil : outcome.errors.joined(separator: "\n")
        if outcome.errors.isEmpty {
            self.review = nil
            section = .history
        }
    }

    func confirmPermanentDelete() {
        guard let review, PermanentDeleteGate.accepts(reviewPhrase) else {
            reviewError = "Type DELETE to confirm permanent removal. This cannot be undone."
            return
        }
        let paths = review.entries.map(\.path)
        let outcome = TrashService.deletePermanently(paths: paths)
        history.record(
            items: outcome.removed,
            byteEstimate: review.reclaimableBytes,
            permanent: true,
            note: "Deleted permanently"
        )
        reviewError = outcome.errors.isEmpty ? nil : outcome.errors.joined(separator: "\n")
        if outcome.errors.isEmpty {
            self.review = nil
            section = .history
        }
    }

    func putBack(_ entry: HistoryEntry) async {
        var failures: [String] = []
        for item in entry.items where item.trashedPath != nil {
            do {
                try TrashService.putBack(item)
            } catch {
                failures.append("\(item.originalPath): \(error.localizedDescription)")
            }
        }
        if failures.isEmpty {
            history.markUndone(id: entry.id)
        } else {
            history.lastError = failures.joined(separator: "\n")
        }
    }

    func quickLook(paths: [String]) {
        quickLook.present(paths: paths)
    }

    private var currentDuplicateSession: DuplicateSession?

    private func adoptScan(_ session: ScanSession, root: String) {
        self.session = session
        scanRoot = root
        currentPath = root
        scanActivity = .ready
        reloadMap()
        reloadLargeFiles()
    }

    private func finishScanFailure(_ error: Error) {
        if let scan = error as? ScanError, case .Cancelled = scan {
            scanActivity = .cancelled
            return
        }
        scanActivity = .failed(ScanErrorText.message(for: error))
    }

    private func adoptDuplicates(_ found: DuplicateSession, root: String) {
        currentDuplicateSession = found
        duplicateRoot = root
        duplicateTotal = Int(found.groupCount())
        unreadableCount = found.unreadable().count
        groups = found.groups(offset: 0, limit: 100)
        duplicateLoaded = groups.count
        decisions = Array(found.applySelection(policy: settings.policy()).prefix(groups.count))
        duplicateActivity = .ready
    }

    private func finishDuplicateFailure(_ error: Error) {
        if let scan = error as? ScanError, case .Cancelled = scan {
            duplicateActivity = .cancelled
            return
        }
        duplicateActivity = .failed(ScanErrorText.message(for: error))
    }

    private func reloadMap() {
        guard let session, let currentPath else {
            children = []
            arcs = []
            rects = []
            categories = []
            return
        }
        children = session.children(path: currentPath)
        arcs = session.sunburst(path: currentPath, metric: metric, maxDepth: 2, minFraction: 0.004)
        categories = session.categories()
    }
}
