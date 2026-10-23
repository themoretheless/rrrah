import AVFoundation
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers
// Video decoder is outside this image-only control. Any accidental call aborts.
enum CancellableAssetImageGenerator {
 static func image(at time: CMTime, using generator: AVAssetImageGenerator) async throws -> CGImage { fatalError("unused video decoder") }
}
func image(_ size: Int, _ value: UInt8) -> CGImage {
 let data = Data(repeating: value, count: size * size)
 return CGImage(width: size, height: size, bitsPerComponent: 8, bitsPerPixel: 8, bytesPerRow: size, space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGBitmapInfo(rawValue: 0), provider: CGDataProvider(data: data as CFData)!, decode: nil, shouldInterpolate: false, intent: .defaultIntent)!
}
@main struct URLProbe {
 static func main() throws {
 let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
 try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
 defer { try? FileManager.default.removeItem(at: dir) }
 var hashes: [ImagePerceptualHash] = []
 for size in [1, 2, 16, 32, 64] {
 let original = image(size, 255)
 let url = dir.appendingPathComponent("white-\(size).png")
 let destination = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
 CGImageDestinationAddImage(destination, original, nil)
 precondition(CGImageDestinationFinalize(destination))
 let fromURL = try ImagePerceptualHasher.hash(for: url)!
 let direct = ImagePerceptualHasher.hash(image: original)
 precondition(fromURL.hashBits == direct.hashBits)
 hashes.append(fromURL)
 print("white_size=\(size),url_matches_direct=true,hash=\(fromURL.hashBits)")
 }
 print("white_1_vs_32_similarity=\(hashes[0].similarity(to: hashes[3]))")
 precondition(hashes[0].hashBits != hashes[3].hashBits)
 }
}
