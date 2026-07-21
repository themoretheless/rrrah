# Synthetic GPU sweep: experiment gates

Дата: 2026-07-21. Это protocol для deterministic GPU experiments плюс первый
exploratory synthetic Metal run. Его цель — разделить upload, shader/render,
present и quality correctness так, чтобы GPU speedup нельзя было получить за
счёт измерительной ошибки.

## Current output audit

Полноценного release GPU gate всё ещё нет, но runnable screening smoke теперь
есть:

- `target/bench/gpu-smoke-metal.csv` содержит 36 комбинаций (3 synthetic RAW
  размера × 3 viewport × 4 zoom), 2 warmups и 20 render samples per cell;
- adapter: Apple M4 Max, Metal, `max_texture_dimension_2d=16384`,
  `max_texture_array_layers=2048`;
- render p95 по группам: примерно 0.31–1.86 ms; это GPU-complete CPU wait
  observation на synthetic Bayer, не UI present и не CR2/DNG open.

Исторический process-only output остаётся отдельным:

- `target/bench/results.csv` содержит старый process-only CSV для
  `/tmp/rrrah-sample-1.cr2`: 5 `cold-no-cache` samples (0.39–0.40 s) и 5
  `warm-cache` samples (0.09–0.10 s), status=0;
- в нём нет adapter, driver, GPU timestamps, upload/render/present spans,
  memory, quality tier или fixture hash/license manifest;
- `scripts/bench-report.py` корректно помечает обе группы предупреждениями
  `n<30` и `n<10`, то есть этот output нельзя использовать как release gate;
- `warm-cache` p50/p50 ratio около 4.44× — только full-mosaic disk-cache
  process latency, не GPU speedup и не RAW decode speedup.

Существует `/tmp/rrrah-sample-1.cr2`, но путь не является лицензированным
fixture manifest и сам CSV не содержит его hash. Поэтому результат сохраняется
как исторический exploratory artifact; он не подменяет synthetic sweep.

## Experimental units and matrix

Synthetic input создаётся один раз в CPU reference и имеет content hash. Для
каждого case сохраняются input seed, dimensions, CFA, bit depth, halo, tile size
и expected output digest. Минимальная матрица:

```text
image: 2048², 4096², 8192² (plus adapter-limit edge case)
tile: 256, 512, 1024, 2048, 4096
halo: 0, 1, 2
pattern: flat, linear ramp, diagonal edge, saturated highlight, deterministic noise
path: fragment render, compute (если реализован), CPU reference
cache: pipeline-cold, pipeline-warm, resource-resident, resource-evicted
upload: one tile, visible tile batch, full synthetic atlas
```

Не смешивать в одной группе разные tile size, halo, quality algorithm,
texture format, display resolution или cache state. Генератор не использует
RAW embedded JPEG и не включает file I/O: это experiment GPU, не ingest.

## Warm-up and repetition gates

Each case runs in independent batches, чтобы thermal/driver drift не исчезал в
одном большом среднем.

| Experiment | Warm-up | Measured | Independent batches | Release use |
|---|---:|---:|---:|---|
| shader/pipeline cold | none; new process per sample | 30 launches | 3 batches of 10 | p50/p95; p99 only exploratory unless n≥100 |
| upload microbench | 20 uploads | 100 uploads | 3 | p50/p95/p99, block bootstrap |
| render microbench | 60 frames | 300 frames | 3 | p50/p95/p99, frame deadline |
| steady 60/120 Hz | 120 frames | 600 frames | 3 | 1,800 frame samples, p95/p99 |
| scripted pan/zoom/exposure | 10 sequences | 30 sequences × 60 frames | 3 | event→present and deadline miss |
| device-loss/fallback | 5 injected losses | 30 injected losses | 3 | correctness, no latency claim |

For expensive 8k/16k cases a local exploratory run may use one batch, but must
be marked `exploratory` and never pass CI. A release latency group requires at
least 30 successful independent observations; a p99 gate requires at least 100
observations, and steady-frame p99 uses the 1,800-frame protocol above.

Shader-cold and shader-warm are different groups. A warm-up frame cannot make a
pipeline-cold compile disappear. Resource-resident and resource-evicted groups
must be reset explicitly with a fence and documented cache state.

## Percentiles and uncertainty

Report, without outlier deletion:

```text
n, min, p50, p95, p99, mean, MAD, 95% CI, deadline_miss_ratio
```

Use percentile interpolation from the current reporter, but bootstrap over
independent batches (or fixed 30-frame blocks), not iid individual frames. GPU
frame samples are autocorrelated; iid bootstrap materially understates the
uncertainty. If only one batch exists, CI is exploratory.

CI policy:

- p95 is the primary latency gate;
- p99 is a tail diagnostic and a 120-Hz deadline gate only with `n≥100`;
- all samples remain in headline statistics;
- `--exclude-outliers` is forbidden for release gates;
- p95 of upload plus p95 of render is **not** end-to-end p95. Measure the
  end-to-end span directly because queue overlap and scheduling covariance matter;
- a regression requires both absolute delta and relative delta (default 5%)
  on the same adapter/case; if the ratio CI crosses 1, mark inconclusive.

## Upload versus render separation

`queue.write_texture` CPU duration is not GPU copy duration. To measure upload
without ambiguity, use a staging buffer and explicit copy command, then record:

```text
upload_prepare_cpu_ns     # packing/padding, no device wait
upload_enqueue_cpu_ns     # encoder + submit call
upload_gpu_copy_ns        # GPU timestamps around copy command
upload_fence_wait_ns      # CPU wait until copy is usable (reported separately)
upload_bytes
upload_effective_gib_s = bytes / upload_gpu_copy_ns
```

Render uses a separate command or pass:

```text
shader_compile_ns         # cold and warm groups, never hidden in render
render_gpu_ns              # timestamps around fragment/compute pass
render_submit_cpu_ns       # CPU encoding/submission
queue_wait_ns              # time queued before execution, if observable
present_cpu_ns             # submit-to-present observation
vsync_wait_ns              # present pacing, separate from render
```

Do not call `device.poll(Wait)` per tile or include readback synchronization in
`render_gpu_ns`. For correctness/readback, wait after the measured command and
record the wait as a separate span. Use one realistic batch submit for the
steady path; a per-tile submit benchmark measures an artificial worst case.

If timestamps are unavailable, report CPU submit-to-present only and set
`gpu_timestamp=false`; never infer GPU duration by subtracting unrelated CPU
spans. On Apple GPUs, timestamp features may be unavailable; that is an honest
`unsupported_timestamp` result, not zero GPU time.

## Adapter capability gate (wgpu 30)

Before timing, emit a capability manifest. An adapter is **ineligible**, not
slow, when a required capability is absent. Never silently replace it with CPU
or WARP and call that a GPU result.

Required limits depend on the selected case:

```text
max_texture_dimension_2d >= tile_size + 2*halo
max_texture_array_layers >= ceil(width/tile_size) * ceil(height/tile_size)
max_uniform_buffer_binding_size >= sizeof(GpuParameters) (224 bytes today)
max_bind_groups >= shader bind-group count
max_compute_workgroup_size_x/y >= chosen workgroup dimensions
max_compute_workgroup_invocations >= x*y for compute cases
max_compute_workgroups_per_dimension >= dispatch dimension
```

Also validate `R16Uint` format features for the requested usages
(`COPY_DST`, `TEXTURE_BINDING`), target surface format for
`RENDER_ATTACHMENT`, and compute output format for `STORAGE_BINDING`/`COPY_SRC`
when those paths are measured. Check row pitch against
`wgpu::COPY_BYTES_PER_ROW_ALIGNMENT` (256 bytes).

Timestamp classification:

| Capability | Meaning | Gate |
|---|---|---|
| `TIMESTAMP_QUERY` | timestamp query set | required for GPU stage timings |
| `TIMESTAMP_QUERY_INSIDE_ENCODERS` | copy/encoder boundaries | required for explicit upload-copy timestamps |
| `TIMESTAMP_QUERY_INSIDE_PASSES` | render/compute boundaries | required for render/compute GPU timestamps |
| `Queue::get_timestamp_period()` | tick→ns conversion | record value; non-positive/unknown invalid |

When a capability is missing, the run may still be useful for CPU end-to-end
observation, but its manifest must say `gpu_timestamp=false` and it cannot be
compared to timestamped GPU runs as a GPU-time regression.

Record at minimum:

```text
backend/API, adapter_name, vendor_id, device_id, driver/runtime version,
features, limits, surface format/color space, display refresh/resolution,
unified-memory flag, shader-cache state, power mode, CPU affinity
```

Unified memory is not VRAM. Report `gpu_resident_bytes` and process RSS as
separate observations, with platform semantics documented.

## Correctness gates for synthetic output

Every performance case first passes a correctness run on the same adapter:

```text
tile output == monolithic CPU reference at halo-valid pixels
max_abs linear error <= 1e-4 for float path (or exact u16 for copy path)
NaN/Inf count == 0
stale_generation_publish == 0
shader_validation_errors == 0
```

For GPU readback, use a separate `COPY_SRC`/`MAP_READ` path and exclude its
readback time from upload/render timings. Store output digest and quality tier
in the result. A timing sample that fails correctness is discarded as a failed
experiment, not treated as a fast result.

## Acceptance gates

The sweep has two different gates:

**Correctness gate:** all synthetic patterns pass oracle, zero validation/device
errors, zero stale publishes, no fallback; missing capability is `skip` with a
reason.

**Performance gate (same adapter/case baseline):**

```text
steady_frame p95 <= 16.7 ms at 60 Hz
steady_frame p95 <= 8.3 ms at 120 Hz
deadline_miss_ratio < 1%
first_visible/first_present p95 <= named hardware target
upload/render p95 regression <= 5% versus baseline
RSS and resident GPU bytes <= explicit budget
```

Thresholds are hardware-labelled engineering targets. They are not universal
claims across Metal/Vulkan/DX12, integrated/discrete GPUs or unified/discrete
memory. For a new adapter, first establish a baseline artifact; do not compare
its absolute milliseconds with a different device.

## Current sweep stop result

The available `target/bench/results.csv` fails the synthetic GPU gate because it
has no GPU spans, no capability manifest, no synthetic input hash, no memory
metrics and only 5 samples per group. The correct status is:

```text
status = exploratory_process_decode
gpu_sweep = not_run
release_gate = blocked
```

The `UI_BENCHMARK_RUN_2026-07-21.md` record is honest that real RAW first-present
and pan/zoom were skipped without licensed fixtures. It should not be expanded
with synthetic numbers until the runner emits the upload/render/present fields
above.

## Implementation order

1. Add deterministic synthetic generator + CPU oracle and output digest.
2. Add adapter capability manifest and explicit ineligible/skip status.
3. Encode staging-copy and render/compute timestamp spans, with CPU fallback
   labels when timestamp features are absent.
4. Implement batch protocol and block-bootstrap aggregation; fail on incomplete
   samples and hidden fallback.
5. Only then publish GPU sweep tables and compare adapters/backends.
