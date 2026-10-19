# Independent upstream Type6/Type7 qualification boundary

Pinned SerenityOS tests at revision31ed5be3770a30cd70b2d83af7047aa65408ba90 provide two300x520 pages. Original files remain outside the checkout; sources.json records immutable raw URLs, byte lengths and SHA256, verified before either renderer. [Original Coons source](https://github.com/SerenityOS/serenity/blob/31ed5be3770a30cd70b2d83af7047aa65408ba90/Tests/LibPDF/shade-coons.pdf). Coons contains two Type6 dictionaries, Tensor two Type7. No claim of complete document support follows from these examples.

Strict72dpi white-composite comparison against Poppler26.10.0, no tolerance:

| Page | Previous differing pixels / MAE | Current differing pixels / MAE |
|---|---:|---:|
| Coons |42772 /1.1577777778|42556 /1.1553589744|
| Tensor |42373 /1.1367179487|42373 /1.1367179487|

Maximum channel error255 in both cases. Both comparator commands intentionally exit1. Previous dump binary uses adaptive Tensor but fixed Coons; current uses adaptive both. This is a bounded before/after result: Coons improves slightly, not fully qualified. Raw binary hashes and all metrics accompany the report. No timing comparison was made.

The two pages each paint /Sh1 then /Sh2. Same-length replacement of one sh invocation by spaces isolates ordinary DeviceRGB multi-patch color from scalar shading through a stitching function. xref offsets, source stream lengths and all remaining bytes stay intact. isolate.py checks pinned originals and derived SHA256s before writing derivatives outside the checkout. Run qualify.py with --manifest isolated-sources.json to reproduce. Do not use --fetch on derived sources: their upstream URL identifies their parent, not a directly downloadable derivative.

| Isolated source | Different pixels | MAE | Max |
|---|---:|---:|---:|
| Coons RGB Sh1 |25996|0.4812115385|255|
| Coons function Sh2 |16560|0.6741474359|255|
| Tensor RGB Sh1 |25907|0.4733205128|255|
| Tensor function Sh2 |16466|0.6633974359|255|

Both branches differ, so function handling alone cannot explain the full mismatch. Narrow synthetic analytic coverage remains proven separately, not a certificate for this broader geometry/color case. Next work should isolate edge coverage, shared continuation geometry and source-color integration on these pinned cases before changing policy or tolerances. CPU/GPU transport, physical HDR and complete-format qualification are separate gates.

## Solid coverage and independent PDFium convergence

New isolate-solid.py replaces only source component codes by zero in the original ASCIIHex mesh streams, including continuation records. All positions, flags, control points, stream lengths, xref offsets and total file lengths are preserved. Both source and derivative hashes are recorded and checked. RGB becomes black; the scalar function at zero becomes red. Thus coverage can be tested independently of varying color. No parent artwork or derived PDF/image is copied into the checkout.

Solid cases still differ against direct72dpi Poppler by964/731/955/731pixels, maximum255, so varying source color is not the sole cause. Visual comparison of Coons RGB reveals a stepped Poppler contour versus smooth native coverage. That observation alone does not establish which is correct.

Bundled pypdfium2 provides an additional independent renderer. compare-pdfium.py renders each page at1/4/8/16scale then averages with PIL BOX at source resolution. Reference engine/version, native binary hash, scales and raw metrics are recorded. BOX supersampling is a numerical approximation, not exact mathematical area or a new acceptance tolerance.

| Solid source | PDFium1 max / MAE | PDFium16 max / MAE | Pixels above32 at16 |
|---|---:|---:|---:|
| Coons RGB |255 /0.761955|20 /0.050013|0|
| Coons function |255 /0.396081|17 /0.025128|0|
| Tensor RGB |255 /0.751385|54 /0.056378|22|
| Tensor function |255 /0.396081|17 /0.025128|0|

All four original colored isolated cases have max20/17/20/17 against PDFium16, with zero pixels above32. Lower-resolution255-level boundary differences substantially converge as reference resolution rises. This contradicts treating every direct-Poppler255-level boundary mismatch as proof of a native geometric defect. It does not prove all shared continuations or geometry correct; the22Tensor-solid pixels above32 remain open. Direct strict Poppler failures remain failures and are not reclassified as passing.

## Interior color remains a separate open issue

compare-interior.py intersects fully covered pixels of native and PDFium16 solid counterparts, then erodes the mask by2pixels (5x5 minimum filter). Only matching fully solid interior areas are used for colored comparison; boundary pixels are excluded. No pass threshold is introduced.

| Source | Interior pixels | Max color error | Interior MAE | Pixels above8 |
|---|---:|---:|---:|---:|
| Coons RGB |25606|6|1.591840|0|
| Coons function |23587|15|2.396405|2959|
| Tensor RGB |25608|6|1.597704|0|
| Tensor function |23587|15|2.357867|2922|

Interior color differences remain even after boundary separation. Their cause and which renderer is closer are not established. Next useful evidence is independent source-parameter inversion/function integration at interior pixels, rather than changing tessellation depth or tolerances to chase a low-resolution renderer. This investigation changes no production rendering code, memory/GPU budget, or format qualification claim. Prior native analytic and Metal transport tests remain separate evidence.

Reproduce using the bundled Python containing pypdfium2/PIL: isolate.py CORPUS; isolate-solid.py CORPUS; qualify.py CORPUS --manifest solid-sources.json --dump DUMP --report solid-poppler.json (expected exit1); compare-pdfium.py CORPUS --dump DUMP; compare-pdfium.py CORPUS --manifest isolated-sources.json --report pdfium-colored.json --dump DUMP; compare-interior.py CORPUS --dump DUMP. Existing manifests/parents are hash guarded; derived manifests point to parent URLs and must not be fetched as original bytes.

## Independent source-parameter/function integration

integrate-function.py decodes the pinned single-patch Coons Sh2 control net directly from16-bit ASCIIHex coordinates and8-bit scalar corners [0,255,255,0]. It verifies the declared Decode and exact Type2/Type3 functions before evaluation. Independent double-precision cubic boundary/transfinite blend, finite-difference Newton inverse, bilinear scalar v and cubic stitched tint define the source function. No native mesh or reference-renderer output enters the expected RGB calculation.

Eighteen selected interior pixels (three u seeds, six v seeds away from boundaries/stitch) have one center inverse found across nine initial guesses. This sampling is not a global injectivity proof. Midpoint integration at4x4/8x8/16x16 converges: maximum8→16delta0.00041410471681047056RGB8level. Every native RGB channel equals the rounded16x16 integral; maximum native deviation0.44452739345169334level, consistent with final8-bit rounding. Only these18pixels are qualified by this numerical control. It is not an interval or complete-page certificate.

compare-function-oracle.py checks the very same source pixels against independent renderers. Maximum deviation from the integral: PDFium1x6.66746, PDFium16BOX7.73582, Poppler1x9.32518levels. Reference supersampling improves geometry but does not eliminate these selected tint/color approximations. Thus the measured interior reference disagreement does not establish a native Type3 tint defect. Native rendering is not changed to imitate the less accurate values on this control. Broader RGB nets, overlap, continuation geometry and the unresolved22solid-Tensor pixels remain open.

The Rust ignored external regression uses a pinned BLAKE3 guard,18independent integral expectations (not the native rows recorded in JSON), exact rounded-channel comparisons and managed-owner release. RRRAH_COONS_CORPUS points to the verified external derivative corpus. No source artwork is embedded in the repository and no budget is increased. Numerical script and raw per-pixel values are preserved in function-oracle.json; renderer comparisons are in function-oracle-renderers.json.

Rust regression executed successfully:1passed/0failed,18exact rounded source-integral samples. The300x520page cannot fit8MiB: admission refuses an additional2097152bytes with7161011held. This is now an explicit typed-memory-refusal control, followed by root0 and successful small-page retry under the same8MiB gate. The complete source is then tested under the existing64MiB real-AI qualification root: accounted peak10306739bytes, retained final pixel buffer624000bytes, last-owner0. No production limit is changed. The initial8MiB test expectation is preserved in function-eight-mib-refusal.log; the final dual-budget control is in function-regression.log. Full-page memory reduction remains work, not a claim that this page fits8MiB.
