# Bounded bulk carry of unchanged retained fragments

The preceding retention profile observes tens of millions of unchanged old-region carries per VectorApple.ai attempt. This change reuses a proven adjacent-boundary property and copies eligible runs in bulk, removing repeated neighbor/merge and per-element push work. Fragment data is still materialized in the next vector; shared/arena storage is not implemented.

## Cache invariant and transfer

Fragment stores no_merge_with_previous in existing padding; its size remains 504 bytes. Normal keep computes this flag when it proves different owners or a failed exact convex union. An empty prefix is safe; a protected merge-floor boundary remains conservatively false if it was not checked. Existing snapshot restoration preserves the flag with the original prefix geometry.

A retained-old keep reports whether it appended that exact polygon/owner without merging. Only after this establishes an unchanged old tail can following old records use their cached neighbor property. Every copied record must have positive represented area and a proved unmergeable previous boundary. With a remaining incoming polygon, the old bounds must also be disjoint from the original clipped incoming triangle, which contains every incoming remainder. Changed, dropped, mergeable and unproved boundaries take the ordinary path and reestablish the tail condition. New incoming fragments still use normal keep.

AdmittedVec::extend_copy uses the same make_room capacity growth, admission, replacement-credit lifetime and maximum as individual pushes. It checks/debits each capacity chunk and bulk-copies into already reserved space. Preflight checks/debits each candidate old record. The count is additionally bounded for zero-sized types, whose Vec capacity can be usize::MAX. No quota, depth, vertex, fragment or memory cap changes; no work refund or area tolerance is introduced.

## Refusal-bound regression

An initial prototype looked ahead across an entire eligible run before discovering that its output could not fit. A 64-region regression fails that prototype: ordinary refusal costs 775 work units, batch refusal 806. Look-ahead is now capped at the remaining fragment slots. The next boundary/refusal is handled normally. Both versions cost 775 on the regression, preserve all 64 published regions, and return Limit. This prevents wasted scans at the fragment cap before cell refinement. The failed run is retained as overflow-before.log; prototype qualification files are labeled separately.

## Correctness, accounting and work

All existing exact Fraction geometry/ownership controls remain. Cached area and asserted neighbor properties are checked against direct classification/convex-union results for 4143 retained states in real numeric fixtures. Four generated seeds with 32 paints each add 4862 state checks and 20 refused paints; every refusal preserves owner, geometry, tags and cache flags. These are cache-invariant checks, separate from the independent geometry oracles. A protected-prefix regression explicitly retains an unknown boundary that could merge, preventing its use in a bulk run.

The public 64-fragment sparse carry control covers every reached 7 admission refusals, 113 cancellation checkpoints and 113 shorter work quotas. Failures preserve old integrals, fragment count and retained memory, then retry successfully and release all credit. The resulting 65 fragments have analytic red area 1/128 and blue area 1/2048.

The 138-record replay preserves every prefix area, pair overlap and fragment count. Final area remains 1 with 93 fragments and max floating pair overlap 7.806255641895632e-18; work decreases 37108→35261. Dense50 work decreases 82618→82522 and dense75 38057→37590, with unchanged independent area integrals and retained counts. These are local work units, not page time.

Final source checks: vector 230 passed/11 ignored; interpreter 165 passed/3 ignored; native 319 passed/13 ignored; kernel 20 passed/2 manual timing probes ignored. No failures.

## Paired local CPU evidence

RRRAH_CARRY_PROBE_NO_BATCH=1 selects compile-time BATCH=false versus BATCH=true specializations in one release test executable. Dispatch happens once per paint, so the control does not incur a selector on every old record. Both paths use the same new cached metadata and layout. Production instantiates the enabled path and omits the selector. Warmup, six AA controls and sixteen alternating AB/BA pairs run serially on M4 Max (opt3, one codegen unit, thin LTO). Admission callbacks use zero-sized credits; real budget bookkeeping is outside the probe.

All initial, noisy, per-record-selector, specialized and bounded repeats are retained. The first probe has a large AA outlier and is not the performance gate. The final bounded direct repeat gives:

| Workload | Mean elapsed reduction | 95% paired bootstrap interval | AA interval |
|---|---:|---:|---:|
| Pair | 0.25% | 0.01..0.49% | -1.86..1.13% |
| Dense50 | 0.35% | -0.02..0.73% | -0.76..0.63% |
| Dense75 | 1.35% | 1.04..1.68% | -1.13..0.57% |
| Sparse64 | 20.37% | 19.95..20.84% | -0.78..1.22% |

The established substantial benefit is the carry-heavy sparse pattern. Small/general benefits are not claimed. This measures within-session elapsed CPU-kernel blocks in a test harness, not page loading, viewer FPS, GPU or cross-machine throughput. The baseline also computes the new metadata, which is part of this controlled comparison. Reproduce the ignored dense_subtraction_cpu_pairs with the environment variable above, --release and --test-threads=1; analyze with scripts/analyze-split-cpu.py.

## Pinned-file progress before the unchanged quota

Counter-only before/after executables additionally record main 4x4 tiles entered, last tile coordinates and attempted paint API invocations (including retries and failed calls). Both are frozen after successful builds; source instrumentation is restored before the after executable runs. Counters do not alter geometry or work accounting and are not timing measurements.

| Patches | Main tiles before→after | Paint invocations before→after |
|---|---:|---:|
| 1455 | 31→31 | 134154→160964 |
| 323 | 1154→1337 | 450245→510272 |
| 245 | 15→19 | 140123→165062 |
| 788 | 98→98 | 207492→239955 |

Two attempts advance to later tile rows; the others do more work inside their last tile. This is internal progress, not a partially delivered image: all four attempts still return incomplete at 100M, and their temporary maps are discarded. Tile counts include the unfinished last tile; invocation counts do not equal successfully completed triangles.

Final normal decoder hash matches the frozen executable used by poppler.json. No coverage-over-one refusal is observed in captured terminal stderr. one.ai remains unchanged: page 0 2522 differing pixels/max58, page 1 exact. Temporary progress/event counters are absent from source. Full AI correctness, all 100 formats, long cache/swap pressure and physical hardware gates remain open. A spatial range index/shared storage is a future step; this change still scans and materializes eligible records.
