// Independent diagnostic renderer; never used by the production decoder.
import Foundation
import CoreGraphics

let args = CommandLine.arguments
precondition(args.count == 3, "input PDF directory and fresh output directory required")
let source = URL(fileURLWithPath: args[1], isDirectory: true)
let output = URL(fileURLWithPath: args[2], isDirectory: true)
precondition(!FileManager.default.fileExists(atPath: output.path))
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: false)
let files = try FileManager.default.contentsOfDirectory(at: source, includingPropertiesForKeys: nil)
    .filter { $0.pathExtension == "pdf" && $0.lastPathComponent.hasPrefix("ko-") }
    .sorted { $0.lastPathComponent < $1.lastPathComponent }
let space = CGColorSpace(name: CGColorSpace.sRGB)!
for file in files {
    let document = CGPDFDocument(file as CFURL)!
    let page = document.page(at: 1)!
    let bounds = page.getBoxRect(.mediaBox)
    precondition(bounds == CGRect(x: 0, y: 0, width: 32, height: 16))
    var data = [UInt8](repeating: 0, count: 32 * 16 * 4)
    data.withUnsafeMutableBytes { bytes in
        let ctx = CGContext(data: bytes.baseAddress, width: 32, height: 16,
            bitsPerComponent: 8, bytesPerRow: 32 * 4, space: space,
            bitmapInfo: CGBitmapInfo.byteOrder32Big.rawValue | CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
        ctx.fill(bounds)
        ctx.drawPDFPage(page)
        ctx.flush()
    }
    try Data(data).write(to: output.appendingPathComponent(file.deletingPathExtension().lastPathComponent + ".rgba"))
}
print("Quartz rendered \(files.count) authored PDF pages")
