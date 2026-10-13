# Native affine evidence for actual evaluated points

The earlier hull certificate required sampler points to remain inside a supplied source-triangle hull. This evidence object instead prepares the raw-weight arithmetic envelope over an explicitly checked closed device rectangle. It borrows one immutable NativeShadingSource and binds one original owner index. Every observation checks its finite device point against that rectangle and evaluates the original selected-owner raw weight equations, without evaluating any source colors.

Let λ be the corresponding exact affine weights and r the computed raw weights. Preparation bounds Σ|r_i-λ_i| by E throughout the checked rectangle. Observation obtains D = Σmax(-r_i,0), padding the rounded three-term diagnostic sum outward. The accumulated maximum D_upper covers only successful observed points. The exact sum of positive computed weights is bounded by1+E+D_upper because Σλ=1. The existing projection/normalization/f32 component-envelope formula then applies to every successful observation. An empty observation set cannot produce a component certificate.

Outside-rectangle, invalid, exhausted or canceled observations preserve previous evidence and report failure. The caller must refuse a combined result if any required point was not observed successfully. These numerical checks do not authorize extrapolated rendering or establish correct geometric ownership relative to the original PDF.

The explicit diagnostic prepares an admitted vector for every retained candidate owner, sorts small owner indices with charged comparisons/swaps, and charges each comparison in later binary searches before observing a point. Preparation is bounded by4Munits per source and observation/finalization by16M. Failure retains baseline rendering. A new128byte sampler-capture check verifies the preexisting metadata admission assumption. The evidence vector and its credit remain owned together and release with the sampler.

The original five-pixel test observed1,352,772selected-owner points, including moment-centroid evaluations, with zero observation or finalization refusals. Source bounds are at most1.193366e-7. The1,347unowned samples retain their original fallback; unused owners are not falsely counted as observed. All baseline pixel values, identity/centroid/integral controls and peak credit are unchanged; final retained credit is zero.

No native color calculation is skipped. Ordinary4Mcommand/full-page qualification, complete combined replacement error, true clipping/source geometry and independent color/display correctness remain open.

## Reproduce test workspaces

The previous temporary manifests disappeared during this run. `python3 scripts/prepare-ai-native-harness.py` now creates standalone manifests under ignored `target/ai-native-harness-20261013/`, with library paths pointing directly to current vendored sources. It resolves copied root-lock versions offline, checks that no absent versions were introduced, and leaves the root lock unchanged. Repeated generation produces identical manifests/locks. Then run the corresponding library suites with `--locked --offline`; the renderer target remains `target/ai-white-probe-20261013` and the source target `target/ai-adaptive-edges-20261013`.
