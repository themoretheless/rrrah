# Rejected retained-polygon bounds cache trial

The actual AI source path uses OrderedPixel to partition overlapping patch triangles. The preceding bulk-carry report suggested a range index/shared storage as a possible next step. This narrower trial tested whether caching each represented fragment's exact four-coordinate bounds, plus computing incoming bounds once, could eliminate repeated vertex scans without changing geometry or work caps.

The candidate adds32bytes of bounds to each Fragment. Admission continues to use size_of::<Fragment>; vector growth admits replacement before releasing old credit. Existing polygons carry bounds unchanged; a convex merge invalidates the cache and rebuilds it. No tolerance, operation/depth/fragment cap or precision changes. A test-only rescan control recomputes original bounds in overlap checks. Generated retention/rollback checks now also verify cached bits; an additional control compares every dense50 and captured138-input prefix's owners, vertices, edge tags, fragment count, bounds and remaining work exactly.

## Correctness evidence for the candidate

Vector library255passed/11ignored. The isolated interpreter test target, pointing to the repository source with the repository's path patches,166passed/3ignored. Ordered-pixel subset21passed/2ignored, included in that full interpreter log. These cover the new prefix equivalence and existing admission/cancel/short-work rollback controls. None of these tests establishes whole AI correctness.

A successfully built normal raster_fixture_dump was then frozen by scripts/qualify-ai-upstream.py and checked against the two SHA-pinned AI sources and Poppler. VectorApple.ai still refuses with InvalidPdf("shading decode/precision limit"). one.ai page0 remains2522different pixels/max channel error58; page1 is exact. The resulting candidate-poppler.json is a negative qualification result, not a successful render. No page-speed comparison is inferred.

## Local CPU controls

The existing dense_subtraction_cpu_pairs probe ran in an optimized release test executable with RRRAH_BOUNDS_PROBE_RESCAN=1, serially after builds and correctness runs finished. Each run contains warmup,6AA controls and16alternating AB/BA pairs for pair/dense50/dense75/sparse64; raw initial and repeated logs are preserved and scripts/analyze-split-cpu.py produces the summaries. The baseline and candidate share the enlarged Fragment layout and publication cache construction. Consequently this isolates repeated lookup arithmetic; it does NOT measure the complete cost of changing the old production layout to the new layout.

The repeated run reports pair -0.07% (95% interval -0.72..0.51), dense50 0.79% (-0.17..1.96), dense75 0.35% (0.04..0.65), sparse64 1.90% (1.12..2.57). Dense50 AA controls are unstable, and dense75's candidate interval overlaps its AA variation. The initial run also has large pair/dense50 noise. Only the sparse lookup component has a small repeated positive signal; it is not a general/page/GPU gain.

## Decision and restored state

Rejected. These controls do not justify32extra bytes on every retained fragment, do not test the full layout tradeoff, and do not resolve the real AI refusal. The complete candidate is retained in rejected.patch and its unique test/negative/performance evidence in this folder. The source is restored byte-for-byte to its pre-trial SHA256; no candidate production code or rescan selector remains active. Existing unrelated work is preserved. Rebuilding the restored public example and rerunning the unchanged ordered-pixel controls are recorded separately below.

The remaining useful direction is admitted spatial range/shared-fragment storage that can skip proven disjoint spans and copying, rather than another per-record metadata cache. It requires independent geometry and exhaustive rollback tests before promotion. AI and the full100-format/physical-hardware objective remain incomplete. This CPU-only experiment provides no new Metal, CUDA, NVIDIA or physical HDR claim.

Restored-state checks complete: normal raster_fixture_dump rebuild succeeds; ordered-pixel20passed/2ignored. Restored source hash equals the exact pre-trial bytes; sources.json pins the rebuilt decoder separately from the rejected candidate.

## Incoming-only follow-up: rejected, no resident-layout growth

A second trial computes only the incoming triangle/visitor clip bounds once, leaving Fragment and all admitted allocation sizes unchanged. Existing old polygons still scan their own vertices. The original comparison is available under a test-only rescan control. Both versions share identical structure sizes and arithmetic. The final control also omits the precomputation in rescan mode, matching the original timing path's lack of an upfront scan.

The21 ordinary ordered-pixel tests pass, including dense50/captured138 prefix comparisons with equal geometry values, tags, owners, fragment counts and remaining work. These comparisons use floating value equality for geometry/fields, not an independently asserted signed-zero bit identity, despite the experimental test name. The limits and admission mechanism are unchanged.

The first two probes before the final upfront-scan correction showed approximately2.4% and3.0% sparse-case gains; they are preliminary controls, not a performance gate. The final first run reports sparse2.10% (1.52..2.74), but its AA interval is -32.45..15.33%, so the run is invalid as evidence of that gain. The final repeat gives sparse1.96% (-1.32..3.90), which crosses zero. Pair AA has a small directional bias; dense50/dense75 candidate intervals also cross zero in the repeat. Raw noisy/preliminary runs are retained with distinct names. There is no qualified general or sparse improvement, and no page/GPU claim.

Rejected rather than retrying controls until a favorable sample. incoming-rejected.patch retains the complete candidate and test-only selector. Source is again SHA-identical to the previous restored baseline. The ordinary raster_fixture_dump was never rebuilt from this second candidate and its SHA still equals the previously rebuilt baseline; no experimental normal executable is left active. The next direction is admitted range/shared-fragment storage that skips whole proven-disjoint spans, not more per-operation bounding-box micro-optimization. AI and the overall objective remain open.

Restored kernel recheck after incoming-only rollback:20passed/2ignored, zero failures (incoming-restored-kernel.log).
