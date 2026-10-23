import Foundation
import XCTest
@testable import SimilarVideoFinder
final class CorruptVideoExactControl: XCTestCase {
 func testIdenticalUndecodableVideosThrowBeforeExactGrouping() async throws {
 let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
 try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
 defer { try? FileManager.default.removeItem(at: root) }
 let urls = [root.appendingPathComponent("first.mp4"), root.appendingPathComponent("second.mp4")]
 for url in urls { try Data([1, 2, 3, 4]).write(to: url) }
 let firstDigest = try await FileHasher.sha256(of: urls[0])
 let secondDigest = try await FileHasher.sha256(of: urls[1])
 XCTAssertEqual(firstDigest, secondDigest)
 let videos = urls.map { MediaItem(kind: .video, url: $0, fileSize: 4, duration: 1, width: 640, height: 480, modifiedAt: nil, thumbnailData: nil) }
 do {
 _ = try await SimilarityPipeline().process(videos: videos, threshold: 0.88) { _ in }
 XCTFail("Original native pipeline was expected to throw for undecodable pHash input")
 } catch is CancellationError {
 XCTFail("No task was cancelled")
 } catch {
 // Reproduction of failure despite equal readable encoded bytes.
 XCTAssertFalse(error is CancellationError)
 }
 }
}
