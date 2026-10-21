# Transactional numeric rectangle recording

A new regression reproduced an arena-frontier leak in paint_rectangle: with max_nodes2 and an existing one-node current path, a failed rectangle left used_nodes2 instead of1. The method restored the active path/style but did not rewind partially appended temporary nodes. Paint-cap refusal after constructing all five nodes and overflow of a later transformed corner had the same logical retention risk. This is consumed logical arena capacity inside an already admitted buffer, not a claim of leaked heap/RSS.

The method now snapshots both used-node and used-paint frontiers and restores them on failure, alongside its existing saved-state restoration. Only unpublished suffixes are rewound; successful geometry, earlier persistent paths and saved state records keep their ownership. Error priority/types and successful device RectFill provenance are unchanged. No new allocation or budget bypass was introduced.

Regression covers node caps1through5, zero paint capacity and f64 coordinate overflow after a valid first transformed corner. Every refusal preserves the original current point/path and logical node count. Where capacity permits, a smaller line operation succeeds on the same graphics instance without increasing limits. The overflow case resets CTM, records a later stroke, and verifies its exact old/new path endpoints. All managed credit returns tozero when state/scene owners drop.

Full vector suite247passed/0failed/11ignored. red.log retains the reproducer; vector.log retains the successful final suite. This corrects native scene recording/recovery; no new format count, GPU/display or performance qualification is claimed.
