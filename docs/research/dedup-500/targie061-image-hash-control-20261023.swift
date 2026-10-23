import AVFoundation
import CoreGraphics
import Foundation
// Video decoder is outside this image-only control. Any accidental call aborts.
enum CancellableAssetImageGenerator {
 static func image(at time: CMTime, using generator: AVAssetImageGenerator) async throws -> CGImage { fatalError("unused video decoder") }
}
func image(_ size: Int, _ value: UInt8) -> CGImage {
 let data = Data(repeating: value, count: size * size)
 return CGImage(width: size, height: size, bitsPerComponent: 8, bitsPerPixel: 8, bytesPerRow: size, space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGBitmapInfo(rawValue: 0), provider: CGDataProvider(data: data as CFData)!, decode: nil, shouldInterpolate: false, intent: .defaultIntent)!
}
@main struct Probe {
 static func main() {
 let small = image(1, 255)
 let gray = PerceptualHasher.downsampleToGray(small, size: 32)
 precondition(gray.count == 1024)
 precondition(gray.filter { $0 > 0 }.count == 1)
 let black = ImagePerceptualHasher.hash(image: image(32, 0))
 let white = ImagePerceptualHasher.hash(image: image(32, 255))
 precondition(black.hashBits.count == 8 && white.hashBits.count == 8)
 precondition(black.similarity(to: black) == 1)
 let short = ImagePerceptualHash(mediaID: UUID(), hashBits: [0])
 let long = ImagePerceptualHash(mediaID: UUID(), hashBits: [0, 0])
 precondition(short.similarity(to: long) == -1)
 precondition(long.similarity(to: short) == 0)
 let constant = Array(repeating: 1.0, count: 64)
 let dct = PerceptualHasher.dct2D(constant, rows: 8, cols: 8)
 precondition(abs(dct[0] - 64) < 1e-10)
 precondition(dct.dropFirst().allSatisfy { abs($0) < 1e-10 })
 print("small_white_nonzero_pixels=\(gray.filter { $0 > 0 }.count)/1024")
 print("black_white_hash_similarity=\(black.similarity(to: white))")
 print("unequal_length_similarity=\(short.similarity(to: long)),reverse=\(long.similarity(to: short))")
 print("self_identity_and_constant_dct=PASS")
 }
}
