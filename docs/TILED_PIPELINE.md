# Full-resolution tiled pipeline

Цель следующего этапа — убрать downsample fallback и показывать полный RAW на
GPU, не создавая texture `8896×5920`, если устройство ограничено `8192`.

## Логическая модель

Изображение остаётся одной виртуальной RAW-плоскостью в координатах сенсора.
Физически она разбита на tiles:

```text
logical RAW:  W × H, CFA phase в глобальных координатах
tile interior: T × T       (начать с 512, сравнить 256/1024)
halo:          H           (bilinear=1, MHC/RCD=2..4)
physical tile: (T+2H)² u16
```

Tile никогда не пересчитывает CFA-фазу относительно своего локального `(0,0)`.
Для каждого sample используется исходная координата:

```text
cfa = pattern[(sensor_y & 1) * 2 + (sensor_x & 1)]
```

Black-level grids и crop также вычисляются в sensor coordinates. Halo копируется
из соседних tiles; на физком краю применяется тот же clamp/mirror, что и в
монолитном reference kernel.

## CPU pipeline

```text
Planner
  → bounded compressed-I/O queue
  → entropy decoder
  → tile assembler + halo
  → CPU tile cache
  → bounded GPU staging queue
```

### CR2

Lossless-JPEG CR2 обычно содержит один entropy stream. Нельзя запускать N
независимых декодеров, если в формате нет доказанных restart markers или
независимых slices: это даст гонки, лишний I/O и неверную bitstream state.

Правильное распараллеливание:

```text
one sequential entropy lane
  → bounded row-band ring
  → N parallel predictor/normalization/tileizers
  → GPU upload batches
```

Entropy lane — узкое место, но predictor reconstruction, black-level
normalization, halo assembly и upload могут работать параллельно. Если parser
доказывает restart boundaries, отдельные entropy ranges включаются только как
opt-in fast path.

### DNG

`TileOffsets`/`TileByteCounts` независимы, поэтому видимые tiles могут читать и
распаковывать отдельные workers. Scheduler не должен делать unbounded
`spawn_blocking`: используются bounded queues и byte credits до allocation.

Каждая задача содержит:

```text
{ generation, content_key, mip, tile_x, tile_y, halo, priority, deadline }
```

Переход файла увеличивает generation. Старый worker может завершиться, но не
может publish результат в UI или GPU residency map.

## GPU residency

Используется `texture_2d_array<R16Uint>` как tile atlas:

```text
layer 0: tile (0,0)
layer 1: tile (1,0)
layer 2: tile (2,0)
...
```

CPU хранит `logical tile → physical layer` mapping. В production mapping лучше
поместить в маленькую `R32Uint` page-table texture/storage buffer. Missing tile
не блокирует render pass: shader берёт coarser resident mip либо предыдущий
кадр, а foreground tile подгружается асинхронно.

GPU budget:

```text
tile_bytes = 2 × (T + 2H)²
layers = floor(gpu_tile_budget / tile_bytes)
```

Видимые tiles pin-ятся до завершения GPU submission. Prefetch tiles вытесняются
первыми. Upload идёт через 3–4 staging slots и batch submit; нельзя делать
`device.poll(Wait)` на каждый tile.

## Compute path

Для bilinear fragment shader допустим и прост: он делает несколько
`textureLoad` на пиксель. Для MHC/RCD и повторно используемых mip tiles лучше
compute pass:

```text
source R16Uint tile+halo
  → workgroup cooperative load
  → workgroupBarrier()
  → demosaic/color output RGBA16F tile
  → final fullscreen fragment
```

Начальная конфигурация: workgroup `16×16`, halo `2..4`. Каждая workgroup пишет
только свой interior, поэтому нет глобального barrier. Требования к
`max_compute_workgroup_storage_size` и `max_compute_workgroup_size_*` проверяются
при инициализации адаптера.

## Mip levels

Нельзя просто усреднять Bayer mosaic как обычную серую картинку: это смешивает
красный, зелёный и синий samples. Возможны два корректных пути:

1. demosaic full-resolution tile → RGB area filter → RGB mip;
2. decimate четыре CFA planes отдельно, сохраняя фазу 2×2.

Mip tile зависит от 2×2 source tiles и filter halo, поэтому это DAG задач,
который строится только при необходимости. При `zoom >= 1` используется mip 0
без downsample; при fit-to-window сначала показывается resident coarser mip,
затем постепенно заменяется mip 0.

## Приоритеты scheduler-а

```text
P0 current viewport + halo
P1 соседние tiles текущего viewport
P2 следующий/предыдущий файл
P3 idle mips и каталог
```

Префетч останавливается, если foreground backlog превышает примерно два кадра.
Количество workers ограничивается:

```text
n_workers ≤ min(
    physical_cores - 1,                 // оставить UI/submit thread
    decoded_budget / tile_bytes,
    io_depth
)
```

Эффективная пропускная способность:

```text
throughput = min(io_rate, decode_rate, postprocess_rate, upload_rate)
```

Добавление потоков после насыщения минимального из этих этапов только увеличит
очередь и memory pressure.

## Тесты корректности

- monolithic reference vs tiled output на одинаковом RAW;
- случайные размеры tile и нечётные crop origins;
- все четыре Bayer phases;
- EXIF orientations 1–8;
- seam test на границах tiles;
- synthetic ramp/edge/checkerboard;
- DNG с independent tiles и CR2 с одним entropy stream;
- cancellation: старый generation не публикуется;
- GPU/CPU output: exact integer source, затем PSNR/ΔE для float stages.

## Порядок реализации

1. Tile key/state machine и bounded scheduler.
2. CPU tile extraction с halo из уже декодированной mosaic.
3. `texture_2d_array` atlas и точное sensor-coordinate mapping.
4. Full-res bilinear fragment path без downsample.
5. GPU compute demosaic для MHC/RCD.
6. Page table, mip residency и persistent per-tile cache.
7. DNG independent-tile decode; затем restart-aware CR2 optimization.
