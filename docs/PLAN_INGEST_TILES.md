# P0 ingest: metadata-only probe, DNG tiles и CR2 streaming

Статус: design package A, 2026-07-21. Документ задаёт контракт ingest-слоя и
план реализации. Он не объявляет, что текущий `rawler 0.7.2` уже поддерживает
viewport-first decode: его публичный `Decoder::raw_image` материализует
`RawImage`, а `RawSource::new` создаёт `mmap` с `populate`. До реализации
отдельного bounded probe/reader это остаётся full-file/full-frame путём.

## 1. Граница ответственности

Ingest отвечает только за безопасное превращение недоверенного файла в
планируемые sensor samples:

```text
file bytes -> Probe -> DecodePlan -> bounded read/decode -> RawTile/RawBand
```

Он не делает demosaic, цвет, tone map, GPU upload или каталог. Это важно для
SOLID: изменение DNG OpcodeList или CR2 entropy reader не должно менять
рендерер. `rrrah-core` получает только проверенные геометрию и samples; cache
видит immutable tile/band и ключ семантики декодера.

Зоны работ для команды:

| Пакет | Владелец | Выход | Критик обязан проверить |
|---|---|---|---|
| Probe/IFD | ingest | `RawProbe` без pixel allocation | offset/count/IFD cycles и full-file mmap |
| DNG planner | tile | `DngPlan` и `TileSpan` | независимость, endian, strips/tiles/planar |
| CR2 lane | sequential | row-band stream | запрещённое деление entropy stream |
| Budget/scheduler | systems | permits + generation gate | bounded RSS и stale publish |
| Decoder adapter | compatibility | rawler fallback | private API и panic/OOM assumptions |
| Fuzz/oracle | QA | corpus + invariants | corrupt/truncated/hostile input |

## 2. Публичные контракты Rust

Имена ниже являются целевым API; сначала они могут жить в
`rrrah-decode::ingest`, затем выделяются в более узкий crate. Traits не должны
возвращать `Rawler`-типы: иначе нельзя заменить decoder или вынести его в
процесс.

```rust
#[derive(Debug, Clone)]
pub struct ProbeRequest {
    pub path: PathBuf,
    pub image_index: u16,
    pub limits: DecodeLimits,
}

#[derive(Debug, Clone)]
pub struct RawProbe {
    pub source: SourceId,          // file identity, not a preview digest
    pub format: RawFormat,         // Cr2, Dng, TiffRaw, Unknown
    pub width: u32,
    pub height: u32,
    pub cpp: u8,
    pub bits_per_sample: u8,
    pub sample_format: SampleFormat,
    pub photometric: Photometric,
    pub endian: Endian,
    pub orientation: Orientation,
    pub active_area: Option<Rect>,
    pub storage: StoragePlan,
    pub metadata: RawMetadataLite,
    pub metadata_digest: [u8; 32],
}

pub trait RawProbe: Send + Sync {
    fn probe(&self, request: &ProbeRequest) -> Result<RawProbe, IngestError>;
}

pub trait TileDecoder: Send + Sync {
    fn decode_tile(
        &self,
        source: &SourceHandle,
        plan: &TilePlan,
        tile: TileId,
        output: &mut [u16],
        cancel: &GenerationToken,
    ) -> Result<TileOutput, IngestError>;
}

pub trait BandDecoder: Send + Sync {
    fn next_band(&mut self, output: &mut [u16], cancel: &GenerationToken)
        -> Result<Option<RowBand>, IngestError>;
}
```

`RawProbe` обязан завершаться без allocation, зависящей от `width * height`.
Metadata may contain bounded vectors (for example at most 4096 IFD entries,
4096 linearization values, and a bounded opcode byte payload). `SourceHandle`
не передаёт произвольный `&[u8]` decoder-у: чтение идёт через checked
`read_exact_at(offset, len)` или immutable mapped window после проверки.

`DecodePlan` следует разделить на storage и operation semantics:

```rust
#[derive(Debug, Clone)]
pub enum StoragePlan {
    Cr2(Cr2Plan),
    Dng(DngPlan),
    SingleStream(SingleStreamPlan),
}

#[derive(Debug, Clone)]
pub struct TileSpan {
    pub id: TileId,
    pub plane: u16,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub file_offset: u64,
    pub compressed_len: u64,
    pub decoded_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct DngPlan {
    pub image_size: (u32, u32),
    pub tile_size: (u32, u32),
    pub planes: u16,
    pub spans: Arc<[TileSpan]>,
    pub compression: Compression,
    pub predictor: Option<u16>,
    pub linearization: Option<LinearizationPlan>,
    pub opcodes: OpcodeDigest,
}
```

`TileId` — newtype over `u32`, not a naked index. For planar data the plane is
part of the identity; for chunky data `plane == 0`. `TileOutput` содержит
`tile`, `valid_rect`, `samples: Arc<[u16]>`, `checksum` and the exact
`metadata_digest`; a tile нельзя опубликовать под другой probe.

## 3. Metadata-only probe

### 3.1 Почему не вызывать rawler для probe

В `rawler 0.7.2`:

* `RawSource::new` открывает файл, `mmap`-ит его и вызывает `populate`/`WillNeed`;
* `Decoder::raw_metadata` остаётся привязан к `RawSource`;
* `Decoder::ifd` возвращает `Rc<IFD>`, но не предоставляет bounded random-read
  abstraction;
* `raw_image` вызывает private `plain_image_from_ifd`, выделяющий полную
  destination mosaic. Внутри `decode_tiles` Rayon распараллеливает tile decode,
  но затем всё равно возвращается полный `RawImage`.

Следовательно, P0 probe открывает `File` отдельно и читает только TIFF/CR2
заголовок, IFD и необходимые tag payloads. `rawler` остаётся compatibility
fallback после quota admission. Если upstream даст public `probe`/tile API,
адаптер можно заменить без изменения `RawProbe`.

### 3.2 Ограниченный TIFF/DNG probe

Алгоритм:

1. Прочитать первые 8 байт, проверить `II/MM`, magic 42/43 и первый IFD offset.
2. Для каждого IFD держать `visited: HashSet<u64>` и `depth`; reject повтор,
   depth > 16, entry count > 4096.
3. Прочитать только entry table (12/20 байт на entry в зависимости BigTIFF),
   inline values — из entry, большие — отдельным checked read.
4. Проверить type/count product (`element_size * count`) до allocation и
   `offset + byte_len <= file_size`.
5. Найти raw IFD по `NewSubfileType`, `PhotometricInterpretation=CFA/LinearRaw`,
   dimensions и storage tags. Preview/thumbnail IFD не становится raw target.
6. Сохранить минимальный набор metadata; большие maker notes/XMP считать
   opaque bounded blob и не декодировать в P0.
7. Для `TileOffsets/TileByteCounts` построить `DngPlan`; для strips —
   `SingleStreamPlan`/strip spans с отдельными limits.

TIFF tag arrays могут быть `SHORT`, `LONG`, `LONG8`, `IFD8`, а DNG producer может
поставить malformed type. P0 не должен делать `force_usize`-подобный silent
default: wrong type → explicit `UnsupportedTagType` или degraded metadata state.

### 3.3 CR2 probe

CR2 — TIFF-подобный container плюс Canon MakerNote и lossless-JPEG stream. Probe
извлекает dimensions, bits, CFA, black/white metadata, JPEG SOF/DHT/SOS and
Canon slice descriptors, но не считает slice независимым compressed stream.
Если JPEG markers или slice table не подтверждают безопасные row boundaries,
план помечается `SequentialOnly`.

`SourceId` должен включать file size, mtime/file-id и BLAKE3 sample digest для
быстрого cache lookup. Полный digest строится один раз после успешного decode,
а не блокирует metadata-ready.

## 4. DNG TilePlan: checked arithmetic и независимые задачи

Для dimensions `w`, `h`, tile `tw`, `th`:

```text
cols = checked_ceil_div(w, tw)
rows = checked_ceil_div(h, th)
expected_spans = checked_mul(cols, rows, planes)
```

`tw == 0`, `th == 0`, `w == 0`, `h == 0`, overflow или span count mismatch —
ошибка. Для edge tile:

```text
valid_w = min(tw, w - tile_x * tw)
valid_h = min(th, h - tile_y * th)
padded_w = tw; padded_h = th  // only if decoder requires it
```

До чтения проверять:

```text
file_offset <= file_size
compressed_len <= max_compressed_tile
file_offset.checked_add(compressed_len) <= file_size
decoded_bytes = ceil(bits * cpp * tw * th / 8)
decoded_bytes <= max_decoded_tile
```

Для chunky layout `id = (plane * rows + tile_y) * cols + tile_x`; для planar
тот же порядок, но samples каждого plane публикуются отдельно. Нельзя
предположить, что offsets physically contiguous или monotonically increasing;
использовать `read_at`. Adjacent spans можно coalesce только после сохранения
исходных subranges и при ограничении coalesced bytes.

Поддерживаемые P0 storage contracts:

| Storage | P0 | Параллелизм |
|---|---|---|
| Uncompressed tiles | да | независимый `read_at` + unpack |
| LJPEG/JPEG tiles | после per-tile decoder validation | decoder state per task |
| Deflate tiles | после predictor reset test | tile-local inflate |
| JPEG-XL tiles | feature-gated | one context per task + quota |
| Multistrip | ограниченно | независимый strip только при reset |
| Single strip | нет viewport-first | sequential lane |

`OpcodeList1`/linearization может менять значение до demosaic; его digest входит
в `TileOutput` и persistent key. Если opcode имеет неизвестный радиус/читает за
пределами tile, planner должен вернуть `NeedsFullFrame` либо
`UnsupportedDngOpcode`, а не тихо пропустить операцию. P0 разрешает
`LinearizationTable` только с bounded lookup; dither seed —
`(source_id, tile_id, row)` для детерминизма при разных порядках workers.

## 5. CR2 sequential lane и row ring

Типичный CR2 lossless-JPEG — один entropy stream. Huffman/DC predictor и restart
semantics могут связывать соседние MCU; Canon slice descriptors описывают
раскладку результата, но не доказывают независимость bitstream. Поэтому:

```text
read-ahead -> one entropy decoder -> checked row bands -> fan-out
                                      ├─ RAM tile conversion
                                      ├─ GPU staging
                                      └─ persistent cache writer
```

`BandDecoder` держит decoder state только в ingest lane. После каждой полностью
декодированной band (например, 32–128 sensor rows) проверяются cancellation,
checked output range и finite counters, затем band перемещается в bounded
`RowRing`. Ring capacity = `min(lookahead_rows, max_inflight_rows)`; producer
блокируется на byte permit, а consumer освобождает permit только после copy/cache
commit.

Без отдельного upstream/API нельзя обещать row-band visibility из rawler: его
`raw_image` возвращает только полный `RawImage`. Для CR2 P0 допускается два
режима: (a) rawler full-frame compatibility; (b) собственный/IPC LJPEG lane,
включённый только после golden-oracle сравнения с dcraw/LibRaw/RawSpeed.

Параллелить разрешено Huffman table setup, read-ahead, black/linearization и
copy уже завершённых bands. Нельзя делить один compressed stream на worker-per-
row или worker-per-slice без доказанной restart boundary. Ожидаемый gain —
overlap/SIMD, а не линейный speedup от количества CPU workers.

## 6. Scheduler, quotas и state machine

Каждый open получает `GenerationId`. Состояния:

```text
Idle
  -> Opening(file handle)
  -> Probing(metadata only)
  -> Planned(Dng tiles | CR2 sequential | fallback full)
  -> CacheLookup
  -> DecodingVisible
  -> FirstRawPresent
  -> Refining
  -> Ready | Failed | Cancelled
```

Переходы `publish`, `cache insert`, `GPU upload` принимают generation. Если
`token.is_cancelled()` или generation mismatch — completion может быть discarded,
но stale frame никогда не публикуется. Cancellation checkpoints: после probe,
каждого `read_at`, после entropy MCU/row-band/tile, перед cache commit и перед
GPU submission.

Целевой budget API:

```rust
#[derive(Debug, Clone, Copy)]
pub struct DecodeLimits {
    pub max_file_bytes: u64,
    pub max_metadata_bytes: u64,
    pub max_ifd_entries: u32,
    pub max_ifd_depth: u8,
    pub max_pixels: u64,
    pub max_planes: u16,
    pub max_compressed_tile_bytes: u64,
    pub max_decoded_tile_bytes: u64,
    pub max_inflight_compressed: u64,
    pub max_inflight_decoded: u64,
    pub max_wall_time: Duration,
}
```

Scheduler не создаёт unbounded `spawn_blocking` и не накладывает Rayon поверх
Rayon rawler. Каждая task сначала резервирует `compressed_len + decoded_bytes +
staging_bytes`; при отказе permit задача не читает файл. Workers = min(ready
tasks, physical cores minus UI reserve, memory-bandwidth estimate), но это
параметр benchmark, не correctness invariant. I/O queue depth ограничен
отдельным semaphore.

## 7. Cache identity и публикация

Raw tile cache key:

```text
source identity/full digest (when available)
image index + tile id + valid rect
storage/compression/predictor/endian
linearization + opcode digest
decoder ABI + ingest semantic version
```

Exposure/WB/tone-map не инвалидируют decoded sensor tile; изменение
linearization/opcode или endian semantics — инвалидирует. RAM cache byte-weighted
с pin видимых tiles. Persistent cache — один immutable blob с TOC и per-tile
BLAKE3; temporary file + `sync_data` + atomic rename. Corrupt tile удаляется
точечно и не превращает весь каталог в trusted input.

Нельзя использовать sampled fingerprint как криптографическую гарантию
тождественности: он только быстрый hint. Для export/sidecar и cache commit нужен
full digest или file-id/size/mtime policy, зафиксированная в key version.

## 8. Fuzz, security и oracle tests

До production ingest должен пройти:

* TIFF/BigTIFF: endian swap, malformed type/count, offset overflow, loops,
  deeply nested SubIFD, huge arrays and trailing bytes;
* DNG: tiles/strips, edge padding, planar/chunky, uncompressed/LJPEG/Deflate/JXL,
  wrong TileByteCounts, duplicate offsets, predictor reset and linearization;
* CR2: truncation at every JPEG marker, bad DHT/SOS, restart markers, slice count,
  dimensions mismatch and random payload;
* concurrency: cancellation at every checkpoint, generation reorder, cache
  checksum corruption and permit starvation.

Fuzz invariant: no panic, OOM-triggered allocation, unsafe behavior or unbounded
wall time; result is either deterministic validated output or typed error.
`catch_unwind` вокруг rawler ловит Rust panic, но не OOM/abort и не заменяет
process boundary. Untrusted parser/decoder следует вынести в read-only worker
process с IPC length limits, RSS/CPU/wall quotas and kill-on-hang. В process не
передаётся GPU context.

Golden oracle must compare tile/row output with a full-frame reference on a
licensed corpus. At minimum: exact u16 match for uncompressed DNG; bounded error
for compressed decoder; tile-vs-monolithic max absolute error <= 1 LSB after
linearization; no seams at tile boundaries; deterministic output independent of
worker order. Без corpus нельзя заявлять camera-wide correctness.

## 9. Реализация по шагам

1. `rrrah-core`: добавить checked arithmetic helpers (`ceil_div`, byte count,
   `TileId`, `SourceId`) и unit tests for overflow.
2. `rrrah-decode`: `ProbeSource` с `FileExt::read_at`, strict TIFF/BigTIFF tags,
   no rawler `RawSource` in metadata-only path.
3. Build `DngPlan`/`TileSpan`, uncompressed tile decoder and visible tile tests.
4. Add bounded permits + generation-aware scheduler; telemetry records probe,
   first tile, decode, cache and cancellation reasons.
5. Add LJPEG/Deflate tile backends only with independent decoder states and
   golden tests; mark JXL/unknown Opcode as feature/degraded.
6. Implement CR2 row lane behind an explicit backend flag; retain rawler full
   fallback until oracle coverage exists.
7. Move hostile decoder to process boundary; add corpus fuzz CI and RSS/time
   gates.

Performance gates are hardware-labelled, not universal promises: metadata-ready
must not allocate full pixels; first visible DNG tile must be measured separately
from full decode; RSS must remain below configured limits; cancellation must
prevent stale publish; 8-worker scaling must be reported only until memory
bandwidth plateau.

## 10. Critic: атака собственного решения

1. Нельзя обещать metadata-only open через текущий public rawler API: `RawSource`
   eagerly populated mmap, а `raw_metadata` не отделён от него. Нужен свой TIFF
   probe или upstream extension.
2. Нельзя обещать DNG viewport-first для каждого файла. Single-strip DNG,
   interleaved JPEG-XL, unknown OpcodeList и некоторые maker-specific layouts
   требуют full/degraded path.
3. Нельзя обещать CR2 N× speedup. Один lossless-JPEG entropy stream остаётся
   последовательным; slice count не является доказательством независимости.
4. Нельзя считать `IFD::tile_data` безопасным planner-ом: он принимает
   `RawSource`, возвращает borrowed slices и не задаёт наши quotas/checked
   `u64` arithmetic. Это reference для semantics, не готовый public contract.
5. `catch_unwind` не защищает от OOM, abort, OS resource exhaustion или
   библиотечного UB. Production isolation/process и OS quotas всё ещё обязательны.
6. `LinearizationTable`/dither и DNG OpcodeList могут быть семантически сложнее
   tile-local operation. Пропуск opcode с красивой картинкой хуже явной
   `UnsupportedDngOpcode`; качество должно быть сверено с oracle.
7. `read_at` и parallel workers не гарантируют ускорение: NVMe queue depth,
   page cache, JPEG decoder lock, DRAM bandwidth и thermal throttling могут
   сделать 8 workers медленнее одного. Только p50/p95 на corpus имеют значение.
8. Sampled BLAKE3 не доказывает, что файл неизменён. Persistent cache key должен
   версионировать identity policy и переходить к full hash там, где correctness
   важнее latency.
9. Tile halo зависит от конкретного demosaic, defect correction и opcode. Нельзя
   зафиксировать `halo = 1` для MHC/RCD или неизвестного edge-aware kernel.
10. Точные типы выше не являются готовым patch: rawler API, TIFF BigTIFF tags,
    JXL decoder licensing и доступный test corpus должны быть проверены перед
    стабилизацией semver. До этого следует держать experimental feature flag и
    full-frame fallback.

