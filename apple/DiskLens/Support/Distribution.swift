import Foundation

/// Which DiskLens product is running.
///
/// The direct-distribution target is unsandboxed. The App Store target is
/// built with `DISKLENS_SANDBOX` and can only read folders the user picks.
enum Distribution {
    static var isAppStore: Bool {
        #if DISKLENS_SANDBOX
        true
        #else
        false
        #endif
    }
}
