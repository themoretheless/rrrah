# Rank work in dense sampled cells without changing UV ownership

Dense subcells gather intersecting retained triangles into an admitted temporary list. Candidates are ordered by descending patch index, then source v/u evaluated at the fixed cell center. This is a work heuristic only: OrderedPixel still clips and compares the full affine source fields over each overlap. Identical UV fields have identical keys; signed zero is canonicalized, and original ordinal preserves the existing tie order. No source sample ID is used as priority.

The in-place heapsort allocates no scratch and debits/observes cancellation before every comparison. List capacity is admitted before growth with the previous reservation retained. The list and its credit are released before subdivision and before returning integrals. Source fields are reconstructed rather than retained in the sorting list. Existing source-component tolerances, 512-fragment working set, deepest-leaf capacity, recursion bound and shared 100M work allowance are unchanged.

## Evidence

- Six source-module tests passed, including cancellation at every observed sort comparison, short work/retry, signed zero, exact tie preservation and admission/cancellation release under 8 MiB.
- The dense grid now overlays an earlier coarse approximation with identical UV fields; the original later-fine tie result is retained, including 1/16 and 1/256 pixel-edge scales.
- An authored retained-field crossing control checks independent rational integrals: area 1/2, red 5/32, blue 11/32. The center key favors one candidate while the actual UV winner changes at x=1/2; the expected split remains intact.
- Full vector suite: 218 passed, 11 ignored. Native suite: 319 passed, 13 ignored.

## Same-process logical work control

A 1024-triangle fine grid followed by a later opaque covering patch is integrated in the original source order and in ranked order, using the same current ownership kernel and the same immutable records. Both results are exactly equal: [area=1, component integrals=0.25,0.25,0.25,0]. The ranked count includes candidate gathering, sorting comparisons and painting.

Original source order used 1,113,244 shared-work ticks; ranked order used 50,967 ticks, a 21.84x reduction on this control. Both fit the same 8 MiB managed cap and release temporary credit. This is a deterministic quota-work comparison, not a wall-clock, whole-page or GPU speed benchmark.

## Public-file outcome

The pinned full-page comparison still exits 1: VectorApple.ai refuses with dense work/leaf-capacity limits. one.ai page 0 remains 2522 differing pixels/max58; page 1 is exact. Ranking does not close the AI regression, and no broad rendering speedup or 100-format qualification is claimed. The next bottleneck remains excessive retained ownership regions on the difficult source patches.
