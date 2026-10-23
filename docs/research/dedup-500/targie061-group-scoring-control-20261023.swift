import Foundation
// Compile-only adapters for unused thumbnail/localization methods in real models.
enum AppLanguage { case fixture; static var defaultLanguage: Self { .fixture } }
enum L10n {
    static func unknown(_ language: AppLanguage) -> String { "unknown" }
    static func noVideoTrack(_ language: AppLanguage) -> String { "no-video" }
    static func unreadableImage(_ language: AppLanguage) -> String { "unreadable" }
}
enum ThumbnailStore {
    static func cachedData(at url: URL) -> Data? { nil }
    static func loadPersistedData(at url: URL) async -> Data? { nil }
}
func item(_ name: String, kind: MediaKind = .video) -> MediaItem {
    MediaItem(kind: kind, url: URL(fileURLWithPath: "/authored/" + name), fileSize: 10,
              duration: 10, width: 10, height: 10, modifiedAt: nil, thumbnailData: nil)
}
@main struct Probe {
    static func main() {
        let a = item("a.mp4"), b = item("b.mp4"), c = item("c.mp4")
        func relation(_ first: UUID, _ second: UUID) -> SimilarityRelation {
            SimilarityRelation(firstID: first, secondID: second, score: 0.9, evidence: [.similarFrames])
        }
        let groups = SimilarityGrouper.groups(items: [a,b,c], relations: [relation(a.id,b.id),relation(b.id,c.id)], threshold: 0.8)
        precondition(groups.count == 1 && groups[0].items.count == 3 && groups[0].relations.count == 2)
        precondition(!groups[0].relations.contains { Set([$0.firstID,$0.secondID]) == Set([a.id,c.id]) })
        let ghost = UUID()
        let bridged = SimilarityGrouper.groups(items: [a,c], relations: [relation(a.id,ghost),relation(ghost,c.id)], threshold: 0.8)
        precondition(bridged.count == 1 && bridged[0].items.count == 2)
        precondition(bridged[0].relations.contains { $0.firstID == ghost || $0.secondID == ghost })
        let image = item("image.png", kind: .image)
        precondition(SimilarityGrouper.groups(items: [a,image], relations: [relation(a.id,image.id)], threshold: 0.8).isEmpty)
        let capped = SimilarityScorer.score(a,b,hashesMatch:false,perceptualSimilarity:1,frameSimilarity:0.4)
        precondition(capped.score <= 0.45)
        let unverified = SimilarityScorer.score(a,b,hashesMatch:false,perceptualSimilarity:1,frameSimilarity:nil,requiresVisualVerification:true)
        precondition(unverified.score <= 0.85)
        let nan = SimilarityScorer.score(a,b,hashesMatch:false,perceptualSimilarity:.nan,frameSimilarity:.nan)
        precondition(nan.score.isFinite && nan.score <= 0.05)
        let exact = SimilarityScorer.score(a,b,hashesMatch:true,frameSimilarity:nil)
        precondition(exact.score == 1 && exact.evidence == [.identicalContentHash])
        print("{\"chain_group_is_connected_component\":true,\"unknown_endpoint_bridge_retained\":true,\"cross_media_group_rejected\":true,\"vision_ceiling_preserved\":true,\"unverified_ceiling_preserved\":true,\"nonfinite_content_scores_ignored\":true,\"hash_identity_priority\":true}")
    }
}
