# Mixed-source navigation with partial protected byte headroom

The opt-in external-collection test now supports
`RRRAH_COLLECTION_PARTIAL_BYTES=1`. Its default count-limited mode remains
available. Both runs use the same 1024 content-unique repository assets and
frozen fixtures, not 1024 independent camera photographs. Every source SHA256
was rechecked: 1024 checked, zero mismatches. Source media remains in place.

## Workload and assertions

Both navigation directions exercise disabled (0/0), ahead-only (0/3), behind-only
(3/0), and mixed (1/2) prefetch windows. Each of eight combinations has 120 or
121 navigation steps. Source preparation accepts 962 displayable paths under
strict application color defaults; 47 require qualified source color, 13 have
unsupported/invalid EXR color metadata, and two AVIF files have nonconforming
pixel-aspect declarations. All result rows, digests, assumptions and refusal
statuses are exactly equal across byte and count runs.

For partial-byte checks, selected and nearest frames remain protected. After
nearest admission, the count limit is removed and byte capacity becomes current
resident bytes plus half the smallest far-neighbor logical pixel weight.
Each far payload is independently decoded, checked against its baseline digest,
and seeded into temporary swap before admission; the queue is drained. Count
admission and the positive spare headroom explicitly fit, but the actual logical
frame floor does not. Each far refusal must leave swap reads and managed
used/peak unchanged. This is deliberately controlled admission pressure, not a
claim about cold-source prediction or native memory exhaustion.

## Results

- Partial bytes: 1438 byte refusals, 720 nearest navigation hits without decoding,
  61 TTL restore/refusal/retry checks, 1920 swap writes, 979 reads, zero errors and
  zero dropped writes. About 11.99 seconds excluding build.
- Count control: 1438 count refusals, the same 720 nearest hits and 61 TTL checks,
  960 writes, 643 reads, zero errors and six expected bounded write drops. About
  7.02 seconds excluding build.
- Both runs end with eight disk objects / 22,579,712 bytes, within configured
  bounds, no queued credit, and final managed use zero.
- Full ordinary application suite: 294 passed, zero failures, 84 ignored.
- Formatter and patch whitespace checks passed.

The two modes perform different seeding work, so their runtimes/read counts are
not a performance comparison. The 512 MiB peak includes artificial reservation-
only pressure; it does not measure RSS or physical memory exhaustion. Unchanged
peak during far admission becomes a weaker signal after a prior full-reservation
step; the independent zero-read assertion still applies to every far refusal.
Same-decoder hashes establish transport consistency, not independent color or
format correctness. This is application-helper navigation, not a live GUI,
physical slow-disk, NVIDIA/CUDA or physical HDR-display qualification.

## Reproduction

Set `RRRAH_COLLECTION_VARY_PREFETCH=1`, the preserved manifest path in
`RRRAH_COLLECTION_MANIFEST`, an output path in `RRRAH_COLLECTION_REPORT`, and
optionally `RRRAH_COLLECTION_PARTIAL_BYTES=1`, then run the ignored application
test `external_mixed_collection_preserves_navigation_prefetch_ttl_swap_and_pressure`.
See `source-identity.json`, `partial.json`, `count-control.json`, and their logs.
