# Tiled RAW display: математические инварианты

Этот документ фиксирует правила, необходимые для перехода от одной большой
`R16Uint` текстуры к полному RAW в виде resident/non-resident tiles.

## Tile contract

Tile адресуется в координатах исходного сенсора:

```text
TileId { frame, mip, tile_x, tile_y }
```

Входной буфер имеет размер `(tile_size + 2H)²`, где `H` — радиус алгоритма.
Алгоритм пишет только центральный `tile_size²` прямоугольник. Координаты CFA,
black-level и crop всегда вычисляются от `global_origin`, никогда от локального
`(0, 0)` tile.

Минимальные радиусы:

| алгоритм | halo `H` |
|---|---:|
| Bayer bilinear | 1 |
| MHC (5×5) | 2 |
| RCD directional | 2–3 |
| AMaZE-like iterative | 4–6 |

Любой новый kernel обязан явно объявлять `halo_radius()`. Если соседний tile
отсутствует, scheduler сначала поднимает tile с halo; fallback на parent mip
допустим только до момента появления полного-resolution tile.

## CFA и сенсорная линейзация

Для глобального sample `(x, y)`:

```text
phase = (global_y & 1) * 2 + (global_x & 1)
L = max((raw - black(global_x, global_y)) /
        max(white(global_x, global_y) - black(global_x, global_y), 1), 0)
```

`L` is scene-linear and may exceed one while preserving highlight headroom. The
upper clamp is applied only by the final display/export tone/quantization pass.

`black()` и `white()` могут быть scalar, 2×2 repeat-grid, row/column delta или
spatial grid из DNG. Lookup получает глобальные координаты до crop/orientation.
White balance применяется после linearization, camera matrix — после demosaic.
Orientation выполняется только в display mapping, иначе меняется CFA phase.

## Mip levels

Усреднять raw samples напрямую нельзя: соседние значения принадлежат разным
цветовым каналам. Безопасные варианты:

1. demosaic в linear RGB на full resolution, затем RGB-filter;
2. держать четыре CFA-плоскости и уменьшать каждую отдельно;
3. строить mip tile из 2×2 соседних tiles плюс filter halo.

Уровень `mip + 1` зависит от четырёх tiles уровня `mip`. Границы вычисляются
одинаково независимо от того, были ли соседи уже resident: сначала используются
полные исходные samples, затем выполняется reduction.

## Seam-free guarantee

Плиточный результат должен совпадать с монолитным oracle внутри допустимой
погрешности float rounding. Запись выполняется только в interior, поэтому
пересечение halo не создаёт гонки. На настоящей границе сенсора используется
тот же clamp/mirror policy, что и у monolithic kernel.

Для каждого corpus frame проверяются:

- `max_abs(tile - monolithic)` и `mean_abs`;
- PSNR/SSIM после tone-map;
- разность градиентов в полосе шириной 2–4 px по всем tile boundaries;
- все 8 EXIF orientations и crop offsets 0/1 по обеим осям;
- нечётные размеры tiles (257, 511), неполные края и размеры 1×N.

Для deterministic kernels целевой порог — `max_abs <= 1 LSB` (или `PSNR > 60 dB`
для float MHC/RCD). GPU shader сравнивается с маленьким CPU reference на случайных
патчах, затем с реальными CR2/DNG.

## Parallel execution

* CR2 lossless-JPEG entropy stream в основном последовательный; после entropy
  decode row bands можно распараллелить по tile rows.
* DNG `TileOffsets/TileByteCounts` независимы и планируются bounded worker pool.
* Normalize, demosaic и RGB/tone-map — независимые workgroups по tiles/pixels.
* Mip reduction — DAG: каждый output tile ждёт 2×2 source tiles.
* Decode, upload и render перекрываются ring buffers; отмена выполняется через
  generation token, чтобы старый prefetch не публиковал результат нового кадра.

Не допускается unlimited `spawn_blocking`: лимитируется одновременно занятая
RAM для compressed bytes, decoded mosaic и GPU staging.
