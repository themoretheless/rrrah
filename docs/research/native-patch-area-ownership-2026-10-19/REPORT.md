# Native patch pixel-area ownership — 2026-10-19

## Implemented path

The test-gated normal-blend native page replay now integrates patch ownership over the pixel, even when its center is covered. It queries native triangles, retains original source patch/v/u priority in `OrderedPixel`, then intersects those disjoint regions with convex strips from the paint path and all clips. This partitions ownership where priority changes inside overlaps; sorting triangle indices or picking an owner only at the pixel center is not used for this path.

A sampling identifier accompanies each owner without affecting geometric priority. After clipping, native evaluation uses that original triangle at the fragment centroid, including bounded source-function evaluation. The new convex-clip visitor uses fixed stack polygons and shares its work budget with the caller. Polygon intersection callbacks debit the same geometry allowance; attempted callback budget increases are rejected and capped, including callback refusal. Existing prepaid legacy callback APIs retain their behavior.

The native triangle query prepays its 100,000-operation quota. Ownership, clipping and source evaluation debit the remaining allowance. Ownership remains capped at 4096 regions per pixel; capacity/query work refusals can trigger the existing depth-two bounded subdivision after releasing parent scratch. All temporary allocations remain admitted before materialization.

Pattern backgrounds occupy a lower-priority base region. Actual patch priorities are shifted above it; the background identifier bypasses source-function evaluation because the background is already declared in color-space coordinates. This preserves background color in uncovered portions of a partially covered pixel. A CC0 Type 7 pattern fixture has a black fold over green, independently expecting black / half-green / green opaque columns.

## Rejected pilots and numerical correction

Initial exact native page replay refused a represented coverage of 1.0000000000000007. Coverage is now capped to the tile area only after the existing small numerical-overcoverage check; significant overcoverage still refuses. The next pilot produced alpha 127 instead of 128 in one exact half-covered pixel. Compensated accumulation of area, opacity and component integrals fixed the original unchanged byte control; no expected values or precision/root limits were relaxed. The rejected logs are retained.

The colored sheared-fold PDF was rejected during compatibility display capture even though native source capture succeeded. An explicit native-only recorder mode can now retain the native source without building its additional display sample. Default capture still builds the compatibility sample. A display replay of a native-only source refuses through an absent encoded sample rather than treating it as transparent or inventing a substitute. This mode is confined to the existing test-gated replay prototype.

## Independent controls

Full native 3x3 replay matches all 36 expected RGBA8 bytes for the exact folded-solid, red/blue partial overlap, and green-background folded-pattern controls. Expectations derive from source geometry and source order, not renderer output. The same background fixture is checked through the ordinary viewer decoder with an 8 MiB managed root.

For x(u)=9u-15u²+6u³ and y(u,v)=3v+0.75u, greatest v selects the smaller-u branch in pixel [0,1]×[1,2]. Independent 64-step monotone bisection finds u1 where x(u1)=1. The exact color integral is 4.5u1²−10u1³+4.5u1⁴ = 0.06497376506212095. Measured native integration was 0.06529714, error 0.00032337660905978, below 1/1024. Both the isolated area helper and complete native page replay enforce 1/1024; this tightens the initial 0.01 branch-selection tolerance after measuring the result. This is one source control, not a general projected-component certificate.

An even-odd clip with a central hole preserves area 3/4 and equal red/blue integrals. Its temporary operation storage is capped at 8 MiB (scene/source capture precedes that cap). Refusal at every exercised allocation and cancellation checkpoint releases operation credit, publishes no result, and allows retry. A separate kernel control checks a crossing-v priority field, exact 1/8 and 3/8 clipped areas, both clip orientations, source sampling identifiers, nonconvex rejection and immutable partitions on refusal. The polygon shared-budget control checks exact callback work debits and monotonic remaining work even after a malicious callback attempts to enlarge it.

Final checks: **316 native replay tests passed / 12 ignored; 139 interpreter tests passed / 1 ignored; 210 vector tests passed / 11 ignored. Zero failures.** The separate final background proof also confirms that the pattern background is actually retained on the native source (not supplied by an unrelated paint event). Changed kernel/vector sources and the new replay module pass targeted formatting; Git whitespace checks pass. Logs and the isolated native-replay harness manifest/lock are retained here. No production code was changed after these runs; the final additional assertion only strengthens the background test.

## Still unqualified

This implements the boundary/native page replay path. The separate fast spatial replay still selects mesh owners by triangle rank and needs source-UV integration. Native Coons capture, general projected-component error qualification for compact native capture, nonlinear source-function spatial integration, interval-certified geometric/byte bounds, and the compatibility sampler refusal on the sheared control remain open. No large-page or paired latency measurements were made; elapsed unit-test time is not a benchmark. This does not qualify full-page AI fidelity, 100-format support, long live GUI endurance, physical HDR or NVIDIA/CUDA.
