import Foundation
import Observation

/// Preferences stored in UserDefaults. Pass a suite in tests so they do not
/// touch the real defaults database.
@MainActor
@Observable
final class SettingsStore {
    private let defaults: UserDefaults

    var keepStrategy: KeepStrategy {
        didSet { defaults.set(KeepStrategyStorage.encode(keepStrategy), forKey: Key.strategy) }
    }

    var preferredDirectory: String {
        didSet { defaults.set(preferredDirectory, forKey: Key.preferred) }
    }

    var protectSystem: Bool {
        didSet { defaults.set(protectSystem, forKey: Key.protectSystem) }
    }

    var protectLibrary: Bool {
        didSet { defaults.set(protectLibrary, forKey: Key.protectLibrary) }
    }

    var protectPackages: Bool {
        didSet { defaults.set(protectPackages, forKey: Key.protectPackages) }
    }

    var extraProtectedPrefixes: [String] {
        didSet { defaults.set(extraProtectedPrefixes, forKey: Key.extraPrefixes) }
    }

    var minimumDuplicateSize: UInt64 {
        didSet { defaults.set(String(minimumDuplicateSize), forKey: Key.minSize) }
    }

    var followSymlinks: Bool {
        didSet { defaults.set(followSymlinks, forKey: Key.follow) }
    }

    var includeHidden: Bool {
        didSet { defaults.set(includeHidden, forKey: Key.hidden) }
    }

    var detectClones: Bool {
        didSet { defaults.set(detectClones, forKey: Key.clones) }
    }

    var didFinishOnboarding: Bool {
        didSet { defaults.set(didFinishOnboarding, forKey: Key.onboarding) }
    }

    init(defaults: UserDefaults) {
        self.defaults = defaults
        keepStrategy = KeepStrategyStorage.decode(defaults.string(forKey: Key.strategy) ?? "newest")
        preferredDirectory = defaults.string(forKey: Key.preferred) ?? ""
        protectSystem = defaults.object(forKey: Key.protectSystem) as? Bool ?? true
        protectLibrary = defaults.object(forKey: Key.protectLibrary) as? Bool ?? true
        protectPackages = defaults.object(forKey: Key.protectPackages) as? Bool ?? true
        extraProtectedPrefixes = defaults.stringArray(forKey: Key.extraPrefixes) ?? []
        if let raw = defaults.string(forKey: Key.minSize), let value = UInt64(raw) {
            minimumDuplicateSize = value
        } else {
            minimumDuplicateSize = 1
        }
        followSymlinks = defaults.object(forKey: Key.follow) as? Bool ?? false
        includeHidden = defaults.object(forKey: Key.hidden) as? Bool ?? true
        detectClones = defaults.object(forKey: Key.clones) as? Bool ?? true
        didFinishOnboarding = defaults.object(forKey: Key.onboarding) as? Bool ?? false
    }

    convenience init() {
        self.init(defaults: .standard)
    }

    func scanOptions(stayOnFilesystem: Bool) -> ScanOptions {
        ScanOptions(
            followSymlinks: followSymlinks,
            includeHidden: includeHidden,
            detectClones: detectClones,
            stayOnFilesystem: stayOnFilesystem
        )
    }

    func duplicateOptions() -> DuplicateOptions {
        DuplicateOptions(
            minSize: minimumDuplicateSize,
            partialBytes: 65_536,
            followSymlinks: followSymlinks,
            includeHidden: includeHidden
        )
    }

    func policy() -> SelectionPolicyRecord {
        var record = defaultSelectionPolicy()
        record.strategy = keepStrategy
        let trimmed = preferredDirectory.trimmingCharacters(in: .whitespacesAndNewlines)
        record.preferredDirectory = trimmed.isEmpty ? nil : trimmed
        record.protectSystem = protectSystem
        record.protectLibrary = protectLibrary
        record.protectPackages = protectPackages
        record.extraProtectedPrefixes = extraProtectedPrefixes
        return record
    }

    private enum Key {
        static let strategy = "keepStrategy"
        static let preferred = "preferredDirectory"
        static let protectSystem = "protectSystem"
        static let protectLibrary = "protectLibrary"
        static let protectPackages = "protectPackages"
        static let extraPrefixes = "extraProtectedPrefixes"
        static let minSize = "minimumDuplicateSize"
        static let follow = "followSymlinks"
        static let hidden = "includeHidden"
        static let clones = "detectClones"
        static let onboarding = "didFinishOnboarding"
    }
}

enum KeepStrategyStorage {
    static func encode(_ strategy: KeepStrategy) -> String {
        switch strategy {
        case .newest: return "newest"
        case .oldest: return "oldest"
        case .preferredDirectory: return "preferred"
        case .shortestPath: return "shortest"
        case .mostOriginalName: return "original"
        }
    }

    static func decode(_ raw: String) -> KeepStrategy {
        switch raw {
        case "oldest": return .oldest
        case "preferred": return .preferredDirectory
        case "shortest": return .shortestPath
        case "original": return .mostOriginalName
        default: return .newest
        }
    }
}
