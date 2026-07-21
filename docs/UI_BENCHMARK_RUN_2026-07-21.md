# UI benchmark run — 2026-07-21

## Result

The executable and host graphics path passed the available UI-adjacent smoke
gates. A real RAW `first_present`/interaction benchmark was **skipped**:
there is no licensed CR2/DNG fixture in the workspace or local Cargo/cache
directories. The application requires a regular RAW path, so inventing a
latency number here would be misleading.

## Environment

| Field | Observed value |
|---|---|
| OS | macOS (Darwin) |
| CPU/GPU | Apple M4 Max, 40 GPU cores |
| Graphics API | Metal 4 available |
| Displays | 3456×2234 Retina; 6400×3600 external at 60 Hz |
| Release binary | `target/release/rrrah`, rebuilt with `cargo build --release --locked` |
| RAW fixtures | none found (`*.cr2`, `*.dng`) |

## Gates actually run

| Gate | Result | Notes |
|---|---:|---|
| Full Rust test suite | PASS | 54 test functions; 3 licensed-fixture cases report an explicit skip because their env vars are unset |
| WGSL/Naga validation | PASS | `rrrah-gpu`: 6/6 |
| Benchmark telemetry tests | PASS | `rrrah-bench`: 2/2 |
| Python benchmark-schema tests | PASS | 6/6 |
| Clippy | PASS | `-D warnings`, all targets/features |
| Rustdoc | PASS | `RUSTDOCFLAGS=-Dwarnings` |
| Release CLI startup/help | PASS | binary responds and exposes RAW/inspect/cache flags |
| UI window + GPU first present on RAW | SKIP | no CR2/DNG fixture |
| Pan/zoom p95 and dropped-frame rate | SKIP | requires a live window, input replay and telemetry spans |

The negative-path probe also behaved as designed: a missing RAW path exits
with a clear error instead of opening a blank window or substituting an
embedded JPEG preview.

## Reproduce the real UI run

After placing a licensed fixture in `tests/fixtures/` and verifying its hash:

```bash
RRRAH_REQUIRE_FIXTURES=1 scripts/fetch-fixtures.sh --verify-only
cargo build --release --locked
WGPU_BACKEND=metal target/release/rrrah --inspect tests/fixtures/sample.CR2
python3 scripts/bench-harness.py \
  --binary target/release/rrrah \
  --backend wgpu-metal \
  --repetitions 10 \
  --output target/bench/raw-metal.jsonl \
  tests/fixtures/sample.CR2
python3 scripts/bench-report.py target/bench/raw-metal.jsonl \
  --json-out target/bench/raw-metal-report.json
```

The release gate must report, separately, `T_metadata`, `T_first_raw`,
`T_first_present`, `T_visible_complete`, warm/cold cache state, p50/p95/p99,
RSS/VRAM and dropped frames. Until those spans are emitted by the live window
path, process-boundary `--inspect` numbers are not UI frame-time numbers.
