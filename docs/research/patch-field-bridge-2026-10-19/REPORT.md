# Triangle provenance to local source-field bridge

The adaptive-stream provenance is now connected to the admitted pixel kernel in
controlled tessellator→field→partition tests. This is not yet a production PDF
sampler route. One stronger colored-fold control still fails; the original
full-PDF controls remain known failures as well. AI remains Pending.

## Implementation and corrected geometry invariants

`PixelSourceOwner::from_triangle` computes local affine source components and
u/v fields from a device-space triangle and its original per-vertex parameters.
It performs no allocation, checks finite/component/parameter bounds, uses a
compensated determinant, rejects unresolved/singular inversion explicitly, and
checks reconstruction at all source vertices. The reconstruction error bound
includes the magnitude of all input values, necessary at zero-valued vertices.
This is a numerical affine-triangle check, not an interval certificate for the
original curved surface or equality of fields fitted to different triangles.

The connected curved test exposed defects not visible in the first small kernel
controls: repeated clipping created tiny nonconvex/invalid rounded vertex runs,
which violate the convex half-plane subtraction invariant. A retained bad-pilot
trace independently checked with GEOS contains1622regions,102invalid rings and
171clockwise rings; summed area1.0004407342573187 versus union0.9999999999999983.

The kernel now avoids cutting polygons with disjoint bounds, computes moments
relative to a polygon vertex, and evaluates geometric half-planes relative to an
edge anchor with compensated products. Each clipped vertex set is normalized to
its convex hull without an epsilon or a small-area discard threshold. The same
fragment/vertex/work limits and8MiB test root remain. Numerical convex-hull
normalization is not a general exact/interval clipping certificate.

## Passing evidence

Full vector suite:198passed, zero ordinary failures,14ignored. Three bridge
controls pass:

- Reflected/sheared transformed triangles reconstruct their original source
  components and u/v; singular and nonfinite input return explicit errors.
- Partial red/blue patch overlap reproduces all36expected RGBA8bytes of the
  exact3x3source-order control. Four triangles,13fragments,16,384-byte managed
  peak in the recorded diagnostic pilot; final root zero.
- The folded black surface x(u)=6u(1−u), y(v)=3v produces projected-union coverage
  [255,128,0] on each row. Coverage never exceeds the unit pixel in the final
  test. Quantization includes the renderer's existing f32 sample/coverage stage;
  raw f64 rounding alone had exposed a127/128half-coverage rounding tie.

The six prior kernel tests still cover affine integration, priority crossings,
allocation/cancellation/work/fragment failures and transactional retry. Source
components/functions remain unaveraged per winning fragment in the kernel.

## Still failing: colored folded parameter priority

The stronger control colors the same fold by source u. The greatest-u inverse
is (1+sqrt(1−2x/3))/2. Its analytic pixel integrals give RGB230and177in the first
and second columns, with alpha255and128. The connected candidate instead yields
rows222/171,230/176and220/172: maximum RGB error10. Explicit execution of the
known ignored test reproduces this failure; it is not counted as qualification.

Although this source has exact global y=3v, independently fitted triangle v
coefficients differ from the shared affine identity by up to5.11591e-13. The
kernel's exact lexicographic comparison of supplied fields therefore does not
establish the original surface's v-equality/u-priority. Small field differences
can choose a visibly different folded branch. Coherent/certified source
parameter identities or original-surface branch resolution are required before
production use; vertex reconstruction alone is insufficient.

This9pixel colored diagnostic emits2496triangles, retains6495total fragments,
uses5,407,984peak managed bytes, and consumes3,165,802units of the shared work
budget. Its root returns to zero. These costs are not practical full-page
qualification or a speedup; bounded spatial candidate processing remains work.

## Reproduction and artifacts

`cargo +stable test -p rrrah-vector --lib --locked --offline` runs the ordinary
passing suite. To reproduce the current colored failure, filter
`patch_bridge_colored_fold` and add `-- --ignored --nocapture` (expected exit101).
`RRRAH_ORDERED_PIXEL_TRACE` optionally writes failed-partition geometry only for
these owned diagnostic tests; no external/private artwork is copied.

Final and significant pilot logs accompany this report. The bad-pilot geometry
witness and GEOS summary are explicitly historical diagnostics, not final output.
No production PDF path, display setting, memory policy or acceptance tolerance
was changed. Next work must resolve folded priority, then bound page-level cost,
nonlinear source integration and actual AI/RAM/swap/Metal qualification.
