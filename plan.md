# План: поддержка RAW-форматов популярных камер

Цель: расширить `rrrah-decode` за пределы CR3 (Canon EOS R8) и DNG/TIFF —
добавить форматы популярных камер: Canon CR2, Nikon NEF, Sony ARW,
Olympus/OM System ORF, Pentax PEF, Panasonic RW2, Fujifilm RAF.

Архитектурная база: все перечисленные форматы, кроме RAF-контейнера и CR3,
являются TIFF-подобными; в проекте уже есть чистый TIFF-парсер
(`crates/rrrah-decode/src/dng/tiff.rs`), распаковщик uncompressed strip/tile
(`dng/uncompressed.rs`) и lossless-JPEG кодек (`dng/lossless_jpeg.rs`).

## Этап 1 — Фундамент (один coder-агент, последовательно)

- `sniff`-модуль: определение формата по magic bytes (не только расширение):
  CR3 (BMFF/ftyp), CR2 (TIFF + «CR» маркер), NEF, ARW, ORF (IIRO/IIRS),
  PEF, RW2 (IIU\0), RAF (FUJIFILMCCD-RAW), DNG/TIFF.
- Расширение `native_router.rs`: маршрутизация по magic с fallback на
  расширение; новые варианты `NativeFormat`.
- Общее ядро `camtiff`: выбор CFA-IFD в камерном TIFF, общий контракт
  (trait/хуки) для per-format модулей: выбор raw IFD, маппинг тегов
  (black/white level, CFA, WB, ColorMatrix), матрица поддерживаемых
  компрессий, типизированные отказы для неподдержанного.
- Контракт для Этапа 2: каждый формат — отдельный файл(ы) модуля, без правок
  общих файлов.
- Гейт: `cargo check` + unit-тесты sniff-модуля проходят.

## Этап 2 — Per-format модули (AgentSwarm, параллельно, по одному агенту на формат)

Каждый агент создаёт ТОЛЬКО новые файлы своего модуля + unit-тесты на
синтетических минимальных заголовках (лицензионные файлы не коммитим —
см. `tests/fixtures/README.md`, `docs/DECODE_FORMAT_AUDIT.md`):

1. Canon CR2 — TIFF + raw IFD по смещению из заголовка, lossless JPEG,
   слайсы (tag 0xC640).
2. Nikon NEF — uncompressed/packed 12/14-bit; Nikon-compressed — если
   осуществимо через существующий LJPEG + curve, иначе типизированный отказ.
3. Sony ARW — uncompressed (ARW 1.0) и LJPEG-варианты; Sony-теги уровней.
4. Olympus ORF — IIRO/IIRS magic, packed 12-bit, Olympus-теги.
5. Pentax PEF — uncompressed + LJPEG, Pentax-теги.
6. Panasonic RW2 — IIU\0 magic, Panasonic-теги, распаковка; typed rejection
   для проприетарной компрессии.
7. Fujifilm RAF — контейнер + встроенный TIFF; Bayer-модели декодируются,
   X-Trans 6x6 — метаданные сохраняются, fast path отвергает (как в аудите).

Гейт: модуль компилируется автономно в составе crate (агент проверяет
`cargo check -p rrrah-decode` и свои тесты, не трогая чужие файлы).

## Этап 3 — Интеграция (один coder-агент, последовательно)

- Регистрация всех модулей в router/registry, правки общих файлов.
- `cargo test --workspace --locked`, clippy — зелёные.
- Fixture-gated env-тесты по образцу `RRRAH_CR2_FIXTURE` для каждого формата.
- Обновить README (список форматов) и `docs/DECODE_FORMAT_AUDIT.md`
  (матрица возможностей: что декодируется, что отвергается типизированно).

## Принципы (из DECODE_FORMAT_AUDIT.md)

- Никогда не подставлять embedded JPEG вместо сенсорной мозаики.
- Неподдержанное — явный типизированный результат, не молчаливая деградация.
- Никаких фейковых фикстур, выдаваемых за камерную совместимость.
- Проверяемая арифметика для offsets/counts (недоверенный ввод).
