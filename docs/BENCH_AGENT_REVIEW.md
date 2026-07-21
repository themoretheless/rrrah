# Benchmark review: 20 specialist lanes

Benchmark quality is a systems problem. The following twenty lanes review the
same run manifest instead of producing incomparable one-off numbers.

| # | Роль | Что проверяет | Артефакт |
|---:|---|---|---|
| 1 | System architect | boundaries, process model, stage spans | run manifest |
| 2 | Telemetry architect | live events, counters, tracing | JSONL event schema |
| 3 | Statistician | bootstrap CI, outliers, effect size | report algorithm |
| 4 | CPU performance engineer | cycles/pixel, SIMD, affinity | scaling suite |
| 5 | GPU performance engineer | occupancy, timestamps, stalls | GPU suite |
| 6 | Memory architect | RSS/VRAM/staging/page cache | budget ledger |
| 7 | Storage engineer | SSD throughput, fsync, cache state | I/O suite |
| 8 | Scheduler engineer | priority, cancellation, queue tail | queue metrics |
| 9 | RAW algorithm engineer | CR2/DNG stage decomposition | decode suite |
| 10 | DNG/TIFF specialist | independent tiles/strips/opcodes | DNG corpus |
| 11 | Demography/demosaic specialist | CFA quality and seams | quality crops |
| 12 | Color scientist | WB, matrices, ICC, ΔE00 | color oracle |
| 13 | Computational photographer | highlight/noise/lens metrics | photo corpus |
| 14 | Numerical analyst | precision, determinism, NaN/Inf | invariants |
| 15 | Cache specialist | hit ratio, admission, eviction | cache workload |
| 16 | QA/test engineer | golden, property, fuzz, flaky runs | test gates |
| 17 | Security engineer | malformed RAW, allocation limits | adversarial corpus |
| 18 | UX/perceptual latency tester | first paint, input-to-frame | interaction suite |
| 19 | OSS comparison analyst | fair RawSpeed/LibRaw/darktable/RT/RapidRAW stages | adapter matrix |
| 20 | Release/CI engineer | regression budget and reproducibility | CI policy |

## Canonical run manifest

Every result must be self-describing:

```json
{
  "run_id":"uuid",
  "git":"commit",
  "fixture":"blake3:...",
  "fixture_class":"CR2|DNG_TILE|DNG_FLOAT|XTRANS|MALFORMED",
  "cache_state":"cold-os|os-warm|decoded-warm|gpu-warm",
  "backend":"native|wgpu-metal|wgpu-vulkan|wgpu-dx12",
  "workers":8,
  "tile_size":512,
  "quality_tier":"fast|balanced|quality",
  "cpu":"model + physical/logical cores",
  "gpu":"model + driver + API",
  "spans":[{"name":"decode","start_ns":0,"duration_ns":123}],
  "metrics":{"rss_peak_bytes":0,"vram_peak_bytes":0,"frames":0},
  "quality":{"delta_e00_p95":0,"psnr_db":0,"ssim":0},
  "status":"pass|fail|skip",
  "reason":null
}
```

`skip` обязан иметь причину. Отсутствующая реализация не превращается в ноль
миллисекунд.

## Mandatory derived metrics

```text
T_first = probe + visible_decode + postprocess + upload + first_present
T_full  = probe + full_decode + cache + all_upload + export/present
speedup(P) = T(1) / T(P)
efficiency(P) = speedup(P) / P
hit_ratio = cache_hits / cache_requests
miss_rate = frames_over_budget / rendered_frames
memory_per_MP = peak_bytes / decoded_megapixels
```

For a stage with arithmetic intensity `I = FLOPs / byte`:

```text
T_lower_bound >= max(bytes / memory_bandwidth, FLOPs / peak_FLOPs) + sync_cost
```

This prevents claiming that more workers improved a stage which is already
limited by DRAM or PCIe.

## Regression policy

- p95 latency regression >10%: fail;
- peak RSS/VRAM regression >10%: fail;
- cache hit ratio drop >5 percentage points: fail;
- tiled seam error >1 LSB: fail;
- deterministic export checksum change: fail unless fixture/version changed;
- ΔE00 p95 increase >0.2 in a fixed quality tier: fail;
- unsupported backend: `skip` with explicit reason.

External projects are compared only at the same stage and quality target:
RawSpeed/LibRaw decode-only, darktable/RawTherapee final-quality processing,
RapidRAW GPU editor. No external wall-time claim is copied without running the
same fixture, cache state, compiler/build mode and output-quality gate.
