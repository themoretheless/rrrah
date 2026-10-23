import Foundation
// Authored minimal test interfaces and single-entry memory adapter.
// Does not qualify upstream HashCache/GRDB behavior.
enum MediaKind: String, Sendable { case video, image }
struct CacheRecord: Sendable {}
protocol HashCaching: Sendable {
    func lookupSHA256(filePath: String, fileSize: Int64, modifiedAt: Date?, mediaKind: MediaKind) async -> String?
    func upsertSHA256(filePath: String, fileSize: Int64, modifiedAt: Date?, mediaKind: MediaKind, sha256: String) async
}
actor InMemoryHashCache: HashCaching {
    private var record: (String, Int64, Date?, MediaKind, String)?
    func lookupSHA256(filePath: String, fileSize: Int64, modifiedAt: Date?, mediaKind: MediaKind) -> String? {
        guard let r = record, r.0 == filePath, r.1 == fileSize, r.2 == modifiedAt, r.3 == mediaKind else { return nil }
        return r.4
    }
    func upsertSHA256(filePath: String, fileSize: Int64, modifiedAt: Date?, mediaKind: MediaKind, sha256: String) {
        record = (filePath, fileSize, modifiedAt, mediaKind, sha256)
    }
}
