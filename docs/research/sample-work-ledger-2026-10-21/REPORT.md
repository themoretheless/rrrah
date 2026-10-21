# Exact sampled-work phase ledger — 2026-10-21

The temporary instrumentation adds read-only counters to the main sampled pass and brackets painting, result visits and recursive replay. Every successful block/record work decrement is charged to scan; failed/cancelled function calls are also debited before checking their result. No geometry, work quota or allocation policy is changed. instrumentation.patch preserves the exact change; it was removed before finishing.

## Closed logical-work ledger

| Source patches | Main scan | Main painting | Recursive replay | Main result visits | Unclassified |
|---|---:|---:|---:|---:|---:|
| 1455 | 290950 | 26532278 | 73148386 | 28386 | 0 |
| 323 | 3735238 | 90982119 | 4946830 | 335813 | 0 |
| 245 | 101228 | 30902256 | 68994728 | 1788 | 0 |
| 788 | 413342 | 31890112 | 67634816 | 61730 | 0 |

Each row sums exactly to 100M, zero remaining, and the attempt returns incomplete. The scan column covers the main tiled pass only; scans/sorting/painting/visits inside recursive integration are included in replay. Thus this is an exhaustive partition of the shared work ledger, not a partition of elapsed CPU time or of all decoder preparation work.

The 323-patch attempt spends 90.982119% of its quota in main painting and 3.735238% in main scanning. Merely accelerating that main scan cannot recover most of its current budget. The other attempts are dominated by replay at this coarse classification. Earlier thresholded profiles show expensive painting inside replay, but this ledger does not separate every recursive subcategory.

## Resulting experiment

An earlier transition out of main painting was tried separately; see ../main-pixel-budget-2026-10-21/REPORT.md. That pilot also failed to complete any real sampled attempt and is rejected. Further work should measure inside OrderedPixel painting—unchanged-region traversal/copying, priority comparison and clipping—before choosing a new representation or scheduling policy.

No wall-time, FPS, GPU or full-format qualification follows from these counters. AI remains Pending, and the broader format/cache/HDR/NVIDIA requirements remain open.
