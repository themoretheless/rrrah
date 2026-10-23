import Foundation
import XCTest
@testable import SimilarVideoFinder
@MainActor
final class BrowseMetadataRefreshControl: XCTestCase {
 func testSameOrderedIDsKeepStaleDisplayedMetadataAfterRevision() async throws {
 let id = UUID()
 func item(size: Int64, width: Int) -> MediaItem {
 MediaItem(id: id, kind: .image, url: URL(fileURLWithPath: "/tmp/authored-metadata-refresh.png"), fileSize: size, duration: nil, width: width, height: 64, modifiedAt: nil, thumbnailData: nil)
 }
 let scan = ScanViewModel(hashCache: nil)
 scan.replaceResultsForTesting(items: [item(size: 10, width: 64)], relations: [])
 let browse = BrowseViewModel(scanModel: scan)
 XCTAssertEqual(browse.displayedItems.first?.fileSize, 10)
 let before = browse.displayedItemsRecomputeCount
 scan.replaceResultsForTesting(items: [item(size: 20, width: 128)], relations: [])
 for _ in 0..<100 {
 if browse.displayedItemsRecomputeCount > before { break }
 try await Task.sleep(for: .milliseconds(2))
 }
 XCTAssertGreaterThan(browse.displayedItemsRecomputeCount, before)
 XCTAssertEqual(scan.items.first?.fileSize, 20)
 XCTAssertEqual(scan.items.first?.width, 128)
 XCTAssertEqual(browse.displayedItems.first?.fileSize, 10)
 XCTAssertEqual(browse.displayedItems.first?.width, 64)
 XCTAssertEqual(browse.displayedItems.first?.id, id)
 }
}
