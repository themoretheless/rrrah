# Rrrah

Rrrah is a native Rust viewer whose first displayed image is developed from the
sensor mosaic itself. It never substitutes the embedded JPEG for the main view.

The current fast path is deliberately narrow and measurable:

1. parse Canon EOS R8 CR3/CRX and decode four lossless parity planes in native Rust;
2. cache that decoded mosaic;
3. upload it once as an integer GPU texture;
4. normalize, demosaic, white-balance, color-convert, and tone-map only the
   visible viewport in WGSL.

The current decoder accepts the confirmed full-resolution, one-tile, 14-bit
Canon EOS R8 CR3 profile. It has no external RAW-decoder dependency. Other
cameras and RAW formats are rejected instead of being guessed.

Detailed design, equations, budgets, and benchmark protocol live in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
The full-resolution tiled phase is implemented as a first atlas-backed step and
specified in
[`docs/TILED_PIPELINE.md`](docs/TILED_PIPELINE.md) and
[`docs/TILED_MATH.md`](docs/TILED_MATH.md); it replaces the temporary
large-RAW downsample fallback with GPU tile residency.

The production editor roadmap is [EDITOR_100.md](docs/EDITOR_100.md), with the
mathematical contract in [EDITOR_MATH.md](docs/EDITOR_MATH.md) and the canonical
benchmark matrix in [BENCHMARK_MATRIX.md](docs/BENCHMARK_MATRIX.md).
The live HUD/telemetry design is [LIVE_BENCHMARKS.md](docs/LIVE_BENCHMARKS.md),
and the twenty-role review is [BENCH_AGENT_REVIEW.md](docs/BENCH_AGENT_REVIEW.md).
The parameter-sweep matrix and runnable synthetic GPU smoke are documented in
[PARAMETER_SWEEP_ARCHITECTURE.md](docs/PARAMETER_SWEEP_ARCHITECTURE.md) and
[GPU_SYNTHETIC_SWEEP_GATES.md](docs/GPU_SYNTHETIC_SWEEP_GATES.md).
The extension backlog is [EDITOR_101_200.md](docs/EDITOR_101_200.md), with its
math contract in [EDITOR_200_MATH.md](docs/EDITOR_200_MATH.md) and critical stop
conditions in [EDITOR_100_CRITIQUE.md](docs/EDITOR_100_CRITIQUE.md).

The 50-role competitor/paper/practice audit, current implementation scorecard,
parallelism model, innovation review, and production gates are consolidated in
[`docs/RESEARCH_DEEP_DIVE.md`](docs/RESEARCH_DEEP_DIVE.md). Supporting evidence is
split into [competitors](docs/RESEARCH_COMPETITORS.md),
[papers](docs/RESEARCH_PAPERS.md), and [practice/forums](docs/RESEARCH_PRACTICE.md).

The execution breakdown for ingest, scheduler/GPU residency, quality, and their
adversarial critic gates is [IMPLEMENTATION_AGENT_PLAN.md](docs/IMPLEMENTATION_AGENT_PLAN.md).
The three detailed work packages are [ingest/tiles](docs/PLAN_INGEST_TILES.md),
[scheduler/residency](docs/PLAN_SCHEDULER_RESIDENCY.md), and
[quality/critic](docs/PLAN_QUALITY_CRITIC.md).

Dependency, security, test, benchmark and lint audits are tracked in
[DEPENDENCY_UPDATE_AUDIT.md](docs/DEPENDENCY_UPDATE_AUDIT.md),
[SECURITY_DEPENDENCY_AUDIT.md](docs/SECURITY_DEPENDENCY_AUDIT.md), and
[TEST_BENCH_LINT_AUDIT.md](docs/TEST_BENCH_LINT_AUDIT.md).
The latest GPU, decoder, fuzz, cache and final adversarial reviews are
[GPU_VALIDATION_AUDIT.md](docs/GPU_VALIDATION_AUDIT.md),
[DECODE_FORMAT_AUDIT.md](docs/DECODE_FORMAT_AUDIT.md),
[FUZZ_HARDENING_AUDIT.md](docs/FUZZ_HARDENING_AUDIT.md),
[CACHE_STRESS_AUDIT.md](docs/CACHE_STRESS_AUDIT.md),
[CI_LINT_AUDIT.md](docs/CI_LINT_AUDIT.md), and
[FINAL_AUDIT_CRITIC.md](docs/FINAL_AUDIT_CRITIC.md).
The latest UI benchmark run and its explicit fixture gate are recorded in
[UI_BENCHMARK_RUN_2026-07-21.md](docs/UI_BENCHMARK_RUN_2026-07-21.md).
The folder gallery architecture, preload policy, security gates, and benchmark
contract are recorded in [GALLERY_ARCHITECTURE.md](docs/GALLERY_ARCHITECTURE.md).

## Run

```bash
cargo run --release -p rrrah -- --no-cache path/to/image.CR3
cargo run --release -p rrrah -- --no-cache --inspect path/to/image.CR3
```

Controls: drop an EOS R8 CR3 file or folder onto the window; a dropped folder opens
its first CR3 and `←`/`→` navigate the folder. Mouse wheel zooms, left-drag
pans, `+`/`-` changes exposure, `F` returns to fit, and `R` resets the view.
The in-image HUD reports decode/cache/adapt/upload/open timings and a live frame
encode sample.

## Status

This is an architecture-first prototype. It provides a real native EOS R8 full-RAW decode,
full-resolution tiled GPU upload for adapters with texture-array capacity,
timing instrumentation, and warm-open cache; it is not yet a replacement for a
color-managed production raw developer. Additional camera profiles require
separate framing, metadata and pixel-oracle validation.
