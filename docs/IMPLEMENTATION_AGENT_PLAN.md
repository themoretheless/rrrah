# План доведения P0/P1: рабочие пакеты агентов и критические проверки

Документ превращает аудит в исполнимую программу. Три пакета разрабатываются
параллельно, но интегрируются только через описанные порты. Каждый пакет обязан
содержать adversarial review: что может быть неверно, какой fixture это ловит и
какое утверждение пока нельзя делать.

## Правило владения файлами

| Пакет | Основные владельцы | Не изменяет без согласования |
|---|---|---|
| A — ingest/tiles | `rrrah-decode`, `rrrah-core` RAW types, decode/cache docs | `rrrah-gpu`, UI scheduler |
| B — scheduler/residency | `rrrah-app`, `rrrah-cache`, `rrrah-gpu` resource policy | decoder semantics/color formulas |
| C — quality/critic | `rrrah-core` math/oracles, GPU shader tests, benchmark docs | decoder ownership/scheduler plumbing |

Если изменение пересекает две колонки, сначала добавляется trait/contract и тест,
а реализация откладывается до интеграционного прохода. Никаких широких
рефакторингов ради “красивого” интерфейса.

## Граф зависимостей

```text
                   ┌──────────────────────┐
                   │ A: Probe + TilePlan  │
                   │ RAW layout/quotas    │
                   └──────────┬───────────┘
                              │ TileRequest/TileResult
                              ▼
┌─────────────────────┐  ┌──────────────────────┐  ┌────────────────────────┐
│ C: scalar quality   │─▶│ B: priority scheduler│─▶│ GPU residency/staging  │
│ oracle + corpus     │  │ generation/backpress │  │ present/fallback       │
└─────────────────────┘  └──────────┬───────────┘  └───────────┬────────────┘
                                    │ telemetry                 │ golden render
                                    └──────────────┬──────────────┘
                                                   ▼
                                     integration benchmark gates
```

Сначала стабилизируется `TileRequest` и bounded memory accounting, затем
подключается GPU residency. Quality oracle независим от GPU и определяет
корректность результата; GPU не может сам быть эталоном.

## Общие контракты

### Immutable source и generation

```rust
struct SourceId { fingerprint: [u8; 32], frame: u32 }
struct Generation(u64);

struct TileRequest {
    source: SourceId,
    generation: Generation,
    tile: TileCoord,
    mip: u8,
    halo: u8,
    priority: Priority,
}
```

`SourceId` не меняется при exposure/WB/edit changes. `Generation` меняется при
выборе другого кадра или viewport transaction. Результат с устаревшим
generation можно освободить, но нельзя публиковать, кэшировать как текущий или
загружать в GPU.

### Byte credits

До чтения резервируются `compressed + decoded + halo + scratch + staging`
bytes. Резервирование атомарно; ошибка/cancel обязаны вернуть credit в `Drop`
guard. Оценка worker count:

\[
N=\min(N_{cpu},\lfloor B_{ram}/B_{working}\rfloor,
       N_{io},N_{staging}) .
\]

Запрещён неограниченный `spawn_blocking`; очередь должна иметь capacity и
метрику отказа admission.

### Stage keys

```text
H(source fingerprint, frame, tile, mip, edit subgraph,
  decoder ABI, shader ABI, semantic version)
```

Изменение edit graph инвалидирует только зависимые downstream stages. Cache
формат не считается совместимым только по имени файла или размеру.

## Пакет A — ingest / DNG tiles / CR2

**Цель:** первый видимый RAW-derived tile без materialize всего кадра, с
безопасными offsets/counts и честной моделью CR2/DNG параллелизма.

Обязательные результаты:

1. `ProbeResult`: dimensions, CFA, bit depth, active area, black/white levels,
   offsets/counts, orientation, opcodes и capability/degraded flags без полного
   mosaic allocation.
2. `TilePlan`: independent DNG tile/strip ranges, compression kind, required
   halo, CFA origin, memory cost и retry/error state.
3. CR2 plan: один entropy lane; row ring; postprocess fan-out. Parallel split
   разрешён только после доказанных restart markers.
4. Checked arithmetic: `offset + length`, pixel count, IFD depth/count, tile
   dimensions, compressed/decompressed quotas.
5. Generation-aware decode checkpoints между дорогими блоками.
6. Corpus/fuzz cases: truncation, cyclic IFD, absurd tile count, wrong byte
   count, all CR2 slice/restart variants, DNG float/OpcodeList.

**Критерий готовности:** `ProbeResult` не создаёт full-frame buffer; DNG visible
tile can reach `CPUReady` independently; CR2 remains bit-exact; malformed input
returns bounded deterministic error; cancellation does not leak a credit.

**Запреты:** не обещать “20× CR2 speedup”; не объявлять OpcodeList supported,
пока каждый opcode не имеет oracle; не использовать embedded JPEG как fallback
для RAW-derived frame.

## Пакет B — scheduler / cache / GPU residency

**Цель:** visible-first pipeline с bounded RAM/VRAM и нулевым stale publish.

Обязательные результаты:

1. Priority queue: visible > halo > next/previous > thumbnail > idle.
2. Generation gate на enqueue, decode completion, cache admission, upload и
   present.
3. Weighted RAM policy (2Q/TinyLFU admission) и GPU byte-LRU с pin/fence state.
4. Persistent staging ring с aligned rows; eviction only after submission fence.
5. Adapter capability probe, shader/pipeline cache, CPU fallback и device-loss
   recovery; no blank window on texture-limit failure.
6. Live telemetry: queue depth, admission wait, cache hit, bytes, stale drops,
   upload, first-present, dropped frames, backend.
7. Backpressure tests with tiny budgets (one tile, one staging slot) and slow
   I/O; no deadlock, unbounded queue or UI starvation.

**Критерий готовности:** при смене кадра старые completion не видны; текущий
viewport получает first present при бюджете меньше full-frame; steady p95 and
RSS/VRAM stay within configured budgets; simulated device loss falls back.

**Запреты:** не считать eager atlas production residency; не освобождать GPU
resource до fence; не делать CPU readback в zoom/pan path; не смешивать
background export с interactive queue.

## Пакет C — quality / benchmark / adversarial critic

**Цель:** сделать скорость измеримой только на корректном изображении.

Обязательные результаты:

1. Scalar bilinear и MHC golden implementations; SIMD/WGSL differential tests,
   four Bayer phases, seam/halo tests, finite/NaN guards.
2. DNG black/white/linearization grids, camera matrices, DCP/ICC/OCIO and
   OpcodeList acceptance/degraded tests.
3. Demosaic tiers: fast bilinear, balanced MHC, quality RCD/AMaZE-like;
   optional learned path is explicitly non-default.
4. Physical noise model `Var(y|x)=ax+b`; calibrated fixtures and detail/
   hallucination checks.
5. GPU headless/reference corpus with linear diff, PSNR/SSIM, CIEDE2000,
   neutral drift, MTF and seam ratio.
6. Benchmark matrix separating `T_meta`, `T_first_raw`, `T_first_present`,
   `T_visible_complete`, `T_steady`, `T_export`; p50/p95/p99/RSS/VRAM and
   cache state are mandatory.

**Критерий готовности:** tile-vs-monolithic differs by at most the declared
LSB tolerance; quality tier is never compared with the wrong oracle; unsupported
color operations are visible as degraded; no “fast” result is reported for a
skipped fixture.

## Критический проход перед merge

Критик проверяет не стиль, а следующие failure modes:

| Вопрос | Доказательство, которое требуется |
|---|---|
| Можно ли разбить CR2 entropy? | restart-marker/format proof или отказ от split |
| Можно ли показать tile без full frame? | allocation trace и independent DNG fixture |
| Может ли stale result попасть на экран? | forced generation race, stale counter = 0 |
| Укладываемся ли в RAM/VRAM? | tiny-budget test, peak bytes, fence-aware eviction |
| Не разъехалась ли CFA phase на seam? | all four Bayer phases + boundary oracle |
| GPU совпадает с CPU? | linear numeric diff, не screenshot |
| Что происходит при driver/device loss? | simulated failure и CPU fallback |
| Реальна ли заявленная скорость? | same fixture/hardware, p95/p99, no JPEG shortcut |
| Не сломали ли цвет DNG? | black-grid/Opcode/DCP corpus + degraded status |
| Можно ли остановить работу? | bounded queue, cancellation latency, credit recovery |

## Порядок интеграции

1. Сначала принять контракты `ProbeResult`, `TilePlan`, `TileRequest`,
   `Generation`, `ByteCredits` и synthetic tests.
2. Интегрировать A в inspect/metadata path без изменения GPU.
3. Интегрировать B с synthetic `TileSource`, затем подключить реальный A.
4. Интегрировать C scalar oracle и только потом менять WGSL/quality kernels.
5. Прогнать adversarial critic на искусственно маленьком бюджете и forced
   races.
6. Только после gates подключать реальный corpus и сравнение с OSS-продуктами.

## Definition of done для P0.1

```text
[ ] metadata-only open does not allocate full mosaic
[ ] DNG visible tile independent; CR2 serial dependency documented/enforced
[ ] bounded priority queues and byte credits
[ ] generation stale-publish counter = 0 in stress test
[ ] GPU residency/staging works below full-frame budget
[ ] CPU fallback/device-loss path is observable
[ ] scalar-vs-GPU tile output within declared tolerance
[ ] live telemetry contains first-present/cache/RSS/VRAM
[ ] cargo test/clippy/fuzz smoke green
[ ] no claim of industry speed until same-fixture comparator run
```

P0.1 не включает AI, panorama, face detection или полный export. Эти функции
подключаются после того, как базовый RAW path bounded, deterministic и
photographically correct.
