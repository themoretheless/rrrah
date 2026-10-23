import Foundation
import CryptoKit
// Compile-only interfaces used by the original hasher; no cache backend adopted.
enum MediaKind: String, Sendable { case video, image }
protocol HashCaching: Sendable {
    func lookupSHA256(filePath: String, fileSize: Int64, modifiedAt: Date?, mediaKind: MediaKind) async -> String?
    func upsertSHA256(filePath: String, fileSize: Int64, modifiedAt: Date?, mediaKind: MediaKind, sha256: String) async
}
@main struct Probe {
    static func main() async throws {
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: folder) }
        let path = folder.appendingPathComponent("source.bin")
        let old = Data(repeating: 0x61, count: 64), new = Data(repeating: 0x62, count: 64)
        try old.write(to: path)
        let fixed = Date(timeIntervalSince1970: 1_600_000_000)
        try FileManager.default.setAttributes([.modificationDate: fixed], ofItemAtPath: path.path)
        let baseline = try await FileHasher.sha256(of: path)
        let raced = try await FileHasher.sha256(of: path, mediaKind: .video, cache: nil, afterReadingChunk: {
            try new.write(to: path, options: .atomic)
            try FileManager.default.setAttributes([.modificationDate: fixed], ofItemAtPath: path.path)
        })
        let current = try await FileHasher.sha256(of: path)
        precondition(raced == baseline && raced != current)
        let liveBytes = try Data(contentsOf: path)
        precondition(liveBytes == new)
        precondition(FileHasher.shouldCacheSHA256(originalSize: 64, originalModifiedAt: fixed, currentSize: 64, currentModifiedAt: fixed))
        precondition(!FileHasher.shouldCacheSHA256(originalSize: 64, originalModifiedAt: fixed, currentSize: 65, currentModifiedAt: fixed))
        print("{\"unchanged_hash_control\":true,\"same_size_restored_mtime_replacement_returns_old_hash\":true,\"fresh_hash_differs\":true,\"size_change_identity_rejected\":true,\"real_cache_backend_run\":false}")
    }
}
