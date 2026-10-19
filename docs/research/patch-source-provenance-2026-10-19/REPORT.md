# Source provenance for ordered adaptive patch integration

The previous investigation proved folded coverage multiplicity and partial-patch
color ownership defects. This change provides the source data needed to fix
both; it does not yet replace the production pixel-area accumulation algorithm.
The two known exact rendering regressions remain unresolved/ignored.

## Implementation

`tessellate_patch_mesh_stream_ordered_admitted` adds callback-only
`PatchTriangleSource`: source-slice patch index and original (u,v) for every
emitted triangle vertex. Both simple two-triangle cells and conforming boundary
fans retain their parameters. Patch indices follow the supplied mixed
Tensor/Coons slice. A later index identifies a later patch; folded mapping
priority requires interpolated v then u, never device winding or triangle
emission order.

The existing streaming API delegates to this API and discards the metadata.
Owned/compact output continues to discard it through the default sink hook.
`Triangle`, compact retained geometry, color capacity and on-disk cache/swap
formats are unchanged. No additional retained mesh or per-triangle allocation
is introduced. Resource limits, adaptive geometry/component bounds, cancellation
and consumer-failure semantics remain in force.

## Validation

Full vector library: 189 passed, zero failures, 13 ignored. Two added tests
verify mixed Tensor/Coons/Tensor provenance through reflected/sheared transformed
geometry, source component interpolation, nontrivial curved refinement/fans,
and nondecreasing source indices. Each original parameter remains inside [0,1]
and reconstructs its emitted transformed vertex to floating-point evaluation
precision (1e-10 point consistency, not a pixel-acceptance tolerance).

Callback failure, cancellation after an emitted triangle, refused planner
admission and retry return expected typed results; all managed planner credit
returns to zero. Existing independent parabolic pixel-area, shared/reversed-edge,
no-T-junction and eight-MiB ownership checks pass in the same suite.

The vendor dependency cannot be tested directly with workspace `-p` because its
dev-dependencies are outside the workspace. Verification uses the public API
through `rrrah-vector`, linking the patched vendor code. One test-source pilot
failed type inference for a sum; the corrected final suite above passes.

## Remaining correctness work

The production sampler still sums triangle multiplicity and averages source
components on overlap. It must partition projected coverage and select source
ownership using this provenance before integrating the winning components and
functions. Both exact3x3controls must become ordinary passing regressions, and
original AI pages, admission/cancellation, performance and Metal/cache transport
must then be requalified. This preparatory API does not qualify AI, the complete
100-format catalog, physical HDR or NVIDIA/CUDA.
