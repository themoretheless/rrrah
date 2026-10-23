import Foundation
@main struct Control {
 static func main() async throws {
  let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
  try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
  defer { try? FileManager.default.removeItem(at: root) }
  for name in ["a", "b"] { try Data("original bytes".utf8).write(to: root.appendingPathComponent(name)) }
  let scanner = DuplicateScanner()
  let groups = await scanner.scan(root: root)
  precondition(groups.count == 1 && groups[0].files.count == 2)
  let group = groups[0]
  try Data("changed keeper".utf8).write(to: group.keeper)
  var proposed = [String]()
  let count = try await scanner.trashExtras(in: group, trash: { proposed.append($0.lastPathComponent) })
  precondition(count == 1 && proposed.count == 1)
  precondition(FileManager.default.fileExists(atPath: group.extras[0].path))
  print("PASS basic duplicate grouping; CONFIRMED changed keeper accepted, 1 simulated trash callback; no trash operation executed")
 }
}
