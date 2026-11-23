import XCTest
#if DISKLENS_SANDBOX
@testable import DiskLensStore
#else
@testable import DiskLens
#endif

final class PathAndHistoryTests: XCTestCase {
    func testCrumbsStayInsideTheScanRoot() {
        let crumbs = DiskPaths.crumbs(root: "/Users/ada/Projects", current: "/Users/ada/Projects/app/src")
        XCTAssertEqual(crumbs.map(\.path), [
            "/Users/ada/Projects",
            "/Users/ada/Projects/app",
            "/Users/ada/Projects/app/src",
        ])
        XCTAssertEqual(crumbs.map(\.title), ["Projects", "app", "src"])
        XCTAssertEqual(DiskPaths.parent(root: "/Users/ada/Projects", current: "/Users/ada/Projects/app/src"), "/Users/ada/Projects/app")
        XCTAssertNil(DiskPaths.parent(root: "/Users/ada/Projects", current: "/Users/ada/Projects"))
    }

    func testBroadSystemLocationsAreExact() {
        XCTAssertTrue(DiskPaths.isBroadSystemLocation("/Applications"))
        XCTAssertTrue(DiskPaths.isBroadSystemLocation("/Applications/"))
        XCTAssertTrue(DiskPaths.isBroadSystemLocation("/"))
        XCTAssertTrue(DiskPaths.isBroadSystemLocation("/Library"))
        XCTAssertFalse(DiskPaths.isBroadSystemLocation("/Applications/Xcode.app"))
        XCTAssertFalse(DiskPaths.isBroadSystemLocation("/Users/ada/Documents"))
        XCTAssertFalse(DiskPaths.isBroadSystemLocation("/Applications-old"))
    }

    func testDisplayNameOfRoot() {
        XCTAssertEqual(DiskPaths.displayName("/"), "/")
        XCTAssertEqual(DiskPaths.displayName("/tmp/photos"), "photos")
    }

    func testSavedFolderCoveringUsesAPathBoundary() {
        let docs = StoredBookmark(id: UUID(), path: "/Users/ada/Documents", name: "Documents", bookmark: Data())
        let other = StoredBookmark(id: UUID(), path: "/Users/ada/Documents-old", name: "Old", bookmark: Data())
        let nested = StoredBookmark(id: UUID(), path: "/Users/ada/Documents/Taxes", name: "Taxes", bookmark: Data())
        let match = FolderAccess.bestCovering(
            path: "/Users/ada/Documents/Taxes/2024/return.pdf",
            folders: [docs, other, nested]
        )
        XCTAssertEqual(match?.id, nested.id)
        XCTAssertNil(FolderAccess.bestCovering(path: "/Users/ada/Desktop/photo.jpg", folders: [docs, other]))
        XCTAssertFalse(FolderAccess.covers(root: "/Users/ada/Documents", path: "/Users/ada/Documents-old/file"))
        XCTAssertTrue(FolderAccess.covers(root: "/Users/ada/Documents", path: "/Users/ada/Documents"))
    }

    func testProtectedRemovalWaitsForAcknowledgement() {
        XCTAssertTrue(ProtectedRemovalGate.allowsDeletion(hasProtectedItems: false, acknowledged: false))
        XCTAssertFalse(ProtectedRemovalGate.allowsDeletion(hasProtectedItems: true, acknowledged: false))
        XCTAssertTrue(ProtectedRemovalGate.allowsDeletion(hasProtectedItems: true, acknowledged: true))
    }

    func testPermanentDeleteRequiresTheExactWord() {
        XCTAssertTrue(PermanentDeleteGate.accepts("DELETE"))
        XCTAssertTrue(PermanentDeleteGate.accepts("  DELETE\n"))
        XCTAssertTrue(PermanentDeleteGate.accepts("DELETE "))
        XCTAssertFalse(PermanentDeleteGate.accepts("delete"))
        XCTAssertFalse(PermanentDeleteGate.accepts("DELETE please"))
    }

    @MainActor
    func testHistoryRoundTrip() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let store = HistoryStore(directory: directory)
        let item = TrashedItem(id: UUID(), originalPath: "/Users/ada/old.zip", trashedPath: "/Users/ada/.Trash/old.zip")
        store.record(items: [item], byteEstimate: 42, permanent: false, note: "Moved to Trash")
        let reloaded = HistoryStore(directory: directory)
        XCTAssertEqual(reloaded.entries.count, 1)
        XCTAssertEqual(reloaded.entries[0].items, [item])
        XCTAssertEqual(reloaded.entries[0].byteEstimate, 42)
        XCTAssertFalse(reloaded.entries[0].undone)
        reloaded.markUndone(id: reloaded.entries[0].id)
        let again = HistoryStore(directory: directory)
        XCTAssertTrue(again.entries[0].undone)
    }

    func testKeepStrategyStorage() {
        XCTAssertEqual(KeepStrategyStorage.decode("oldest"), .oldest)
        XCTAssertEqual(KeepStrategyStorage.decode("preferred"), .preferredDirectory)
        XCTAssertEqual(KeepStrategyStorage.decode("shortest"), .shortestPath)
        XCTAssertEqual(KeepStrategyStorage.decode("original"), .mostOriginalName)
        XCTAssertEqual(KeepStrategyStorage.decode("anything-else"), .newest)
        XCTAssertEqual(KeepStrategyStorage.encode(.shortestPath), "shortest")
    }
}
