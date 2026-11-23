import XCTest
@testable import DiskLens

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

    func testDisplayNameOfRoot() {
        XCTAssertEqual(DiskPaths.displayName("/"), "/")
        XCTAssertEqual(DiskPaths.displayName("/tmp/photos"), "photos")
    }

    func testPermanentDeleteRequiresTheExactWord() {
        XCTAssertTrue(PermanentDeleteGate.accepts("DELETE"))
        XCTAssertTrue(PermanentDeleteGate.accepts("  DELETE\n"))
        XCTAssertTrue(PermanentDeleteGate.accepts("DELETE "))
        XCTAssertFalse(PermanentDeleteGate.accepts("delete"))
        XCTAssertFalse(PermanentDeleteGate.accepts("DELETE please"))
    }

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
