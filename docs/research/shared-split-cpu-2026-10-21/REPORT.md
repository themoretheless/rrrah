# Shared split: paired local CPU measurements

The preceding shared-halfplane-split report establishes bit-exact geometry equivalence and reduced work units, but does not measure CPU time. This report closes only that local timing gap. The full VectorApple.ai renderer still refuses at the shared work quota.

## Method

Both old two-clip and new shared-split implementations run in one executable, with identical inputs and output handling. Warmup precedes six AA baseline controls and sixteen candidate pairs, alternating AB/BA order. Tests run serially with one test thread. std::time::Instant measures elapsed CPU-kernel work, not process CPU counters. There are no simultaneous rrrah rendering/test jobs at initial inventory; ordinary macOS/UI processes remain active. AA controls quantify local drift. Every raw block, including initial short/inconclusive observations, is retained.

The triangle microprobe traverses all 515 independent intersection inputs with four axis/oblique fields. It black-boxes inputs and both output polygons. The dense probes replay complete committed 2-, 50- and 75-record fixtures through OrderedPixel::paint_triangle, including allocation and destruction. A cfg(test) thread-local switch selects legacy subtraction for controls and is reset after each run; it is absent from production builds. The no-accounting admission callback returns zero-sized credits; real viewer memory-budget costs are outside this probe.

Long blocks use 527360 triangle splits, 8192 pair replays, 32 dense50 replays or 64 dense75 replays. The harness test profile uses opt-level=1. The separate release probe uses opt-level=3, one codegen unit, thin LTO and stripped symbols, matching the workspace release optimization settings. Both remain test executables, not the released viewer.

## Results

The first short triangle probe was inconclusive: mean 0.002% reduction, 95% bootstrap interval -0.48..0.53%. Long opt-level=1 triangles remain inconclusive (-0.18..1.32%). Dense long opt-level=1 probes reduce elapsed time by about 2.70%/6.08%/6.38% for pair/dense50/dense75.

Release mean paired reductions (positive is faster):

| Workload | Mean reduction | 95% percentile bootstrap interval | AA control interval |
|---|---:|---:|---:|
| Triangles | 1.22% | 0.89..1.56% | -0.42..0.21% |
| Pair | 5.02% | 4.48..5.49% | -0.75..0.62% |
| Dense50 | 9.79% | 9.08..10.36% | -0.92..1.06% |
| Dense75 | 9.50% | 8.96..10.25% | -1.46..0.84% |

Intervals describe these local paired blocks (deterministic 20000-resample percentile bootstrap of mean per-pair percentage reduction), not variability across machines/files. No result is a page, startup, viewer-FPS, GPU or HDR speed claim. The test and compiler context can affect inlining/layout. The accepted conclusion is a measurable local CPU benefit on these dense fixtures, with a small triangle-only release gain and no established triangle-only opt-level=1 gain.

## Reproduction

Manual ignored tests are encode::ordered_pixel::exact_orientation_tests::{shared_split_cpu_pairs,dense_subtraction_cpu_pairs}. Run the interpreter validation harness with filter cpu_pairs, --ignored --nocapture --test-threads=1. For release add --release and CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1, CARGO_PROFILE_RELEASE_LTO=thin, CARGO_PROFILE_RELEASE_STRIP=symbols. Use `python3 scripts/analyze-split-cpu.py <log>` to reconstruct summaries; incomplete probes are rejected. Compiler/profile/method metadata are retained in metadata.json.

Post-instrumentation checks: all 20 vector partition controls passed; full interpreter 160 passed/3 ignored. The two new ignored tests are manual timing probes; both were executed in test and release profiles above. Production builds omit the legacy selection flag. No geometry policy or quota was changed in this measurement step.
