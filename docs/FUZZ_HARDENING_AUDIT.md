# Fuzz и hardening audit

Дата: 2026-07-21  
Владелец: ingest/security  
Статус: bounded unit/invariant tests добавлены; полноценный RAW fuzzing
остаётся отдельным sandboxed job.

## Что проверено в коде

Новые тесты намеренно маленькие и не строят `Vec(width * height)`, где
`width/height` приходят из тестового либо будущего fuzz-входа:

| Контур | Проверка | Гарантия |
|---|---|---|
| `rrrah-cache` | `payload_byte_arithmetic_is_checked_before_allocation` | pixel count сначала проходит hard cap и checked `u64 → usize`/`bytes` conversion |
| `rrrah-cache` | `malformed_header_lengths_are_bounded_and_never_panic` | `u32::MAX` и `MAX_HEADER_BYTES + 1` отбрасываются до чтения/аллокации header body |
| `rrrah-cache` | `invalid_dimensions_are_rejected_before_payload_allocation` | `u32::MAX`/zero width дают typed error до payload allocation |
| `rrrah-core` | `decoded_mosaic_rejects_extreme_dimensions_without_allocating` | экстремальные dimensions с пустым pixel slice не могут вызвать большую аллокацию |
| `rrrah-decode` | `decode_request_rejects_stale_generation_before_io` | отменённый generation останавливается до открытия файла |

Существующие cache-тесты дополнительно покрывают bad magic, truncated header,
checksum mismatch, schema/key mismatch, inconsistent pixel count и trailing
bytes. `catch_unwind` в тесте parser-а — только assertion «нет panic»; он не
считается OOM-защитой.

## Инварианты до любой аллокации

1. Header length ограничен `MAX_HEADER_BYTES` (сейчас 1 MiB), затем читается
   только этот bounded буфер.
2. После JSON decode проверяются schema/key и `RawMetadata::validate`.
3. `width * height * components` считается в `u64` через checked операции и
   сравнивается с объявленным `pixel_count`.
4. `pixel_count` проходит hard cap, затем `pixel_count * size_of::<u16>()`
   переводится в `usize` через `checked_mul`/`try_from`.
5. Только после всех проверок создаётся payload `Vec<u8>`. Тесты используют
   tiny fixtures; они не принимают attacker-controlled dimensions в качестве
   аргумента `vec![]`.

Текущий предел `MAX_CACHED_PIXELS = 250_000_000` равен примерно 500 MiB
сырого payload. Это guard от integer overflow, а не достаточный process
quota для hostile files. В production worker предел должен быть ниже и
конфигурируемым (рекомендуется начать с 128--256 MiB RSS/output budget и
проводить decode в отдельном процессе).

## Cargo-fuzz availability и план targets

На машине разработки установлен `cargo-fuzz 0.13.2`, но targets пока не
коммитятся: у parser-а ещё нет отдельного byte-slice API, а RAW decoder должен
сначала получить process boundary. До этого `cargo fuzz` не запускается как
часть обычного workspace CI. Следующие targets являются точным контрактом для
будущего `fuzz/` package:

| Target | Input/corpus format | Property |
|---|---|---|
| `cache_header` | до 1 MiB: `RRRAHRC1` (8 bytes), LE `u32 header_len`, UTF-8 JSON `CacheHeader`, затем LE `u16` payload; seed-файлы `fuzz/corpus/cache_header/*.bin` | no panic; typed `CacheError` or valid frame; no allocation before header/pixel caps |
| `metadata_json` | raw UTF-8 JSON bytes, `-max_len=1048576`; seeds are one valid `RawMetadata` plus one-field mutations | `serde_json`/validation only; dimensions, grids and rectangles stay bounded; no NaN/Inf publish |
| `generation_token` | exactly 16 bytes: LE `u64 current`, LE `u64 expected` | `current != expected ⇒ cancelled`; no I/O/allocation; atomic ordering remains deterministic |
| `dng_tile_plan` | synthetic little-endian TIFF/IFD fixture with bounded entry count (≤4096), offsets/counts as `u64`; no real RAW decode in-process | reject overlap, wraparound, out-of-file ranges and absurd tile count before worker allocation |
| `cr2_entropy` | bounded CR2/JPEG strip bytes (≤16 MiB), seeds for valid restart markers plus truncation/mutation variants | no panic/UB; worker exits on timeout/RSS; no claim of in-process OOM safety |

The first two targets should call a byte-slice parser directly. If the only
available API is `DiskMosaicCache::load(path, key)`, the harness writes a
single bounded file into a `tempdir`, uses one fixed valid key, and removes it
after the iteration; it must never derive a filesystem path from fuzz bytes.

## Reproducible commands and resource limits

After the targets exist:

```bash
cargo fuzz check
cargo fuzz run cache_header -- \
  -max_len=1048576 -timeout=2 -rss_limit_mb=256 -runs=10000
cargo fuzz run metadata_json -- \
  -max_len=1048576 -timeout=2 -rss_limit_mb=256 -runs=10000
cargo fuzz run generation_token -- \
  -max_len=16 -timeout=1 -rss_limit_mb=128 -runs=10000
```

The `-rss_limit_mb` and `-timeout` flags are libFuzzer tripwires, not a
security boundary. The RAW targets must run in a short-lived decoder worker
with OS enforcement (macOS sandbox/`launchd` job, Linux `RLIMIT_AS`/cgroup,
Windows Job Object), no network, bounded input file (start at 512 MiB), bounded
decoded output (128--256 MiB), one worker per job, and kill/restart on timeout
or RSS breach. A panic is a typed failure; OOM, stack exhaustion and a stuck
third-party decompressor require killing the process and are failing fuzz
outcomes, never retries inside the UI process.

CI should retain minimized corpus and crash artifacts, run a short PR smoke
(1--10k executions), and schedule longer jobs with fixed seed/time budget.
An absent fuzz tool is a visible `skip` with tool/version and owner, not a
green fake pass. No advisory is added to `deny.toml`'s ignore list.

## Adversarial critic / stop conditions

This hardening slice is not complete production isolation. Do not claim the
viewer can safely open arbitrary hostile RAW files until all of these hold:

* every malformed cache/header/IFD/CR2 input returns without panic and without
  unbounded in-process allocation;
* process RSS and wall time are measured externally and kill the worker;
* cancellation is observed before publish and stale generation count is zero;
* minimized corpus files are content-addressed and replayed in CI;
* `cargo audit`/`cargo deny` remain fail-closed for advisories; fuzzing must not
  be used to justify an advisory ignore.

