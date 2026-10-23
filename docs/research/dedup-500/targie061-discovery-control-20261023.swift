import Foundation
@main struct DiscoveryProbe {
 static func main() throws {
 let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
 try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
 defer { try? FileManager.default.removeItem(at: root) }
 for path in ["valid-name.PNG", "corrupt.jpg", "unsupported.avif", "extensionless", ".hidden.png", "nested/inner.webp", ".hidden-dir/inside.png"] {
 let url = root.appendingPathComponent(path)
 try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
 try Data("not an image".utf8).write(to: url)
 }
 let paths = try ImageScanner.discoverImageURLs(in: root).map { url in
 let components = url.pathComponents
 let index = components.firstIndex(of: root.lastPathComponent)!
 return components.dropFirst(index + 1).joined(separator: "/")
 }
 print("root=\(root.path),observed=\(paths)")
 precondition(Set(paths) == Set(["valid-name.PNG", "corrupt.jpg", "nested/inner.webp"]))
 print("discovered=\(paths)")
 do {
 let missing = try ImageScanner.discoverImageURLs(in: root.appendingPathComponent("does-not-exist"))
 print("missing_root_returned_count=\(missing.count)")
 } catch { print("missing_root_error=\(error)") }
 print("name_only_filter=true,hidden_skipped=true,unsupported_extension_skipped=true,nested_visible_included=true")
 }
}
