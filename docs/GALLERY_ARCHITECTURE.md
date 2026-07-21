# Галерея RAW-папки: архитектура и критические гейты

Статус: проектный результат multi-agent review, 2026-07-21.

Документ описывает следующий продуктовый слой после текущего single-file
viewer. Он не считает встроенный JPEG источником корректного изображения:
каталоговые миниатюры, filmstrip и основной кадр должны быть производными от
RAW mosaic. JPEG допускается только как явно помеченный degraded catalog
fallback, если пользователь сам его включил.

## 1. Что есть сейчас и что добавляем

Текущий `rrrah` принимает один путь, делает full-frame decode через `rawler`,
кеширует `u16` mosaic и отображает его через wgpu. Gallery mode должен добавить:

```text
CLI / folder picker / dropped path
              |
        rrrah-gallery
  catalog + session + scheduler
       /       |        \
  probe    thumbnail    full RAW
      \       |          /
       RAM/disk caches -> GPU residency
```

Каталог, просмотр и decode — разные жизненные циклы:

- `catalog_revision` меняется при scan/rescan и не должен сбрасывать текущий
  кадр;
- `view_generation` меняется при выборе файла, смене папки и viewport;
- завершение старого поколения может освободить ресурсы, но никогда не может
  публиковать UI, GPU или cache result.

## 2. SOLID-порты

Новый crate `rrrah-gallery` не должен знать о `rawler` и wgpu деталях:

```rust
trait DirectoryScanner {
    fn scan(&self, root: RootId, cancel: CancelToken) -> CatalogStream;
}

trait CatalogStore {
    fn snapshot(&self, root: RootId, range: Range) -> CatalogPage;
    fn apply(&mut self, delta: CatalogDelta);
}

trait RawProbe {
    fn probe(&self, source: OpenSource, cancel: CancelToken) -> ProbeResult;
}

trait ThumbnailProvider {
    fn request(&self, asset: AssetId, variant: ThumbVariant, stamp: Stamp)
        -> SharedResult<Tile>;
}

trait GalleryScheduler {
    fn submit(&self, work: WorkItem) -> Admission;
    fn cancel_generation(&self, session: SessionId, generation: Generation);
}

trait GpuResidency {
    fn request(&mut self, tile: TileKey, bytes: TileBytes) -> UploadTicket;
    fn poll_fences(&mut self) -> FenceProgress;
    fn recover(&mut self) -> Recovery;
}
```

UI владеет только model/render state. Worker не вызывает winit и не меняет
`current_frame` напрямую. Замена rawler на LibRaw/RawSpeed, RAM cache на
SQLite/redb или Metal/Vulkan backend не должна менять `GalleryModel`.

## 3. Каталог и UX

### 3.1 Открытие

Поддержать три эквивалентных пути:

1. `rrrah [OPTIONS] <DIR|RAW>` (CR2/CR3/DNG in the current fast path);
2. `Cmd/Ctrl+O` и native folder picker за портом `FolderPicker`;
3. `WindowEvent::DroppedFile`: папка открывается как root, RAW-файл открывает
   его parent и выбирает этот item.

Диалог не блокирует event loop. Отмена picker — no-op. По умолчанию scan
нерекурсивный; recursive mode требует явного включения и квот.

### 3.2 Модель

```text
FileIdentity = (device, file_id, size, mtime_ns, content_digest)
AssetId      = stable hash(FileIdentity)
GalleryItem  = { AssetId, path, name, format, metadata, thumb, state, flags }
GalleryModel = { root, items, visible_ids, selected AssetId, sort, filters,
                 catalog_revision, view_generation, scan_state }
```

Индекс — не identity: сортировка, поздний probe и фильтры не должны менять
выбранный файл. `visible_ids` — производный список. Сортировка должна быть
детерминированной:

- имя: natural Unicode sort, case-fold tie-break, затем исходные bytes/path;
- CaptureTime: EXIF/TIFF timezone offset, unknown-last; mtime не маскируется
  под capture time;
- size, modified, rating/flag — с тем же стабильным `AssetId` tie-break.

### 3.3 Макет и управление

```text
┌ Open Folder · breadcrumb · scan progress · sort/filter · Grid/Viewer ┐
│ virtualized grid / GPU RAW thumbnails                                │
├──────────────────────────────────────────────────────────────────────┤
│ viewer (fit/zoom/pan/exposure)       filmstrip ± prefetch window      │
└──────────────────────────────────────────────────────────────────────┘
```

- `G` — grid, `Esc` — назад в grid, `Enter`/double-click — открыть;
- `←/→`, `J/K`, `Home/End`, `PageUp/PageDown` — следующая/предыдущая RAW;
- wheel над filmstrip — виртуальный scroll, wheel над viewer — zoom;
- `F`, `R`, `+/-` сохраняют текущие функции;
- card показывает RAW-derived tier, orientation, metadata/error badge и retry;
- удаление файлов отсутствует: «Remove from gallery» только убирает item из
  model, «Reveal in Finder» — безопасное внешнее действие.

Галерея не создаёт GPU texture на каждый файл. Grid/filmstrip виртуализированы:
в памяти находятся видимые карточки и ограниченное окно соседей.

## 4. Двухфазный pipeline

```text
Opening(root)
  -> Enumerating -> Probing batches -> Ready/Rescanning

Select(asset)
  -> AcquireHandle -> MetadataReady -> CacheLookup
  -> Thumb/VisibleTileRequested -> FirstRawPresent -> Refining -> Ready
                         \-> Failed / Cancelled / Stale
```

### 4.1 Scan и probe

Первая фаза перечисляет только directory entries, extension, type и bounded
stat. Вторая отправляет batches по 64–256 items на metadata-only `RawProbe`.
У `rawler 0.7.2` есть `raw_metadata`, поэтому probe не должен вызывать
`raw_image` и выделять полный mosaic. Если конкретный backend не может сделать
probe, item получает `Unknown`, а UI не блокируется.

UI получает `CatalogDelta` постепенно: ошибки одного файла не останавливают
scan. Очередь probe ограничена (например, 2 000 pending), сканер поддерживает
cancel/backpressure и watcher debounce для rescan.

### 4.2 Миниатюра и основной кадр

Приоритеты:

```text
P0 selected metadata + first visible RAW tile/mip
P1 visible filmstrip items + CFA/halo tiles
P2 next/previous in direction of navigation
P3 current-frame MHC/RCD refinement
P4 idle catalog/background work
```

Миниатюра строится из RAW bilinear mip, например longest edge 256/1024 px.
Демозаика, crop, orientation, WB и color profile должны использовать тот же
metadata digest, что и full view. Для zoom/refinement GPU residency загружает
только видимые tile + halo.

## 5. Scheduler, preload и память

Очереди bounded и раздельны:

- probe I/O: 1–2 workers;
- decode: `min(configured, physical_cores - 1)`;
- postprocess: примерно половина physical cores;
- один GPU submit owner;
- background scan/export только при отсутствии visible deadline.

Каждая работа получает `session`, `generation`, `AssetId`, tile/quality и RAII
reservation. До enqueue резервируются compressed, decoded, staging, GPU и
metadata bytes:

\[
  R_p + N_p \le C_p,\qquad
  N_p = B_{compressed}+B_{decoded}+B_{staging}+B_{gpu}+B_{metadata}.
\]

Если бюджет не позволяет admission, отбрасывается самый низкий prefetch, а не
visible work. Один и тот же `(asset, variant, tile)` coalesces в producer +
несколько subscribers; отмена одного subscriber не убивает producer, нужный
новому поколению.

Политика соседнего preload адаптивна:

\[
K = clamp(ceil(|v|\tau / item\_extent), 1, 16),
\]

где `v` — скорость навигации, `τ` — измеренный decode+upload latency. В кэшах
pin-ятся выбранный item, visible/halo tiles и in-flight GPU slots. Eviction
возможна только после submission fence.

RAM: byte-weighted 2Q/TinyLFU (20–25% probation, 75–80% protected). GPU:
logical LRU/page table с фиксированными slots, valid rect, tile generation и
fence state. При device loss RAM/disk сохраняются, максимум две попытки recovery,
затем видимый CPU-bilinear fallback с degraded badge.

## 6. CR2 и DNG: разные модели параллелизма

### CR2

Canon lossless-JPEG entropy/predictor обычно образует одну последовательную
lane. Слайды/stripe descriptors сами по себе не дают независимого bitstream.
Правильная схема: один reader + read-ahead row ring (32–128 rows), затем
параллельные unpack, black/linearization, postprocess и upload. Linear speedup
для entropy decode не заявлять.

### DNG

Проверенные `TileOffsets`/`TileByteCounts` дают независимые `read_at` tasks.
Число workers ограничивается минимумом CPU, I/O queue depth и compressed/decoded
budget. Эффективность отчётная:

\[
  \eta_n = T_1/(nT_n),
\]

цель для 8 workers — `η ≥ 0.5` до насыщения I/O/DRAM. Single-strip, unknown
OpcodeList или tile, пересекающий halo dependency, явно переходит в sequential
fallback.

## 7. Безопасность и корректность — P0 до folder release

RAW и папка считаются недоверенным вводом.

1. Не делать `is_file → fingerprint → повторное открытие path → decode`.
   Открывать один handle с no-follow, проверять `fstat` regular/size budget и
   читать через handle/`read_at`. Иначе возможен TOCTOU и запись RAW B под key A.
2. Sampled head/middle/tail fingerprint не годится для cache admission: внутренние
   байты могут измениться при том же size/mtime. Нужен full BLAKE3 либо проверка
   содержимого на том же handle перед publish/cache commit.
3. Не следовать symlink/reparse points; отбрасывать FIFO, device и socket.
   Ограничить depth, entries, суммарные bytes, имя и wall time; не допускать
   symlink loops и million-entry OOM.
4. `rawler`/entropy decode для произвольной папки должен идти в короткоживущем
   sandbox worker с CPU/RSS/wall/output quotas. `catch_unwind` внутри процесса
   недостаточен против зависания или memory exhaustion.
5. Перед serde allocation ограничить TIFF/IFD counts, vector/string lengths,
   tile count и decompression ratio. Cache: checksum, atomic rename,
   owner-only permissions, quarantine corrupt entries и disk high/low watermarks.
6. Все publish operations проверяют `(session, generation, AssetId, variant)`;
   stale UI/GPU/cache commits запрещены.

### Обязательные adversarial tests

- замена файла между fingerprint/decode, rename/delete/permission race;
- изменение внутренних bytes без изменения size/mtime/sampled bytes;
- symlink loop, FIFO/device/reparse point, hardlink aliases;
- million-entry/deep folder с bounded memory и cancellation;
- corrupt/truncated TIFF/IFD, u64 overflow, CR2 bad markers, DNG tile overlap,
  Deflate/LJPEG compression bomb и OpcodeList fixtures;
- 10 000 generation races, cancellation в каждой checkpoint, duplicate request
  coalescing и fence-aware eviction;
- concurrent cache reader/writer, poisoned path/cache entry, device loss;
- orientation/crop/CFA/color ΔE против CPU/LibRaw/RawSpeed oracle.

## 8. Benchmark gates

Benchmarks должны быть hardware-labelled, одинаковый fixture, без embedded JPEG.
Состояния: cold/warm DB, OS page cache (если измерим), RAM tile cache, GPU
residency; минимум 9 repetitions с p50/p95/p99.

Обязательные события:

```text
T_scan_stat             folder -> first enumerated row
T_metadata_first        folder -> first validated ProbeResult
T_thumb_first_raw       select -> first RAW-derived thumbnail present
T_first_raw_present     select -> first visible RAW pixel present
T_visible_complete      select -> all visible tiles ready
T_adjacent              next/prev -> first visible RAW present
T_steady                p95 over 300 pan/zoom/scroll events
T_catalog_update        filesystem event -> stable model delta
```

Цели для фиксированной NVMe-машины:

| Gate | Target |
|---|---:|
| first catalog row, 1k entries | ≤100 ms warm / ≤250 ms cold |
| metadata probe | p95 ≤20 ms/file |
| first RAW thumbnail | p95 ≤300 ms cold / ≤100 ms warm |
| adjacent warm navigation | p95 ≤100–150 ms |
| steady interaction | p95 ≤16.7 ms, dropped <1% |
| stale publishes | 0 / 10k transitions |
| DNG 8-worker efficiency | `η ≥ 0.5` until saturation |
| memory | never exceed declared RAM/VRAM caps |

JSONL row обязан содержать root/file identity digest (без утечки полного path),
catalog/view generation, cache state, queue wait/run, I/O bytes, decode
serial/parallel time, RSS, logical VRAM/staging, hit/eviction/duplicate/stale
counts и backend. Cache-hit latency нельзя выдавать за RAW decode speed.

## 9. План реализации

### P0 — безопасная usable gallery

1. `rrrah-gallery` model, CLI `DIR|RAW`, folder picker и dropped-file path;
2. bounded non-recursive scanner + metadata-only `RawProbe` + incremental model;
3. virtualized grid/filmstrip и selection by `AssetId`;
4. `view_generation` gate, cancellation, RAII byte reservations;
5. RAW bilinear thumbnail tier, no JPEG substitution, visible progress/errors;
6. no-follow/open-handle identity и adversarial scanner/decode tests.

### P0.1 — fast navigation

1. in-flight coalescing и directional ±K preload;
2. RAM 2Q/TinyLFU, tiered thumbnail/mosaic cache и GC/quota;
3. GPU tile residency, pin/fence protection, staging ring;
4. DNG `TilePlan`/independent decode; CR2 sequential lane + row ring;
5. live telemetry и benchmark harness с first-visible/adjacent gates.

### P1/P2 — product workflow

Сортировка/фильтры/rating/reject, filesystem watcher, recent folders, SQLite/redb
catalog, XMP/sidecars, compare mode, export и high-quality demosaic. Эти функции
не должны попадать в P0 до generation/security/cache gates.

## 10. Критический вердикт

Главная ошибка была бы сделать «галерею» как цикл из `spawn` для каждого файла.
Правильный объект масштабирования — видимые RAW-derived tiles и bounded
reservations, а не количество потоков. До folder release обязательны generation
gate, safe-handle/content identity, bounded traversal и sandboxed decoder; после
них наибольший выигрыш дадут metadata-only probe, DNG tile parallelism и
directional preload.
