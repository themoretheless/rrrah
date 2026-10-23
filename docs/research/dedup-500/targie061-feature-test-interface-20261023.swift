import Foundation
// Minimal persistent-cache interface; all original tests use persistentCache:nil.
protocol HashCaching: Sendable {
 func lookupImageFeature(filePath: String, fileSize: Int64, modifiedAt: Date?, algorithmVersion: String) async -> Data?
 func upsertImageFeature(filePath: String, fileSize: Int64, modifiedAt: Date?, algorithmVersion: String, featureData: Data) async
}
