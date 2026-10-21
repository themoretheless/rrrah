# Preserve whole polygons for disjoint cuts — 2026-10-21

## Reproduced failure and independent area

Temporary diagnostics captured 75 bit-exact inputs in actual dense-candidate painting order (after center ranking) from cell (138.4296875,39.08984375), side 1/256, including failing input 74. The original replay refused at its unchanged 4096-fragment bound with 855 previously published fragments. Temporary capture code is removed.

Data are in tests/fixtures/geometry/dense-lineage-138.json and its generated Rust counterpart. The exact Fraction sweep generator now accepts an optional fixture path, while its original default still regenerates the earlier 50-input oracle unchanged. For this 75-input sequence it finds 54 nonempty clipped polygons, 218 nonzero edges and 151 x-events; exact union rounded to f64 is 0.5681429001746927.

## Changes and intermediate evidence

1. Hull reconstruction preserves a support when all vertices along a boundary chain belong to it, even with mixed edge labels. It prefers the traversed edge ID and declines when a vertex lacks the constraint. The new unit test originally failed (ID zero instead of one); cyclic rotations and a missing-constraint negative control now pass. This alone did not fix the replay: record 74 still refused with 749 published fragments.
2. Identical-owner convex pieces can merge across exactly collinear stitches. Ambiguous turn arithmetic falls back to the existing exact dyadic orientation predicate; negative turns and collinear backtracking are rejected before hull simplification. Adjacent merges cascade, with each attempt charged/cancellable. A protected output prefix preserves the existing whole-old-region restoration contract. The prefix rollback unit control and 32 adjacent-rectangle area/count control pass. A cascade-only pilot still hit the 75-input limit.
3. Subtraction formerly emitted each outside piece before discovering whether the entire intersection was empty. This could subdivide a polygon even when the cut was disjoint. It now stores at most 16 outside Polygon values in a fixed stack array, computes the remaining intersection, and emits the pieces only for a nonempty overlap. If the overlap is empty, it retains the original polygon whole. No heap allocation is added for the staging array, no positive-area intersection is deliberately discarded, and memory/vertex/fragment/work limits are unchanged.

The final 75-input replay succeeds with 75 published fragments, 77919 painting ticks under 8 MiB and the original 4096 fragment/100M work bounds. Native area is 0.5681429001746925, within the existing new oracle check of absolute 1e-13. This verifies summed area against an exact oracle for the represented triangle union; it does not independently verify source-priority/color integrals.

The preceding 50-input control still matches its exact union, 0.8462670619378092, within 1e-13; its retained fragments change from 3427 to 1083. The five-triangle support control still has no material pair overlap and matches each Fraction union prefix. Fragment reductions and logical ticks are not a wall-time, page-throughput or GPU claim.

## Verification

- vector: 224 passed, 11 ignored.
- interpreter: 156 passed, 1 ignored, including the new chain/prefix controls, exact-orientation oracles and pool failure/retry checks.
- native: 319 passed, 13 ignored.
- target ordered-pixel controls: 17 passed, including all reached seven admission refusals, 240 cancellation checkpoints and 240 shorter work quotas in the dominant-strip control, with prior state/credit restoration and successful retry.

## Full-page result and remaining scope

Fresh rebuilt VectorApple.ai still refuses. Its complete captured stderr is 12585 characters, below the qualification runner's 16384-character cap, so the early trace was not lost. No DENSE_CELL_DEPTH or overcoverage refusal appears. Two terminal failures remain, both with shared work zero:

- (171.5,58.75), side 1/4: 678/862 candidates processed, 414 published fragments.
- (74.3046875,102.8515625), side 1/128: 219/356 processed, 219 published fragments.

The old deep-cell refusal at (138.4296875,39.08984375) does not recur in this run. one.ai page 0 remains 2522 differing pixels/max58; page 1 exact. Binary/input hashes and raw diagnostics are retained in poppler.json. AI stays Pending. Full-format viewing, long memory-pressure collections, physical HDR and deferred NVIDIA remain incomplete.

Next: capture a work-exhausting cell and measure where repeated retained-region traversal/copying consumes the budget, preserving source ownership and existing limits.
