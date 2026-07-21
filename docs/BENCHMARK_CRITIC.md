# Adversarial benchmark review

Дата: 2026-07-21. Область: `scripts/bench-harness.py`,
`scripts/bench-report.py`, `scripts/bench-matrix.sh`, `scripts/bench-raw.sh`,
`BENCHMARKS.md`, `BENCHMARK_MATRIX.md`, `QUALITY_BENCHMARKS.md` и
`LIVE_BENCHMARKS.md`.

Цель этого документа — не дать benchmark-числам выглядеть как proof скорости,
если измерялся другой pipeline, другой cache state или другой quality tier.

## Verdict

**RED для UI/GPU и индустриальных speed claims.** Текущий JSONL harness запускает
`rrrah --inspect`: он измеряет process wall time для полного `rawler.raw_image`
и disk-mosaic cache, но не создаёт окно, swapchain, upload, shader, present,
GPU timestamp или pan/zoom event. `--backend` и `--workers` записываются как
labels; worker count не передаётся приложению и не меняет вычисление. Поэтому
текущие результаты нельзя называть first-visible, frame-time, GPU speedup или
parallel scaling.

**YELLOW для decode/cache latency.** Основа полезна (monotonic wall clock,
fixture SHA-256, binary SHA-256, host manifest, p50/p95/p99, MAD и bootstrap CI),
но release gate пока допускает несколько false-positive сценариев:

1. `parse_inspect()` начинает с `embedded_jpeg_used = False`. Если строка
   `embedded JPEG is not used by this path` исчезнет или изменится, пропуск
   assertion превратится в ложное доказательство RAW-only path.
2. JSONL row со status `0`, но без `raw`, `cache_hit`, timing или explicit
   JPEG marker, всё равно получает `wall_ms` и попадает в report. Reporter также
   пропускает malformed JSONL и non-zero rows без обязательной expected-count
   проверки.
3. Warm seed failure только печатается и не останавливает harness. Warm-серия
   может состоять из cache misses, хотя mode называется `warm-persistent-cache`.
4. Baseline `speedup_vs_cold` выбирается как первая cold-группа для fixture,
   без фиксации `backend`, `workers`, algorithm/quality tier и decoder ABI.
   При нескольких backend-ах это сравнивает разные системы.
5. Память, RSS, VRAM, bytes read, page faults, queue wait и dropped frames не
   снимаются. Они присутствуют в целевых документах, но отсутствуют в текущем
   runner-е, поэтому их нельзя заявлять измеренными.

**GREEN для exploratory decode statistics**, если одновременно указаны fixture,
binary hash, cache mode и `os_page_cache_state=unknown`, а результат не
используется как release gate. `no-persistent-cache` честно не означает cold
OS page cache.

## Что реально измеряется сейчас

```text
inspect process wall time
  = source fingerprint (если cache включён)
  + full rawler raw_image(dummy=false)
  + adapter metadata/pixel validation
  + optional disk cache read/write
```

Не измеряется:

- metadata-only probe или first visible tile;
- DNG independent tile decode и worker scaling;
- GPU upload, shader compile, demosaic, queue wait и `present`;
- input-event-to-present latency для pan/zoom/exposure;
- peak RSS/VRAM/staging residency;
- cache hit validity как обязательное условие группы;
- качество output или отсутствие preview shortcut как строгий schema field.

`docs/BENCHMARKS.md` уже описывает эту границу, но `bench-raw.sh` всё ещё
печатает `cold full RAW decode`; при неочищенном page cache это следует называть
`no-persistent-cache / OS-cache-unknown`.

## Adversarial failure modes и gates

| Риск | Как возникает | Обязательный gate |
|---|---|---|
| JPEG false negative | parser default `False`, marker отсутствует | `embedded_jpeg_used: true/false/unknown`; `unknown` = invalid |
| fake warm cache | seed упал или cache_hit не распарсен | seed status=0 и 100% warm rows `cache_hit=true` |
| partial sample group | timeout/error rows отфильтрованы | expected count, status=0 и valid schema для каждой репетиции |
| truncation hidden | malformed JSONL тихо пропущен | release mode завершается non-zero при любом битом row |
| wrong baseline | cold group другого backend/worker/tier | baseline key включает все dimensions |
| worker placebo | `--workers=8` только label | manifest содержит `workers_requested` и `workers_actual`; actual scheduler counter |
| quality speedup | Fast bilinear сравнен с AMaZE/RCD | `(algorithm, quality_tier, profile, metadata_digest)` в group key |
| cache-state mixing | OS-warm, disk-warm и GPU-resident смешаны | отдельные enum states: `process_cold`, `os_unknown`, `disk_warm`, `ram_warm`, `gpu_resident` |
| stale binary | binary path старый или debug | release profile, non-empty binary SHA, toolchain and git/build ID required |
| timer illusion | CPU submit time выдан за GPU time | `gpu_timestamp=true` only with calibrated query; otherwise label CPU observation |
| display illusion | vsync/surface hidden, screenshot overhead included | fixed refresh/API, scripted events, present timestamps, no capture in measured span |
| outlier laundering | `--exclude-outliers` headline used as speedup | CI uses all samples; filtered metrics diagnostic-only |
| tiny-n CI | default harness 5 reps, p95/p99 unstable | release minimum n≥30 latency; exports n≥10; n below gate = fail/skip |

## Строгий benchmark record

Для release report каждая строка должна иметь не nullable fields:

```json
{
  "schema": "rrrah.benchmark-sample.v2",
  "status": "ok",
  "fixture": {
    "id": "canon_5ds_cr2",
    "sha256": "...",
    "license_ref": "...",
    "format": "CR2",
    "width": 8896,
    "height": 5920,
    "cfa": "RGGB"
  },
  "pipeline": {
    "decoder": "rawler-0.7.2",
    "decode_abi": 1,
    "algorithm": "bilinear",
    "quality_tier": "fast",
    "metadata_digest": "..."
  },
  "execution": {
    "mode": "os_unknown",
    "backend": "cpu",
    "workers_requested": 8,
    "workers_actual": 1,
    "jpeg_shortcut_used": false,
    "gpu_timestamp": false
  },
  "samples": {
    "iteration": 1,
    "wall_ms": 396.6,
    "first_visible_ms": null,
    "frame_ms": null,
    "peak_rss_bytes": null,
    "peak_vram_bytes": null
  }
}
```

`first_visible_ms`, `frame_ms` и GPU/RSS поля допускаются `null` только в
decode-only suite. Они не должны превращаться в нули. `jpeg_shortcut_used`
должен быть explicit; отсутствие marker — `unknown` и invalid для release.

Group key для regression:

```text
(fixture_id, fixture_sha, format, decoder, decode_abi,
 algorithm, quality_tier, metadata_digest, mode,
 backend, adapter_id, driver, workers_actual)
```

`speedup_vs_baseline` разрешён только внутри одинакового key кроме одной
изменяемой величины (например, `workers_actual`). Сравнивать CPU с Metal,
CR2 с DNG или bilinear с AMaZE одним числом запрещено.

## Правильные suites и acceptance gates

### S1 — ingest/open

Separate spans: `probe`, `fingerprint`, `entropy`, `predictor`, `metadata`,
`cache_read/write`, `first_visible_tile`, `full_mosaic`. Для CR2 отдельно
показывать serial fraction; для DNG tiled — tile count, workers, SSD/read
bandwidth и queue wait. Публиковать p50/p95/p99, n, CI и max RSS.

Гейты:

- no-preview marker explicit and false;
- dimensions/CFA/bit depth/output digest match fixture contract;
- CR2 and DNG never averaged into one scaling curve;
- stale/error count is zero;
- n≥30 for latency gate, otherwise `exploratory`.

### S2 — cache

Run separate groups: new process + no persistent cache, OS-cache unknown,
disk-cache warm, RAM tile warm, GPU-resident. Warm seed must be recorded and
every warm sample must report `cache_hit=true`; otherwise group fails. Report
fingerprint bytes and cache serialization separately from decode.

Do not call cache read speed “RAW decode speed”. Do not compare a 500 MiB full
mosaic cache hit with a future 4 MiB visible-tile cache hit.

### S3 — GPU/display

Use a hardware-labelled runner that creates the same window/surface as the app.
Record adapter/vendor/device/driver/API/limits, display refresh, resolution,
shader-cache state, CPU affinity and power mode. Prefer GPU timestamp queries;
if unavailable, report CPU submit-to-present and set `gpu_timestamp=false`.

Measure separately:

```text
shader_compile_cold
upload_visible_tiles
first_present
steady_frame (300+ scripted frames)
pan/zoom/exposure input -> present
device_loss / CPU fallback
```

Acceptance targets are hardware-labelled, not universal claims:

```text
first_visible p95 <= target for named fixture/adapter
steady frame p95 <= 16.7 ms at 60 Hz (<=8.3 ms at 120 Hz)
deadline_miss_ratio < 1%
dropped_frames == 0 in correctness run
stale_generation_publish == 0
```

### S4 — quality/throughput

Use synthetic RAW only for bit-exact decode/demosaic math. Real camera RAW is
needed for compatibility and photographer workflow, but has no unique ground
truth. Every real fixture must have a license reference, immutable SHA-256 or
BLAKE3, camera metadata and reference digest. Embedded JPEG is never an oracle.

Quality tiers (`fast`, `balanced`, `quality`) need separate baselines. Report
linear-light PSNR/SSIM, CIEDE2000 patches, clipping/headroom, tile seam ΔLSB and
algorithm/profile. A speed gain that changes the tier or silently drops DCP,
OpcodeList, black-level grids or highlights is not a speedup.

### S5 — editor/UI

Use deterministic scripted event traces (open, wheel, drag, exposure, resize),
not screen-capture timing. Measure event timestamp to first frame present and
steady p95; exclude input injection and screenshot transport. Keep UI thread
CPU budget, queue depth, stale cancellations, RAM/VRAM high-water mark and
telemetry overhead visible. Live HUD is observational only and cannot be a CI
result before process completion.

## Licensed RAW fixture policy

Required fixture manifest fields:

```text
id, source/license URL or permission record, SHA-256/BLAKE3,
format/frame index, camera make/model, dimensions, bit depth, CFA,
black/white levels, orientation, expected metadata digest, decoder ABI
```

Synthetic fixture is allowed for mathematical or adversarial tests, not for
claiming Canon/Nikon/Fuji compatibility. A missing manifest must be an explicit
`fixture_unavailable` skip or a failed required CI job; it must not look like a
green camera test. Vendor benchmark numbers from a website/forum are context,
not evidence, until run with the same licensed file, parameters and hardware.

Minimum useful corpus for performance claims:

- at least two CR2 sizes, including a large lossless-JPEG file;
- uncompressed-strip and tiled DNG with edge tiles;
- one restart-marker CR2 and one malformed/truncated negative;
- one black-level-grid/orientation/opcode fixture;
- one >100 MP or deliberately oversized stress case;
- synthetic ground-truth RAW for each active demosaic tier.

## False-speedup checklist

Before accepting a change, answer yes to all:

1. Same fixture hash, dimensions, frame index and metadata digest?
2. Same decoder ABI, algorithm, quality tier and color profile?
3. Same release binary profile, CPU/GPU/driver, refresh and power mode?
4. Same cache/page-cache/shader state, explicitly labelled?
5. Same number of successful samples with no hidden errors or malformed rows?
6. Same first-visible definition and present synchronization?
7. Same output digest/quality gate and no embedded-JPEG shortcut?
8. Speedup reported with p50/p95/p99, CI, absolute ms and memory cost?
9. For scaling, actual workers/queues measured and serial fraction shown?
10. For GPU, GPU time distinguished from CPU submit/present time?

If any answer is no, the result is `exploratory`, not a regression-gate or
industry comparison.

## Implementation order

P0 benchmark hygiene:

1. Make parser fields tri-state and validate explicit RAW marker, dimensions,
   cache hit and timings; fail on missing required fields.
2. Abort on warm seed failure and enforce expected sample count/status. Preserve
   malformed-row count in report and fail release mode.
3. Extend group/baseline key with quality tier, decoder ABI, cache state,
   adapter/driver and actual workers. Remove cross-backend `speedup_vs_cold`.
4. Rename `bench-raw.sh`/CSV `cold-*` labels to OS-cache-unknown unless a
   privileged flush protocol is recorded.

P1 measurement:

5. Add RSS/CPU counters and hardware GPU runner with first-present/frame spans.
6. Wire workers to the real scheduler and emit `workers_actual`, queue wait,
   stale drops and tile throughput.
7. Add licensed fixture manifest and CI policy where missing corpus is visible,
   never a silent pass.

Until P0 is merged, current reports may be used for local exploratory decoder and
cache comparisons only. They cannot substantiate «GPU accelerated», «fastest»,
«linear scaling», first-visible UI or industry-standard benchmark claims.
