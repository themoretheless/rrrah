import Foundation
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers
import XCTest
@testable import SimilarVideoFinder
final class ThumbnailRepairControl: XCTestCase {
 func testRepairReturnsDecodedImageButLeavesCorruptDiskBytes() async throws {
 let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
 try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
 defer { try? FileManager.default.removeItem(at: root) }
 let source = root.appendingPathComponent("source.png")
 let context = CGContext(data: nil, width: 64, height: 64, bitsPerComponent: 8, bytesPerRow: 256, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
 context.setFillColor(CGColor(gray: 1, alpha: 1))
 context.fill(CGRect(x: 0, y: 0, width: 64, height: 64))
 let destination = CGImageDestinationCreateWithURL(source as CFURL, UTType.png.identifier as CFString, 1, nil)!
 CGImageDestinationAddImage(destination, context.makeImage()!, nil)
 XCTAssertTrue(CGImageDestinationFinalize(destination))
 let store = ThumbnailStore(directoryURL: root.appendingPathComponent("thumbs"))
 let corrupt = Data("not-a-jpeg".utf8)
 let date = Date(timeIntervalSince1970: 100)
 let url = try store.persist(corrupt, sourceURL: source, modifiedAt: date)
 let maybeRepaired = await store.imageThumbnailData(for: source, modifiedAt: date)
 let repaired = try XCTUnwrap(maybeRepaired)
 XCTAssertNotEqual(repaired, corrupt)
 let imageSource = try XCTUnwrap(CGImageSourceCreateWithData(repaired as CFData, nil))
 XCTAssertNotNil(CGImageSourceCreateImageAtIndex(imageSource, 0, nil))
 XCTAssertEqual(try Data(contentsOf: url), corrupt)
 let cached = await ThumbnailStore.loadPersistedData(at: url)
 XCTAssertEqual(cached, repaired)
 }
}
