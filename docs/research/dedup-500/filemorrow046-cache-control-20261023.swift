import Foundation
@main struct CacheControl {
 static func main() async throws {
  let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
  try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
  defer { try? FileManager.default.removeItem(at: root) }
  let a = root.appendingPathComponent("a.bin")
  let b = root.appendingPathComponent("b.bin")
  let bytes = Data(repeating: 61, count: 1_048_576)
  try bytes.write(to: a)
  try bytes.write(to: b)
  let scanner = DuplicateScanner()
  let first = await scanner.scan(root: root)
  precondition(first.count == 1)
  let stamp = try FileManager.default.attributesOfItem(atPath: b.path)[.modificationDate] as! Date
  var changed = bytes
  changed[300_000] ^= 1
  try changed.write(to: b)
  try FileManager.default.setAttributes([.modificationDate: stamp], ofItemAtPath: b.path)
  let cached = await scanner.scan(root: root)
  let fresh = await DuplicateScanner().scan(root: root)
  precondition(cached.count == 1)
  precondition(fresh.isEmpty)
  let actualA = try Data(contentsOf: a)
  let actualB = try Data(contentsOf: b)
  precondition(actualA != actualB)
  print("CONFIRMED cached scanner returns 1 stale group; fresh scanner returns 0; same size, restored mtime, changed middle bytes")
 }
}
