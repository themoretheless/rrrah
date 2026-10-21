# Clip-line provenance controls — 2026-10-21

This validates the clip-line identity checkpoint published in a8fe2114c; production geometry is unchanged in this continuation. Exact keys distinguish nearby lines, canonicalize reversed edges / negated fields / signed zero, and keep boundary identities through clipping and hull reconstruction. No area epsilon, tiny-fragment deletion or higher memory/work limit is introduced.

## Controls

- Repeating a known cut preserves polygon points and moments; clipping its opposite has no area.
- A distinct parallel cut preserves a positive thin band on the unit right triangle. With d=2^-20 the independent analytic area is d/4+d*d/2, checked to absolute 1e-20.
- 128 non-dyadic inclined cuts preserve points and moments on repetition and have no area under opposite clipping.
- A two-key committed pool grows to 66 entries in a transaction. Every reached allocation refusal, cancellation checkpoint and shorter work quota must restore original IDs, length, allocation size and live admitted bytes; dropping the pool releases all credit. Retry additionally refills and commits all 66 keys.
- Prior independent Fraction orientation/intersection controls remain in the same test module.

The full interpreter run passed 152 tests, with one ignored. That run included the first three new controls; the later 128-case repetition control is covered by the final focused run. The final focused run passed all eight tests, including complete refill/commit retry after every injected failure. Focused results are retained in controls-final.log. The production raster dump rebuilt successfully.

## Pinned real files

`poppler.json` contains input hashes, decoder hash, reference version and per-page evidence. Strict white-composite RGB comparison at 72 dpi, with no tolerance:

- VectorApple.ai still refuses with shading decode/precision limit. Captured terminal diagnostics exhaust the shared work quota; no depth-limit failure appears in this new captured trace. This is incomplete, not a repaired page.
- one.ai page 0: 2522 differing pixels, maximum channel error 58; page 1 exact. Results are unchanged.

Four terminal region snapshots are retained in dense-region-stats.txt. Their origins and processing progress differ from the prior run, so their region counts are not a controlled speed or quality comparison. Numerical tiny fragments still occur. No wall-time or GPU gain is claimed.

Next: reduce ownership partition work on dense overlapping source triangles without discarding positive area or raising limits. Full AI, all 100 formats, long memory-pressure collections and hardware/HDR qualification remain open; NVIDIA remains deferred by the user.
