import Foundation
import XCTest
@testable import SimilarVideoFinder
final class CacheStorageFailureControl: XCTestCase {
 private func directory() throws -> URL {
 let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
 try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
 return root
 }
 func testParentRegularFileRejectsDatabaseCreationAndPreservesBytes() throws {
 let root = try directory()
 defer { try? FileManager.default.removeItem(at: root) }
 let parent = root.appendingPathComponent("parent")
 let marker = Data("owned-parent-marker".utf8)
 try marker.write(to: parent)
 XCTAssertThrowsError(try HashCache(databaseURL: parent.appendingPathComponent("cache.sqlite")))
 XCTAssertEqual(try Data(contentsOf: parent), marker)
 }
 func testDirectoryAtDatabasePathIsRejectedAndChildPreserved() throws {
 let root = try directory()
 defer { try? FileManager.default.removeItem(at: root) }
 let path = root.appendingPathComponent("cache.sqlite")
 try FileManager.default.createDirectory(at: path, withIntermediateDirectories: true)
 let child = path.appendingPathComponent("marker")
 let bytes = Data("owned-directory-marker".utf8)
 try bytes.write(to: child)
 XCTAssertThrowsError(try HashCache(databaseURL: path))
 XCTAssertEqual(try Data(contentsOf: child), bytes)
 }
 func testCorruptDatabaseIsRejectedWithoutReplacingOriginalBytes() throws {
 let root = try directory()
 defer { try? FileManager.default.removeItem(at: root) }
 let path = root.appendingPathComponent("cache.sqlite")
 let bytes = Data(repeating: 0xA5, count: 4096)
 try bytes.write(to: path)
 XCTAssertThrowsError(try HashCache(databaseURL: path))
 XCTAssertEqual(try Data(contentsOf: path), bytes)
 }
}
