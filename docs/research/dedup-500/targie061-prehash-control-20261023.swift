import Foundation
import AppKit
// Unused model adapters; actual image decoding/downsampling is not qualified.
enum AppLanguage { case fixture; static var defaultLanguage: Self { .fixture } }
enum L10n { static func unknown(_ l: AppLanguage) -> String { "unknown" }; static func noVideoTrack(_ l: AppLanguage) -> String { "none" }; static func unreadableImage(_ l: AppLanguage) -> String { "none" } }
enum ThumbnailStore { static func cachedData(at u: URL) -> Data? { nil }; static func loadPersistedData(at u: URL) async -> Data? { nil } }
enum PerceptualHasher { static func downsampleToGray(_ image: CGImage, size: Int) -> [Double] { fatalError("unexpected image path") } }
func media(_ name: String) -> MediaItem { MediaItem(kind:.video,url:URL(fileURLWithPath:"/authored/"+name),fileSize:10,duration:10,width:10,height:10,modifiedAt:nil,thumbnailData:nil) }
func hash(_ item: MediaItem,_ duration: Int,_ aspect: Int,_ mean: UInt8) -> QuickPrehash { QuickPrehash(videoID:item.id,durationBucket:duration,sizeBucket:0,aspectBucket:aspect,thumbnailMean:mean,thumbnailVariance:0) }
@main struct Probe {
 static func main() {
  let items=(0..<64).map { media("clip\($0).mp4") }
  let indexes=Dictionary(uniqueKeysWithValues:items.enumerated().map { ($0.element.id,$0.offset) })
  let hashes=Dictionary(uniqueKeysWithValues:items.enumerated().map { ($0.element.id,hash($0.element,$0.offset % 11,($0.offset*3)%13,UInt8(($0.offset*17)%256))) })
  let found=PrehashCandidateFinder.find(videos:items,prehashes:hashes)
  func key(_ a:Int,_ b:Int)->String { "\(min(a,b)):\(max(a,b))" }
  let actual=Set(found.pairs.map { key(indexes[$0.0.id]!,indexes[$0.1.id]!) })
  var expected=Set<String>()
  for i in 0..<64 { for j in (i+1)..<64 {
   let a=hashes[items[i].id]!,b=hashes[items[j].id]!
   if abs(a.durationBucket-b.durationBucket)<=2 && abs(a.aspectBucket-b.aspectBucket)<=2 && abs(Int(a.thumbnailMean)-Int(b.thumbnailMean))<=40 { expected.insert(key(i,j)) }
  } }
  precondition(actual==expected && actual.count==found.pairs.count)
  let a=media("same.mp4"),b=media("same-copy.mp4")
  let h1=hash(a,1,1,0),h2=hash(b,99,99,255)
  precondition(!h1.isCompatible(with:h2))
  let fallback=PrehashCandidateFinder.find(videos:[a,b],prehashes:[a.id:h1,b.id:h2]);precondition(fallback.pairs.count==1)
  precondition(PrehashCandidateFinder.find(videos:[a,b],prehashes:[a.id:h1]).pairs.isEmpty)
  let repeated=PrehashCandidateFinder.find(videos:[a,a],prehashes:[a.id:h1])
  precondition(repeated.pairs.count==1 && repeated.pairs[0].0.id==repeated.pairs[0].1.id)
  print("{\"bruteforce_pairs\":2016,\"index_parity\":true,\"candidates\":\(actual.count),\"compatibility_checks\":\(found.compatibilityChecks),\"name_rescue_bypasses_incompatible_metadata\":true,\"missing_prehash_skips_name_rescue\":true,\"repeated_id_can_form_self_pair\":true}")
 }
}
