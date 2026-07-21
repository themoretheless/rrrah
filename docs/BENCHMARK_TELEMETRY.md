# Live telemetry contract

Benchmark numbers must be useful while an image is being opened, not only
after the process exits. The editor and the headless harness therefore share a
small, append-only event contract. Telemetry is observational: it must never
take a mutex on the decode or render hot path and must be safe to drop when the
ring is full.

## Event model

All timestamps are monotonic nanoseconds from one process clock. Wall-clock
`started_at` belongs only to the run manifest. A span is represented by two
events, which permits a live UI to show in-flight work:

```json
{"v":1,"type":"span_begin","id":41,"parent":12,"name":"dng.tile_decode",
 "ts_ns":1842230012,"thread":"decode-3","file":"a.dng","tile":[3,1],"bytes":524288}
{"v":1,"type":"span_end","id":41,"ts_ns":1842239017,"status":"ok","cache":"miss"}
```

Counter events do not have a matching end:

```json
{"v":1,"type":"counter","name":"gpu.resident_tiles","ts_ns":1842240000,
 "value":8,"unit":"tiles"}
```

Required names are stable IDs rather than free-form UI labels:

```text
probe, source_open, entropy_decode, predictor, metadata_adapt,
tile_decode, tile_postprocess, cache_lookup, cache_read, cache_write,
prefetch_enqueue, prefetch_cancel, gpu_upload, gpu_demosaic,
first_visible_tile, first_present, frame, export_encode
```

Each event may contain `generation`, `fixture_sha256`, `frame_index`, `tile`,
`mip`, `bytes`, `pixels`, and `backend`. Unknown fields are ignored by readers;
`v` is incremented only for incompatible changes.

## Producer/consumer design

The producer is a bounded lock-free SPSC/MPSC ring of fixed-size records. A
record contains an interned name ID, timestamps, numeric payload and a compact
attribute bitset; strings are interned outside the hot path. On overflow the
producer increments `telemetry.dropped_events` and continues. The UI drains at
most a fixed budget per frame (for example 1024 records), so telemetry cannot
starve rendering. The headless runner drains to JSONL or Chrome trace format.

No `Instant` is converted to wall time in the hot path. The run manifest stores
the clock source, monotonic resolution and whether GPU timestamp queries were
available. CPU spans use `std::time::Instant`; GPU spans use a timestamp-query
calibration pair and are labelled `gpu_clock` rather than being mixed with CPU
time.

## Derived live metrics

The dashboard computes, without changing raw events:

```text
T_first = end(first_present) - begin(open)
T_visible = end(first_visible_tile) - begin(open)
queue_wait = begin(stage) - end(parent enqueue)
decode_MP_s = pixels / entropy_decode_seconds / 1e6
upload_GB_s = bytes / gpu_upload_seconds / 1e9
deadline_miss = frame_duration > 1 / refresh_rate
```

For p95/p99, retain a bounded HDR-style histogram per `(stage, backend,
fixture_class)` plus an exact short window for the last 256 samples. The UI
shows confidence only after at least five independent samples; one live open is
not a statistically meaningful percentile.

## Reproducibility and benchmark boundaries

Every exported telemetry file begins with one `run_manifest` record containing
fixture hash, binary hash, commit, CPU/GPU/API, compiler flags, cache state,
power mode, display refresh rate and OS page-cache state. A `no-persistent-cache`
run is not called cold unless a privileged cache flush was recorded. Separate
processes are used for independent repetitions; the viewer's shader cache and
allocator state must not accidentally become part of the cold series.

The event stream records `status=cancelled` and `generation` for stale work.
Cancelled work is excluded from throughput but included in wasted-work and
queue-latency reports. A cache hit is a successful stage with `bytes_read` and
`source_hash`; it is never counted as a decode result.

## CI gates

CI runs deterministic microbenchmarks on fixed fixtures and rejects only when:

* the same hardware runner has p95 regression above 5% for two consecutive
  samples (bootstrap CI must overlap the baseline before warning becomes a
  failure);
* peak RSS exceeds the declared budget by 10%;
* `telemetry.dropped_events > 0` in a correctness benchmark;
* a tile seam exceeds one linear 16-bit LSB or a quality gate fails;
* a benchmark reports `skip` without an explicit unsupported-feature reason.

Results from different GPUs are never collapsed into one score. They remain
separate groups keyed by API/backend/device/driver and are compared with the
same fixture and quality tier.
