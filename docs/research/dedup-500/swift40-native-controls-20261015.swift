import Foundation

final class OwnedIterator: PathIterator {
    var urls: [URL]; var count = 0
    init(_ urls: [URL]) { self.urls = urls }
    func next() -> URL? { count += 1; return urls.isEmpty ? nil : urls.removeFirst() }
}
final class GateIterator: PathIterator {
    let entered = DispatchSemaphore(value: 0), release = DispatchSemaphore(value: 0)
    var urls: [URL]; var count = 0
    init(_ urls: [URL]) { self.urls = urls }
    func next() -> URL? {
        count += 1
        if count == 1 { entered.signal(); precondition(release.wait(timeout: .now() + 5) == .success) }
        return urls.isEmpty ? nil : urls.removeFirst()
    }
}

@main struct NativeControls {
    static func find(_ explorer: FileExplorer, _ iterator: PathIterator) -> [URL] {
        let done = DispatchSemaphore(value: 0); var result: [URL] = []
        explorer.findDuplicatedFile(with: iterator) { r in
            switch r { case .success(let paths): result = paths; case .failure: preconditionFailure("unexpected failure") }
            done.signal()
        }
        precondition(done.wait(timeout: .now() + 5) == .success)
        return result
    }
    static func main() throws {
        let root = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
        let a = root.appendingPathComponent("a", isDirectory: true), b = root.appendingPathComponent("b", isDirectory: true)
        try FileManager.default.createDirectory(at: a, withIntermediateDirectories: false)
        try FileManager.default.createDirectory(at: b, withIntermediateDirectories: false)
        let x = a.appendingPathComponent("same.bin"), y = b.appendingPathComponent("same.bin")
        try Data("left".utf8).write(to: x); try Data("unequal-longer".utf8).write(to: y)
        var cases: [String] = []
        let done = DispatchSemaphore(value: 0); var diskResult: [URL] = []
        FileExplorer().findDuplicatedFile(at: root) { r in diskResult = try! r.get(); done.signal() }
        precondition(done.wait(timeout: .now() + 5) == .success)
        precondition(Set(diskResult) == Set([x,y])); let left = try Data(contentsOf: x); let right = try Data(contentsOf: y); precondition(left != right)
        cases.append("real_disk_same_name_unequal_size_and_bytes_grouped_name_contract")

        let nonexistent = [root.appendingPathComponent("missing/a.bin"),root.appendingPathComponent("other/a.bin")]
        precondition(find(FileExplorer(), OwnedIterator(nonexistent)).count == 2)
        cases.append("iterator_names_need_no_existing_files")

        let excluded = FileExplorer(excludedInfo: ExcludedInfo(directories: [a]))
        precondition(find(excluded, OwnedIterator([x,y])) == [y])
        cases.append("filter_after_group_leaves_single_result")

        let prefix = URL(string: "file:///owned/path")!, neighbor = URL(string: "file:///owned/path-other/file.bin")!
        let validator = PathValidator(excludedInfo: ExcludedInfo(directories: [prefix]))
        precondition(validator.verifyPathNeedExcluded(neighbor))
        cases.append("non_directory_url_prefix_excludes_neighbor")

        let directoryWithoutFlag = URL(fileURLWithPath: a.path, isDirectory: false)
        precondition(PathValidator(excludedInfo: ExcludedInfo()).verifyPathIsFile(directoryWithoutFlag))
        cases.append("file_validator_trusts_url_directory_flag_not_actual_stat")

        let oneShot = FileExplorer(); precondition(find(oneShot, OwnedIterator([x,y])).count == 2)
        let retryIterator = OwnedIterator([x,y]); var retryCalled = false
        oneShot.findDuplicatedFile(with: retryIterator) { _ in retryCalled = true }
        precondition(retryIterator.count == 0 && !retryCalled && oneShot.state == .finish)
        cases.append("finished_instance_silently_drops_second_request")

        let stopped = FileExplorer(); stopped.stopSearch(); let stoppedIterator = OwnedIterator([x,y]); var stoppedCalled = false
        stopped.findDuplicatedFile(with: stoppedIterator) { _ in stoppedCalled = true }
        precondition(stoppedIterator.count == 0 && !stoppedCalled)
        cases.append("stop_before_start_silently_drops_request")

        let gate = GateIterator([x,y]); let active = FileExplorer(); let finished = DispatchSemaphore(value: 0); var afterStop: [URL] = []
        active.findDuplicatedFile(with: gate) { r in afterStop = try! r.get(); finished.signal() }
        precondition(gate.entered.wait(timeout: .now() + 5) == .success)
        active.stopSearch(); precondition(active.state == .finish); gate.release.signal()
        precondition(finished.wait(timeout: .now() + 5) == .success)
        precondition(Set(afterStop) == Set([x,y]) && gate.count == 3)
        cases.append("stop_during_blocked_iteration_continues_and_publishes_success")

        let output: [String: Any] = ["status":"passed", "controls": cases.count, "cases":cases, "scope":"Unchanged fullFileExplorer source compiled natively; actual Foundation disk enumerator for firstcase, explicit supplied iterators and blocked iterator schedule for others. No AppKit GUI, UserDefaults, network, performance or Rootlibrary execution."]
        print(String(data: try JSONSerialization.data(withJSONObject: output, options: [.prettyPrinted,.sortedKeys]), encoding: .utf8)!)
    }
}
