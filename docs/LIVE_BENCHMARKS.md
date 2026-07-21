# Live benchmark dashboard

The benchmark runner must expose metrics while the RAW editor is running, not
only after process exit. The live path is observational and must never block the
decode or render queues.

```text
decoder/cache/GPU stages
  → bounded telemetry channel
  → lock-free counters + span aggregator
  → live HUD / JSONL / Chrome trace
```

## Event contract

```json
{"kind":"span_end","stage":"dng_tile_decode","generation":4,
 "tile":[12,8],"start_ns":123,"duration_ns":48120,
 "bytes":524288,"workers":8}
```

Counters are sampled at 10–20 Hz for the HUD and emitted losslessly at stage
boundaries for the report. If the channel is full, low-priority samples may be
dropped, but span completion, errors, generation changes and frame-present
events may not be dropped.

## HUD panels

- first-visible latency with stage waterfall;
- current/next tile queues and generation ID;
- CPU workers, queue wait, throughput and memory credits;
- RAM/VRAM/staging usage and cache hit ratio;
- GPU upload/demosaic/render timestamps;
- frame p50/p95/p99 and missed-deadline counter;
- fast/balanced/quality tier and quality reference status.

The HUD has controls for fixture, worker count, tile size, cache state, quality
tier and prefetch depth. A run can be frozen and exported as one JSONL manifest.

## Overhead budget

Telemetry is accepted only if:

```text
CPU overhead < 1% in steady state
HUD render < 0.2 ms/frame
telemetry allocations = 0 in hot pixel/tile loops
```

Use atomics for counters, a fixed-capacity ring for events, and preallocated
strings/IDs. Never call synchronous disk I/O, GPU waits, or `device.poll(Wait)`
from the telemetry path.

## Offline continuation

The same event stream feeds `bench-report.py`: live values are provisional;
final p50/p95/p99 and bootstrap confidence intervals are computed from complete
runs. A live graph must label itself as `in-progress` and cannot be used as a CI
regression result until the process exits successfully.
