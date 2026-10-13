# Selected mesh source component error boundary

This certificate is pointwise over the convex hull supplied to the source API. It compares successful selected-source samples with an exact affine expression using the retained inverse-transform coefficients, rounded constant source edge differences, and the sampler's rounded constant denominator. It does not assert ideal geometric barycentric coordinates of the original triangle. No fragment sample is skipped.

Let λ be those exact affine weights (sum λ = 1), r the computed raw weights, and E the sum of the three absolute raw-evaluation error envelopes. The interval/hull evaluation bounds D = sum max(0,-r_i) and P = sum max(0,r_i) from above over the supplied hull. All positive bound arithmetic rounds outward.

Clipping r to u = max(r,0) changes its L1 norm by D. The exact sum U of u satisfies |U-1| ≤ E+D. Exact normalization q = u/U therefore obeys ||q-λ||1 ≤ 2(E+D). The implementation separately bounds rounding of the positive sum and three divisions by η, so ||p-λ||1 ≤ 2(E+D)+η and |sum p-1| ≤ η for the actually normalized weights p.

The positive sum rounding bound τ uses 4*f64::EPSILON*P plus a subnormal allowance. A conservative lower bound on the rounded denominator is 1-E-τ. Refuse unless it is finite and exceeds 0.5. This also avoids admitting an ill-conditioned normalization certificate. η includes τ divided by this lower bound, division rounding, and subnormal allowances.

For one channel subtract any constant vertex color c0 before applying the L1 bound. Then the component discrepancy is at most span(colors)*||p-λ||1 + |c0|*η, plus color product/sum rounding. Vertex components must be finite in0..1, so η safely bounds the constant term. A conservative16*f64::EPSILON arithmetic allowance and one whole f32 ulp at1 cover native interpolation and final rounding of a successful0..1sample.

The exact affine coefficient intervals are separate from the evaluation error envelope: outward coefficient arithmetic encloses the mathematical linear expression; operation error envelopes bound the sampler's ordered floating evaluation relative to that expression. Extrema of an affine expression occur at hull vertices. Rectangular ranges are used only to bound intermediate rounding magnitudes, not to discard the final x/y correlation.

This source certificate does not establish hull membership of rounded clipped polygons or their computed centroids. It also excludes color functions/conversion, blend/group propagation, accumulated area/first-moment error, pixel unpremultiplication and display conversion. Those remain required before an integrated centroid replacement is enabled.
