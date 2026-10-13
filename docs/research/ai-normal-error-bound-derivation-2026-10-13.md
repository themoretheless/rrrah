# Fixed-alpha Normal error propagation

Scope: one native Normal paint step with the same shape and alpha values on both sides of the comparison. Previous and backdrop error bounds refer to additive premultiplied storage; source error bounds refer to native components before Pixel::from_native. Coordinate identity is a caller obligation. This helper does not certify a route or group boundary.

Exact Normal algebra for a component is

    C = a*Cs + (shape-a)*Cb + (1-shape)*Cp

because the source overlap blend equals Cs. Thus absolute errors propagate with nonnegative gains a, shape-a and1-shape, respectively. CMYK complementation changes source discrepancy sign without changing its absolute gain. The helper encloses coefficient arithmetic and positive bound accumulation outward.

A256*f64::EPSILON absolute allowance covers conversion of a checked0..1source component to premultiplied storage (including CMYK complement), source normalization and the ordered Normal arithmetic. Every actual color term is bounded: inputs are checked native samples, opacity/shape lie in0..1, and previous/backdrop storage components lie in0..1. Positive source alpha below1e-12 refuses, preventing normalization of subnormal premultiplied data from being admitted by this envelope. Alpha0 follows the sampler's explicit zero-source branch. This source normalization envelope is intentionally conservative; it is not a claim that general Pixel::valid tolerances alone establish these input assumptions.

Alpha discrepancy is excluded: alpha/shape must be the same constants over the compared field. Group removal, unpremultiplication, color transforms and nonlinear blending require separate error propagation. Unchanged constant subgroups may be treated as exact constant inputs only after their exact field/route identity is proved.

Tests compare4608pairs of actual Pixel::composite outputs across Gray/RGB/CMYK, source/background opacity and shape combinations, yielding12288active component comparisons. Invalid bounds, inactive channels, alpha/shape, tiny alpha, every insufficient budget and every cancellation checkpoint refuse. Original AI diagnostic evaluates the source contribution of one Normal step with zero preceding error; that measurement is not the full rendered-pixel error.
