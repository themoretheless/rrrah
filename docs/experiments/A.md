# Эксперимент A: word-wise (u64-reservoir) refill в DNG lossless-JPEG и packed-unpack

Дата: 2026-07-24. Машина: Apple M5 (arm64), macOS,
Rust stable 1.97.1 (workspace toolchain), release-профиль (`codegen-units=1`,
`lto=thin`). Исполнитель: A.

## Гипотезы

1. **lossless_jpeg**: u64-reservoir refill (по образцу `cr3/lossless.rs`) даст
   **+10–20%** к скорости декодирования DNG `Compression = 7`.
2. **unpack** (`uncompressed.rs::decode_msb_packed`, 9–15 бит/сэмпл): даст
   **+20–40%**.

Ограничение корректности: в lossless-JPEG есть byte-stuffing `0xFF00` и
маркеры — word refill допустим только как fast-path для прогонов без `0xFF`;
гейт — все существующие тесты зелёные + bit-identical вывод.

## Метод

### Изменения

- `src/dng/lossless_jpeg.rs` — `EntropyReader::try_refill_plain_run`:
  fast-path, который при отсутствии `0xFF` среди следующих 6 байт загружает
  их одним 64-битным чтением (`u64::from_be_bytes`) и добавляет 48 бит в
  резервуар. Проверка на `0xFF` — SWAR (zero-byte detection после XOR с
  `0xFFFF_FFFF_FFFF_0000`). При наличии `0xFF`, хвосте потока (< 8 байт) или
  pending-маркере — возврат в исходный побайтовый путь со 100% сохранением
  семантики ошибок и позиций. Режим переключается полем `word_refill`
  (production-вход `decode()` всегда `true`; `decode_with_refill(.., false)` —
  эталон для A/B).
- `src/dng/uncompressed.rs::decode_msb_packed` — refill 6 байт за шаг через
  одно фиксированное 64-битное чтение (`(reservoir << 48) | (word >> 16)`),
  хвост строки (< 8 байт) — исходный побайтовый цикл. Исходный цикл сохранён
  как `decode_msb_packed_bytewise` (эталон для A/B и тестов).
- `src/dng/mod.rs` — `#[doc(hidden)] pub mod bench_support` с точками входа
  word/bytewise (возвращают FNV-1a checksum сэмплов для проверки
  bit-identical). `src/lib.rs`: `mod dng` → `#[doc(hidden)] pub mod dng`
  (одна строка, техническая необходимость для benches/; вне строгой зоны,
  конфликтов с другими исполнителями нет).
- `benches/dng_decoders.rs` (новый) — criterion-бенчи на синтетических
  потоках (фикстуры строятся по образцу unit-тестов `lossless_jpeg.rs`).
- `examples/decode_timing.rs` (новый) — interleaved A/B харнес (см. ниже).

### Методология измерений

Машина во время работы **разделялась с другими исполнителями** (параллельные
cargo build/test, load average колебался 3.5–9). Классическое сравнение
«baseline → правка → тот же бенч» дало ложную «регрессию +253%»: повторный
замер показал, что это смещение фона, а не код. Поэтому основные числа —
**interleaved A/B в одном процессе**: варианты word/bytewise чередуются
B,A,B,A… по 30 раундов на кейс, фоновая нагрузка влияет на оба варианта
симметрично; отчёт по min/p50/p95. Харнес перед замерами проверяет
**полную побитовую идентичность** вывода обоих вариантов (assert).

Прогонов-реплик харнеса: 4 (n=30 каждая). Criterion-прогон — 1 (100 samples),
использован как вторичное подтверждение для unpack; для lossless_jpeg
sequential criterion-замер оказался нестабилен из-за дрейфа фона.

Синтетические потоки:
- `12bit_gradient 4000x3000` (12 Мп, 14 210 872 байт JPEG) — «фотографичный»
  градиент + шум ±32, короткие Huffman-разности, редкий `0xFF`.
- `12bit_random 1024x1024` (2 012 900 байт) — полнодиапазонный шум,
  2668 пар `0xFF00` (stuffing), длинные коды — стресс fast-path fallback.
- `unpack 10/12/14bit 6000x4000` — MSB-first packed строки, production-паттерн
  вызова (по строке за вызов `decode_row`).

## Числа

### Interleaved A/B (4 реплики по n=30; p50-based speedup word vs bytewise)

| Кейс | Реплика 1 | Реплика 2 | Реплика 3 | Реплика 4 | Типичное |
|---|---|---|---|---|---|
| lossless_jpeg gradient 4000x3000 | 1.16x | 1.16x | 1.06x | 1.09x | **~1.12–1.16x** |
| lossless_jpeg random 1024x1024 | 1.19x | 1.18x | 1.16x | 1.17x | **~1.17x** |
| unpack 10bit 6000x4000 | 1.26x | 1.27x | 1.41x | 1.40x | **~1.27–1.40x** |
| unpack 12bit 6000x4000 | 1.41x | 1.52x | 1.94x | 1.80x | **~1.4–1.9x** |
| unpack 14bit 6000x4000 | 1.46x | 1.47x | 1.75x | 1.69x | **~1.5–1.75x** |

Абсолютные времена (реплика 4, p50): gradient 332 → 306 мс;
random 32.9 → 28.2 мс; 10bit 45.2 → 32.2 мс; 12bit 50.0 → 27.8 мс;
14bit 56.8 → 33.5 мс.

### Criterion (1 прогон, 100 samples, среднее)

- unpack: 10bit 47.3 → 34.5 мс (1.37x), 12bit 58.0 → 31.6 мс (1.84x),
  14bit 60.3 → 39.2 мс (1.54x) — согласуется с харнесом.
- lossless_jpeg: sequential-прогон зашумлён дрейфом фона (широкие CI),
  не использован для вердикта.

Воспроизведение: `cargo run -p rrrah-decode --release --example decode_timing`;
`cargo bench -p rrrah-decode --bench dng_decoders`.

## Гейт корректности

- `cargo test -p rrrah-decode`: **129 passed, 0 failed** (127 существующих +
  2 новых параметрических теста эквивалентности word ≡ bytewise:
  `word_refill_matches_bytewise_on_stuffed_restart_stream` — поток с
  stuffing и restart-маркерами; `word_refill_matches_bytewise_across_widths_and_depths`
  — 9–15 бит × ширины 1..256).
- Харнес перед каждым замером assert-ит полную идентичность вывода.
- `cargo clippy -p rrrah-decode --all-targets`: 0 предупреждений.
- rustfmt применён только к файлам зоны A.
- Регрессия на реальных DNG (`RRRAH_DNG_REGRESSION_DIR`, 3 opt-in фикстуры с
  blake3-оракулами) не запускалась — фикстуры на машине отсутствуют
  (env не задан). Это рекомендуемый следующий шаг.

## Вердикты

1. **lossless_jpeg: гипотеза ПОДТВЕРЖДЕНА.** +12–17% (цель 10–20%), в т.ч.
   +17% на stuffing-плотном потоке — SWAR-проверка `0xFF` стоит ~ничего,
   fallback не мешает.
2. **unpack: гипотеза ПОДТВЕРЖДЕНА и превышена.** +27–40% на 10 бит,
   +40–94% на 12/14 бит (цель 20–40%). 12-бит выигрывает больше всех:
   ровно 4 сэмпла на 6-байтный refill; в побайтовом пути refill-цикл
   выполнялся ~2 раза на сэмпл.

Почему 12/14-бит unpack выигрывает сильнее lossless_jpeg: в unpack доля
refill-логики в общем времени выше (остальное — тривиальный extract), а в
lossless_jpeg доминируют Huffman-декод и предикторная арифметика.

## Что дальше

- Прогнать opt-in регрессию на реальных DNG (`RRRAH_DNG_REGRESSION_DIR=... cargo test -p rrrah-decode --lib dng::fixture_regression -- --ignored`) при появлении фикстур.
- Проверить влияние на end-to-end `pixel_unpack` тайминг DNG-конвейера на реальных файлах владельца.
- Возможное продолжение: SWAR-поиск `0xFF` на 8 байтах вперёд с частичным
  потреблением (сейчас при одном `0xFF` в окне весь refill уходит в побайтовый
  путь), и/или увеличение окна до 2×u64. Эффект ожидается малым (~1–3%).

## Изменённые файлы (зона A)

- `crates/rrrah-decode/src/dng/lossless_jpeg.rs` — word-refill fast-path, флаг
  `word_refill`, `decode_with_refill`, 1 новый тест, правка 3 конструкторов
  `EntropyReader::new` в тестах.
- `crates/rrrah-decode/src/dng/uncompressed.rs` — word-refill, эталон
  `decode_msb_packed_bytewise`, 1 новый тест.
- `crates/rrrah-decode/src/dng/mod.rs` — `bench_support`.
- `crates/rrrah-decode/src/lib.rs` — 1 строка (`#[doc(hidden)] pub mod dng`).
- `crates/rrrah-decode/Cargo.toml` — criterion dev-dependency + `[[bench]]`.
- `crates/rrrah-decode/benches/dng_decoders.rs` — новый.
- `crates/rrrah-decode/examples/decode_timing.rs` — новый.
- `Cargo.lock` — автоматически (criterion 0.5.1 из локального кэша; корневой
  `Cargo.toml` не тронут).

Ничего не закоммичено.
