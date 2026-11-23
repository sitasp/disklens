import XCTest
#if DISKLENS_SANDBOX
@testable import DiskLensStore
#else
@testable import DiskLens
#endif

final class ReleaseAndGeometryTests: XCTestCase {
    func testStableChannelIsOmittedFromTheLabel() {
        XCTAssertEqual(AppRelease.normalizedChannel("stable"), "")
        XCTAssertEqual(AppRelease.normalizedChannel(nil), "")
        XCTAssertEqual(AppRelease.normalizedChannel("alpha"), "alpha")
        XCTAssertEqual(AppRelease.label(marketing: "0.1.0", channel: ""), "0.1.0")
        XCTAssertEqual(AppRelease.label(marketing: "0.1.0", channel: "alpha"), "0.1.0 alpha")
        XCTAssertEqual(AppRelease.label(marketing: "0.2.0", channel: "beta"), "0.2.0 beta")
    }

    func testReleaseNoteMatchesTheChannel() {
        XCTAssertTrue(AppRelease.note(marketing: "0.1.0", build: "1", channel: "alpha").contains("alpha"))
        XCTAssertTrue(AppRelease.note(marketing: "0.1.0", build: "1", channel: "beta").contains("beta"))
        XCTAssertEqual(
            AppRelease.note(marketing: "1.0.0", build: "4", channel: ""),
            "DiskLens 1.0.0 (4)."
        )
    }

    func testAngleContainsTheSliceAndRejectsTheRest() {
        XCTAssertTrue(angleContains(0.2, start: 0, end: 0.5))
        XCTAssertFalse(angleContains(1, start: 0, end: 0.5))
        XCTAssertTrue(angleContains(-0.1, start: -0.2, end: 0.2))
        XCTAssertTrue(angleContains(0.5, start: 0, end: 0.5))
    }
}
