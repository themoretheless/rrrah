# DNG tile decode и параллельный full-quality путь

Этот документ описывает следующий этап после downsample fallback: полный RAW не
уменьшается перед GPU, а хранится как набор immutable tiles и в GPU резидентны
только tiles, пересекающие viewport.

## Что уже умеет rawler

`rawler 0.7.2` распознаёт DNG storage mode (`Strips`/`Tiles`), compression
`None`, lossless/modern JPEG, lossy JPEG и JPEG-XL. Для tiled DNG его внутренний
`decode_tiles`:

1. читает `TileOffsets`/`TileByteCounts`;
2. проверяет ожидаемое число tiles;
3. выделяет полную destination mosaic;
4. декодирует каждый compressed tile независимо через Rayon;
5. применяет DNG `LinearizationTable` после decode;
6. crop-ит padding и обрабатывает `RowInterleaveFactor`/`ColumnInterleaveFactor`.

Это полезно для cold full-frame decode, но не для fast viewport open: API
`Decoder::raw_image` возвращает только полный `RawImage`, поэтому видимый tile
нельзя запросить без декодирования и allocation всего кадра. Full-quality
viewer должен либо получить upstream tile API, либо иметь отдельный bounded
TIFF/DNG planner/decompressor.

## DNG tile planner

При probe извлекаются и валидируются:

```text
ImageWidth, ImageLength
TileWidth, TileLength
TileOffsets[], TileByteCounts[]
Compression, Predictor, PhotometricInterpretation
BitsPerSample, SamplesPerPixel, SampleFormat, PlanarConfiguration
LinearizationTable
OpcodeList1/2/3
```

Для chunky planar configuration индекс определяется row-major:

```text
cols = ceil(image_width / tile_width)
rows = ceil(image_height / tile_length)
tile_id = tile_y * cols + tile_x
```

Для каждого tile планировщик хранит checked span:

```rust
struct TileSpan {
    id: u32,
    x: u32,
    y: u32,
    width: u32,       // edge tile may be smaller than TileWidth
    height: u32,
    file_offset: u64,
    compressed_len: u64,
    decoded_len: u64,
    halo: u32,
}
```

`file_offset + compressed_len <= file_size`, количество spans совпадает с
`cols * rows`, а `decoded_len` считается checked-арифметикой. Никаких allocation
по значениям из TIFF без pixel/byte budget.

## Bounded parallel scheduler

Параллелится не весь pipeline целиком, а независимые фазы:

```text
probe/IFD ── sequential
     ↓
tile spans ── parallel pread/read_at
     ↓
decompress ── parallel, one decoder state per tile
     ↓
linearization/opcode-1 ── parallel tile-local operations
     ↓
publish immutable tile ── single cache transaction
     ↓
GPU upload ── one submission thread, bounded staging ring
```

Количество CPU workers не должно бездумно равняться числу логических потоков:

```text
workers = min(tile_count,
              physical_cores,
              floor(memory_bandwidth / bytes_per_tile / target_tile_rate))
```

Практический стартовый режим: `physical_cores - 1`, но с одним I/O permit на
NVMe queue depth и с отдельным лимитом decoded bytes. Rayon global pool rawler-а
уже применяет parallel decode, поэтому нельзя поверх него запускать ещё один
unbounded Rayon/task pool. Для нашего planner-а лучше отдельный named pool или
использование rawler pool с `join`/channel credits.

Каждая задача резервирует два веса до чтения:

```text
compressed_budget += compressed_len
decoded_budget += decoded_len * (1 + halo_overhead)
```

Если budget исчерпан, задача ждёт permit. Это предотвращает пиковое потребление
RAM при DNG с тысячами больших tiles.

## Viewport-first и halo

Для bilinear Bayer демозаики tile достаточно расширить на `halo = 1` sensor
pixel. Для MHC/RCD/edge-aware kernels halo зависит от radius:

```text
halo = max(demosaic_radius, color_filter_radius, defect_radius)
```

Соседний tile может быть cache-hit; иначе scheduler добавляет его как low-priority
dependency. Нельзя демозаицировать tile без halo и потом склеивать края: появится
тонкая seam-полоса при zoom.

При viewport `(x, y, w, h)` сначала планируются tiles, пересекающие
`expand(viewport, halo)`. Порядок приоритета:

```text
visible + halo > next viewport direction > opposite direction > idle mip fill
```

Первый frame публикуется после минимального visible set, а не после всего DNG.

## Compression и независимость

| DNG storage | Параллелизация | Важное условие |
|---|---:|---|
| Uncompressed tiles | Отличная | `pread` по spans, endian/bit unpack tile-local |
| Lossless JPEG tiles | Отличная | отдельный JPEG decoder state; predictor не пересекает tiles |
| Deflate tiles | Отличная | Predictor 2 reset per row/tile; проверять compressed bounds |
| JPEG-XL tiles | Отличная | отдельный decoder context и memory budget |
| Strips | По strips | независимы только если offsets/bytecounts раздельные |
| Single strip | Ограничена | сначала sequential entropy stream, как CR2 |

DNG tile row-major layout не означает, что tiles можно читать одним contiguous
range: offsets могут быть разрежены. Используется `pread`/`read_at`, а не общий
seekable cursor. Для NVMe можно группировать близкие spans, но нельзя нарушать
tile id mapping.

## Linearization и OpcodeList

После entropy decode порядок семантических стадий должен оставаться DNG
совместимым: `OpcodeList1` → `LinearizationTable`/black-level mapping →
`OpcodeList2` → demosaic → `OpcodeList3`. Для integer data lookup должен быть
детерминированным; dithering, если используется, получает seed от
`(source_hash, tile_id, row)` и поэтому не зависит от порядка worker-ов.

Opcode boundaries обязаны быть явными:

```text
OpcodeList1: raw-space operations, до linearization
OpcodeList2: operations после linearization, до demosaic
OpcodeList3: operations после demosaic (до final export/display stages)
```

Любой opcode, который меняет соседние pixels, расширяет halo или заставляет
смотреть за пределы tile. В MVP лучше вернуть `UnsupportedDngOpcode`/degraded
state, чем тихо пропустить OpcodeList и показать неверный цвет. Cache key обязан
включать digest применённых opcode lists и semantic version pipeline.

## CR2: почему другой алгоритм

Обычный CR2 содержит один lossless-JPEG entropy stream. Canon slice table
описывает раскладку строк/сегментов, но не создаёт независимые JPEG bitstreams.
Поэтому безопасная схема:

```text
sequential entropy decode → row-band publish → prefetch next file
```

Параллелить можно:

- Huffman table preparation;
- post-decode linearization/black correction по row bands;
- копирование уже декодированных bands в cache/GPU staging;
- соседние файлы в bounded prefetch pool.

Нельзя без доказательства делить один bitstream по строкам: JPEG predictor и
entropy state могут зависеть от предыдущего MCU/scan. Для CR2 ускорение full
decode достигается SIMD Huffman/predictor и overlap I/O, а не worker-per-row.

## Cache identity

Tile cache key:

```text
full_file_digest
frame/image index
tile_id, tile_x, tile_y
tile_width, tile_length
compression + predictor + endian
linearization digest
opcode-list digest
decoder ABI
demosaic/colour semantic version
```

Сырой decoded mosaic tile можно переиспользовать при смене exposure, WB и tone
mapping. Нельзя переиспользовать его после смены linearization/opcode semantics.
Для persistent cache один immutable file с TOC лучше тысячи файлов:

```text
header + tile index + compressed decoded tiles + per-tile BLAKE3
```

Публикация tile атомарна только после checksum; повреждённый tile инвалидирует
одну запись, а не весь каталог.

## Ожидаемое ускорение

Идеальная оценка для `N` независимых tiles:

```text
T_parallel ≈ max(T_read / Q, T_decode / W, T_publish)
```

Реальный speedup ограничивают память и JPEG decoder. Для 50 MP DNG с 256 tiles
при 8 workers разумная цель — 4–7× против single-thread tile decode; после
достижения memory-bandwidth plateau добавление threads ухудшает latency. CR2
получает обычно 1.2–2× от SIMD/overlap, но не N× от числа slices.

Benchmark должен отдельно публиковать:

```text
probe p50/p95
first visible tile p50/p95
all tiles decode p50/p95
CPU busy%, peak RSS
cache hit/miss
GPU upload and first-frame latency
```

Нельзя сравнивать tiled visible-open с полным `raw_image` как одну цифру: это
разные quality contracts.
