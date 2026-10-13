# Native selected mesh integration: required proof boundary

The executable regression `selected_mesh_owner_identity_does_not_certify_affine_integration_across_projection` uses one original owner, identity coordinates, no function, identical native destination coordinates, fixed opacity, and vertices with component values 0,1,0 at (0,0),(8,0),(0,8). All these source capability checks pass.

Selected samples at (-4,0) and (4,0) are 0 and 0.5. Their mean is 0.25, while the same selected owner sampled at their mean (0,0) produces 0. Negative barycentric weight projection and renormalization caused the difference. The generic point sampler correctly refuses the exterior point. The contained segment (0,0)–(4,0) preserves its mean. This regression does not assert that the current polygon compositor actually emits the exterior segment.

A field key therefore identifies a source field; it does not certify an integration domain. Source-wide component intervals remain valid for successful projected samples, but their observed widths are insufficient for near-constant replacement on the original AI.

A future replacement of fragment samples with one centroid sample must account for all of these:

1. The same exact route generation, source order, selected owner, raster color/alpha cells and group/blend lifecycle.
2. A domain certificate proving the relevant source affine field contains the actual retained fragments, or a quantitative bound on weight projection. A representative witness alone does not prove this for all fragments.
3. Coordinate-transform and barycentric arithmetic error, including conditioning of nearly degenerate triangles. Geometric clipping in device coordinates and rounded inverse source coordinates are distinct operations.
4. Final f32 source component rounding, propagation through native premultiplied composition, and finite precision in area/first-moment accumulation.
5. Centroid evaluation under the retained owner and witness, with exact identity checking. Route generation changes continue to require original-integral fallback.
6. Work, memory, cancellation and refusal accounting within the ordinary budgets. A certificate must not move uncharged work outside the budget.

For a true affine component f, exact area A and first moment M imply integral f = A*f(M/A), including disconnected domains. Thus convexity of a bucket is not required for an affine field defined over the whole domain. What is required here is that projection/normalization does not change the field over that domain, and that the centroid evaluation uses that same field. Affine extension to a centroid outside a nonconvex domain must not silently become a projected sampler call.

No actual color sampling is skipped by this work. The current observers are diagnostics, and the independent AI color discrepancy and ordinary full-page qualification remain unresolved.
