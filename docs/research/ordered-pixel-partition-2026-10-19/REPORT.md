# Admitted local-pixel source ownership partition

A native Rust local-pixel partition is implemented and tested. It is exported by
`hayro-interpret::encode` for integration work, but is not yet called by the
production PDF mesh sampler. The original two full-PDF rendering regressions
remain known ignored failures; AI is not requalified by these kernel tests.

## Geometry and ownership

`OrderedPixel` clips incoming triangles to the unit pixel and stores disjoint
convex source fragments. It splits existing regions around incoming geometry,
selects ownership by source patch index then affine v and u, and removes the
losing portions. Processing a previously higher-priority owner preserves it even
when lower-priority geometry is submitted later. Area is projected union, not
triangle multiplicity. Each winning fragment retains its original component
fields, so affine components integrate at its geometric centroid. The visitor
also exposes polygon vertices: nonlinear function/color integration must occur
over those domains rather than after averaging their input components.

The kernel operates on supplied local affine source/parameter fields. Equality
of v fields uses exact equality of those coefficients; source-to-field
conditioning, bounds and correspondence to the original curved mapping still
belong to the future bridge. Floating-point clipping/moments are numerical
geometry, not an interval certificate for arbitrary original PDF surfaces.

## Resource and failure semantics

Fragment-vector storage is fallibly allocated under the supplied admission
callback. Replacement is admitted while previous credit is live. Both retained
and scratch vectors have configured fragment caps, each convex polygon has at
most16vertices, and a shared decrementing work budget bounds clipping. Invalid
or nonfinite inputs fail explicitly. Cancellation is checked throughout clipping
and immediately before publication. Painting one triangle is transactional: all
scratch regions are dropped on failure and the previous partition survives.
Visitor callers must discard partial accumulators on visitor failure.

Managed weights cover vector payload capacities; allocator/credit-box overhead
and stack storage are not process-RSS accounting. Fixed vertex bounds and
fragment/work caps can explicitly refuse complex geometry. This implementation
makes no claim that complete pages fit their existing resource limits.

## Evidence

Full vector suite:195passed, zero failures,13ignored. Six kernel tests use1MiB
managed roots and verify:

- Exact projected union of duplicate/folded mappings (half area stays half).
- Correct partial-overlap affine component integrals and later-patch ownership,
  including out-of-order submission of an earlier patch.
- Greatest-v priority whose affine fields cross inside a pixel, and greatest-u
  tie-breaking when the supplied v fields are identical.
- A ramp's winning half-domain integral (1/8), not a blend of hidden components.
- Failure at every allocation point in the exercised transition: prior source
  remains intact, managed use returns to the pre-paint value, and retry succeeds.
- Cancellation at every observed clipping/publication checkpoint; work-budget,
  fragment-count and invalid-input refusal all preserve the prior partition.

All kernel roots return to zero after their last owner drops. Scalar test
comparisons use1e-12for numerical evaluation of exact small rational geometry;
this is not a new image acceptance tolerance. Existing curved analytic source,
shared-edge and eight-MiB tests pass in the same vector suite. Logs are retained
in `vector.log` and `kernel-pilot.log`.

## Remaining production integration

Bridge adaptive `(patch index,u,v)` provenance and source components into these
local fields, establish conditioning and folded priority across tessellated
cells, and process spatial candidates without retaining pixel partitions for an
entire page. Integrate nonlinear source functions over winning fragments. Then
make the two exact full-PDF controls ordinary passing regressions and requalify
original AI, error/cancellation paths, memory admission, paired performance,
RAM/swap and actual Metal output. No runtime limits or tolerances were changed.
