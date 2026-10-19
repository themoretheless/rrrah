# Late EXR unassociation overflow boundary

An independent OpenEXR 3.5.2 producer creates a four-pixel FLOAT Rec709/D65 row. Opaque, fractional and signed-zero transparent samples precede a final finite +/-f32::MAX sample with alpha=f32::MIN_POSITIVE. The reference reader confirms exact source bits and infinite quotients if unassociated. The CC0 source, regeneration script and hashes are retained in scripts/ and tests/fixtures/raster/.

The native Rust decoder retains the whole row as associated linear float, including the earlier fractional pixel and negative zero. Display preparation is bit-exact, shares the managed pixel allocation and releases it to zero. The focused decoder regression passes.

The cache integration restores all source bits from swap, retains association and validates both CPU and swapped area LOD against [f32::MAX/4, -f32::MAX/4, 1.25, 0.375]. Both managed pixel and swap-index budgets release to zero. All three associated-cache integration tests pass.

This qualifies CPU representation and cache/swap arithmetic at finite-f32 extremes. It does not qualify f16 GPU render targets or physical HDR for f32::MAX; these values exceed the finite range of f16. The prior moderate-HDR Metal oracle remains separate evidence.
