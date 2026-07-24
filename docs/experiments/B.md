# Experiment B — Runtime knob for CR3 plane-worker count (`RRRAH_CR3_PLANE_WORKERS`)

Date: 2026-07-23. Author: worker B. Zone: `crates/rrrah-decode/src/native_backend.rs`, `crates/rrrah-decode/src/cr3/**`.

## Hypothesis

A sweep over `workers = 1 | 2 | 4` exercises both CR3 plane-decode schedulers
(streaming vs batch). Previously the request was hardcoded:
`REQUESTED_PLANE_WORKERS = 4` (`native_backend.rs`), which also fed
`MIN_STREAMING_PARALLELISM = REQUESTED_PLANE_WORKERS + 1`.

## Change

`native_backend.rs`:

- New env knob `RRRAH_CR3_PLANE_WORKERS` accepting exactly `1`, `2`, or `4`;
  missing/invalid values fall back to the default `4` (the old hardcoded
  behaviour), parsed by `parse_plane_workers` after the pattern of the
  existing `RRRAH_CR3_STREAM_BATCH_ROWS` / `RRRAH_CR3_STREAM_QUEUE_DEPTH`
  overrides (`cr3/assemble.rs::stream_tuning_value`).
- Branch selection factored into a pure function:
  `should_stream_planes(requested, available) = requested >= CR3_PLANE_COUNT && available > requested`.
  The streaming scheduler always runs one worker per plane (4) plus the
  coordinating caller, so it is only eligible when the request covers all
  four planes and the machine has a spare hardware thread. Values `1`/`2`
  therefore force the bounded batch scheduler (`decode_four_planes`), which
  is what makes the sweep able to compare the two branches at all — with the
  old threshold derived from the knob alone (`requested + 1`), every knob
  value would take the streaming branch on any machine with ≥5 cores and the
  knob would be a no-op there.
- Default behaviour is unchanged: unset/invalid env ⇒ `requested = 4` ⇒
  streaming iff `available_parallelism >= 5`, else batch with 4 workers —
  bit-for-bit the previous decision.

`cr3/fixture_regression.rs`:

- The `worker_count == 4` assertion is now env-aware via
  `native_backend::planned_plane_worker_count()` (test-only helper), so the
  pixel-oracle regression doubles as the bit-identical gate for every knob
  value during a sweep.
- The regression now prints one timing line per fixture
  (`workers=, plane_wall=, interleave=, total=`) under `--nocapture`.

New unit tests (`native_backend::tests`):

- `plane_worker_knob_accepts_only_supported_values` — `1|2|4` accepted;
  `0|3|5|8|""|"four"|" 2"|"4 "` and unset fall back to 4.
- `streaming_requires_full_plane_coverage_and_a_spare_thread` — branch
  selection truth table.

## How to run the sweep (owner fixture)

```bash
export RRRAH_CR3_REGRESSION_DIR=/path/to/private/fixtures   # .cr3 + -u16le.bin oracles
for w in 1 2 4; do
  RRRAH_CR3_PLANE_WORKERS=$w \
    cargo test -p rrrah-decode --release --lib cr3::fixture_regression -- \
      --ignored --nocapture --exact \
      cr3::fixture_regression::eos_r8_full_native_decode_matches_pixel_oracles
done
```

Each run (a) verifies the full 25.5 MP mosaic against the trusted oracle
(bit-identical gate) and (b) prints per-fixture timings. Note: `workers=4`
selects the **batch** scheduler (4 workers) only on machines with ≤4 hardware
threads; on larger machines it selects streaming. Isolating "streaming vs
batch at 4 workers" therefore needs a 4-core machine (or a future
force-batch flag).

## Measurement setup

- Machine: Apple M5, 10 CPU cores, arm64, macOS 27.0.
- Build: `--release` (thin LTO, codegen-units=1).
- Fixtures: both private EOS R8 CR3s (`rrrah-eos-r8-9043.cr3`,
  `rrrah-eos-r8-9074.cr3`) with oracle `.bin` files, `RRRAH_CR3_REGRESSION_DIR=/tmp`.
- 3 repetitions per knob value per fixture; medians reported. Raw log:
  `docs/experiments/sweep-data/B-plane-workers-sweep.txt`.
- On this 10-core machine: knob `1`/`2` → batch scheduler with 1/2 workers;
  knob `4` → streaming scheduler (4 plane workers + coordinator).

## Results

`plane_wall` = wall time of the four-plane entropy decode (median of 6
measurements per knob across both fixtures):

| knob | branch (this machine) | plane_wall median (min–max), ms | total median, ms |
|------|-----------------------|--------------------------------|------------------|
| 1    | batch, 1 worker       | 309.4 (304.2–326.3)            | 317.7            |
| 2    | batch, 2 workers      | 168.5 (159.7–192.3)            | 177.1            |
| 4    | streaming             | 101.9 (96.8–107.4)             | 104.7            |

Speedups on `plane_wall` (medians): 1→2 workers = **1.84×**, 2→4 = **1.65×**,
1→4 = **3.04×**. Post-decode `interleave` is 4–10 ms in all configurations.

Correctness gate: all 9 sweep runs passed the exact per-sample oracle
comparison and the BLAKE3 pixel digest — output is bit-identical for every
knob value. `cargo test -p rrrah-decode`: 127 passed, 0 failed (3 ignored
fixture tests exercised separately above). `cargo clippy -p rrrah-decode
--all-targets`: no new warnings from this change.

## Verdict

**Hypothesis confirmed.** The knob selects the decode branch exactly as
hypothesized: `1|2` run the bounded batch scheduler, `4` runs streaming on
machines with ≥5 hardware threads, and the sweep measures both branches on
real fixtures with bit-identical output. Entropy decode scales near-linearly
in the batch scheduler (1.84× for 1→2 workers); streaming with 4 plane
workers is ≈3.0× faster than single-threaded batch. These numbers do **not**
isolate streaming-vs-batch at equal worker count (batch-4 is unreachable on
this 10-core machine by design); extrapolating the observed batch scaling,
most of streaming's win at 4 workers comes from parallelism itself, not from
the streaming protocol.

## Next steps

- To compare streaming vs batch at exactly 4 workers, run the knob=4 case on
  a 4-core machine, or add an explicit `RRRAH_CR3_FORCE_BATCH=1` experiment
  flag (out of scope here).
- If the knob proves useful beyond experiments, document
  `RRRAH_CR3_PLANE_WORKERS` in README.md next to the `RRRAH_CR3_STREAM_*`
  overrides (README is outside this task's zone).
