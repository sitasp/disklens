import XCTest
#if DISKLENS_SANDBOX
@testable import DiskLensStore
#else
@testable import DiskLens
#endif

final class SettingsStoreTests: XCTestCase {
    private var suiteName = ""

    override func tearDown() {
        if !suiteName.isEmpty {
            UserDefaults().removePersistentDomain(forName: suiteName)
        }
        super.tearDown()
    }

    @MainActor
    func testPreferencesRoundTripInAPrivateSuite() {
        suiteName = "DiskLensTests.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        let store = SettingsStore(defaults: defaults)
        store.keepStrategy = .oldest
        store.preferredDirectory = "  /tmp/keep  "
        store.protectLibrary = false
        store.protectSystem = true
        store.minimumDuplicateSize = 4096
        store.followSymlinks = true
        store.includeHidden = false
        store.detectClones = false

        let reloaded = SettingsStore(defaults: defaults)
        XCTAssertEqual(reloaded.keepStrategy, .oldest)
        XCTAssertEqual(reloaded.preferredDirectory, "  /tmp/keep  ")
        XCTAssertFalse(reloaded.protectLibrary)
        XCTAssertEqual(reloaded.minimumDuplicateSize, 4096)
        XCTAssertTrue(reloaded.followSymlinks)
        XCTAssertFalse(reloaded.includeHidden)
        XCTAssertFalse(reloaded.detectClones)

        let policy = reloaded.policy()
        XCTAssertEqual(policy.strategy, .oldest)
        XCTAssertEqual(policy.preferredDirectory, "/tmp/keep")
        XCTAssertFalse(policy.protectLibrary)
        XCTAssertTrue(policy.protectSystem)

        let options = reloaded.scanOptions(stayOnFilesystem: true)
        XCTAssertTrue(options.stayOnFilesystem)
        XCTAssertTrue(options.followSymlinks)
        XCTAssertFalse(options.includeHidden)
        XCTAssertFalse(options.detectClones)

        reloaded.preferredDirectory = "   "
        XCTAssertNil(reloaded.policy().preferredDirectory)
    }
}
