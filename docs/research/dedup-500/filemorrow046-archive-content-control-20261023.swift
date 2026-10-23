import Foundation
import XCTest
@testable import FileMorrow

final class ArchiveContentControl: XCTestCase {
    func testSameSizeChangedContentIsReportedDespiteDifferentBytes() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root.appendingPathComponent("report"), withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let file = root.appendingPathComponent("report/a.txt")
        try Data("hello".utf8).write(to: file)
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/zip")
        process.arguments = ["-r", "-q", "-X", root.appendingPathComponent("report.zip").path, "report"]
        process.currentDirectoryURL = root
        try process.run()
        process.waitUntilExit()
        XCTAssertEqual(process.terminationStatus, 0)
        try Data("world".utf8).write(to: file)
        let found = await ExtractedArchiveScanner().scan(root: root, profile: TestProfiles.general)
        XCTAssertEqual(found.count, 1, "Reproduces size-only false positive, not correct equality")
        XCTAssertEqual(found.first?.extractedSize, 5)
        let actual = try Data(contentsOf: file)
        XCTAssertNotEqual(actual, Data("hello".utf8))
    }
}
