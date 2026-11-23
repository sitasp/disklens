import Foundation

/// Version shown in Settings. Finder and the bundle use the three-number
/// marketing version. The channel (alpha, beta) comes from the build.
enum AppRelease {
    static func normalizedChannel(_ raw: String?) -> String {
        let value = raw ?? ""
        return value == "stable" ? "" : value
    }

    static func label(marketing: String, channel: String) -> String {
        channel.isEmpty ? marketing : "\(marketing) \(channel)"
    }

    static func note(marketing: String, build: String, channel: String) -> String {
        switch channel {
        case "alpha":
            return "This is an alpha build, signed to run on a Mac without notarization. Open it from the Finder shortcut menu the first time."
        case "beta":
            return "This is a beta build. Open it from the Finder shortcut menu the first time if macOS cannot verify the developer."
        default:
            return "DiskLens \(marketing) (\(build))."
        }
    }

    static var marketing: String {
        string("CFBundleShortVersionString") ?? "0.1.0"
    }

    static var build: String {
        string("CFBundleVersion") ?? "1"
    }

    static var channel: String {
        normalizedChannel(string("DiskLensReleaseChannel"))
    }

    static var label: String {
        label(marketing: marketing, channel: channel)
    }

    static var note: String {
        note(marketing: marketing, build: build, channel: channel)
    }

    private static func string(_ key: String) -> String? {
        Bundle.main.object(forInfoDictionaryKey: key) as? String
    }
}
