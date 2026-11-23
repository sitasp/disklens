import AppKit
import Foundation
import Observation

struct ProtectedPrompt {
    var title: String
    var message: String
    var confirm: () -> Void
}

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
    let folderAccess = FolderAccessStore()
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
    var acknowledgeProtectedRemoval = false
    var deletionAccessExpired = false
    var protectedPrompt: ProtectedPrompt?
    var showOnboarding: Bool

    private var didConsumeLaunchPath = false
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

    /// `open DiskLens.app --args /path/to/folder` scans that directory.
    /// The first existing directory argument wins. Anything else is ignored.
    func openInitialPathIfPresent() {
        guard !didConsumeLaunchPath else { return }
        didConsumeLaunchPath = true
        let path = CommandLine.arguments.dropFirst().first { candidate in
            var isDirectory: ObjCBool = false
            return FileManager.default.fileExists(atPath: candidate, isDirectory: &isDirectory)
                && isDirectory.boolValue
        }
        guard let path else { return }
        dismissOnboarding()
        beginScan(path: path, stayOnFilesystem: false)
        section = .map
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
            volumesError = Distribution.isAppStore
                ? "No volumes were visible inside the sandbox. Choose a folder to map it."
                : "No mounted volumes were visible. Grant Full Disk Access if disks you expect are missing."
        }
    }

    func chooseFolder(stayOnFilesystem: Bool, startingAt: String? = nil) {
        guard let url = openFolderPanel(
            prompt: "Scan",
            message: Distribution.isAppStore
                ? "Choose a folder. DiskLens remembers it so a later rescan does not ask again."
                : "Choose a folder or volume to map.",
            startingAt: startingAt
        ) else { return }
        if let problem = folderAccess.grantPickedFolder(url) {
            scanActivity = .failed(problem)
            section = .map
            return
        }
        beginScan(path: url.path, stayOnFilesystem: stayOnFilesystem || Distribution.isAppStore)
        section = .map
    }

    func chooseDuplicateFolder() {
        guard let url = openFolderPanel(
            prompt: "Compare",
            message: Distribution.isAppStore
                ? "Choose a folder to compare. Access is saved for the next search and for deletion."
                : "Choose a folder to search for duplicate files.",
            startingAt: nil
        ) else { return }
        if let problem = folderAccess.grantPickedFolder(url) {
            duplicateActivity = .failed(problem)
            return
        }
        beginDuplicates(path: url.path)
    }

    func beginScan(path: String, stayOnFilesystem: Bool) {
        if let problem = folderAccess.prepareToRead(path) {
            scanActivity = .failed(problem)
            return
        }
        scanCancel?.cancel()
        scanGeneration += 1
        let generation = scanGeneration
        let flag = CancelFlag()
        scanCancel = flag
        let box = UncheckedSendable(value: self)
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
        let model = UncheckedSendable(value: self)
        Task.detached(priority: .userInitiated) {
            let outcome: Result<ScanSession, Error>
            do {
                outcome = .success(try scanFolder(path: path, options: options, cancel: flag, progress: relay))
            } catch {
                outcome = .failure(error)
            }
            await MainActor.run {
                guard model.value.scanGeneration == generation else { return }
                switch outcome {
                case .success(let session):
                    model.value.adoptScan(session, root: path)
                case .failure(let error):
                    model.value.finishScanFailure(error)
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
        if let problem = folderAccess.prepareToRead(path) {
            duplicateActivity = .failed(problem)
            return
        }
        duplicateCancel?.cancel()
        duplicateGeneration += 1
        let generation = duplicateGeneration
        let flag = CancelFlag()
        duplicateCancel = flag
        let box = UncheckedSendable(value: self)
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
        let model = UncheckedSendable(value: self)
        Task.detached(priority: .userInitiated) {
            let outcome: Result<DuplicateSession, Error>
            do {
                outcome = .success(try findDuplicateFiles(path: path, options: options, cancel: flag, progress: relay))
            } catch {
                outcome = .failure(error)
            }
            await MainActor.run {
                guard model.value.duplicateGeneration == generation else { return }
                switch outcome {
                case .success(let found):
                    model.value.adoptDuplicates(found, root: path)
                case .failure(let error):
                    model.value.finishDuplicateFailure(error)
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

    func requestDuplicateSelection(group: Int, file: Int, select: Bool) {
        guard decisions.indices.contains(group), decisions[group].files.indices.contains(file) else { return }
        let item = decisions[group].files[file]
        if select && item.protected && !item.selectedForRemoval {
            let reason = item.protectionReason ?? "It is in a protected location."
            let name = DiskPaths.displayName(item.path)
            protectedPrompt = ProtectedPrompt(
                title: "This file is protected",
                message: "\(name) is protected: \(reason) Smart selection leaves it alone. You can still select it. The review screen lists protected items again and asks you to confirm before anything is removed.",
                confirm: { [weak self] in
                    self?.setDuplicateSelected(group: group, file: file, selected: true)
                }
            )
            return
        }
        setDuplicateSelected(group: group, file: file, selected: select)
    }

    func proposeLargeFileSelection(_ proposed: Set<String>) {
        let added = proposed.subtracting(selectedLargePaths)
        var blocked: [(path: String, reason: String)] = []
        for path in added.sorted() {
            if let reason = explainProtection(path: path, policy: settings.policy()) {
                blocked.append((path, reason))
            }
        }
        if blocked.isEmpty {
            selectedLargePaths = proposed
            return
        }
        selectedLargePaths = proposed.subtracting(Set(blocked.map(\.path)))
        let details = blocked.map { "\(DiskPaths.displayName($0.path)): \($0.reason)" }.joined(separator: "\n")
        let count = blocked.count
        protectedPrompt = ProtectedPrompt(
            title: count == 1 ? "This file is protected" : "These files are protected",
            message: "\(details)\n\nSmart selection leaves protected locations alone. You can still select \(count == 1 ? "this file" : "these files"). The review screen lists them again and asks you to confirm before anything is removed.",
            confirm: { [weak self] in
                self?.selectedLargePaths.formUnion(blocked.map(\.path))
            }
        )
    }

    func confirmProtectedPrompt() {
        let action = protectedPrompt?.confirm
        protectedPrompt = nil
        action?()
    }

    func setDuplicateSelected(group: Int, file: Int, selected: Bool) {
        guard decisions.indices.contains(group), decisions[group].files.indices.contains(file) else { return }
        decisions[group].files[file].selectedForRemoval = selected
        if selected {
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
        review = reviewDeletion(paths: unique, policy: settings.policy(), includeProtected: true)
        reviewPermanent = false
        reviewPhrase = ""
        reviewError = nil
        acknowledgeProtectedRemoval = false
        deletionAccessExpired = false
    }

    func grantDeletionAccess() {
        guard let url = openFolderPanel(
            prompt: "Grant Access",
            message: "Choose the folder that contains the items you want to remove.",
            startingAt: nil
        ) else { return }
        if let problem = folderAccess.grantPickedFolder(url) {
            reviewError = problem
            deletionAccessExpired = true
            return
        }
        guard let review else { return }
        if let problem = folderAccess.prepareToDelete(review.entries.map(\.path)) {
            reviewError = problem
            deletionAccessExpired = true
        } else {
            reviewError = nil
            deletionAccessExpired = false
        }
    }

    func confirmMoveToTrash() {
        guard let review else { return }
        let paths = review.entries.map(\.path)
        if let problem = folderAccess.prepareToDelete(paths) {
            reviewError = problem
            deletionAccessExpired = true
            return
        }
        deletionAccessExpired = false
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
        if let problem = folderAccess.prepareToDelete(paths) {
            reviewError = problem
            deletionAccessExpired = true
            return
        }
        deletionAccessExpired = false
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

    private func openFolderPanel(prompt: String, message: String, startingAt: String?) -> URL? {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.prompt = prompt
        panel.message = message
        if let startingAt {
            panel.directoryURL = URL(fileURLWithPath: startingAt)
        }
        guard panel.runModal() == .OK else { return nil }
        return panel.url
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
