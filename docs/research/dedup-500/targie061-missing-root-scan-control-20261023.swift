import Foundation
import XCTest
@testable import SimilarVideoFinder
final class MissingRootScanControl: XCTestCase {
 func testBothCompleteScannersAcceptMissingRootAsEmptyWithoutIssues() async throws {
 let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
 try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
 defer { try? FileManager.default.removeItem(at: root) }
 let missing = root.appendingPathComponent("missing")
 let thumbnails = ThumbnailStore(directoryURL: root.appendingPathComponent("thumbnails"))
 let images = try await ImageScanner(thumbnailStore: thumbnails).scan(folder: missing) { _ in }
 XCTAssertTrue(images.images.isEmpty)
 XCTAssertTrue(images.issues.isEmpty)
 let videos = try await VideoScanner(thumbnailStore: thumbnails).scan(folder: missing) { _ in }
 XCTAssertTrue(videos.videos.isEmpty)
 XCTAssertTrue(videos.issues.isEmpty)
 XCTAssertFalse(FileManager.default.fileExists(atPath: missing.path))
 }
}
