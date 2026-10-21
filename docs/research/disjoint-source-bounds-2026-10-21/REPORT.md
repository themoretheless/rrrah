# Disjoint source bounds and all-exit work profile — 2026-10-21

## Accepted change

Before comparing priorities or rebuilding incoming fragments, OrderedPixel now preserves an old region whose bounds do not overlap the clipped incoming triangle. Every incoming remainder is a subset of that triangle. This skips irrelevant clipping/reconstruction while retaining old order, owners and color fields. No allocation, area epsilon, cap increase or relaxed acceptance threshold is added.

An analytic control contains 32 distinct opaque red strips covering x in [0,1/2], followed by an older green triangle wholly in x>=3/4. Before and after it has 33 fragments, red area 1/2, green area 1/8, total 5/8. Logical work: 207 -> 111 ticks. Admission requests: 38 -> 6. These are deterministic API counters, not allocator wall-time, RSS, viewer/FPS or full-page throughput measurements.

The failure/retry control now covers both overlapping and disjoint cases. The disjoint path exercises all three reached admission failures, 55 cancellation checkpoints and 55 shorter work quotas, checking original published integrals/live bytes and successful retry. The original overlapping path still covers seven admission failures and 240 cancellation/short-work points. Earlier 50/75-input Fraction-area controls retain their integrals and fragment counts; painting ticks change from 320437 to 318890 and 77919 to 75936 respectively.

## Verification

- target ordered-pixel controls: 18 passed.
- vector: 225 passed/11 ignored.
- interpreter: 156 passed/1 ignored.
- native: 319 passed/13 ignored.

All runs have zero failures. one.ai page 0 remains 2522 differing pixels/max58; page 1 exact. Fresh normal-binary VectorApple.ai still refuses. Input/normal decoder hashes and reference comparison are in poppler.json.

## Important correction to the earlier failure count

The earlier two visible DENSE_CELL_PAINT messages were not an exhaustive count of unsuccessful sampled shadings. Some returns in the main loop or candidate preparation had no terminal diagnostic. The temporary all-exit profile now records the result and remaining budget for every sampled attempt.

Both the baseline (only this bounds shortcut removed) and current implementation have four sampled attempts: 1455, 323, 245 and 788 source patches. Every attempt returns None after spending its full 100M work quota, with zero remaining. Thus no additional unsuccessful attempt is introduced by the shortcut, and no attempt is completed by it. Prior statements about 'two remaining failures' refer only to explicit dense-cell messages and must not be read as completion of the other two attempts.

This is a before/after logical-work outcome comparison, not a controlled timing benchmark. Different quota-exhaustion origins are not equivalent-work timing observations. The normal qualification report's stderr tail reached its 16384-character cap, so the full profiles are authoritative for outcome counts.

## Cost evidence for the next change

Successful-cell logs include painting >=100k ticks; all split/failure cells are logged. Tile logs include only initial 4x4-tile work >=1M ticks, before child integrations. The following reported category sums are lower bounds; unreported work includes smaller entries, visits and other operations. The categories are disjoint, and their sums are checked against each attempt's 100M total.

| Source patches | Reported tile work | Reported cell painting | Unreported work after tile + full cell costs |
|---|---:|---:|---:|
| 1455 | 23389414 | 66875047 | 7216712 |
| 323 | 9860034 | 4852075 | 85242283 |
| 245 | 30505764 | 60167623 | 6584962 |
| 788 | 20980558 | 59470461 | 16357052 |

For the 1455-patch attempt, one completed cell at (169.65625,58.96875), side 1/32, spends 1891447 painting ticks with 359 candidates and 291 retained fragments. The 245-patch attempt likewise has a completed cell spending 1130606 painting ticks with 453 candidates and 243 fragments. A fragment-count threshold alone does not capture these costly cells. The 323-patch attempt has much more unreported work; its cost cannot yet be attributed to a category from these thresholds alone.

Next: add and independently verify cost-triggered cell refinement before global exhaustion, without raising the shared quota or discarding geometry. Profile the remaining main-loop work separately if that attempt still refuses.

## Reproduction and workspace state

make-cost-profile.py creates an instrumented copy with read-only counters and an outcome closure; profile-instrumentation.patch records its changes. baseline-kernel.patch removes only the new shortcut. analyze-cost-profile.py checks paired begin/end markers and non-overlapping cost sums. The initial incomplete profile is retained as superseded evidence.

Instrumentation and the temporary baseline were removed from source. ordered_samples.rs is byte-identical to HEAD. The normal decoder executable was restored atomically from the frozen validated copy and its SHA matches poppler.json. Full sampled AI, all 100 formats, long collection pressure, physical HDR and deferred NVIDIA remain incomplete.
