# Strict-side classification before complementary clipping

The previous shared split walks every vertex and constructs both complementary outputs. The new split first caches signed distances and known-support flags in fixed VERTICES-sized stack arrays, charging/checking cancellation once per input vertex as before. Every vertex is evaluated once (four rather than five evaluations in the square control). Strictly positive or strictly negative subjects retain the whole polygon with the existing finish_clip canonicalization and an empty opposite output. No boundary tag changes in these strict cases. Zero-distance, known-on-support and mixed-sign inputs take the ordinary shared construction using cached distances, preserving inclusive boundaries. Finalization, original support evaluation, limits, quotas and accounting are unchanged. There is no epsilon or area clamping.

Cached mixed-case assembly is bounded by 16 vertices after the cancellable distance pass. The input polygon remains borrowed and only tentative output is built; caller transaction/rollback is unchanged. The previous shared walk is retained under cfg(test) only, as a direct benchmark control. Production builds omit both test control selectors and the old walk.

## Correctness

12,360 comparisons match both legacy clips bit-for-bit in retained vertex coordinates and edge IDs: 515 independent intersection inputs, tagged/untagged triangles, axis/oblique fields, and unknown/matching/nonmatching support IDs. Existing independent Fraction intersection/union regressions remain enabled; equivalence alone is not an independent oracle. Every prefix of the 138-record replay preserves area, fragment count and final work (37108 units). It ends at area 1, 93 fragments and max floating pair overlap 7.806255641895632e-18. New-case admission/cancel/short-quota rollback and retry controls pass with budget release.

Final checks: vector 227 passed/11 ignored; interpreter 160 passed/3 ignored; native 319 passed/13 ignored; focused kernel 15 passed/2 manual probes ignored. The enlarged equivalence control was additionally run after the full suite. No failures.

## Direct CPU comparison

Run the dense ignored CPU probe with RRRAH_SPLIT_PROBE_WALK_CONTROL=1 to select the previous shared walk as baseline, rather than the original two clips. Both paths run serially in one release test executable on M4 Max with opt-level=3, CGU1, thin LTO. Warmup, six AA controls and sixteen alternating AB/BA pairs use the same committed inputs; long block sizes/method are as documented in ../shared-split-cpu-2026-10-21. The test-only selector exists in both comparison paths. Benchmark admission credits omit real budget bookkeeping. Raw blocks, exact settings and analysis are retained.

| Workload | Mean elapsed reduction | 95% bootstrap interval | AA interval |
|---|---:|---:|---:|
| Pair | 0.84% | -0.02..2.05% | -1.36..0.56% |
| Dense50 | 13.60% | 13.21..13.91% | -0.56..0.50% |
| Dense75 | 11.77% | 11.31..12.25% | -0.44..0.52% |

The pair is inconclusive. Dense fixtures show a local benefit beyond AA variation. These same-session CPU-block intervals are not page/viewer/GPU/FPS claims and do not establish behavior for all files or machines. No work-budget savings are claimed for this step: local counts remain identical. Recompute with `python3 scripts/analyze-split-cpu.py <release.log>`.

## Full-page boundary

The frozen comparison executable was built successfully before invocation. All four VectorApple.ai attempts still exhaust 100M; no coverage-over-one refusal is observed in the captured terminal stderr. one.ai remains unchanged (page 0: 2522 differing pixels/max58; page 1 exact). Faster local work does not establish full AI support or viewing speed. All-format, long pressure and physical hardware gates remain incomplete.
