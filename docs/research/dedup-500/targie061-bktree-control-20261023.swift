import Foundation
func hamming(_ a: Int, _ b: Int) -> Int { (a ^ b).nonzeroBitCount }
var tree = BKTree<Int>()
let values = Array(0..<256) + [0,0]
for value in values { tree.insert(value, distance: hamming) }
precondition(tree.count == values.count)
var comparisons = 0
for query in 0..<256 {
    for radius in 0...8 {
        let actual = tree.searchItems(query, maxDistance: radius, distance: hamming).sorted()
        let expected = values.filter { hamming($0, query) <= radius }.sorted()
        precondition(actual == expected)
        comparisons += 1
    }
}
var duplicates = BKTree<Int>()
var calls = 0
for _ in 0..<256 {
    duplicates.insert(0, distance: { a,b in calls += 1; return hamming(a,b) })
}
precondition(calls == 256 * 255 / 2)
var searchCalls = 0
precondition(duplicates.searchItems(0, maxDistance: 0, distance: { a,b in searchCalls += 1; return hamming(a,b) }).count == 256)
precondition(searchCalls == 256)
print("{\"queries_times_radii\":\(comparisons),\"stored_values_including_duplicates\":\(values.count),\"bruteforce_parity\":true,\"duplicate_insert_distance_calls\":\(calls),\"duplicate_search_distance_calls\":\(searchCalls)}")
