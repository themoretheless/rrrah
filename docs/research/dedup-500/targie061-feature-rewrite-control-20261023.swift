import XCTest
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers
@testable import SimilarVideoFinder
final class FeatureRewriteControl: XCTestCase {
 func testSameURLRewriteRetainsCompletedObservation() async throws {
 let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
 try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
 defer { try? FileManager.default.removeItem(at: root) }
 let url = root.appendingPathComponent("source.png")
 func write(_ width: Int, _ value: CGFloat) throws {
 let c = CGContext(data: nil, width: width, height: 64, bitsPerComponent: 8, bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
 c.setFillColor(CGColor(red: value, green: 0.2, blue: 1-value, alpha: 1))
 c.fill(CGRect(x: 0, y: 0, width: width, height: 64))
 let d = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
 CGImageDestinationAddImage(d, c.makeImage()!, nil)
 XCTAssertTrue(CGImageDestinationFinalize(d))
 }
 try write(64, 0.1)
 let original = try Data(contentsOf: url)
 let cache = ImageFeatureCache()
 let first = try await cache.feature(for: url)
 try write(128, 0.9)
 XCTAssertNotEqual(original, try Data(contentsOf: url))
 let repeated = try await cache.feature(for: url)
 XCTAssertTrue(first.observation === repeated.observation)
 await cache.cancelAll()
 let refreshed = try await cache.feature(for: url)
 XCTAssertFalse(first.observation === refreshed.observation)
 }
}
