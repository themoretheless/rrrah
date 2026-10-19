# Tensor folded coverage and partial-overlap defects

This investigation establishes two current PDF/AI rendering defects. Production
rendering is unchanged; the new exact regressions intentionally fail when run
explicitly. They are marked ignored as known failures, not passed qualification.
The existing coincident-patch regression still passes.

## Independent upstream boundary investigation

The current native dump reproduces the same 22 selected solid Tensor pixels with
more than 32 RGB8 levels disagreement against PDFium at16x followed by BOX
averaging; the maximum is54. The cutoff only selects investigation pixels and
is not an acceptance tolerance. Every source/derivative remains SHA256-pinned to
the previous SerenityOS manifest; artwork stays outside the checkout.

Transparent-background alpha comparison establishes native colored/solid alpha
is exactly equal across the whole page. PDFium16 changes431alpha pixels when
only source color codes become constant, maximum33. This identifies reference
approximation sensitivity, not an excuse to accept the native result.

`integrate-area.py` independently parses the16-bit source controls, flags0/2/3/1,
and the explicit page translation. The Tensor basis, stream order and continuation
rules follow [PDF Reference1.3 pp240–242](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.3.pdf#page=255).
No native triangles or renderer pixels define its surface or projected union.
Uniform parameter grids32/64/128/256/512/1024 are projected to triangles and
unioned with Shapely2.1.2/GEOS before pixel intersection. At the22selected pixels,
maximum512→1024alpha change is0.01048level. This is numerical convergence,
not an interval/analytic/full-page certificate.

Against the final projected-union estimate, native alpha differs by up to32.6851
levels; PDFium16 differs by22.1929. Native alpha instead agrees with the summed
triangle areas (clamped to255) within0.5791level. Per-patch unions equal the
combined union at these pixels: the extra multiplicity arises inside folded
patch geometry, not from another patch at those selected points.

The renderer implementation in `vendor/hayro-interpret/src/encode.rs`,
`sample_generated_triangles_with_admission`, adds each triangle's covered area
and area-weighted color into one pixel sample, divides color by total coverage,
and clamps coverage to one. This cannot distinguish projected union from
multiple mappings, or retain later ownership in partially overlapping patches.

## Exact authored controls

[PDF Reference1.3 p234](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.3.pdf#page=249)
requires folded mappings to resolve by greatest v, then greatest u, and later
patches to cover earlier patches. The owned CC0 generator and two3x3controls
provide exact expectations without choosing a reference renderer:

- `tensor-folded-solid.pdf`: x(u)=6u(1−u), y(v)=3v. Its black projected image is
  [0,1.5]×[0,3], so the second column has alpha128. Current output has255 in all
  three second-column pixels: three differing pixels, maximum127levels.
- `tensor-partial-overlap.pdf`: red rectangle[0,1.5] followed by blue[.5,2], both
  full height. The first column averages equal red and blue; the second is all
  blue. Current output instead averages multiplicities (170,0,85) and(85,0,170):
  six differing pixels, maximum85levels.

The two Rust regressions decode under8MiB, release the image and verify managed
root zero before comparing all36RGBA bytes to these mathematical expectations.
Explicit execution confirms both failures. The ordinary overlap module gives
1passed/2ignored; explicit ignored run gives0passed/2failed, as recorded.

## Required implementation work

Replace multiplicity averaging with admitted geometric partition/ownership
integration. Source patch order and folded parameter priority must survive
adaptive tessellation; source components/functions must be evaluated only for
the winning mapping. Cover solid union, colored overlap and folded priorities
without special-casing these fixtures. Preserve cancellation, bounded work,
allocation admission, fractional alpha, existing nonoverlap accuracy and actual
Metal/cache transport. No production policy, tolerance or memory budget was
changed here. AI remains pending; original full-page mismatches remain failures.

## Reproduction and evidence

`probe.py` and `compare-alpha.py` use the bundled Python with numpy/PIL/PDFium.
`integrate-area.py` additionally uses Shapely installed only into a temporary
Python path, never as a runtime dependency. The scripts accept explicit corpus,
dump and report paths. `scripts/generate-pdf-tensor-overlap-controls.py` regenerates
the owned fixtures/manifest. Run `cargo +stable test -p rrrah-vector --lib
overlap_patch_tests --locked --offline` for the existing pass and known skips;
add `-- --ignored --nocapture` to reproduce the two failures. Raw logs and
numerical per-pixel reports accompany this document. No source artwork is copied.
