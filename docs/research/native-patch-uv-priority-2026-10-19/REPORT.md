# Native Tensor point ownership uses original patch UV — 2026-10-19

## Reproduction

An unshifted folded control passed the old triangle-order point selector, so it was not used as evidence that a fix was needed. The counterexample has tensor control columns x=[0,3,1,0] and y=j+0.75*i/3, with first native CMYK component u. Therefore x(u)=3u(1-u)(3-2u), y(u,v)=3v+0.75u. The maximum x occurs at u=(5-sqrt(7))/6. When both branches project onto the same device point, greatest v means the **smaller** u, independently of adaptive emission order.

At x=0.1,y=0.8 the old native point sampler returned 0.9681696891784668, while independent 64-step bisection gives the greatest-v branch u=0.011323859392943357. The original failing assertion and the unshifted passing pilot are retained.

## Change

Tensor native capture now retains admitted patch-index/u/v affine priority fields for each native triangle. A second ordered metadata stream uses the same existing compact tessellation settings (1/64 pixel, 1/1024 parameter-component bound, depth 12, 1,048,576 triangles). Every triangle index, device coordinate bit and component bit is compared against the retained compact geometry before publishing priorities. Exact parameter identities are applied where proved. Priority capacity and temporary reference/identity vectors are admitted before allocation.

The native spatial point query selects later patch, greatest v, then greatest u. Triangle-index pruning is retained for original triangle meshes and disabled for patch priorities, because emission index cannot bound source UV priority. Priority comparisons debit shared work. Nonfinite or unresolvable fields refuse rather than publish incomplete metadata.

`mesh_triangle_patch_owner` exposes unconverted component fields and source priority relative to a device pixel origin for the next area-integration step; `mesh_has_patch_priority` distinguishes those sources. Original triangle meshes preserve their index ownership and return no patch owner. Point selection remains a numerical tessellated-source approximation, not exact nonlinear inversion or an interval certificate.

## Validation and limits

The mathematical fold regression checks 190 points against the independent monotone-branch inverse with absolute u discrepancy below 0.01. A separate test covers reflected/sheared/translated device coordinates and later-patch override. The admitted test caps capture at 8 MiB, checks owner UV/source-component reconstruction at visited vertices, injects refusal at every exercised capture admission, cancels at first/middle/final metadata-stage checkpoints, verifies zero retained accounting after every failure, and retries successfully. Work-zero/cancel/invalid-index refusals are explicit.

The regular vector suite passes 208 tests, 0 failures, 11 ignored, including unchanged 1 MiB planar and 8 MiB curved PDF controls. The final vendor-source suite passes **139 tests, 0 failures, 1 ignored**. See vendor-final.log. Targeted formatting and Git whitespace checks pass. The isolated harness is the same original-source manifest documented under ../ordered-patch-sampling-2026-10-19/validation-harness.toml.

This does **not** fix full native area integration: polygon/spatial replay still has triangle-rank owner selection and must consume source UV fields with crossing-aware geometric partitioning. Native Coons capture remains unsupported. Existing native compact capture has not been switched to the projected-component checked stream; its diagnostic bound is not a general projected source-component certificate. Second-pass capture cost and overlapping-point traversal cost have not been paired-benchmarked. Full-page AI fidelity, 100-format qualification, prolonged GUI endurance, physical HDR and NVIDIA/CUDA remain open.
