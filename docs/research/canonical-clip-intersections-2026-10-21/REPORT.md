# Canonical shared-edge clipping intersections

The previous interpolation formula depended on segment traversal order. A seeded independent Fraction oracle provides 515 finite axis-plane controls, including endpoint-scale asymmetry. The old formula produces different forward/reverse coordinate bits in 219 cases.

Intersections now canonicalize segment endpoint order, choose the endpoint nearest the cut as the arithmetic anchor, and use FMA for interpolation. Axis-plane intersections set the constrained coordinate directly. Geometric edge evaluation likewise uses a canonical anchor and applies the original orientation only by a final sign reversal. No area epsilon or fragment-dropping threshold was introduced.

## Controls and limits

- All 515 controls have identical intersection bits for forward traversal, reverse traversal and the opposite half-plane. The constrained coordinate is exact; the other coordinate stays within 16 ULP of the independently rounded rational oracle. This numerical test is not an interval certificate for arbitrary clipping.
- 128 adjacent-triangle pairs retain bit-identical shared clipped vertices.
- Canonical edge evaluation is exactly antisymmetric on the finite controls; the existing exact orientation oracle also passes.
- Vector suite: 218 passed / 11 ignored. Native suite: 319 passed / 13 ignored.
- Source-module tests: six passed, including dense area/component integrals, source-order ties, crossing UV fields, memory refusal/cancellation/retry and ranked-work controls.
- Four kernel oracle/adjacency/orientation tests passed.

The existing authored PDF bytes, 1/64 pixel and 1/1024 source-component tolerances, memory caps and shared work limits remain unchanged.

## Full-file finding

The pinned public VectorApple.ai still refuses at dense work/leaf-capacity limits. one.ai page 0 retains 2522 differing pixels/max58; page 1 remains exact. This change repairs a numerical consistency defect but does not close AI support or establish full-page speed.

Opt-in failure diagnostics now summarize retained local areas with a separate bounded read-only visit; they do not change production ownership or discard small areas. At cell origin (151.0625,59.9375), side 1/256, 51 candidates and 48 processed candidates leave 3057 fragments before refusal, with patch IDs 315..316. Of those fragments, 3013 have local area below 1e-12; the minimum is about 7.7e-34. The reported sum is only the currently retained partial partition, not completed cell coverage. Raw data is retained in dense-region-stats.txt and region-stats-poppler.json.

This identifies repeat-cut boundary identity/rounding as the next investigation rather than a need to raise general limits. Merely dropping tiny fragments is not accepted as a repair. Broader AI, 100-format, cache, GPU and physical HDR qualification remain open.
