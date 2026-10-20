# Local conforming-fan refinement and bounded dense pixel integration

Projected capture now dry-runs the actual conforming fan triangles before publishing output. Only failing leaf cells are forced to split. Neighbor leaf edges and shared patch boundaries are rebuilt each pass. The pass count is bounded by the supported depth; triangle count, admission and cancellation guards remain active. This replaces the rejected global-tightening pilot, which refined unrelated cells and still hit geometry limits.

The deterministic authored numerical control (seed 20261021 + 2*101) exercises 29 forced leaves in two passes and checks every emitted triangle's component bound. A two-patch control checks bit-identical shared boundary samples after refinement, sampled admission failures, cancellation and retry under 8 MiB. Exactly zero-area retained triangles are omitted from projected publication using the existing filtered/exact orientation predicate. A subnormal nonzero triangle is explicitly preserved; nonfinite inputs are not classified as zero area.

## Dense sampled pixels

The original full-page failure moved from Depth/ComponentAccuracy to a 4096-region pixel partition limit with tens of millions of work units still available. Dense pixels now integrate recursively in square subcells, preserving original source order and transforming owner affine fields with the exact power-of-two local scale. Child integrals return device-pixel area, and compensated sums combine coverage and components. Depth and shared work bound recursion. Partial partitions are discarded and release their reservations before subdivision.

A retained working set of 512 fragments triggers subdivision earlier instead of spending the quota building thousands of fragments. Bounds on each existing 128-record block skip whole blocks outside a tile/subcell without changing record order. Block metadata is included in the existing capacity admission. The shared 100M work allowance and source 1/64 pixel, 1/1024 component tolerances were not raised.

A 512-triangle authored grid with linear u/v and bilinear uv colors verifies exact coverage and independently calculated piecewise-affine integrals after subdivision. It also checks admission failures, cancellation, short work, credit release and retry under 8 MiB.

## Full page and validation status

Vector 217 passed / 11 ignored; native 319 / 13. Current pinned VectorApple.ai still refuses, so the compatibility regression is not closed. one.ai page 0 retains 2522 differing pixels and page 1 is exact. Earlier full-page reports and each intermediate failure are retained separately. Removing color/depth refusals and reducing unnecessary work does not establish full AI fidelity, whole-page speed, 100-format support, GPU or physical HDR qualification.

## Retained depth/capacity policy

The four-level pilot reached its explicit recursion bound with substantial work still available. Eight levels also reached its limit on a tiny dense cell. The retained policy uses at most eight subcell levels and permits the original 4096-region capacity at the deepest leaf, after starting with a 512-region working set. It never drops unresolved geometry or substitutes point samples. The same grid compressed to 1/16 and 1/256 pixel edges verifies the deeper traversal and retained leaf capacity against exactly scaled integrals.

Full interpreter suite before this depth/leaf extension: 143 passed / 1 ignored. The extended dense-cell control then passed separately, including the microscopic scale case. Initial synthetic family controls that did not actually exercise refinement were rejected; the retained fixed family 2 does require refinement. The final full-page outcome is recorded separately in leaf-capacity-poppler.json, which supersedes intermediate refusals.

## Final page outcome

The retained leaf-capacity run still exits 1: VectorApple.ai refuses. Diagnostics now show dense-cell work exhaustion in several shadings and an eight-level deepest-cell capacity refusal in another; the earlier component/depth tessellation failures are gone. This is progress in source geometry acceptance, not closure of the public-file regression. Further work must reduce overlapping owner candidates/partition growth rather than keep increasing limits. one.ai page 0 remains 2522 differing pixels/max58/MAE0.03313926, page 1 exact. No timeouts occurred.
