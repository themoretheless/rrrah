# Сводка экспериментов: перформанс-гипотезы и уточнение цветов

Дата: 2026-07-24. Машина: Apple M5, macOS, Metal (headless), release-профиль.
Исполнение: оркестратор + параллельные субагенты (задачи A–H). Отчёты по каждой
задаче — в этом же каталоге (`A.md` … `H.md`, `d.md`), сырые данные — в CSV и
`sweep-data/`.

Чистый A/B `9497871` против объединённого экспериментального дерева:
[`BASELINE_VS_CURRENT.md`](BASELINE_VS_CURRENT.md). Решение по прямому WB:
[`DIRECT_WB_GPU_READBACK.md`](DIRECT_WB_GPU_READBACK.md).

Методологическая оговорка: часть прогонов выполнялась под фоновой нагрузкой
параллельных сборок. Для вердиктов использовались interleaved A/B замеры в
одном процессе (A), min-of-reps (C) или повторные прогоны (B, G) — см.
отдельные отчёты.

## Производительность

| # | Гипотеза | Вердикт | Ключевые числа |
|---|----------|---------|----------------|
| A1 | u64 word-refill в DNG lossless-JPEG даст +10–20% | **Подтверждена** | speedup 1.06–1.17× (interleaved A/B, 30 раундов × 4 реплики); SWAR fast-path с корректной обработкой 0xFF00 stuffing |
| A2 | word-refill в packed-unpack даст +20–40% | **Подтверждена и превышена** | 10-bit: 1.27–1.40×; 12-bit: 1.4–1.9×; 14-bit: 1.5–1.75× |
| B | Runtime-knob plane workers (1/2/4) позволит измерить ветки batch/streaming | **Подтверждена** | `RRRAH_CR3_PLANE_WORKERS`: 1→309 мс, 2→168 мс, 4 (streaming)→102 мс plane_wall; batch масштабируется почти линейно; bit-identical при любом knob |
| C1 | Фиксированная стоимость GPU upload доминирует над per-byte | **Опровергнута** | константные фазы < 0.5 мс при любом кадре; доминируют `halo_pack` (~60%) и `write_enqueue` (~25%), масштабируются с байтами |
| C2 | tile_size=4094 выбран эвристически | **Подтверждена; закрыта адаптивно** | aligned 1022 сохраняется для многослойного atlas; один frame-sized tile выбирается, когда уменьшает atlas минимум на 12.5%. Контрбалансированный rerun устранил регрессию 4096×3072 и сохранил выигрыш на 6240×4160 |
| E | `scan_folder` на 1k/10k файлов превысит гейт 100 мс до первого кадра | **Опровергнута** (warm) | 1k: p50 3.6–5.4 мс; 10k: p50 41.8–53.2 мс, p95 до 93.2 мс. Cold-гейт 250 мс не измерен (нет root для сброса page cache). Но синхронный вызов на 10k нарушает бюджет кадра 16.7 мс — кандидат на фоновый сканер |
| G | Safe-Rust оптимизация CR3 entropy decode (Rice+MED) даст ещё заметный выигрыш | **Подтверждена частично** | early-refill (порог 16 бит) + `#[inline(always)]` + byte-aligned append refill: plane_wall 91.8–92.9 → 73.7–82.6 мс (≈ −15–20%), регрессия bit-identical. Прерванные направления (u128 reservoir, multi-symbol-per-refill) не завершены |
| H | GPU-декодинг CR3 entropy окупится для single-image latency | **Опровергнута аналитически** | двухпроходная схема scan→parallel-decode конструктивно невозможна: длина кода зависит от значений всех предыдущих сэмплов (адаптивный k, run-режим по реконструированным пикселям). Один тайл, слайсов/restart-маркеров нет. Оценка 4-workgroup схемы: 170–285 мс против 102 мс CPU → 0.35–0.6×, регрессия. См. `H.md` |

### Выводы по производительности

1. CPU-путь ещё не исчерпан: задачи A и G дали суммарно заметный выигрыш без
   нарушения `deny(unsafe_code)`. SIMD через intrinsics требует смены политики
   unsafe — решение владельца; MED-предиктор последователен по строке, потолок
   автовекторизации низок (см. `H.md`, раздел про SIMD).
2. GPU entropy decode — тупик для одиночного CR3 (H); разумная GPU-смежная
   оптимизация — decode прямо в mapped buffer.
3. Следующие кандидаты по приоритету (из разведки): ~~параллельный
   декодинг DNG-сегментов~~ (выполнен 2026-07-24, см. «Выполненные шаги»
   ниже); ~~tile_size≈1024 как дефолт~~ (выполнен); batch staging ring
   для upload; фоновый сканер папки с инкрементальной выдачей.

## Цвет

| # | Гипотеза | Вердикт | Ключевые числа |
|---|----------|---------|----------------|
| D1 | f32-инверсия камерных матриц нестабильна | **Подтверждена** | f32 round-trip > 1e-5 уже при |det| ≈ 5e-4; при eps=1e-8 — 0.56. f64 ≤ 1.1e-13 на всём семействе. Переведено на f64-ядро с f32-даункастом только в uniform |
| D2 | WB без luminance-нормализации меняет Rec.709-взвешенный масштаб camera-space gains | **Гипотеза измерена, policy пересмотрена** | Rec.709 weights неприменимы до camera matrix. Нормализация давала скрытые −0.239 EV на EOS R8 (`141→129` в readback); renderer снова сохраняет backend gains бит-в-бит, exposure остаётся отдельным |
| D3 | Молчаливая редукция G1/G2 маскирует ошибку | **Закрыта** | `diagnose_green_planes` + явный отказ при расхождении > 1e-3 |

### Открытые цветовые вопросы (приоритет из разведки)

1. ~~Сквозная кривая ACES→sRGB-surface не верифицирована пиксельно~~ —
   **закрыт** readback-харнессом (2026-07-24, см. ниже).
2. ~~Bradford-адаптация не подключена~~ — **закрыт** (2026-07-24):
   выбор DNG-матрицы по CalibrationIlluminant + Bradford к D65.
3. Матрица EOS R8 — эмпирический хардкод, требует сверки с внешним
   референсом (Adobe/LibRaw).
4. Per-channel ACES + клэмп отрицательных → сдвиг оттенков насыщенных
   цветов; контракт требует hue-preserving tone/gamut mapping.
   Разблокирован readback-харнессом.
5. Zoom-out алиасит (4-точечный box по мозаике) → цветной муар в fit-виде.

## Выполненные шаги (2026-07-24, вечер)

Четыре приоритетных шага из этой сводки реализованы; все тесты workspace
зелёные (rrrah-decode 144+3ign, rrrah-core 39+3, rrrah-gpu 36+6,
rrrah-app 43, rrrah-cache 54+5, rrrah-bench 2). Только safe Rust.

1. **Адаптивный GPU tiling** (rrrah-gpu). Базовый interior tile — 1022 при
   halo 1 → extent ровно 1024, кратный 128 сэмплам (питч кратен
   `COPY_BYTES_PER_ROW_ALIGNMENT`, `row_pack` обнуляется). Если один
   frame-sized tile помещается и уменьшает atlas минимум на 12.5%, planner
   выбирает его: это устраняет подтверждённую регрессию 4096×3072. Сверка с
   `max_texture_array_layers`: при нехватке слоёв дефолтный путь удваивает
   extent с сохранением кратности; на пределе `max_dimension` — прежняя
   ошибка `TooManyTiles`. Оверрайды (`TilingOverrides`,
   `RRRAH_GPU_TILE_SIZE`) применяются дословно.
2. **Bradford-адаптация DNG ColorMatrix1** (rrrah-core + rrrah-decode).
   Парсятся теги ColorMatrix2 + CalibrationIlluminant1/2. Порядок выбора
   в `select_dng_xyz_to_camera`: (1) матрица с иллюминантом D65 — дословно;
   (2) известный не-D65 иллюминант — Bradford-адаптация к D65 (в кодовой
   базе матрицы хранятся как XYZ→camera, поэтому `CM_D65 = CM_A ·
   Bradford(D65→A)`; проверено тестом: нейтральная под A сцена → D65 white
   с точностью 1e-9); (3) без иллюминантов — legacy CM1. Таблица
   иллюминантов: CIE A/B/C/D50/D55/D65/D75 + EXIF-алиасы; флуоресцентные
   коды → legacy-фолбэк. Математика в f64, f32-даункаст на границе.
3. **Readback-харнесс верификации цвета** (rrrah-gpu/tests/common +
   tests/readback.rs + examples/gpu_readback.rs). Headless wgpu (Metal),
   рендер в offscreen `Rgba8UnormSrgb`, readback через
   copy_texture_to_buffer + map_async; без GPU тесты скипаются. 6
   проверок: детерминизм (bit-identical), нейтральность/равномерность,
   монотонность tone curve, границы (белый → 232, ACES roll-off,
   задокументировано), 6 известных точек против CPU-референса кривой
   (на M5 совпадение точное, delta 0; допуск ±2 байта), экспозиция +1
   стоп, а также direct-WB через реальную EOS R8 camera matrix.
4. **Параллельный декодинг DNG-сегментов** (rrrah-decode). Единица —
   strip или tile-row band; scoped threads, work-stealing через
   AtomicUsize, запись в непересекающиеся регионы выходного буфера →
   bit-identical при любом числе воркеров (тесты workers 1/2/3/4/7/64,
   tiled/stripped × lossless/uncompressed). Knob `RRRAH_DNG_DECODE_WORKERS`
   (дефолт `available_parallelism`, мелкие файлы не распараллеливаются).
   Замеры на синтетическом tiled DNG 6000×4000 12-bit (384 тайла):
   criterion 1→8 workers 1154→381 мс (3.03×), probe segment-decode
   823→196 мс (4.19×); машина была под фоновой нагрузкой — числа
   консервативные. Ограничения: uncompressed memory-bound; крупные тайлы
   дают мало полос; односегментные DNG не ускоряются.

## Состояние дерева

- Основные performance follow-ups находятся в истории main (`60766c2`,
  `8ba4443`). Adaptive tiling, direct-WB decision и новые regression gates
  пока находятся в рабочем дереве.
- Новые инструменты: criterion-бенчи DNG (`benches/dng_decoders.rs`),
  interleaved DNG A/B (`examples/decode_timing.rs`), CR3 cross-worktree timing
  (`examples/cr3_end_to_end_timing.rs`), GPU sweep/AB
  (`RRRAH_GPU_SWEEP=1`, `RRRAH_GPU_UPLOAD_AB=1`), `folder_scan` и worker knobs.
- Фикстуры tests/IMG_9043.CR3, tests/IMG_9074.CR3 добавлены владельцем;
  оракулы регрессии по-прежнему вне репозитория (`RRRAH_CR3_REGRESSION_DIR`).
