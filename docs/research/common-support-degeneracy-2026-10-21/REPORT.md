# Common-support degeneracy — 2026-10-21

## Diagnosis and correction

The deep cell at origin (151.0625,59.9375), side 1/256, previously refused after processing 49 of 51 candidates with only 119 published fragments. Separate opt-in diagnostics show a partition vector reaching its existing 4096-fragment maximum, not the 16-vertex bound. Published fragments alone had hidden temporary growth. Diagnostics do not allocate or alter the limit/fallback policy.

A polygon's vertices may all belong to one original support even if its incident edge labels are not all equal. The previous symbolic-zero test required every edge label to match. The corrected test considers the two supports incident to vertex zero and checks whether one candidate support contains every vertex through an incident edge. A common nonzero support proves a line, so floating-point offsets cannot give it positive area. This uses identity, not an area epsilon. Unknown/independent constraints retain ordinary area evaluation. The new rotated unit control checks mixed [7,7,8] edge labels; clearing the labels retains the represented positive area. Existing distinct-line thin-band controls remain required.

## Exact real-input control

A temporary selected-cell diagnostic captured 50 bit-exact inputs, including the failed input at zero-based record 49. It is removed from production. Permanent data are in tests/fixtures/geometry/dense-lineage.json and its generated Rust counterpart.

- Before: record 49 refuses with Limit, 119 prior fragments; the full trace confirms a 4096 vector cap.
- After: all 50 captured records paint successfully under the same 4096 fragment, 8 MiB memory and 100M work bounds. Final published fragments: 3427. Painting consumes 425274 logical ticks. Prior and final counts describe different processing progress and are not a speed comparison.
- Independent exact union: 0.8462670619378092. The native summed area matches to absolute 1e-13 (it prints the same f64 value).

scripts/generate-dense-lineage-control.py uses Python Fraction on the exact represented coordinates, clips triangles to the unit square, collects all vertices and segment crossings, and integrates merged vertical intervals over the resulting slabs. Midpoint integration is exact between events because all boundaries are affine and their order cannot change there. The control contains 50 clipped polygons, 162 nonzero segments and 298 x-events. This sweep is independent of the Rust fragment ownership/line-ID implementation. It checks union area; it is not a complete source-color/priority certificate.

## Verification and remaining real-page failures

Current vector suite: 222 passed/11 ignored, zero failures. Interpreter/native final results are appended below. The independent five-triangle support regression, thin-band controls and existing admission/cancellation/retry gates remain enabled. No acceptance tolerance, memory/work bound or format status was relaxed.

Pinned AI comparison remains unsuccessful for VectorApple.ai. Because the qualification report keeps only bounded stderr, a second run of the same binary captured full stderr to resolve whether the original cell was gone. The complete trace has no terminal refusal at (151.0625,59.9375) and no coverage-over-one refusal. Remaining terminal events:

- origin (170.8125,58.9375), side 1/16: shared work exhausted.
- origin (138.4296875,39.08984375), side 1/256: deep bounded partition refusal, 74/91 candidates processed, 855 published fragments.
- origin (74.29296875,102.8515625), side 1/256: shared work exhausted, 308/409 processed, 1508 published fragments.

one.ai page 0 remains 2522 differing pixels/max channel error 58; page 1 exact. poppler.json retains input and decoder hashes. No full-page correctness, wall-time/GPU gain, 100-format or hardware/HDR completion is claimed. NVIDIA remains deferred.

Next: capture the (138.4296875,39.08984375) bounded partition sequence, distinguish genuine ownership complexity from redundant constraints, and preserve exact area while addressing work growth.

Final library results: interpreter 154 passed/1 ignored; native 319 passed/13 ignored. Zero failures.
