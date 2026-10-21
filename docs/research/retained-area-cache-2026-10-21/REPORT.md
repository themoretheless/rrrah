# Publication-time area classification cache for retained fragments

The target retention profile (../fragment-retention-profile-2026-10-21) observes 13.14M–33.39M old-region visits per attempt, mostly disjoint carry, and 14.20M–34.07M positive keep inputs versus fewer than 2600 successful merges. Previously every unchanged published region recomputed moments (area and centroid) before being retained again.

## Implementation and invariant

Fragment now stores has_area for its immutable represented polygon, including support tags. The four unchanged-old carry paths use keep_existing. New geometry still passes the original moments check. keep_positive retains the previous merge loop, work debits, cancellation checks and admission path. If any merge changes the polygon, the final result is reclassified before publication; a rare degenerate merged result is cached false and is dropped on later carry exactly as a normal moments check would do. Ties, source priority, convex merging and existing published geometry are unchanged.

The flag grows Fragment from 496 to 504 bytes (+8, about 1.61%). AdmittedVec reserves its actual size_of::<Fragment>() at capacity growth, so the extra storage is accounted for by the same budget and released normally. No memory/work/count cap is raised. The cache is per immutable fragment lifetime, separate from image-cache TTL/size/count policy. Both true and false classifications are immutable; only construction or merged geometry generates new metadata.

An invariant regression compares cached/direct classification for 4143 retained fragment states after every paint of the 50-/75-/2-record real numeric controls. A symbolic zero-area control carries with zero work and zero admission calls. Existing exact Fraction geometry and exhaustive transaction/retry controls remain enabled.

## CPU evidence and tradeoff

A cfg(test)-only RECHECK_RETAINED_CONTROL selects the previous repeated moment check for before blocks. Both before/after keep the same clipping implementation and new 504-byte layout. Therefore this measures the effect of eliminating repeated classification under that layout, not a full old-496-byte-layout versus new-layout comparison. Production builds omit selectors. Warmup, six AA controls and sixteen alternating AB/BA pairs run serially in a release test executable on M4 Max (opt3, CGU1, thin LTO). Admission uses zero-sized credits in these probes; real budget bookkeeping is outside their scope. All initial inconclusive/noisy runs are retained.

The initial old fixtures showed no dependable general gain. The target profile motivates a separate sparse64 control (64 nonintersecting dyadic triangles, 8x8 grid, side 1/64, distinct patches) rather than silently replacing the old fixtures. Its short run gives 5.13% mean reduction but AA drift is broad. Increasing sparse blocks to 512 whole replays gives:

| Workload | Mean elapsed reduction | 95% paired bootstrap interval | AA interval |
|---|---:|---:|---:|
| Pair | -0.60% | -1.14..-0.07% | -0.33..0.52% |
| Dense50 | 0.43% | -0.26..1.03% | -0.56..1.69% |
| Dense75 | 0.34% | -0.17..0.84% | -0.09..0.56% |
| Sparse64 | 5.75% | 5.44..6.06% | -0.17..0.47% |

The pair has a small measured regression; ordinary dense gain is unproven. The accepted conclusion is a local benefit for the high-volume disjoint-retention pattern, with an explicit memory/small-case tradeoff. These within-session CPU-block intervals do not establish page loading, viewer FPS, GPU, or cross-machine speed. Reproduce with RRRAH_KEEP_PROBE_RECHECK=1 and ignored dense_subtraction_cpu_pairs, --test-threads=1; analyze using scripts/analyze-split-cpu.py.

## Verification and remaining boundaries

Full vector 229 passed/11 ignored; interpreter 162 passed/3 ignored; native 319 passed/13 ignored; kernel 17 passed/2 manual probes ignored. The 138-record replay preserves every reported prefix area, overlap, fragment count and work; final area 1, 93 fragments, 37108 units. The frozen successful-build qualification executable retains four 100M incomplete VectorApple.ai attempts, no observed coverage-over-one refusal, and unchanged one.ai (page 0: 2522 pixels/max58; page 1 exact). Profile instrumentation is removed.

Repeated scalar classification is reduced; fixed-size fragment copying and old-region traversal remain. Shared/arena-backed region storage is not implemented by this change. Full AI, all 100 formats, long image-cache/swap pressure and physical hardware gates remain incomplete.
