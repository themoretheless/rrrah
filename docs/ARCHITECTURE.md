# Rrrah: архитектура fast-open RAW viewer

## Цель и инварианты

Rrrah — native viewer для CR2/DNG, где первый корректный кадр строится из
сенсорных данных. Встроенный JPEG не является частью основного пути: декодер
вызывает `rawler::Decoder::raw_image(..., false)`, а GPU получает исходную
мозаику `u16`.

Целевые KPI следует измерять на конкретном железе, а не обещать универсальные
миллисекунды:

- `open → metadata-ready` (обычно десятки KiB I/O);
- `open → first RAW-derived pixels`;
- `open → viewport-complete`;
- `open → full decoded mosaic`;
- отдельно cold/warm page cache, CR2/DNG, p50/p95;
- GPU upload, shader time, dropped frames, cache byte-hit ratio.

Типичный 24 MP Bayer-файл занимает `24e6 × 2 ≈ 48 MB` в `R16Uint`. Полный
`RGBA16F` занял бы 192 MB, а `RGBA32F` — 384 MB. Поэтому CPU-demosaic всего
кадра до upload не допускается.

## Слои и SOLID

```text
CLI / winit UI
    │  bounded events, generation gate
FastOpenCoordinator
    ├── RawDecoder port ── rawler adapter (optional LibRaw/worker fallback)
    ├── Cache port ─────── RAM weighted LRU + persistent mosaic blobs
    └── Gpu port ──────── wgpu R16Uint → WGSL demosaic/display
```

- **S**: `rrrah-core` хранит только инварианты и математику; `rrrah-decode`
  адаптирует rawler; `rrrah-cache` отвечает за fingerprints/атомарные blobs;
  `rrrah-gpu` — только ресурсы и shader; app — оркестрация и ввод.
- **O**: `RawDecoder` уже является исполняемым портом; `CachePort` и `GpuPort`
  пока задокументированы как целевые границы coordinator (MVP использует
  конкретные `DiskMosaicCache`/`RawRenderer`). Их выделение в traits — первый
  шаг перед добавлением LibRaw/Metal/Vulkan бэкендов.
- **L**: любой декодер обязан вернуть `DecodedMosaic`, валидированный по
  `width × height × components`; preview-only результат не удовлетворяет
  контракту.
- **I**: probe, I/O, entropy decode, cache и UI не зависят от конкретных
  реализаций друг друга.
- **D**: coordinator зависит от портов, а не от TIFF/Metal/Vulkan деталей.

## Путь данных и математика

### 1. Сенсорная линейзация

Для пикселя `(x,y)` и CFA-фазы `c`:

\[
  L(x,y) = \frac{\max(raw(x,y)-B_c(x,y),\ 0)}
                 {W_c(x,y)-B_c(x,y)}.
\]

`BlackLevelRepeatDim`, row/column deltas и `LinearizationTable` должны быть
применены до demosaic (в текущем fast path DNG repeat-grid хранится полностью,
а WGSL использует top-left 2×2; tiled backend расширит lookup). Верх выше единицы
не обрезается до tone mapping, чтобы не терять highlight headroom.

### 2. Bayer bilinear

Для красного sample:

\[
R=C,\quad G=\frac{N+S+E+W}{4},\quad
B=\frac{NW+NE+SW+SE}{4}.
\]

Для синего формулы симметричны. Для зелёного два соседних по горизонтали
пикселя определяют, интерполировать ли R горизонтально, а B вертикально, либо
наоборот. На границе координаты clamp-ятся. У bilinear есть быстрый путь для
первого кадра; production-quality idle path — MHC 5×5 с halo 2.

При `scale < 0.5` обычный Bayer mip недопустим: он смешивает разные цветовые
фазы. Сейчас shader использует четыре CFA-aware stratified samples; следующий
этап — half/quarter linear-RGB tiles. Для MHC/RCD tile нужен halo, иначе на стыках
появятся швы.

### 3. Цвет

Rawler предоставляет `wb_coeffs` и `xyz_to_cam`. Для D65:

\[
M_{rgb\to cam}=normalize(M_{xyz\to cam}M_{sRGB\to XYZ}),\quad
M_{cam\to rgb}=M_{rgb\to cam}^{-1}.
\]

Нормализация выполняется по сумме строки. Затем применяется диагональный WB,
матричное преобразование и exposure `2^stops`. Сингулярная матрица не должна
ронять viewer: shader использует identity fallback и показывает предупреждение.

Выходной render target — sRGB surface. Shader возвращает linear RGB, а аппаратный
sRGB view выполняет transfer function. До отображения применяется компактный
ACES-fitted tone map:

\[
f(x)=\frac{x(2.51x+0.03)}{x(2.43x+0.59)+0.14}.
\]

Для color-managed production режима добавляются D50 Bradford adaptation,
ForwardMatrix/ColorMatrix interpolation, ICC/DCP и Display-P3/HDR output.

## GPU стратегия

`R16Uint` нельзя линейно фильтровать; WGSL использует `textureLoad`, что сохраняет
точные 12/14/16-bit values. Один fullscreen triangle вычисляет только видимую
область: normalize → demosaic → WB → camera→sRGB → tone map.

```text
CPU entropy decode → R16Uint texture (COPY_DST | TEXTURE_BINDING)
                       ↓
                 viewport shader
                       ↓
             sRGB render attachment
```

`Queue::write_texture` выбран в прототипе за простоту. Для production больших
файлов нужен persistent triple staging ring; `copy_buffer_to_texture` требует
`bytes_per_row` 256-byte alignment. GPU resources освобождаются только после
submission fence, а не при eviction request.

Обычный CR2 — один lossless-JPEG entropy stream: Canon slice table описывает
раскладку результата, не независимые bitstreams. Поэтому CR2 имеет
sequential-prefetch plan. Tiled DNG имеет независимые `TileOffsets` и
`TileByteCounts`: viewport tiles можно читать и декодировать параллельно.

Источники и практические антипримеры: [rawler](https://github.com/dnglab/dnglab),
[RapidRAW raw processing](https://github.com/CyberTimon/RapidRAW/blob/main/src-tauri/src/raw_processing.rs),
[wgpu](https://github.com/gfx-rs/wgpu),
[DNG 1.7.1](https://helpx.adobe.com/content/dam/help/en/camera-raw/digital-negative/jcr_content/root/content/flex/items/position/position-par/download_section_733958301/download-1/DNG_Spec_1_7_1_0.pdf),
[CR2 structure](https://libopenraw.freedesktop.org/formats/cr2/),
[lossless JPEG](https://libopenraw.freedesktop.org/formats/ljpeg/).

## Координаты, crop и orientation

GPU uniform хранит crop в **полных sensor coordinates**, поэтому CFA parity не
сбивается при `ActiveArea` offset. Для EXIF orientation shader применяет inverse
mapping display→raw; для 90°/transpose меняются display width/height. Zoom/pan
живут в screen pixels, поэтому resize не меняет sensor coordinates.

Для zoom-to-cursor при старом zoom `z`, новом `z'` и cursor `p`:

\[
pan' = pan + (p-C)(1-z'/z),\quad C=viewport/2+pan.
\]

Это устраняет ощущение «прыжка» изображения при колесе мыши.

## Кеши

### L0/L1

ОС page cache обслуживает исходный RAW. Не использовать `MAP_POPULATE` в UI и не
хешировать весь файл перед каждым открытием. Legacy V2 `SourceFingerprint`
читает первые/средние/последние 64 KiB и не является безопасным content ID.
V3 вычисляет полный `SourceId` из того же открытого snapshot, который декодирует,
а проверенный результат memoize-ится по `(volume,file_id,size,mtime_ns,ctime_ns)`.

RAM cache — byte-weighted LRU с обязательным pin текущего кадра. Для production
tile cache используется 2Q/TinyLFU: 20–25% probation защищает горячие tiles от
одноразового прохода по каталогу.

### Persistent mosaic blob

Legacy V2 `rrrah-cache` хранит `magic + schema + JSON metadata + little-endian
u16 payload + BLAKE3 checksum`, пишет во временный файл в той же директории,
делает `sync_data` и atomic rename. Его ABI заморожен и не смешивается с V3.

Новый semantic protocol разделяет полный `SourceId`, 64-byte decoder recipe и
106-byte artifact transcript; container/layout versions не входят в semantic
key, а exposure/tone/GPU state не инвалидируют mosaic. Нормативные byte layouts,
registry и bump rules находятся в [CACHE_IDENTITY_SPEC.md](CACHE_IDENTITY_SPEC.md)
и [CACHE_OBJECT_V3_SPEC.md](CACHE_OBJECT_V3_SPEC.md).

Единый application-use-case для inspect, foreground и speculative prefetch,
его typed outcomes, latest-wins mailbox и транзакционное разрешение
файла/папки определены в [OPEN_MOSAIC_DESIGN.md](OPEN_MOSAIC_DESIGN.md).
Временная модель этапов от чтения RAW до экрана приведена в
[RAW_DISPLAY_PIPELINE.md](RAW_DISPLAY_PIPELINE.md).

Production format следует расширить до одного immutable blob с таблицей
512×512 tiles, независимо сжатых LZ4/Zstd. Это исключает сотни тысяч inode и
позволяет mmap только собственных immutable artifacts.

Полный алгоритм DNG tile planner, compression-specific параллелизм, halo
зависимости демозаики, opcode boundaries и сравнение с последовательным CR2
путём вынесены в [DNG_PARALLEL.md](DNG_PARALLEL.md). Важно: rawler уже
распараллеливает tiled DNG внутри `raw_image`, но его публичный API всё равно
материализует весь кадр. Для viewport-first открытия нужен отдельный tile API
или upstream extension, иначе downsample/full-frame allocation остаются
ограничением.

Память для tile `T×T`:

\[
M_{mosaic}=2T^2,\qquad M_{RGBA16F}=8T^2.
\]

Для 45 MP полная `RGBA16F` mip-chain ≈ `8 × 45e6 × 4/3 ≈ 480 MB`; поэтому
полный RGB cache обязан быть tiled.

## Fast-open state machine

```text
Idle → AcquireHandle → Probe → Planned → CacheLookup
                         ├─ CR2 sequential flow
                         └─ DNG visible-tile flow
                                ↓
                    FirstRawPresent → Refining → Ready
```

Каждый Open получает monotonic `generation`. После read/decode/upload/publish
проверяется generation gate; старый completion может физически закончиться, но
никогда не публикуется. I/O и decode очереди bounded: сначала резервируются
compressed bytes, decoded bytes и GPU staging bytes. Неограниченный
`spawn_blocking` запрещён — блокирующую задачу нельзя надёжно abort-нуть.

## Безопасность

RAW — недоверенный input. До allocations нужны checked `offset + length`, лимиты
IFD depth/count, cycle detection, tile/strip count validation и max pixel budget.
Cache header должен проверять `pixel_count == width×height×cpp`, finite metadata,
payload length/checksum и отсутствие trailing garbage.

Текущий адаптер ловит panic rawler и превращает его в ошибку. Production должен
вынести parser/entropy decoder в отдельный read-only worker process с лимитами CPU,
RAM, wall time, IPC length и kill-on-hang; GPU context worker-у не передаётся.
Fuzz corpus: CR2 generations/slices, tiled/strip DNG, LJPEG/Deflate/JXL, big/little
endian TIFF, циклические IFD, truncation и случайные offsets. Инвариант fuzz:
«нет panic/OOM/UB, deterministic error, bounded time».

## Лицензии и ограничения

Собственный код — MIT OR Apache-2.0. `rawler` — LGPL-2.1; LibRaw — LGPL-2.1 или
CDDL-1.0; wgpu — MIT/Apache-2.0. Для закрытой поставки decoder лучше держать
отдельным динамически загружаемым/IPC backend с relinkable objects и notices;
для открытой сборки добавить LICENSE/NOTICE соответствующих компонентов.

DNG реализует технологию Adobe; в исходниках и документации должен быть заметный
notice: `This product includes DNG technology under license by Adobe.`

## Roadmap после прототипа

1. Вынести GPU init из `resumed()` в неблокирующий async handoff; заменить busy
   redraw на `EventLoopProxy` wake-up.
2. Реализовать DNG tile planner, CR2 row-band streaming и generation cancellation.
3. Добавить MHC/RCD quality pipeline, CFA-aware linear-RGB mip tiles и histogram
   compute/readback.
4. Sandbox worker, hard resource limits, cargo-fuzz и differential oracle LibRaw.
5. Прогнать CC0 corpus из raw.pixls.us и зафиксировать p50/p95 gates на NVMe,
   SD и HDD. Не публиковать миллисекундные claims без этого стенда.
