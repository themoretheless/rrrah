# Benchmark matrix for the full editor

Один benchmark не может одновременно измерить decoder, качество демозаики и
интерактивность. Поэтому suite разделён на четыре независимых контура.

## Corpus

| Case | Пример | Что выявляет |
|---|---|---|
| C1 | CR2 Bayer 50–60 MP, lossless JPEG | последовательный entropy/predictor |
| C2 | DNG uncompressed strips | I/O и memory bandwidth |
| C3 | DNG tiled 256/512/1024 | parallel tile scheduler |
| C4 | DNG float/LinearRaw | 32-bit path и linearization |
| C5 | X-Trans/4-color CFA | generic CFA correctness |
| C6 | corrupted/truncated files | bounds, cancellation, OOM budget |
| C7 | 24/50/100 MP | scaling RAM/VRAM и mip residency |

Для каждого файла хранить SHA-256/BLAKE3, dimensions, bit depth, CFA, camera,
storage type и reference 16-bit TIFF. Embedded JPEG не используется ни в одном
decode или quality тесте.

## Four suites

### S1: Decode/open

Запускать cold OS page-cache и warm page-cache; `workers=1,2,4,8`. Снимать:

```text
probe_ns, entropy_ns, predictor_ns, tile_post_ns,
cache_read_ns, cache_write_ns, first_visible_tile_ns, full_mosaic_ns,
peak_rss, bytes_read, stale_tasks, errors
```

Ожидаемая форма: CR2 показывает слабый scaling (serial entropy); tiled DNG —
рост throughput до насыщения SSD/RAM.

### S2: GPU/render

Проверять full-screen view при fit, 1:1, 2×, 4× zoom и pan. Снимать GPU timer
queries, frame p50/p95/p99, dropped frames, upload bandwidth, resident tiles,
peak VRAM. Acceptance: 60 Hz означает `frame p95 ≤ 16.7 ms`, 120 Hz — `≤ 8.3 ms`.

### S3: Quality

Рендерить эталонный TIFF и текущий output в linear light. Сравнивать:

```text
ΔE2000 (median/p95), PSNR, SSIM, clipped-highlight area,
neutral-gray error, edge acutance, CFA seam error
```

Сравнение decode-only с AMaZE/RCD/darktable unfair: качество-профили должны
иметь отдельные tiers `fast`, `balanced`, `quality` и одинаковые input metadata.

Подробный corpus, synthetic ground truth, ColorChecker/ISO 12233 protocol,
ΔE00/MTF/seam/noise формулы и regression gates находятся в
[QUALITY_BENCHMARKS.md](QUALITY_BENCHMARKS.md). Quality suite обязан сохранять
linear TIFF/EXR outputs и metadata sidecar; screenshot или embedded JPEG не
являются допустимым эталоном.

### S4: Editor/memory

Сценарии: каталог 1000 файлов, быстрое листание вперёд/назад, отмена decode,
zoom/pan, пять масок, batch export 100 кадров. Снимать input→frame latency,
cache hit ratio, queue depth, CPU/RAM/VRAM high-water mark, journal recovery.

## Derived metrics

```text
open_latency = probe + entropy + post + cache + upload + first_visible
effective_MP_s = decoded_pixels / (stage_ns / 1e9) / 1e6
memory_per_MP = peak_bytes / megapixels
speedup(P) = T(1) / T(P)
parallel_efficiency(P) = speedup(P) / P
```

Теоретический потолок:

\[
S(P)=\frac{1}{f_s+f_p/P},\qquad
T_{roofline}\ge\max(\frac{bytes}{BW},\frac{FLOPs}{FLOP/s}).
\]

Каждый результат публиковать с p50/p95/p99 и 95% bootstrap CI, минимум 5
прогонов после двух прогревочных. Не усреднять cold и warm состояния.

## Open-source comparison protocol

Запускать одинаковые C1–C7 на rrrah, RawSpeed loader, LibRaw, darktable и
RawTherapee. Для darktable/RawTherapee отдельно фиксировать first preview и
final-quality 16-bit export. RapidRAW измерять отдельно как GPU editor path.
Публиковать версии, compiler flags, SIMD target, GPU driver и cache state; не
переносить чужие цифры без воспроизводимого harness.
